use std::fs;
use std::io::{self, Stdout, Write};
use std::path::PathBuf;
use std::time::Duration;

use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers,
    MouseButton, MouseEvent, MouseEventKind,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
#[cfg(test)]
use ratatui::widgets::ListItem;
use ratatui::widgets::Paragraph;
use ratatui::Terminal;
use tree_sitter_highlight::{
    Error as HighlightError, Highlight, HighlightConfiguration, HighlightEvent, Highlighter,
};
use tree_sitter_language::LanguageFn;

const USER_BG: Color = Color::Rgb(20, 22, 30);
const OUTPUT_BG: Color = Color::Rgb(34, 36, 40);
const ERROR_BG: Color = Color::Rgb(36, 18, 18);
const ERROR_FG: Color = Color::LightRed;
const SELECTED_USER_BG: Color = Color::Rgb(42, 45, 64);
const SELECTED_OUTPUT_BG: Color = Color::Rgb(24, 70, 50);
const SELECTED_ERROR_BG: Color = Color::Rgb(70, 30, 30);
const COMPLETION_FG: Color = Color::LightCyan;
const COMPLETION_HINT_FG: Color = Color::DarkGray;
const COMMAND_FG: Color = Color::LightGreen;
const KEYMAP_FG: Color = Color::LightBlue;
const INPUT_PLACEHOLDER_FG: Color = Color::DarkGray;
const LINE_NUMBER_FG: Color = Color::DarkGray;
const FEED_X_OFFSET: u16 = 3;
const FEED_ENTRY_MIN_HEIGHT: u16 = 3;
const FEED_ENTRY_GAP: u16 = 1;
const INPUT_TOP_GAP: u16 = 1;
const INPUT_BOTTOM_GAP: u16 = 1;
const TEXT_BOX_INNER_LEFT_PADDING: u16 = 1;
const INPUT_PLACEHOLDER: &str = "Type Lane code...";
const ERROR_LINE_MARKER: &str = "";

const HIGHLIGHT_NAMES: &[&str] = &[
    "attribute",
    "comment",
    "constant",
    "constant.builtin",
    "constructor",
    "function",
    "function.builtin",
    "keyword",
    "keyword.conditional",
    "keyword.directive",
    "number",
    "operator",
    "property",
    "punctuation.bracket",
    "punctuation.delimiter",
    "punctuation.special",
    "string",
    "type",
    "type.builtin",
    "variable",
    "variable.parameter",
];

// FFI hook exported by tree-sitter for Lane syntax highlighting.
unsafe extern "C" {
    /// Performs `tree_sitter_lane` behavior.
    fn tree_sitter_lane() -> *const ();
}

const LANGUAGE_LANE: LanguageFn = unsafe { LanguageFn::from_raw(tree_sitter_lane) };

/// Run the REPL event loop and keep creating terminal sessions until the user
/// chooses to exit.
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut app = App::new();
    loop {
        let mut terminal = ReplTerminal::enter()?;
        let action = app.run(&mut terminal.terminal);
        terminal.leave()?;
        match action? {
            ReplAction::Exit => return Ok(()),
            ReplAction::Show(source) => match crate::run_preview_source(&source) {
                Ok(()) => app
                    .transcript
                    .push(TranscriptEntry::system("Preview closed.")),
                Err(err) => app.transcript.push(TranscriptEntry::error(
                    format!("preview error: {}", err),
                    None,
                )),
            },
        }
    }
}

struct ReplTerminal {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    active: bool,
}

impl ReplTerminal {
    /// Enter raw terminal mode and allocate an alternate-screen crossterm
    /// context for REPL rendering.
    fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        Ok(Self {
            terminal,
            active: true,
        })
    }

    /// Restore the terminal to its normal mode and leave the alternate screen.
    fn leave(&mut self) -> io::Result<()> {
        if !self.active {
            return Ok(());
        }
        disable_raw_mode()?;
        execute!(
            self.terminal.backend_mut(),
            DisableMouseCapture,
            LeaveAlternateScreen
        )?;
        self.terminal.show_cursor()?;
        self.active = false;
        Ok(())
    }
}

impl Drop for ReplTerminal {
    /// Ensure terminal mode is reset if the run loop panics or exits early.
    fn drop(&mut self) {
        if self.active {
            let _ = disable_raw_mode();
            let _ = execute!(
                self.terminal.backend_mut(),
                DisableMouseCapture,
                LeaveAlternateScreen
            );
            let _ = self.terminal.show_cursor();
        }
    }
}

struct App {
    session: ReplSession,
    input: String,
    input_cursor: usize,
    input_history: Vec<String>,
    history_file: Option<PathBuf>,
    history_position: Option<usize>,
    history_draft: String,
    completion_matches: Vec<lane::LaneCompletionItem>,
    completion_index: usize,
    transcript: Vec<TranscriptEntry>,
    highlighter: SyntaxHighlighter,
    split_mode: bool,
    selected_group: Option<usize>,
    transcript_scroll: usize,
    split_user_scroll: usize,
    split_glsl_scroll: usize,
    next_group: usize,
    layout: TranscriptLayout,
}

impl App {
    /// Create a new REPL app using the default history location.
    fn new() -> Self {
        Self::new_with_history_file(default_history_file())
    }

    /// Build an app instance and optionally preload input history from disk.
    fn new_with_history_file(history_file: Option<PathBuf>) -> Self {
        let input_history = history_file
            .as_ref()
            .map(|path| load_repl_history(path))
            .unwrap_or_default();
        Self {
            session: ReplSession::default(),
            input: String::new(),
            input_cursor: 0,
            input_history,
            history_file,
            history_position: None,
            history_draft: String::new(),
            completion_matches: Vec::new(),
            completion_index: 0,
            transcript: vec![TranscriptEntry::welcome(format!(
                "Lane {}",
                env!("CARGO_PKG_VERSION")
            ))],
            highlighter: SyntaxHighlighter::new(),
            split_mode: false,
            selected_group: None,
            transcript_scroll: 0,
            split_user_scroll: 0,
            split_glsl_scroll: 0,
            next_group: 0,
            layout: TranscriptLayout::default(),
        }
    }

    /// Render, read one crossterm event, and route it to key/mouse handlers.
    /// Return the resulting action only when a key interaction requests it.
    fn run(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    ) -> Result<ReplAction, Box<dyn std::error::Error>> {
        loop {
            terminal.draw(|frame| self.draw(frame))?;
            if !event::poll(Duration::from_millis(80))? {
                continue;
            }
            match event::read()? {
                Event::Key(key) => {
                    if let Some(action) = self.handle_key(key) {
                        return Ok(action);
                    }
                }
                Event::Mouse(mouse) => self.handle_mouse(mouse),
                _ => {}
            }
        }
    }

    /// Handle one key event and return a REPL action when the key triggers it.
    fn handle_key(&mut self, key: KeyEvent) -> Option<ReplAction> {
        match (key.code, key.modifiers) {
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => return Some(ReplAction::Exit),
            (KeyCode::Char('f'), KeyModifiers::CONTROL) => self.format_current_input(),
            (KeyCode::Enter, modifiers)
                if modifiers.contains(KeyModifiers::SHIFT)
                    || modifiers.contains(KeyModifiers::ALT) =>
            {
                self.clear_history_navigation();
                self.clear_completion();
                self.insert_input_char('\n');
            }
            (KeyCode::Enter, _) => return self.submit_input(),
            (KeyCode::Tab, _) => self.apply_completion(),
            (KeyCode::Up, _) => self.recall_older_input(),
            (KeyCode::Down, _) => self.recall_newer_input(),
            (KeyCode::Left, _) => self.move_input_cursor_left(),
            (KeyCode::Right, _) => self.move_input_cursor_right(),
            (KeyCode::PageUp, _) => self.scroll_transcript_up(),
            (KeyCode::PageDown, _) => self.scroll_transcript_down(),
            (KeyCode::Backspace, _) => {
                self.clear_history_navigation();
                self.clear_completion();
                self.backspace_input_char();
            }
            (KeyCode::Char(ch), _) => {
                self.clear_history_navigation();
                self.clear_completion();
                self.insert_input_char(ch);
            }
            _ => {}
        }
        None
    }

    /// Dispatch mouse events for entry selection, copy, and pane scrolling.
    fn handle_mouse(&mut self, mouse: MouseEvent) {
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                self.select_entry_at(mouse.column, mouse.row)
            }
            MouseEventKind::Down(MouseButton::Right) => {
                if let Some(text) = self.copyable_text_at(mouse.column, mouse.row) {
                    let _ = copy_text_to_clipboard(&text);
                }
            }
            MouseEventKind::ScrollUp => {
                self.scroll_at(mouse.column, mouse.row, ScrollDirection::Up)
            }
            MouseEventKind::ScrollDown => {
                self.scroll_at(mouse.column, mouse.row, ScrollDirection::Down)
            }
            _ => {}
        }
    }

    /// Map a mouse hit in the transcript region to the corresponding entry and
    /// mark it as the currently selected group.
    fn select_entry_at(&mut self, x: u16, y: u16) {
        self.selected_group = self
            .layout
            .entry_at(x, y)
            .and_then(|index| self.transcript.get(index))
            .and_then(|entry| entry.group);
    }

    /// Return copyable text for a transcript entry at a screen position.
    fn copyable_text_at(&self, x: u16, y: u16) -> Option<String> {
        let index = self.layout.entry_at(x, y)?;
        let entry = self.transcript.get(index)?;
        entry.copyable_text()
    }

    /// Insert a new character at the current input cursor position.
    fn insert_input_char(&mut self, ch: char) {
        self.input.insert(self.input_cursor, ch);
        self.input_cursor += ch.len_utf8();
    }

    /// Remove the codepoint immediately before the input cursor, if present.
    fn backspace_input_char(&mut self) {
        let Some((previous, _)) = self.input[..self.input_cursor].char_indices().next_back() else {
            return;
        };
        self.input.drain(previous..self.input_cursor);
        self.input_cursor = previous;
    }

    /// Move the cursor left by one UTF-8 scalar value.
    fn move_input_cursor_left(&mut self) {
        if let Some((previous, _)) = self.input[..self.input_cursor].char_indices().next_back() {
            self.input_cursor = previous;
        }
    }

    /// Move the cursor right by one UTF-8 scalar value.
    fn move_input_cursor_right(&mut self) {
        if self.input_cursor >= self.input.len() {
            return;
        }
        let next = self.input[self.input_cursor..]
            .chars()
            .next()
            .map(|ch| self.input_cursor + ch.len_utf8())
            .unwrap_or(self.input.len());
        self.input_cursor = next;
    }

    /// Scroll backwards in transcript history (or user pane when split mode is on).
    fn scroll_transcript_up(&mut self) {
        if self.split_mode {
            self.scroll_split_pane_up(SplitPane::User);
        } else {
            self.transcript_scroll = self.transcript_scroll.saturating_add(1).min(
                transcript_row_count(
                    &linear_transcript_entries(&self.transcript),
                    &self.transcript,
                )
                .saturating_sub(1),
            );
        }
    }

    /// Scroll forward in transcript history (or user pane when split mode is on).
    fn scroll_transcript_down(&mut self) {
        if self.split_mode {
            self.scroll_split_pane_down(SplitPane::User);
        } else {
            self.transcript_scroll = self.transcript_scroll.saturating_sub(1);
        }
    }

    /// Scroll the active mouse-targeted pane up/down.
    fn scroll_at(&mut self, x: u16, y: u16, direction: ScrollDirection) {
        if !self.split_mode {
            match direction {
                ScrollDirection::Up => self.scroll_transcript_up(),
                ScrollDirection::Down => self.scroll_transcript_down(),
            }
            return;
        }

        let pane = self.layout.pane_at(x, y).unwrap_or(SplitPane::User);
        match direction {
            ScrollDirection::Up => self.scroll_split_pane_up(pane),
            ScrollDirection::Down => self.scroll_split_pane_down(pane),
        }
    }

    /// Scroll one split pane upward up to its available max row count.
    fn scroll_split_pane_up(&mut self, pane: SplitPane) {
        let max_scroll = transcript_row_count(&self.split_pane_entries(pane), &self.transcript)
            .saturating_sub(1);
        match pane {
            SplitPane::User => {
                self.split_user_scroll = self.split_user_scroll.saturating_add(1).min(max_scroll)
            }
            SplitPane::Glsl => {
                self.split_glsl_scroll = self.split_glsl_scroll.saturating_add(1).min(max_scroll)
            }
        }
    }

    /// Scroll one split pane downward.
    fn scroll_split_pane_down(&mut self, pane: SplitPane) {
        match pane {
            SplitPane::User => self.split_user_scroll = self.split_user_scroll.saturating_sub(1),
            SplitPane::Glsl => self.split_glsl_scroll = self.split_glsl_scroll.saturating_sub(1),
        }
    }

    /// Select transcript entry indices that belong to a split pane.
    fn split_pane_entries(&self, pane: SplitPane) -> Vec<usize> {
        self.transcript
            .iter()
            .enumerate()
            .rev()
            .filter(|(_, entry)| match pane {
                SplitPane::User => !matches!(entry.kind, TranscriptKind::Glsl),
                SplitPane::Glsl => matches!(entry.kind, TranscriptKind::Glsl),
            })
            .map(|(index, _)| index)
            .collect()
    }

    /// Add a plain command row to transcript output.
    fn push_command_entry(&mut self, input: &str) {
        self.transcript
            .push(TranscriptEntry::command(input.to_string(), None));
    }

    /// Handle a full input submission and append resulting transcript entries.
    fn submit_input(&mut self) -> Option<ReplAction> {
        self.clear_history_navigation();
        self.clear_completion();
        let input = std::mem::take(&mut self.input);
        self.input_cursor = 0;
        if input.trim().is_empty() {
            return None;
        }

        self.reset_submit_scroll_offsets();

        self.record_input_history(&input);
        let is_command = input.starts_with('/');
        let line_start = (!is_command).then(|| self.session.next_line_number());
        self.resolve_submit_outcome(input, is_command, line_start)
    }

    /// Reset all transcript scroll offsets after a new submission is accepted.
    fn reset_submit_scroll_offsets(&mut self) {
        self.transcript_scroll = 0;
        self.split_user_scroll = 0;
        self.split_glsl_scroll = 0;
    }

    /// Handle session outcome for one submitted block and emit transcript rows.
    fn resolve_submit_outcome(
        &mut self,
        input: String,
        is_command: bool,
        line_start: Option<usize>,
    ) -> Option<ReplAction> {
        match self.session.submit(&input) {
            SubmitOutcome::Accepted => {
                if !is_command {
                    self.push_submitted_input(input, line_start, true);
                }
            }
            SubmitOutcome::Emitted(glsl) => {
                let group = self.push_submitted_input(input, line_start, true);
                if glsl.is_empty() {
                    return None;
                }
                self.push_glsl_output(glsl, group);
            }
            SubmitOutcome::Cleared => {
                self.with_command_input(&input, |app| app.clear_transcript());
            }
            SubmitOutcome::Restarted => {
                self.with_command_input(&input, |app| {
                    app.clear_transcript();
                    app.next_group = 0;
                    app.transcript
                        .push(TranscriptEntry::system("Session restarted."));
                });
            }
            SubmitOutcome::Help => {
                self.push_command_with_entry(&input, TranscriptEntry::help(help_text()));
            }
            SubmitOutcome::Info(info) => {
                self.push_command_with_entry(&input, TranscriptEntry::system(info));
            }
            SubmitOutcome::Code(source) => {
                self.push_command_with_entry(
                    &input,
                    TranscriptEntry::submitted(source, None, Some(1)),
                );
            }
            SubmitOutcome::Saved(message) | SubmitOutcome::Exported(message) => {
                self.push_command_with_entry(&input, TranscriptEntry::system(message));
            }
            SubmitOutcome::Show(source) => return Some(ReplAction::Show(source)),
            SubmitOutcome::ToggleSplit => {
                self.toggle_split();
            }
            SubmitOutcome::Exit => {
                self.push_command_entry(&input);
                return Some(ReplAction::Exit);
            }
            SubmitOutcome::Error(error) => {
                if is_command {
                    self.transcript.push(TranscriptEntry::error(error, None));
                } else {
                    let group = self.push_submitted_input(input, line_start, false);
                    if let Some(group) = group {
                        self.attach_group_error(group, error);
                    } else {
                        self.transcript.push(TranscriptEntry::error(error, None));
                    }
                }
            }
        }
        None
    }

    /// Clear transcript content and reset click selection.
    fn clear_transcript(&mut self) {
        self.transcript.clear();
        self.selected_group = None;
    }

    /// Record a command row and pass `self` to a follow-up mutator.
    fn with_command_input<F>(&mut self, input: &str, update: F)
    where
        F: FnOnce(&mut Self),
    {
        self.push_command_entry(input);
        update(self);
    }

    /// Record a command row and its immediate response entry.
    fn push_command_with_entry(&mut self, input: &str, response: TranscriptEntry) {
        self.push_command_entry(input);
        self.transcript.push(response);
    }

    /// Format the current input using the lane formatter.
    fn format_current_input(&mut self) {
        if self.input.is_empty() {
            return;
        }
        self.clear_completion();
        self.input = lane::format_lane_source(&self.input)
            .trim_end_matches(['\r', '\n'])
            .to_string();
        self.input_cursor = self.input.len();
    }

    /// Expand completion at cursor with matching entries, if available.
    fn apply_completion(&mut self) {
        let Some((start, prefix)) = completion_token(&self.input[..self.input_cursor]) else {
            self.clear_completion();
            return;
        };
        if prefix.is_empty() {
            self.clear_completion();
            return;
        }
        self.completion_matches = self
            .active_completion_items()
            .into_iter()
            .filter(|item| item.label.starts_with(&prefix))
            .collect();
        self.completion_index = 0;
        let Some(completed) = completion_target(&prefix, &self.completion_matches) else {
            return;
        };
        self.input
            .replace_range(start..self.input_cursor, &completed);
        self.input_cursor = start + completed.len();
    }

    /// Clear completion cache, selected index, and hints.
    fn clear_completion(&mut self) {
        self.completion_matches.clear();
        self.completion_index = 0;
    }

    /// Add a non-empty input line to in-memory history and persist it.
    fn record_input_history(&mut self, input: &str) {
        if self.input_history.last().is_some_and(|last| last == input) {
            return;
        }
        self.input_history.push(input.to_string());
        self.persist_input_history();
    }

    /// Persist history entries to disk if a history path is configured.
    fn persist_input_history(&self) {
        let Some(path) = self.history_file.as_ref() else {
            return;
        };
        if let Some(parent) = path.parent() {
            if fs::create_dir_all(parent).is_err() {
                return;
            }
        }
        let mut serialized = self.input_history.join("\n");
        if !serialized.is_empty() {
            serialized.push('\n');
        }
        let _ = fs::write(path, serialized);
    }

    /// Restore the previous entered line from history for up-arrow navigation.
    fn recall_older_input(&mut self) {
        if self.input_history.is_empty() {
            return;
        }
        let position = match self.history_position {
            Some(0) => 0,
            Some(position) => position.saturating_sub(1),
            None => {
                self.history_draft = self.input.clone();
                self.input_history.len().saturating_sub(1)
            }
        };
        self.history_position = Some(position);
        self.input = self.input_history[position].clone();
        self.input_cursor = self.input.len();
        self.clear_completion();
    }

    /// Restore the next line from history, or the draft when no more entries.
    fn recall_newer_input(&mut self) {
        let Some(position) = self.history_position else {
            return;
        };
        let next_position = position.saturating_add(1);
        if next_position < self.input_history.len() {
            self.history_position = Some(next_position);
            self.input = self.input_history[next_position].clone();
        } else {
            self.history_position = None;
            self.input = std::mem::take(&mut self.history_draft);
        }
        self.input_cursor = self.input.len();
        self.clear_completion();
    }

    /// Exit history browsing mode and drop draft buffer.
    fn clear_history_navigation(&mut self) {
        self.history_position = None;
        self.history_draft.clear();
    }

    /// Allocate a new transcript group identifier.
    fn allocate_group(&mut self) -> usize {
        let group = self.next_group;
        self.next_group += 1;
        group
    }

    /// Insert submitted lane text as a transcript entry; merge with previous lane
    /// entry when allowed.
    fn push_submitted_input(
        &mut self,
        input: String,
        line_start: Option<usize>,
        allow_merge: bool,
    ) -> Option<usize> {
        if input.starts_with('/') {
            return None;
        }
        if allow_merge {
            if let Some(index) = self.latest_mergeable_lane_entry() {
                let entry = &mut self.transcript[index];
                if !entry.text.ends_with('\n') {
                    entry.text.push('\n');
                }
                entry.text.push_str(&input);
                return entry.group;
            }
        }
        let group = self.allocate_group();
        self.transcript
            .push(TranscriptEntry::submitted(input, Some(group), line_start));
        Some(group)
    }

    /// Add a generated GLSL block and merge with prior block in same group.
    fn push_glsl_output(&mut self, glsl: GlslOutput, group: Option<usize>) {
        if let Some(entry) = self.transcript.last_mut() {
            if matches!(entry.kind, TranscriptKind::Glsl) && entry.group == group {
                if !entry.text.ends_with('\n') {
                    entry.text.push('\n');
                }
                entry.text.push_str(&glsl.text);
                return;
            }
        }
        self.transcript.push(TranscriptEntry::glsl(
            glsl.text,
            group,
            Some(glsl.line_start),
        ));
    }

    /// Return the latest non-errored lane entry for appending a merged line.
    fn latest_mergeable_lane_entry(&self) -> Option<usize> {
        let index = self.transcript.len().checked_sub(1)?;
        let entry = self.transcript.get(index)?;
        if matches!(entry.kind, TranscriptKind::Lane) && !entry.errored {
            Some(index)
        } else {
            None
        }
    }

    /// Attach an error marker to the most recent lane entry in `group`.
    fn attach_group_error(&mut self, group: usize, error: String) {
        for entry in self.transcript.iter_mut().rev() {
            if entry.group == Some(group) && matches!(entry.kind, TranscriptKind::Lane) {
                entry.errored = true;
                entry.error = Some(error.clone());
                break;
            }
        }
    }

    /// Switch between split and single-pane transcript rendering.
    fn toggle_split(&mut self) {
        self.split_mode = !self.split_mode;
    }

    /// Draw transcript and command input for the active frame.
    fn draw(&mut self, frame: &mut ratatui::Frame) {
        self.layout = TranscriptLayout::default();
        if self.split_mode {
            let split = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(frame.area());
            let user_area = transcript_area(split[0]);
            let input_area = input_area(split[0]);
            let user_entries = self.split_pane_entries(SplitPane::User);
            let glsl_entries = self.split_pane_entries(SplitPane::Glsl);
            self.layout.record_pane(SplitPane::User, user_area);
            self.layout
                .record_pane(SplitPane::Glsl, transcript_feed_area(split[1]));
            self.render_transcript_entries(
                frame,
                user_entries.as_slice(),
                user_area,
                self.split_user_scroll,
            );
            self.render_transcript_entries(
                frame,
                glsl_entries.as_slice(),
                transcript_feed_area(split[1]),
                self.split_glsl_scroll,
            );
            self.render_input(frame, input_area);
        } else {
            let entries = linear_transcript_entries(&self.transcript);
            self.render_transcript_entries(
                frame,
                entries.as_slice(),
                transcript_area(frame.area()),
                self.transcript_scroll,
            );
            self.render_input(frame, input_area(frame.area()));
        }
    }

    /// Render transcript entries into a region using layout records.
    fn render_transcript_entries(
        &mut self,
        frame: &mut ratatui::Frame,
        entries: &[usize],
        area: Rect,
        scroll_rows: usize,
    ) {
        let first_new_layout_entry = self.layout.entries.len();
        self.layout
            .record_bottom_to_top(area, entries, &self.transcript, scroll_rows);
        let visible_entries = self.layout.entries[first_new_layout_entry..].to_vec();
        for rendered in visible_entries {
            let Some(entry) = self.transcript.get(rendered.index).cloned() else {
                continue;
            };
            let paragraph = Paragraph::new(self.render_entry_text(&entry))
                .style(entry.style(self.selected_group));
            frame.render_widget(paragraph, rendered.area);
        }
    }

    /// Draw the current input block and cursor with visible placeholder offsets.
    fn render_input(&mut self, frame: &mut ratatui::Frame, area: Rect) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let input = Paragraph::new(self.input_text()).style(Style::default().bg(USER_BG));
        frame.render_widget(input, area);

        let (cursor_line, cursor_width) = input_cursor_position(&self.input, self.input_cursor);
        let cursor_column = cursor_width
            .saturating_add(TEXT_BOX_INNER_LEFT_PADDING)
            .saturating_add(self.current_input_gutter_width())
            .min(area.width.saturating_sub(1));
        let cursor_x = area.x.saturating_add(cursor_column);
        let total_lines = input_line_count(&self.input);
        let cursor_row = if total_lines <= 1 {
            1.min(area.height.saturating_sub(1))
        } else {
            let visible_start = total_lines.saturating_sub(area.height as usize);
            cursor_line.saturating_sub(visible_start) as u16
        };
        let cursor_y = area
            .y
            .saturating_add(cursor_row.min(area.height.saturating_sub(1)));
        frame.set_cursor_position(Position::new(cursor_x, cursor_y));
    }

    /// Build styled input text with optional syntax highlighting and hints.
    fn input_text(&mut self) -> Text<'static> {
        if self.input.is_empty() {
            return placeholder_input_text(self.current_input_gutter_width());
        }

        let mut visible = self.input.lines().rev().take(3).collect::<Vec<_>>();
        visible.reverse();

        if self.input.starts_with('/') {
            let completion_hint = self.completion_hint_suffix();
            let mut lines = visible
                .iter()
                .map(|line| {
                    Line::from(Span::styled(
                        (*line).to_string(),
                        Style::default().fg(COMMAND_FG).add_modifier(Modifier::BOLD),
                    ))
                })
                .collect::<Vec<_>>();
            if let Some(hint) = completion_hint {
                if let Some(line) = lines.last_mut() {
                    line.spans
                        .push(Span::styled(hint, Style::default().fg(COMPLETION_HINT_FG)));
                }
            }
            if lines.len() <= 1 {
                lines.insert(0, Line::raw(""));
                lines.push(self.input_status_line());
            } else {
                while lines.len() < 3 {
                    lines.insert(0, Line::raw(""));
                }
            }
            return current_input_text(Text::from(lines), self.current_input_gutter_width());
        }

        let completion_hint = self.completion_hint_suffix();
        let source = if visible.len() <= 1 {
            self.input.clone()
        } else {
            visible.join("\n")
        };
        let mut text = self.highlighter.highlight_lane(&source);
        append_completion_hint(&mut text, completion_hint);
        if visible.len() <= 1 {
            text.lines.insert(0, Line::raw(""));
            text.lines.push(self.input_status_line());
        } else {
            while text.lines.len() < 3 {
                text.lines.insert(0, Line::raw(""));
            }
        }
        current_input_text(text, self.current_input_gutter_width())
    }

    /// Compute gutter width from the next source line number.
    fn current_input_gutter_width(&self) -> u16 {
        line_number_gutter_width(self.session.next_line_number())
    }

    /// Compose a short help label describing available completions.
    fn input_status_line(&self) -> Line<'static> {
        if self.completion_matches.len() > 1 {
            let labels = self
                .completion_matches
                .iter()
                .take(5)
                .map(|item| item.label.as_str())
                .collect::<Vec<_>>()
                .join("  ");
            return Line::from(Span::styled(
                format!("Tab: {labels}"),
                Style::default().fg(COMPLETION_FG),
            ));
        }
        Line::raw("")
    }

    /// Resolve the current completion suffix from selected or first matching item.
    fn completion_hint_suffix(&self) -> Option<String> {
        let (_, prefix) = completion_token(&self.input)?;
        if prefix.is_empty() {
            return None;
        }
        self.selected_completion_label(&prefix)
            .and_then(|label| label.strip_prefix(&prefix).map(str::to_string))
            .filter(|suffix| !suffix.is_empty())
    }

    /// Return the currently-selected completion label when it matches the prefix.
    fn selected_completion_label(&self, prefix: &str) -> Option<String> {
        if let Some(item) = self.completion_matches.get(self.completion_index) {
            if item.label.starts_with(prefix) {
                return Some(item.label.clone());
            }
        }
        self.active_completion_items()
            .into_iter()
            .find(|item| item.label.starts_with(prefix))
            .map(|item| item.label)
    }

    /// Return completion candidates for current context.
    fn active_completion_items(&self) -> Vec<lane::LaneCompletionItem> {
        if self.input.starts_with('/') {
            return repl_command_completion_items();
        }
        lane::lane_completion_items()
    }

    #[cfg(test)]
    /// Render a single transcript entry for tests.
    fn render_entry(&mut self, entry: &TranscriptEntry) -> ListItem<'static> {
        ListItem::new(self.render_entry_text(entry)).style(entry.style(self.selected_group))
    }

    /// Render an entry using source-highlighting and optional error annotations.
    fn render_entry_text(&mut self, entry: &TranscriptEntry) -> Text<'static> {
        let text = match entry.kind {
            TranscriptKind::Lane => {
                if let (Some(line_start), Some(error)) = (entry.line_start, entry.error.as_deref())
                {
                    errored_lane_text(
                        self.highlighter.highlight_lane(&entry.text),
                        line_start,
                        error,
                    )
                } else if let Some(line_start) = entry.line_start {
                    numbered_lane_text(self.highlighter.highlight_lane(&entry.text), line_start)
                } else {
                    self.highlighter.highlight_lane(&entry.text)
                }
            }
            TranscriptKind::Command => Text::from(Line::from(Span::styled(
                entry.text.clone(),
                Style::default().fg(COMMAND_FG).add_modifier(Modifier::BOLD),
            ))),
            TranscriptKind::Glsl => {
                if let Some(line_start) = entry.line_start {
                    numbered_source_text(self.highlighter.highlight_glsl(&entry.text), line_start)
                } else {
                    self.highlighter.highlight_glsl(&entry.text)
                }
            }
            TranscriptKind::Help => highlight_help_text(&entry.text),
            TranscriptKind::Error => error_box_text(&entry.text),
            TranscriptKind::System | TranscriptKind::Welcome => Text::from(entry.text.clone()),
        };
        let text = if matches!(
            entry.kind,
            TranscriptKind::Error | TranscriptKind::Command | TranscriptKind::Welcome
        ) {
            text
        } else {
            padded_feed_text(text)
        };
        left_padded_text(text)
    }
}

/// Choose a history file path for this session, with tests explicitly disabled.
fn default_history_file() -> Option<PathBuf> {
    if cfg!(test) {
        return None;
    }
    if let Some(path) = std::env::var_os("LANE_REPL_HISTORY") {
        if !path.is_empty() {
            return Some(PathBuf::from(path));
        }
    }
    let state_base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state"))
        })?;
    Some(state_base.join("lane/repl_history"))
}

/// Read non-empty history lines from disk.
fn load_repl_history(path: &PathBuf) -> Vec<String> {
    let Ok(contents) = fs::read_to_string(path) else {
        return Vec::new();
    };
    contents
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

enum ReplAction {
    Exit,
    Show(String),
}

/// Calculate transcript area after reserving space for the input box.
fn transcript_area(area: Rect) -> Rect {
    transcript_feed_area(Rect {
        height: area.height.saturating_sub(input_reserved_height(area)),
        ..area
    })
}

/// Shift area right to account for feed gutter and remove gutter width from entries.
fn transcript_feed_area(area: Rect) -> Rect {
    let x_offset = FEED_X_OFFSET.min(area.width);
    Rect {
        x: area.x.saturating_add(x_offset),
        width: area.width.saturating_sub(x_offset),
        ..area
    }
}

/// Compute the active input area pinned at the bottom of the terminal.
fn input_area(area: Rect) -> Rect {
    let x_offset = FEED_X_OFFSET.min(area.width);
    let height = input_height(area);
    let y = area
        .y
        .saturating_add(area.height.saturating_sub(INPUT_BOTTOM_GAP))
        .saturating_sub(height);
    Rect {
        x: area.x.saturating_add(x_offset),
        y,
        width: area.width.saturating_sub(x_offset),
        height,
    }
}

/// Compute input+gaps height that must be reserved from transcript layout.
fn input_reserved_height(area: Rect) -> u16 {
    input_height(area)
        .saturating_add(input_top_gap(area))
        .saturating_add(input_bottom_gap(area))
}

/// Return the visible input box height, capped to at least one row.
fn input_height(area: Rect) -> u16 {
    area.height
        .saturating_sub(input_bottom_gap(area))
        .min(FEED_ENTRY_MIN_HEIGHT)
}

/// Keep a safe bottom gap even on short terminals.
fn input_bottom_gap(area: Rect) -> u16 {
    INPUT_BOTTOM_GAP.min(area.height.saturating_sub(FEED_ENTRY_MIN_HEIGHT))
}

/// Keep top gap so input sits above transcript on constrained heights.
fn input_top_gap(area: Rect) -> u16 {
    INPUT_TOP_GAP.min(
        area.height
            .saturating_sub(FEED_ENTRY_MIN_HEIGHT)
            .saturating_sub(input_bottom_gap(area)),
    )
}

#[cfg(test)]
/// Add blank separator lines between some transcript kinds in tests.
fn spaced_transcript_items(
    items: Vec<(ListItem<'static>, TranscriptKind)>,
) -> Vec<ListItem<'static>> {
    let mut spaced = Vec::with_capacity(items.len().saturating_mul(2).saturating_sub(1));
    let mut previous_kind = None;
    for (item, kind) in items {
        if previous_kind.is_some_and(|previous| !adjacent_without_feed_gap(previous, kind)) {
            spaced.push(ListItem::new(Line::raw("")));
        }
        spaced.push(item);
        previous_kind = Some(kind);
    }
    spaced
}

/// Decide when two neighboring transcript kinds can be displayed with no gap.
fn adjacent_without_feed_gap(previous: TranscriptKind, current: TranscriptKind) -> bool {
    if matches!(previous, TranscriptKind::Command) {
        return matches!(
            current,
            TranscriptKind::Command | TranscriptKind::Help | TranscriptKind::System
        );
    }
    matches!(
        (previous, current),
        (
            TranscriptKind::Help | TranscriptKind::System,
            TranscriptKind::Command
        )
    )
}

/// Return transcript indices newest-first for linear mode rendering.
fn linear_transcript_entries(transcript: &[TranscriptEntry]) -> Vec<usize> {
    transcript
        .iter()
        .enumerate()
        .rev()
        .map(|(index, _)| index)
        .collect()
}

/// Compute rendered row count for entries, including optional spacing rows.
fn transcript_row_count(entries: &[usize], transcript: &[TranscriptEntry]) -> usize {
    let mut rows = 0usize;
    for (position, index) in entries.iter().enumerate() {
        let Some(entry) = transcript.get(*index) else {
            continue;
        };
        rows = rows.saturating_add(entry.line_count() as usize);
        let adjacent_without_gap = entries
            .get(position + 1)
            .and_then(|next_index| {
                transcript
                    .get(*next_index)
                    .map(|next_entry| (*next_index, next_entry))
            })
            .is_some_and(|(next_index, next_entry)| {
                index.abs_diff(next_index) == 1
                    && adjacent_without_feed_gap(entry.kind, next_entry.kind)
            });
        if !adjacent_without_gap && position + 1 < entries.len() {
            rows = rows.saturating_add(FEED_ENTRY_GAP as usize);
        }
    }
    rows
}

/// Add the blank top and bottom rows used by normal feed-like boxes.
fn padded_feed_text(mut text: Text<'static>) -> Text<'static> {
    if text.lines.is_empty() {
        text.lines.push(Line::raw(""));
    }
    text.lines.insert(0, Line::raw(""));
    text.lines.push(Line::raw(""));
    text
}

/// Format an error block with top/bottom padding.
fn error_box_text(source: &str) -> Text<'static> {
    let mut lines = Vec::new();
    lines.push(Line::raw(""));
    lines.extend(source.lines().map(|line| Line::from(line.to_string())));
    if lines.len() == 1 {
        lines.push(Line::raw(""));
    }
    lines.push(Line::raw(""));
    Text::from(lines)
}

/// Add a left gutter pad to every rendered line.
fn left_padded_text(mut text: Text<'static>) -> Text<'static> {
    for line in &mut text.lines {
        line.spans.insert(0, Span::raw(" "));
    }
    text
}

/// Append a completion suffix hint to the final line when present.
fn append_completion_hint(text: &mut Text<'static>, hint: Option<String>) {
    let Some(hint) = hint else {
        return;
    };
    if let Some(line) = text.lines.last_mut() {
        line.spans
            .push(Span::styled(hint, Style::default().fg(COMPLETION_HINT_FG)));
    }
}

/// Insert left gutter spacing (blank + line-number width) in input lines.
fn current_input_text(mut text: Text<'static>, gutter_width: u16) -> Text<'static> {
    let gutter = " ".repeat(gutter_width as usize);
    for line in &mut text.lines {
        line.spans.insert(0, Span::raw(gutter.clone()));
        line.spans.insert(0, Span::raw(" "));
    }
    text
}

/// Count logical lines in an input source block.
fn input_line_count(input: &str) -> usize {
    input.chars().filter(|ch| *ch == '\n').count() + 1
}

/// Compute 0-based line and 0-based UTF-8 column at cursor index.
fn input_cursor_position(input: &str, cursor: usize) -> (usize, u16) {
    let before_cursor = &input[..cursor.min(input.len())];
    let line = before_cursor.chars().filter(|ch| *ch == '\n').count();
    let column = before_cursor
        .rsplit_once('\n')
        .map_or(before_cursor, |(_, suffix)| suffix)
        .chars()
        .count() as u16;
    (line, column)
}

/// Find completion token start and prefix from current input line tail.
fn completion_token(input: &str) -> Option<(usize, String)> {
    let line_start = input.rfind('\n').map_or(0, |index| index + 1);
    let mut start = input.len();
    for (index, ch) in input[line_start..].char_indices().rev() {
        if !is_completion_char(ch) {
            break;
        }
        start = line_start + index;
    }
    Some((start, input[start..].to_string()))
}

/// Choose the string to insert when completing with current match set.
fn completion_target(prefix: &str, matches: &[lane::LaneCompletionItem]) -> Option<String> {
    match matches {
        [] => None,
        [item] => Some(item.label.clone()),
        _ => Some(longest_common_prefix(
            matches
                .iter()
                .map(|item| item.label.as_str())
                .collect::<Vec<_>>()
                .as_slice(),
        ))
        .filter(|common| common.len() > prefix.len())
        .or_else(|| Some(prefix.to_string())),
    }
}

/// Compute the longest common UTF-8 prefix across labels.
fn longest_common_prefix(labels: &[&str]) -> String {
    let Some(first) = labels.first().copied() else {
        return String::new();
    };
    let mut out = String::new();
    for (index, ch) in first.char_indices() {
        let next_index = index + ch.len_utf8();
        let prefix = &first[..next_index];
        if labels.iter().all(|label| label.starts_with(prefix)) {
            out.push(ch);
        } else {
            break;
        }
    }
    out
}

/// Return true when a character may appear in a completion token.
fn is_completion_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == '#' || ch == '/'
}

/// Static command completion items for slash-command mode.
fn repl_command_completion_items() -> Vec<lane::LaneCompletionItem> {
    [
        ("/help", "Show REPL command help"),
        (
            "/info",
            "Show loaded modules, directives, and provided objects",
        ),
        ("/code", "Show the full session source"),
        ("/save", "Save the full session source to a file"),
        ("/export", "Write generated GLSL for the session to a file"),
        ("/show", "Open native preview for current session"),
        ("/split", "Toggle split transcript mode"),
        ("/clear", "Clear transcript and keep session"),
        ("/restart", "Reset session source and GLSL state"),
        ("/exit", "Exit the interactive shell"),
    ]
    .into_iter()
    .map(|(label, detail)| lane::LaneCompletionItem {
        label: label.to_string(),
        kind: lane::LaneCompletionKind::Keyword,
        detail: Some(detail.to_string()),
        documentation: None,
    })
    .collect()
}

/// Build the empty-input placeholder text block.
fn placeholder_input_text(gutter_width: u16) -> Text<'static> {
    current_input_text(
        Text::from(vec![
            Line::raw(""),
            Line::from(Span::styled(
                INPUT_PLACEHOLDER,
                Style::default().fg(INPUT_PLACEHOLDER_FG),
            )),
            Line::raw(""),
        ]),
        gutter_width,
    )
}

/// Prefix source text with fixed-width line numbers.
fn numbered_source_text(mut text: Text<'static>, line_start: usize) -> Text<'static> {
    let line_count = text.lines.len().max(1);
    let line_end = line_start.saturating_add(line_count.saturating_sub(1));
    let width = line_end.to_string().len();
    for (offset, line) in text.lines.iter_mut().enumerate() {
        let line_number = line_start.saturating_add(offset);
        let gutter = format!("{line_number:>width$} | ");
        line.spans
            .insert(0, Span::styled(gutter, Style::default().fg(LINE_NUMBER_FG)));
    }
    text
}

/// Lane-numbered variant wrapper around generic numbered source text.
fn numbered_lane_text(text: Text<'static>, line_start: usize) -> Text<'static> {
    numbered_source_text(text, line_start)
}

/// Width in characters reserved for line-number gutter.
fn line_number_gutter_width(line_number: usize) -> u16 {
    line_number.to_string().len().saturating_add(3) as u16
}

/// Build lane text with error annotations and gutter markers.
fn errored_lane_text(
    mut source_text: Text<'static>,
    line_start: usize,
    error: &str,
) -> Text<'static> {
    let source_lines = source_text.lines.len().max(1);
    let line_end = line_start.saturating_add(source_lines.saturating_sub(1));
    let width = line_end.to_string().len();
    let mut lines = Vec::new();

    let message_padding = " ".repeat(width.saturating_add(3));
    for line in error.lines() {
        let line = strip_error_line_reference(line);
        lines.push(Line::from(vec![
            Span::styled(message_padding.clone(), Style::default().fg(ERROR_FG)),
            Span::styled(line.to_string(), Style::default().fg(ERROR_FG)),
        ]));
    }
    if error.lines().next().is_none() {
        lines.push(Line::from(Span::styled(
            message_padding,
            Style::default().fg(ERROR_FG),
        )));
    }

    for line in source_text.lines.iter_mut() {
        let gutter = format!("{ERROR_LINE_MARKER:>width$} | ");
        line.spans
            .insert(0, Span::styled(gutter, Style::default().fg(LINE_NUMBER_FG)));
    }
    lines.extend(source_text.lines);
    Text::from(lines)
}

/// Build copyable text for errored lane entry including formatted error header.
fn errored_lane_copy_text(source: &str, error: &str) -> String {
    let mut text = error
        .lines()
        .map(strip_error_line_reference)
        .collect::<Vec<_>>()
        .join("\n");
    if !text.is_empty() && !source.is_empty() {
        text.push('\n');
    }
    text.push_str(source);
    text
}

/// Strip compiler-style `line N:` prefix noise from a copied error line.
fn strip_error_line_reference(line: &str) -> String {
    let mut words = line.split_whitespace();
    match (words.next(), words.next()) {
        (Some(_), Some(_)) if line.starts_with("line ") => words.collect::<Vec<_>>().join(" "),
        _ => line.to_string(),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GlslOutput {
    text: String,
    line_start: usize,
}

impl GlslOutput {
    /// True when the emitted GLSL block is empty.
    fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
}

/// Calculate only newly added lines and their starting row in a new GLSL text.
fn new_glsl_since(previous: &str, current: &str) -> GlslOutput {
    if previous.is_empty() {
        return GlslOutput {
            text: current.to_string(),
            line_start: 1,
        };
    }

    let previous_lines = previous.lines().collect::<Vec<_>>();
    let current_lines = current.lines().collect::<Vec<_>>();
    let mut lcs = vec![vec![0; current_lines.len() + 1]; previous_lines.len() + 1];
    for previous_index in (0..previous_lines.len()).rev() {
        for current_index in (0..current_lines.len()).rev() {
            lcs[previous_index][current_index] = if previous_lines[previous_index]
                == current_lines[current_index]
            {
                lcs[previous_index + 1][current_index + 1] + 1
            } else {
                lcs[previous_index + 1][current_index].max(lcs[previous_index][current_index + 1])
            };
        }
    }

    let mut added = Vec::new();
    let mut line_start = None;
    let mut previous_index = 0;
    let mut current_index = 0;
    while current_index < current_lines.len() {
        if previous_index < previous_lines.len()
            && previous_lines[previous_index] == current_lines[current_index]
        {
            previous_index += 1;
            current_index += 1;
        } else if previous_index >= previous_lines.len()
            || lcs[previous_index][current_index + 1] >= lcs[previous_index + 1][current_index]
        {
            line_start.get_or_insert(current_index + 1);
            added.push(current_lines[current_index]);
            current_index += 1;
        } else {
            previous_index += 1;
        }
    }

    while added.first().is_some_and(|line| line.is_empty()) {
        added.remove(0);
        if let Some(start) = &mut line_start {
            *start = start.saturating_add(1);
        }
    }
    while added.last().is_some_and(|line| line.is_empty()) {
        added.pop();
    }
    GlslOutput {
        text: added.join("\n"),
        line_start: line_start.unwrap_or(1),
    }
}

#[derive(Default)]
struct TranscriptLayout {
    entries: Vec<RenderedEntry>,
    panes: Vec<RenderedPane>,
}

impl TranscriptLayout {
    /// Register pane bounds for hit-testing.
    fn record_pane(&mut self, pane: SplitPane, area: Rect) {
        self.panes.push(RenderedPane { pane, area });
    }

    /// Lay out entries bottom-to-top within a rectangle, applying scroll + gaps.
    fn record_bottom_to_top(
        &mut self,
        area: Rect,
        entries: &[usize],
        transcript: &[TranscriptEntry],
        mut scroll_rows: usize,
    ) {
        let mut next_bottom = area.y.saturating_add(area.height);
        for (position, index) in entries.iter().enumerate() {
            let Some(entry) = transcript.get(*index) else {
                continue;
            };
            let entry_rows = entry.line_count() as usize;
            if scroll_rows >= entry_rows {
                scroll_rows -= entry_rows;
            } else {
                let scrolled_height = entry_rows.saturating_sub(scroll_rows) as u16;
                scroll_rows = 0;
                let height = scrolled_height.min(next_bottom.saturating_sub(area.y));
                if height == 0 {
                    break;
                }
                let y = next_bottom.saturating_sub(height);
                self.entries.push(RenderedEntry {
                    index: *index,
                    area: Rect {
                        x: area.x,
                        y,
                        width: area.width,
                        height,
                    },
                });
                next_bottom = y;
            }
            if next_bottom <= area.y {
                break;
            }
            let adjacent_without_gap = entries
                .get(position + 1)
                .and_then(|next_index| {
                    transcript
                        .get(*next_index)
                        .map(|next_entry| (*next_index, next_entry))
                })
                .is_some_and(|(next_index, next_entry)| {
                    index.abs_diff(next_index) == 1
                        && adjacent_without_feed_gap(entry.kind, next_entry.kind)
                });
            if !adjacent_without_gap {
                let gap_rows = FEED_ENTRY_GAP as usize;
                if scroll_rows >= gap_rows {
                    scroll_rows -= gap_rows;
                } else {
                    scroll_rows = 0;
                    next_bottom = next_bottom.saturating_sub(FEED_ENTRY_GAP);
                }
            }
            if next_bottom <= area.y {
                break;
            }
        }
    }

    /// Find an entry id under cursor coordinates.
    fn entry_at(&self, x: u16, y: u16) -> Option<usize> {
        self.entries
            .iter()
            .find(|entry| contains(entry.area, x, y))
            .map(|entry| entry.index)
    }

    /// Find a pane under cursor coordinates.
    fn pane_at(&self, x: u16, y: u16) -> Option<SplitPane> {
        self.panes
            .iter()
            .find(|pane| contains(pane.area, x, y))
            .map(|pane| pane.pane)
    }
}

#[derive(Clone, Copy)]
struct RenderedEntry {
    index: usize,
    area: Rect,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SplitPane {
    User,
    Glsl,
}

#[derive(Clone, Copy)]
enum ScrollDirection {
    Up,
    Down,
}

#[derive(Clone, Copy)]
struct RenderedPane {
    pane: SplitPane,
    area: Rect,
}

/// Hit-test a point inside a rectangle.
fn contains(area: Rect, x: u16, y: u16) -> bool {
    x >= area.x
        && x < area.x.saturating_add(area.width)
        && y >= area.y
        && y < area.y.saturating_add(area.height)
}

/// Write OSC-52 sequence to the current terminal for clipboard copy.
fn copy_text_to_clipboard(text: &str) -> io::Result<()> {
    let mut stdout = io::stdout();
    write_osc52_clipboard(&mut stdout, text)
}

/// Emit OSC-52 escape sequence through a writer.
fn write_osc52_clipboard(writer: &mut impl Write, text: &str) -> io::Result<()> {
    write!(writer, "\x1b]52;c;{}\x07", base64_encode(text.as_bytes()))?;
    writer.flush()
}

/// Base64 encode bytes for OSC-52 clipboard payload.
fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3).saturating_mul(4));
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        encoded.push(TABLE[(first >> 2) as usize] as char);
        encoded.push(TABLE[(((first & 0b0000_0011) << 4) | (second >> 4)) as usize] as char);
        if chunk.len() > 1 {
            encoded.push(TABLE[(((second & 0b0000_1111) << 2) | (third >> 6)) as usize] as char);
        } else {
            encoded.push('=');
        }
        if chunk.len() > 2 {
            encoded.push(TABLE[(third & 0b0011_1111) as usize] as char);
        } else {
            encoded.push('=');
        }
    }
    encoded
}

#[derive(Clone)]
struct TranscriptEntry {
    kind: TranscriptKind,
    text: String,
    group: Option<usize>,
    line_start: Option<usize>,
    errored: bool,
    error: Option<String>,
}

#[derive(Clone, Copy)]
enum TranscriptKind {
    Lane,
    Command,
    Glsl,
    Error,
    Help,
    System,
    Welcome,
}

impl TranscriptEntry {
    /// Create a command transcript row.
    fn command(text: String, group: Option<usize>) -> Self {
        Self {
            kind: TranscriptKind::Command,
            text,
            group,
            line_start: None,
            errored: false,
            error: None,
        }
    }

    /// Create a user/ lane transcript row, or command if it begins with `/`.
    fn submitted(text: String, group: Option<usize>, line_start: Option<usize>) -> Self {
        if text.starts_with('/') {
            Self::command(text, group)
        } else {
            Self {
                kind: TranscriptKind::Lane,
                text,
                group,
                line_start,
                errored: false,
                error: None,
            }
        }
    }

    /// Create a GLSL output transcript row.
    fn glsl(text: String, group: Option<usize>, line_start: Option<usize>) -> Self {
        Self {
            kind: TranscriptKind::Glsl,
            text,
            group,
            line_start,
            errored: false,
            error: None,
        }
    }

    /// Create an error row.
    fn error(text: String, group: Option<usize>) -> Self {
        Self {
            kind: TranscriptKind::Error,
            text,
            group,
            line_start: None,
            errored: false,
            error: None,
        }
    }

    /// Create a system row.
    fn system(text: impl Into<String>) -> Self {
        Self {
            kind: TranscriptKind::System,
            text: text.into(),
            group: None,
            line_start: None,
            errored: false,
            error: None,
        }
    }

    /// Create a welcome row shown on startup.
    fn welcome(text: impl Into<String>) -> Self {
        Self {
            kind: TranscriptKind::Welcome,
            text: text.into(),
            group: None,
            line_start: None,
            errored: false,
            error: None,
        }
    }

    /// Create a help-row used for `/help`.
    fn help(text: impl Into<String>) -> Self {
        Self {
            kind: TranscriptKind::Help,
            text: text.into(),
            group: None,
            line_start: None,
            errored: false,
            error: None,
        }
    }

    /// Return text users can copy from this entry, including error context.
    fn copyable_text(&self) -> Option<String> {
        match self.kind {
            TranscriptKind::Welcome => None,
            TranscriptKind::Lane if self.error.is_some() => Some(errored_lane_copy_text(
                &self.text,
                self.error.as_deref().unwrap_or(""),
            )),
            _ => Some(self.text.clone()),
        }
    }

    /// Count how many screen lines this entry consumes (including feed padding).
    fn line_count(&self) -> u16 {
        let lines = if matches!(self.kind, TranscriptKind::Lane) && self.error.is_some() {
            self.error
                .as_deref()
                .map(|error| error.lines().count().max(1))
                .unwrap_or(1)
                .saturating_add(self.text.lines().count().max(1)) as u16
        } else {
            self.text.lines().count().max(1) as u16
        };
        if matches!(self.kind, TranscriptKind::Command | TranscriptKind::Welcome) {
            return lines;
        }
        lines.saturating_add(2)
    }

    /// Style the row, with selected grouping emphasis and errored variants.
    fn style(&self, selected_group: Option<usize>) -> Style {
        if self.group.is_some() && self.group == selected_group {
            return match self.kind {
                TranscriptKind::Lane if self.errored => Style::default()
                    .fg(ERROR_FG)
                    .bg(SELECTED_ERROR_BG)
                    .add_modifier(Modifier::BOLD),
                TranscriptKind::Lane => Style::default()
                    .bg(SELECTED_USER_BG)
                    .add_modifier(Modifier::BOLD),
                TranscriptKind::Glsl => Style::default()
                    .bg(SELECTED_OUTPUT_BG)
                    .add_modifier(Modifier::BOLD),
                TranscriptKind::Error => Style::default()
                    .fg(ERROR_FG)
                    .bg(SELECTED_ERROR_BG)
                    .add_modifier(Modifier::BOLD),
                _ => self.base_style(),
            };
        }
        self.base_style()
    }

    /// Style for unselected rows.
    fn base_style(&self) -> Style {
        match self.kind {
            TranscriptKind::Lane if self.errored => Style::default().fg(ERROR_FG).bg(ERROR_BG),
            TranscriptKind::Lane => Style::default().bg(USER_BG),
            TranscriptKind::Command => Style::default().fg(COMMAND_FG),
            TranscriptKind::Glsl => Style::default().bg(OUTPUT_BG),
            TranscriptKind::Error => Style::default().fg(ERROR_FG).bg(ERROR_BG),
            TranscriptKind::Help => Style::default(),
            TranscriptKind::System => Style::default(),
            TranscriptKind::Welcome => Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        }
    }
}

#[derive(Default)]
pub(crate) struct ReplSession {
    source: String,
    emitted_glsl: String,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum SubmitOutcome {
    Accepted,
    Emitted(GlslOutput),
    Cleared,
    Restarted,
    Help,
    Info(String),
    Code(String),
    Saved(String),
    Exported(String),
    Show(String),
    ToggleSplit,
    Exit,
    Error(String),
}

impl ReplSession {
    /// Current source line number is one past last stored line.
    fn next_line_number(&self) -> usize {
        self.source.lines().count().saturating_add(1)
    }

    /// Submit a user block: either command, module-safe lane line, or const output.
    pub(crate) fn submit(&mut self, input: &str) -> SubmitOutcome {
        let line = input.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            return SubmitOutcome::Accepted;
        }
        if line.starts_with('/') {
            return self.run_command(line);
        }
        if is_module_directive(line) {
            return SubmitOutcome::Error("#module is not allowed in the interactive shell".into());
        }

        let candidate = append_line(&self.source, line);
        match crate::compile_program_output(&candidate) {
            Ok(glsl) => {
                self.source = candidate;
                if is_const_declaration(line) {
                    let emitted = new_glsl_since(&self.emitted_glsl, &glsl);
                    self.emitted_glsl = glsl;
                    SubmitOutcome::Emitted(emitted)
                } else {
                    SubmitOutcome::Accepted
                }
            }
            Err(err) => SubmitOutcome::Error(err.to_string()),
        }
    }

    /// Dispatch REPL commands beginning with `/`.
    fn run_command(&mut self, command: &str) -> SubmitOutcome {
        let command = command.trim_end();
        if let Some(path) = command.strip_prefix("/save ") {
            return self.save_source(path.trim());
        }
        if let Some(path) = command.strip_prefix("/export ") {
            return self.export_glsl(path.trim());
        }
        match command {
            "/clear" => SubmitOutcome::Cleared,
            "/code" => SubmitOutcome::Code(self.source.trim_end().to_string()),
            "/save" => SubmitOutcome::Error("usage: /save <filename>".to_string()),
            "/export" => SubmitOutcome::Error("usage: /export <filename>".to_string()),
            "/help" => SubmitOutcome::Help,
            "/info" => match lane::program_info(&self.source) {
                Ok(info) => SubmitOutcome::Info(format_program_info(&info)),
                Err(err) => SubmitOutcome::Error(err.to_string()),
            },
            "/show" => SubmitOutcome::Show(self.source.clone()),
            "/split" => SubmitOutcome::ToggleSplit,
            "/restart" => {
                self.source.clear();
                self.emitted_glsl.clear();
                SubmitOutcome::Restarted
            }
            "/exit" => SubmitOutcome::Exit,
            _ => SubmitOutcome::Error(format!("unknown shell command '{command}'")),
        }
    }

    /// Write source buffer to disk.
    fn save_source(&self, path: &str) -> SubmitOutcome {
        if path.is_empty() {
            return SubmitOutcome::Error("usage: /save <filename>".to_string());
        }
        match fs::write(path, &self.source) {
            Ok(()) => SubmitOutcome::Saved(format!("Saved session source to {path}.")),
            Err(err) => SubmitOutcome::Error(format!("save error: {err}")),
        }
    }

    /// Re-run and export the current source to a GLSL output file.
    fn export_glsl(&self, path: &str) -> SubmitOutcome {
        if path.is_empty() {
            return SubmitOutcome::Error("usage: /export <filename>".to_string());
        }
        match crate::compile_program_output(&self.source) {
            Ok(glsl) => match fs::write(path, glsl) {
                Ok(()) => SubmitOutcome::Exported(format!("Exported GLSL to {path}.")),
                Err(err) => SubmitOutcome::Error(format!("export error: {err}")),
            },
            Err(err) => SubmitOutcome::Error(err.to_string()),
        }
    }
}

/// Compose the short `/help` message.
fn help_text() -> String {
    [
        "Ctrl-F formats the current input.",
        "PageUp/PageDown or the mouse wheel scroll the transcript.",
        "Left/Right move through the current input.",
        "Right-click a transcript block to copy its text.",
        "/info shows loaded modules, used directives, and provided objects.",
        "/code shows the full session source.",
        "/save <filename> writes the session source to a file.",
        "/export <filename> writes generated GLSL to a file.",
        "/show opens a native preview window for the current session.",
        "/split toggles split mode.",
        "/clear clears the transcript but keeps the session.",
        "/restart starts from an empty session.",
        "/exit leaves.",
    ]
    .join("\n")
}

/// Convert program info into grouped, human-readable sections.
fn format_program_info(info: &lane::ProgramInfo) -> String {
    [
        info_section("Loaded modules", &info.loaded_modules),
        info_section("Used directives", &info.directives),
        info_section("Provided objects", &info.provided_objects),
    ]
    .join("\n")
}

/// Format a section header and list values or `(none)`.
fn info_section(title: &str, items: &[String]) -> String {
    if items.is_empty() {
        return format!("{title}:\n  (none)");
    }
    format!("{title}:\n  {}", items.join("\n  "))
}

/// Highlight helper text tokens like commands and key combos.
fn highlight_help_text(source: &str) -> Text<'static> {
    let lines = source
        .lines()
        .map(|line| {
            let mut spans = Vec::new();
            for part in line.split_inclusive(' ') {
                let token = part.trim_end_matches(' ');
                let spacing = &part[token.len()..];
                if token.starts_with('/') {
                    spans.push(Span::styled(
                        token.to_string(),
                        Style::default().fg(COMMAND_FG).add_modifier(Modifier::BOLD),
                    ));
                } else if is_help_keymap_token(token) {
                    spans.push(Span::styled(
                        token.to_string(),
                        Style::default().fg(KEYMAP_FG).add_modifier(Modifier::BOLD),
                    ));
                } else if !token.is_empty() {
                    spans.push(Span::raw(token.to_string()));
                }
                if !spacing.is_empty() {
                    spans.push(Span::raw(spacing.to_string()));
                }
            }
            Line::from(spans)
        })
        .collect::<Vec<_>>();
    Text::from(lines)
}

/// True when token resembles a keymap like `Ctrl-C`.
fn is_help_keymap_token(token: &str) -> bool {
    let cleaned = token.trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '-');
    if cleaned.is_empty() || !cleaned.contains('-') {
        return false;
    }
    cleaned
        .split('-')
        .all(|part| !part.is_empty() && part.chars().all(|ch| ch.is_ascii_alphabetic()))
}

/// Append one non-empty line to an existing source body.
fn append_line(source: &str, line: &str) -> String {
    let mut out = source.to_string();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(line);
    out.push('\n');
    out
}

/// Detect if a line is a `const` declaration.
fn is_const_declaration(line: &str) -> bool {
    strip_comment(line).trim_start().starts_with("const ")
}

/// Detect `#module` directives and reject them in interactive mode.
fn is_module_directive(line: &str) -> bool {
    strip_comment(line).trim() == "#module"
}

/// Remove inline `//` comments for declaration checks.
fn strip_comment(line: &str) -> &str {
    line.split_once("//").map_or(line, |(before, _)| before)
}

struct SyntaxHighlighter {
    lane: Option<HighlightConfiguration>,
    glsl: Option<HighlightConfiguration>,
    highlighter: Highlighter,
}

impl SyntaxHighlighter {
    /// Build lane/glsl highlighter configurations when available.
    fn new() -> Self {
        let lane = HighlightConfiguration::new(
            LANGUAGE_LANE.into(),
            "lane",
            include_str!("../tree-sitter-lane/queries/highlights.scm"),
            "",
            "",
        )
        .ok()
        .map(configure_highlights);
        let glsl = HighlightConfiguration::new(
            tree_sitter_glsl::LANGUAGE_GLSL.into(),
            "glsl",
            tree_sitter_glsl::HIGHLIGHTS_QUERY,
            "",
            "",
        )
        .ok()
        .map(configure_highlights);
        Self {
            lane,
            glsl,
            highlighter: Highlighter::new(),
        }
    }

    /// Highlight a Lane snippet.
    fn highlight_lane(&mut self, source: &str) -> Text<'static> {
        self.highlight(source, Syntax::Lane)
    }

    /// Highlight a GLSL snippet.
    fn highlight_glsl(&mut self, source: &str) -> Text<'static> {
        self.highlight(source, Syntax::Glsl)
    }

    /// Highlight source in the requested syntax, with safe fallback.
    fn highlight(&mut self, source: &str, syntax: Syntax) -> Text<'static> {
        let config = match syntax {
            Syntax::Lane => self.lane.as_ref(),
            Syntax::Glsl => self.glsl.as_ref(),
        };
        let Some(config) = config else {
            return Text::from(source.to_string());
        };
        match highlight_spans(&mut self.highlighter, config, source) {
            Ok(spans) => spans_to_text(spans),
            Err(_) => Text::from(source.to_string()),
        }
    }
}

enum Syntax {
    Lane,
    Glsl,
}

/// Configure tree-sitter highlight capture set from canonical capture names.
fn configure_highlights(mut config: HighlightConfiguration) -> HighlightConfiguration {
    config.configure(HIGHLIGHT_NAMES);
    config
}

/// Convert highlight events into styled spans.
fn highlight_spans(
    highlighter: &mut Highlighter,
    config: &HighlightConfiguration,
    source: &str,
) -> Result<Vec<Span<'static>>, HighlightError> {
    let mut spans = Vec::new();
    let mut stack = Vec::new();
    let events = highlighter.highlight(config, source.as_bytes(), None, |_| None)?;
    for event in events {
        match event? {
            HighlightEvent::Source { start, end } => {
                let style = stack
                    .last()
                    .copied()
                    .map_or_else(Style::default, highlight_style);
                spans.push(Span::styled(source[start..end].to_string(), style));
            }
            HighlightEvent::HighlightStart(Highlight(index)) => stack.push(index),
            HighlightEvent::HighlightEnd => {
                stack.pop();
            }
        }
    }
    Ok(spans)
}

/// Build text from styled spans while preserving newlines.
fn spans_to_text(spans: Vec<Span<'static>>) -> Text<'static> {
    let mut lines = vec![Line::default()];
    for span in spans {
        let mut pieces = span.content.split('\n').peekable();
        while let Some(piece) = pieces.next() {
            if !piece.is_empty() {
                lines
                    .last_mut()
                    .unwrap()
                    .spans
                    .push(Span::styled(piece.to_string(), span.style));
            }
            if pieces.peek().is_some() {
                lines.push(Line::default());
            }
        }
    }
    Text::from(lines)
}

/// Translate tree-sitter style indexes into terminal styles.
fn highlight_style(index: usize) -> Style {
    let name = HIGHLIGHT_NAMES.get(index).copied().unwrap_or("");
    match name {
        "comment" => Style::default().fg(Color::DarkGray),
        "constant" | "constant.builtin" => Style::default().fg(Color::LightYellow),
        "constructor" => Style::default().fg(Color::LightCyan),
        "function" | "function.builtin" => Style::default().fg(Color::Blue),
        "keyword" | "keyword.conditional" | "keyword.directive" => Style::default()
            .fg(Color::Rgb(210, 140, 255))
            .add_modifier(Modifier::BOLD),
        "number" => Style::default().fg(Color::LightGreen),
        "operator" | "punctuation.bracket" | "punctuation.delimiter" | "punctuation.special" => {
            Style::default().fg(Color::Gray)
        }
        "property" => Style::default().fg(Color::Cyan),
        "string" => Style::default().fg(Color::Green),
        "type" | "type.builtin" => Style::default().fg(Color::Yellow),
        "variable.parameter" => Style::default().fg(Color::LightRed),
        "variable" => Style::default().fg(Color::White),
        _ => Style::default(),
    }
}

#[cfg(test)]
#[path = "../tests/unit/repl.rs"]
mod tests;
