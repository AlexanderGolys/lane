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
use ratatui::widgets::{List, ListDirection, ListItem, Paragraph};
use ratatui::Terminal;
use tree_sitter_highlight::{
    Error as HighlightError, Highlight, HighlightConfiguration, HighlightEvent, Highlighter,
};
use tree_sitter_language::LanguageFn;

const USER_BG: Color = Color::Rgb(20, 22, 30);
const OUTPUT_BG: Color = Color::Rgb(14, 32, 24);
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

unsafe extern "C" {
    fn tree_sitter_lane() -> *const ();
}

const LANGUAGE_LANE: LanguageFn = unsafe { LanguageFn::from_raw(tree_sitter_lane) };

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut app = App::new();
    loop {
        let mut terminal = ReplTerminal::enter()?;
        let action = app.run(&mut terminal.terminal);
        terminal.leave()?;
        match action? {
            ReplAction::Exit => return Ok(()),
            ReplAction::Show(source) => {
                match crate::run_preview_source(&source) {
                    Ok(()) => app
                        .transcript
                        .push(TranscriptEntry::system("Preview closed.")),
                    Err(err) => app.transcript.push(TranscriptEntry::error(
                        format!("preview error: {}", err),
                        None,
                    )),
                }
            }
        }
    }
}

struct ReplTerminal {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    active: bool,
}

impl ReplTerminal {
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
    next_group: usize,
    layout: TranscriptLayout,
}

impl App {
    fn new() -> Self {
        Self::new_with_history_file(default_history_file())
    }

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
            next_group: 0,
            layout: TranscriptLayout::default(),
        }
    }

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
            MouseEventKind::ScrollUp => self.scroll_transcript_up(),
            MouseEventKind::ScrollDown => self.scroll_transcript_down(),
            _ => {}
        }
    }

    fn select_entry_at(&mut self, x: u16, y: u16) {
        self.selected_group = self
            .layout
            .entry_at(x, y)
            .and_then(|index| self.transcript.get(index))
            .and_then(|entry| entry.group);
    }

    fn copyable_text_at(&self, x: u16, y: u16) -> Option<String> {
        let index = self.layout.entry_at(x, y)?;
        let entry = self.transcript.get(index)?;
        entry.copyable_text().map(str::to_string)
    }

    fn insert_input_char(&mut self, ch: char) {
        self.input.insert(self.input_cursor, ch);
        self.input_cursor += ch.len_utf8();
    }

    fn backspace_input_char(&mut self) {
        let Some((previous, _)) = self.input[..self.input_cursor].char_indices().next_back()
        else {
            return;
        };
        self.input.drain(previous..self.input_cursor);
        self.input_cursor = previous;
    }

    fn move_input_cursor_left(&mut self) {
        if let Some((previous, _)) = self.input[..self.input_cursor].char_indices().next_back() {
            self.input_cursor = previous;
        }
    }

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

    fn scroll_transcript_up(&mut self) {
        self.transcript_scroll = self
            .transcript_scroll
            .saturating_add(1)
            .min(self.transcript.len().saturating_sub(1));
    }

    fn scroll_transcript_down(&mut self) {
        self.transcript_scroll = self.transcript_scroll.saturating_sub(1);
    }

    fn submit_input(&mut self) -> Option<ReplAction> {
        self.clear_history_navigation();
        self.clear_completion();
        let input = std::mem::take(&mut self.input);
        self.input_cursor = 0;
        if input.trim().is_empty() {
            return None;
        }
        self.transcript_scroll = 0;

        self.record_input_history(&input);
        let is_command = input.starts_with('/');
        let line_start = (!is_command).then(|| self.session.next_line_number());
        match self.session.submit(&input) {
            SubmitOutcome::Accepted => {
                if !is_command {
                    self.push_submitted_input(input.clone(), line_start, true);
                }
            }
            SubmitOutcome::Emitted(glsl) => {
                let group = self.push_submitted_input(input.clone(), line_start, true);
                if glsl.is_empty() {
                    return None;
                }
                self.transcript.push(TranscriptEntry::glsl(glsl, group));
            }
            SubmitOutcome::Cleared => {
                self.transcript
                    .push(TranscriptEntry::command(input.clone(), None));
                self.transcript.clear();
                self.selected_group = None;
            }
            SubmitOutcome::Restarted => {
                self.transcript
                    .push(TranscriptEntry::command(input.clone(), None));
                self.transcript.clear();
                self.selected_group = None;
                self.next_group = 0;
                self.transcript
                    .push(TranscriptEntry::system("Session restarted."));
            }
            SubmitOutcome::Help => {
                self.transcript
                    .push(TranscriptEntry::command(input.clone(), None));
                self.transcript.push(TranscriptEntry::help(help_text()));
            }
            SubmitOutcome::Info(info) => {
                self.transcript
                    .push(TranscriptEntry::command(input.clone(), None));
                self.transcript.push(TranscriptEntry::system(info));
            }
            SubmitOutcome::Code(source) => {
                self.transcript
                    .push(TranscriptEntry::command(input.clone(), None));
                self.transcript
                    .push(TranscriptEntry::submitted(source, None, Some(1)));
            }
            SubmitOutcome::Saved(message) | SubmitOutcome::Exported(message) => {
                self.transcript
                    .push(TranscriptEntry::command(input.clone(), None));
                self.transcript.push(TranscriptEntry::system(message));
            }
            SubmitOutcome::Show(source) => return Some(ReplAction::Show(source)),
            SubmitOutcome::ToggleSplit => {
                self.toggle_split();
            }
            SubmitOutcome::Exit => {
                self.transcript
                    .push(TranscriptEntry::command(input.clone(), None));
                return Some(ReplAction::Exit);
            }
            SubmitOutcome::Error(error) => {
                if is_command {
                    self.transcript.push(TranscriptEntry::error(error, None));
                } else {
                    let group = self.push_submitted_input(input.clone(), line_start, false);
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
        self.input.replace_range(start..self.input_cursor, &completed);
        self.input_cursor = start + completed.len();
    }

    fn clear_completion(&mut self) {
        self.completion_matches.clear();
        self.completion_index = 0;
    }

    fn record_input_history(&mut self, input: &str) {
        if self.input_history.last().is_some_and(|last| last == input) {
            return;
        }
        self.input_history.push(input.to_string());
        self.persist_input_history();
    }

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

    fn clear_history_navigation(&mut self) {
        self.history_position = None;
        self.history_draft.clear();
    }

    fn allocate_group(&mut self) -> usize {
        let group = self.next_group;
        self.next_group += 1;
        group
    }

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
            if let Some(entry) = self.transcript.last_mut() {
                if matches!(entry.kind, TranscriptKind::Lane) && !entry.errored {
                    if !entry.text.ends_with('\n') {
                        entry.text.push('\n');
                    }
                    entry.text.push_str(&input);
                    return entry.group;
                }
            }
        }
        let group = self.allocate_group();
        self.transcript
            .push(TranscriptEntry::submitted(input, Some(group), line_start));
        Some(group)
    }

    fn attach_group_error(&mut self, group: usize, error: String) {
        for entry in self.transcript.iter_mut().rev() {
            if entry.group == Some(group) && matches!(entry.kind, TranscriptKind::Lane) {
                entry.errored = true;
                entry.error = Some(error.clone());
                break;
            }
        }
    }

    fn toggle_split(&mut self) {
        self.split_mode = !self.split_mode;
    }

    fn draw(&mut self, frame: &mut ratatui::Frame) {
        self.layout = TranscriptLayout::default();
        if self.split_mode {
            let split = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(frame.area());
            let user_area = transcript_area(split[0]);
            let input_area = input_area(split[0]);
            let user_entries = self
                .transcript
                .iter()
                .enumerate()
                .rev()
                .filter(|(_, entry)| !matches!(entry.kind, TranscriptKind::Glsl))
                .skip(self.transcript_scroll)
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            let glsl_entries = self
                .transcript
                .iter()
                .enumerate()
                .rev()
                .filter(|(_, entry)| matches!(entry.kind, TranscriptKind::Glsl))
                .skip(self.transcript_scroll)
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            self.layout
                .record_bottom_to_top(user_area, user_entries.as_slice(), &self.transcript);
            self.layout.record_bottom_to_top(
                transcript_feed_area(split[1]),
                glsl_entries.as_slice(),
                &self.transcript,
            );
            let user_items = user_entries
                .iter()
                .map(|index| {
                    (
                        self.render_entry(&self.transcript[*index].clone()),
                        self.transcript[*index].kind,
                    )
                })
                .collect::<Vec<_>>();
            let glsl_items = glsl_entries
                .iter()
                .map(|index| {
                    (
                        self.render_entry(&self.transcript[*index].clone()),
                        self.transcript[*index].kind,
                    )
                })
                .collect::<Vec<_>>();
            let user_transcript = List::new(spaced_transcript_items(user_items))
                .direction(ListDirection::BottomToTop);
            let glsl_transcript = List::new(spaced_transcript_items(glsl_items))
                .direction(ListDirection::BottomToTop);
            frame.render_widget(user_transcript, user_area);
            frame.render_widget(glsl_transcript, transcript_feed_area(split[1]));
            self.render_input(frame, input_area);
        } else {
            let entries = self
                .transcript
                .iter()
                .enumerate()
                .rev()
                .skip(self.transcript_scroll)
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            self.layout.record_bottom_to_top(
                transcript_area(frame.area()),
                entries.as_slice(),
                &self.transcript,
            );
            let items = entries
                .iter()
                .map(|index| {
                    (
                        self.render_entry(&self.transcript[*index].clone()),
                        self.transcript[*index].kind,
                    )
                })
                .collect::<Vec<_>>();
            let transcript =
                List::new(spaced_transcript_items(items)).direction(ListDirection::BottomToTop);
            frame.render_widget(transcript, transcript_area(frame.area()));
            self.render_input(frame, input_area(frame.area()));
        }
    }

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
        let cursor_y = area.y.saturating_add(cursor_row.min(area.height.saturating_sub(1)));
        frame.set_cursor_position(Position::new(cursor_x, cursor_y));
    }

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

    fn current_input_gutter_width(&self) -> u16 {
        line_number_gutter_width(self.session.next_line_number())
    }

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

    fn completion_hint_suffix(&self) -> Option<String> {
        let (_, prefix) = completion_token(&self.input)?;
        if prefix.is_empty() {
            return None;
        }
        self.selected_completion_label(&prefix)
            .and_then(|label| label.strip_prefix(&prefix).map(str::to_string))
            .filter(|suffix| !suffix.is_empty())
    }

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

    fn active_completion_items(&self) -> Vec<lane::LaneCompletionItem> {
        if self.input.starts_with('/') {
            return repl_command_completion_items();
        }
        lane::lane_completion_items()
    }

    fn render_entry(&mut self, entry: &TranscriptEntry) -> ListItem<'static> {
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
            TranscriptKind::Glsl => self.highlighter.highlight_glsl(&entry.text),
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
        let text = left_padded_text(text);
        ListItem::new(text).style(entry.style(self.selected_group))
    }
}

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
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state")))?;
    Some(state_base.join("lane/repl_history"))
}

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

fn transcript_area(area: Rect) -> Rect {
    transcript_feed_area(Rect {
        height: area.height.saturating_sub(input_reserved_height(area)),
        ..area
    })
}

fn transcript_feed_area(area: Rect) -> Rect {
    let x_offset = FEED_X_OFFSET.min(area.width);
    Rect {
        x: area.x.saturating_add(x_offset),
        width: area.width.saturating_sub(x_offset),
        ..area
    }
}

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

fn input_reserved_height(area: Rect) -> u16 {
    input_height(area)
        .saturating_add(input_top_gap(area))
        .saturating_add(input_bottom_gap(area))
}

fn input_height(area: Rect) -> u16 {
    area.height
        .saturating_sub(input_bottom_gap(area))
        .min(FEED_ENTRY_MIN_HEIGHT)
}

fn input_bottom_gap(area: Rect) -> u16 {
    INPUT_BOTTOM_GAP.min(area.height.saturating_sub(FEED_ENTRY_MIN_HEIGHT))
}

fn input_top_gap(area: Rect) -> u16 {
    INPUT_TOP_GAP.min(
        area.height
            .saturating_sub(FEED_ENTRY_MIN_HEIGHT)
            .saturating_sub(input_bottom_gap(area)),
    )
}

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

fn adjacent_without_feed_gap(previous: TranscriptKind, current: TranscriptKind) -> bool {
    if matches!(previous, TranscriptKind::Command) {
        return matches!(
            current,
            TranscriptKind::Command
                | TranscriptKind::Help
                | TranscriptKind::System
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

fn padded_feed_text(mut text: Text<'static>) -> Text<'static> {
    if text.lines.is_empty() {
        text.lines.push(Line::raw(""));
    }
    text.lines.insert(0, Line::raw(""));
    text.lines.push(Line::raw(""));
    text
}

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

fn left_padded_text(mut text: Text<'static>) -> Text<'static> {
    for line in &mut text.lines {
        line.spans.insert(0, Span::raw(" "));
    }
    text
}

fn append_completion_hint(text: &mut Text<'static>, hint: Option<String>) {
    let Some(hint) = hint else {
        return;
    };
    if let Some(line) = text.lines.last_mut() {
        line.spans
            .push(Span::styled(hint, Style::default().fg(COMPLETION_HINT_FG)));
    }
}

fn current_input_text(mut text: Text<'static>, gutter_width: u16) -> Text<'static> {
    let gutter = " ".repeat(gutter_width as usize);
    for line in &mut text.lines {
        line.spans.insert(0, Span::raw(gutter.clone()));
        line.spans.insert(0, Span::raw(" "));
    }
    text
}

fn input_line_count(input: &str) -> usize {
    input.chars().filter(|ch| *ch == '\n').count() + 1
}

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

fn is_completion_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == '#' || ch == '/'
}

fn repl_command_completion_items() -> Vec<lane::LaneCompletionItem> {
    [
        ("/help", "Show REPL command help"),
        ("/info", "Show loaded modules, directives, and provided objects"),
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

fn numbered_lane_text(mut text: Text<'static>, line_start: usize) -> Text<'static> {
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

fn line_number_gutter_width(line_number: usize) -> u16 {
    line_number.to_string().len().saturating_add(3) as u16
}

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
            Span::styled(
                message_padding.clone(),
                Style::default().fg(ERROR_FG).bg(ERROR_BG),
            ),
            Span::styled(line.to_string(), Style::default().fg(ERROR_FG).bg(ERROR_BG)),
        ]));
    }
    if error.lines().next().is_none() {
        lines.push(Line::from(Span::styled(
            message_padding,
            Style::default().fg(ERROR_FG).bg(ERROR_BG),
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

fn strip_error_line_reference(line: &str) -> String {
    let mut words = line.split_whitespace();
    match (words.next(), words.next()) {
        (Some(_), Some(_)) if line.starts_with("line ") => words.collect::<Vec<_>>().join(" "),
        _ => line.to_string(),
    }
}

fn new_glsl_since(previous: &str, current: &str) -> String {
    if previous.is_empty() {
        return current.to_string();
    }

    let previous_lines = previous.lines().collect::<Vec<_>>();
    let current_lines = current.lines().collect::<Vec<_>>();
    let mut lcs = vec![vec![0; current_lines.len() + 1]; previous_lines.len() + 1];
    for previous_index in (0..previous_lines.len()).rev() {
        for current_index in (0..current_lines.len()).rev() {
            lcs[previous_index][current_index] =
                if previous_lines[previous_index] == current_lines[current_index] {
                    lcs[previous_index + 1][current_index + 1] + 1
                } else {
                    lcs[previous_index + 1][current_index]
                        .max(lcs[previous_index][current_index + 1])
                };
        }
    }

    let mut added = Vec::new();
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
            added.push(current_lines[current_index]);
            current_index += 1;
        } else {
            previous_index += 1;
        }
    }

    while added.first().is_some_and(|line| line.is_empty()) {
        added.remove(0);
    }
    while added.last().is_some_and(|line| line.is_empty()) {
        added.pop();
    }
    added.join("\n")
}

#[derive(Default)]
struct TranscriptLayout {
    entries: Vec<RenderedEntry>,
}

impl TranscriptLayout {
    fn record_bottom_to_top(
        &mut self,
        area: Rect,
        entries: &[usize],
        transcript: &[TranscriptEntry],
    ) {
        let mut next_bottom = area.y.saturating_add(area.height);
        for (position, index) in entries.iter().enumerate() {
            let Some(entry) = transcript.get(*index) else {
                continue;
            };
            let height = entry.line_count().min(next_bottom.saturating_sub(area.y));
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
            if next_bottom <= area.y {
                break;
            }
            let next_kind = entries
                .get(position + 1)
                .and_then(|next_index| transcript.get(*next_index))
                .map(|entry| entry.kind);
            if !next_kind.is_some_and(|kind| adjacent_without_feed_gap(entry.kind, kind)) {
                next_bottom = next_bottom.saturating_sub(FEED_ENTRY_GAP);
            }
            if next_bottom <= area.y {
                break;
            }
        }
    }

    fn entry_at(&self, x: u16, y: u16) -> Option<usize> {
        self.entries
            .iter()
            .find(|entry| contains(entry.area, x, y))
            .map(|entry| entry.index)
    }
}

struct RenderedEntry {
    index: usize,
    area: Rect,
}

fn contains(area: Rect, x: u16, y: u16) -> bool {
    x >= area.x
        && x < area.x.saturating_add(area.width)
        && y >= area.y
        && y < area.y.saturating_add(area.height)
}

fn copy_text_to_clipboard(text: &str) -> io::Result<()> {
    let mut stdout = io::stdout();
    write_osc52_clipboard(&mut stdout, text)
}

fn write_osc52_clipboard(writer: &mut impl Write, text: &str) -> io::Result<()> {
    write!(writer, "\x1b]52;c;{}\x07", base64_encode(text.as_bytes()))?;
    writer.flush()
}

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

    fn glsl(text: String, group: Option<usize>) -> Self {
        Self {
            kind: TranscriptKind::Glsl,
            text,
            group,
            line_start: None,
            errored: false,
            error: None,
        }
    }

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

    fn copyable_text(&self) -> Option<&str> {
        match self.kind {
            TranscriptKind::Welcome => None,
            _ => Some(self.text.as_str()),
        }
    }

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
    Emitted(String),
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
    fn next_line_number(&self) -> usize {
        self.source.lines().count().saturating_add(1)
    }

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

    fn save_source(&self, path: &str) -> SubmitOutcome {
        if path.is_empty() {
            return SubmitOutcome::Error("usage: /save <filename>".to_string());
        }
        match fs::write(path, &self.source) {
            Ok(()) => SubmitOutcome::Saved(format!("Saved session source to {path}.")),
            Err(err) => SubmitOutcome::Error(format!("save error: {err}")),
        }
    }

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

fn format_program_info(info: &lane::ProgramInfo) -> String {
    [
        info_section("Loaded modules", &info.loaded_modules),
        info_section("Used directives", &info.directives),
        info_section("Provided objects", &info.provided_objects),
    ]
    .join("\n")
}

fn info_section(title: &str, items: &[String]) -> String {
    if items.is_empty() {
        return format!("{title}:\n  (none)");
    }
    format!("{title}:\n  {}", items.join("\n  "))
}

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

fn is_help_keymap_token(token: &str) -> bool {
    let cleaned = token.trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '-');
    if cleaned.is_empty() || !cleaned.contains('-') {
        return false;
    }
    cleaned
        .split('-')
        .all(|part| !part.is_empty() && part.chars().all(|ch| ch.is_ascii_alphabetic()))
}

fn append_line(source: &str, line: &str) -> String {
    let mut out = source.to_string();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(line);
    out.push('\n');
    out
}

fn is_const_declaration(line: &str) -> bool {
    strip_comment(line).trim_start().starts_with("const ")
}

fn is_module_directive(line: &str) -> bool {
    strip_comment(line).trim() == "#module"
}

fn strip_comment(line: &str) -> &str {
    line.split_once("//").map_or(line, |(before, _)| before)
}

struct SyntaxHighlighter {
    lane: Option<HighlightConfiguration>,
    glsl: Option<HighlightConfiguration>,
    highlighter: Highlighter,
}

impl SyntaxHighlighter {
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

    fn highlight_lane(&mut self, source: &str) -> Text<'static> {
        self.highlight(source, Syntax::Lane)
    }

    fn highlight_glsl(&mut self, source: &str) -> Text<'static> {
        self.highlight(source, Syntax::Glsl)
    }

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

fn configure_highlights(mut config: HighlightConfiguration) -> HighlightConfiguration {
    config.configure(HIGHLIGHT_NAMES);
    config
}

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

fn highlight_style(index: usize) -> Style {
    let name = HIGHLIGHT_NAMES.get(index).copied().unwrap_or("");
    match name {
        "comment" => Style::default().fg(Color::DarkGray),
        "constant" | "constant.builtin" => Style::default().fg(Color::LightYellow),
        "constructor" => Style::default().fg(Color::LightCyan),
        "function" | "function.builtin" => Style::default().fg(Color::Blue),
        "keyword" | "keyword.conditional" | "keyword.directive" => Style::default()
            .fg(Color::LightMagenta)
            .add_modifier(Modifier::BOLD),
        "number" => Style::default().fg(Color::LightGreen),
        "operator" | "punctuation.bracket" | "punctuation.delimiter" | "punctuation.special" => {
            Style::default().fg(Color::Gray)
        }
        "property" => Style::default().fg(Color::Cyan),
        "string" => Style::default().fg(Color::Green),
        "type" | "type.builtin" => Style::default().fg(Color::Yellow),
        "variable.parameter" => Style::default().fg(Color::LightBlue),
        "variable" => Style::default().fg(Color::White),
        _ => Style::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_history_file() -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("lane-repl-history-test-{unique}.txt"))
    }

    fn set_app_input(app: &mut App, input: &str) {
        app.input = input.to_string();
        app.input_cursor = app.input.len();
    }

    #[test]
    fn accepts_setup_and_emits_after_const() {
        let mut session = ReplSession::default();
        assert_eq!(session.submit("R radius = 1"), SubmitOutcome::Accepted);
        let outcome = session.submit("const Object output = Ball3D(r=radius)");
        let SubmitOutcome::Emitted(glsl) = outcome else {
            panic!("expected GLSL output");
        };
        assert!(glsl.contains("sdf0_Ball3D(p, ParamBall3D(radius))"));
    }

    #[test]
    fn later_const_emits_only_lines_after_previous_output() {
        let mut session = ReplSession::default();
        let SubmitOutcome::Emitted(first_glsl) = session.submit("const R radius = 1") else {
            panic!("expected first GLSL output");
        };
        assert!(first_glsl.contains("const float radius = 1.0f;"));
        let outcome = session.submit("const Object output = Ball3D(r=radius)");
        let SubmitOutcome::Emitted(glsl) = outcome else {
            panic!("expected GLSL output");
        };
        assert!(glsl.contains("struct ParamBall3D"));
        assert!(glsl.contains("float scene_sdf(vec3 p)"));
        assert!(!glsl.contains("const float radius = 1.0f;"));
    }

    #[test]
    fn later_const_emits_new_product_structs_inserted_before_old_output() {
        let mut session = ReplSession::default();
        assert_eq!(session.submit("Set Pair = R x R"), SubmitOutcome::Accepted);
        let first = session.submit("const Pair pair = Pair(1, 2)");
        assert!(matches!(first, SubmitOutcome::Emitted(_)));

        assert_eq!(
            session.submit("Set Triple = R x R x R"),
            SubmitOutcome::Accepted
        );
        let SubmitOutcome::Emitted(second) = session.submit("const Triple triple = Triple(1, 2, 3)")
        else {
            panic!("expected emitted GLSL");
        };

        assert!(second.contains("struct Triple"));
        assert!(second.contains("const Triple triple = Triple(1.0f, 2.0f, 3.0f);"));
        assert!(!second.contains("struct Pair"));
        assert!(!second.contains("const Pair pair"));
    }

    #[test]
    fn new_glsl_since_keeps_insertions_before_existing_lines() {
        let previous = "struct Pair {\n    float x;\n};\n\nconst Pair pair = Pair(1.0);";
        let current = concat!(
            "struct Pair {\n    float x;\n};\n\n",
            "struct Triple {\n    float x;\n};\n\n",
            "const Pair pair = Pair(1.0);\n",
            "const Triple triple = Triple(1.0);"
        );

        let added = new_glsl_since(previous, current);

        assert!(added.contains("struct Triple"));
        assert!(added.contains("const Triple triple = Triple(1.0);"));
        assert!(!added.contains("struct Pair"));
        assert!(!added.contains("const Pair pair"));
    }

    #[test]
    fn invalid_line_does_not_mutate_session() {
        let mut session = ReplSession::default();
        assert_eq!(session.submit("R radius = 1"), SubmitOutcome::Accepted);
        assert!(matches!(
            session.submit("const Object broken = Missing("),
            SubmitOutcome::Error(_)
        ));
        let outcome = session.submit("const Object output = Ball3D(r=radius)");
        assert!(matches!(outcome, SubmitOutcome::Emitted(_)));
    }

    #[test]
    fn rejects_module_directive() {
        let mut session = ReplSession::default();
        assert_eq!(
            session.submit("#module"),
            SubmitOutcome::Error("#module is not allowed in the interactive shell".into())
        );
    }

    #[test]
    fn restart_clears_accumulated_source() {
        let mut session = ReplSession::default();
        assert_eq!(session.submit("R radius = 1"), SubmitOutcome::Accepted);
        assert_eq!(session.submit("/restart"), SubmitOutcome::Restarted);
        let outcome = session.submit("const Object output = Ball3D(r=radius)");
        assert!(matches!(outcome, SubmitOutcome::Error(_)));
    }

    #[test]
    fn split_command_toggles_split_mode() {
        let mut session = ReplSession::default();
        assert_eq!(session.submit("/split"), SubmitOutcome::ToggleSplit);
    }

    #[test]
    fn split_toggle_preserves_transcript_text() {
        let mut app = App::new();
        app.transcript.clear();
        app.transcript.push(TranscriptEntry::submitted(
            "const R radius = 1".to_string(),
            Some(0),
            Some(1),
        ));
        app.transcript
            .push(TranscriptEntry::glsl("float radius = 1.0;".to_string(), Some(0)));
        app.input = "/split".to_string();

        app.submit_input();

        assert!(app.split_mode);
        assert_eq!(app.transcript.len(), 2);
        assert_eq!(app.transcript[0].text, "const R radius = 1");
        assert_eq!(app.transcript[1].text, "float radius = 1.0;");
    }

    #[test]
    fn left_and_right_arrows_move_input_cursor_for_insertions() {
        let mut app = App::new();
        app.input = "ab".to_string();
        app.input_cursor = app.input.len();

        app.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::Char('X'), KeyModifiers::NONE));
        assert_eq!(app.input, "aXb");

        app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::Char('Y'), KeyModifiers::NONE));
        assert_eq!(app.input, "aXbY");
    }

    #[test]
    fn backspace_removes_character_before_input_cursor() {
        let mut app = App::new();
        app.input = "abc".to_string();
        app.input_cursor = 2;

        app.handle_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));

        assert_eq!(app.input, "ac");
        assert_eq!(app.input_cursor, 1);
    }

    #[test]
    fn transcript_scroll_moves_and_submit_returns_to_latest() {
        let mut app = App::new();
        app.transcript = vec![
            TranscriptEntry::system("old"),
            TranscriptEntry::system("middle"),
            TranscriptEntry::system("new"),
        ];

        app.scroll_transcript_up();
        app.scroll_transcript_up();
        assert_eq!(app.transcript_scroll, 2);
        app.scroll_transcript_down();
        assert_eq!(app.transcript_scroll, 1);

        app.input = "R radius = 1".to_string();
        app.input_cursor = app.input.len();
        app.submit_input();

        assert_eq!(app.transcript_scroll, 0);
    }

    #[test]
    fn input_cursor_position_tracks_multiline_columns() {
        assert_eq!(input_cursor_position("abc\nde", 6), (1, 2));
    }

    #[test]
    fn help_command_prints_help() {
        let mut session = ReplSession::default();
        assert_eq!(session.submit("/help"), SubmitOutcome::Help);
    }

    #[test]
    fn help_command_ignores_trailing_spaces() {
        let mut session = ReplSession::default();
        assert_eq!(session.submit("/help   "), SubmitOutcome::Help);
    }

    #[test]
    fn help_text_omits_redundant_basic_keymap_lines() {
        let text = help_text();
        assert!(!text.contains("Enter submits."));
        assert!(!text.contains("Shift-Enter inserts a newline"));
        assert!(!text.contains("Up and Down recall submitted input history."));
        assert!(!text.contains("Tab completes to the longest unambiguous prefix"));
        assert!(text.contains("Ctrl-F formats the current input."));
        assert!(text.contains("Right-click a transcript block to copy its text."));
    }

    #[test]
    fn help_highlighting_marks_key_combinations_in_blue() {
        let text = highlight_help_text("Ctrl-F formats current input.");
        assert_eq!(text.lines[0].spans[0].content.as_ref(), "Ctrl-F");
        assert_eq!(text.lines[0].spans[0].style.fg, Some(KEYMAP_FG));
    }

    #[test]
    fn unknown_command_ignores_trailing_spaces() {
        let mut session = ReplSession::default();
        assert_eq!(
            session.submit("/wat   "),
            SubmitOutcome::Error("unknown shell command '/wat'".to_string())
        );
    }

    #[test]
    fn info_command_reports_session_metadata() {
        let mut session = ReplSession::default();
        assert_eq!(session.submit("#import std"), SubmitOutcome::Accepted);
        assert_eq!(session.submit("#2D"), SubmitOutcome::Accepted);
        assert_eq!(session.submit("#prec 0.002"), SubmitOutcome::Accepted);
        assert_eq!(session.submit("provided R time"), SubmitOutcome::Accepted);

        let SubmitOutcome::Info(info) = session.submit("/info") else {
            panic!("expected info output");
        };
        assert!(info.contains("Loaded modules:\n  std"));
        assert!(info.contains("Used directives:\n  #2D\n  #prec 0.002"));
        assert!(info.contains("Provided objects:\n  R time"));
    }

    #[test]
    fn info_command_reports_empty_sections() {
        let mut session = ReplSession::default();

        assert_eq!(
            session.submit("/info"),
            SubmitOutcome::Info(
                "Loaded modules:\n  (none)\nUsed directives:\n  (none)\nProvided objects:\n  (none)"
                    .to_string()
            )
        );
    }

    #[test]
    fn show_command_returns_current_source() {
        let mut session = ReplSession::default();
        assert_eq!(session.submit("R radius = 1"), SubmitOutcome::Accepted);
        assert_eq!(
            session.submit("/show"),
            SubmitOutcome::Show("R radius = 1\n".to_string())
        );
    }

    #[test]
    fn code_command_returns_full_session_source() {
        let mut session = ReplSession::default();
        assert_eq!(session.submit("R radius = 1"), SubmitOutcome::Accepted);
        assert_eq!(
            session.submit("/code"),
            SubmitOutcome::Code("R radius = 1".to_string())
        );
    }

    #[test]
    fn save_command_writes_full_session_source() {
        let path = temp_history_file();
        let mut session = ReplSession::default();
        assert_eq!(session.submit("R radius = 1"), SubmitOutcome::Accepted);

        let outcome = session.submit(&format!("/save {}", path.display()));

        assert!(matches!(outcome, SubmitOutcome::Saved(_)));
        assert_eq!(fs::read_to_string(&path).unwrap(), "R radius = 1\n");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn export_command_writes_generated_glsl() {
        let path = temp_history_file();
        let mut session = ReplSession::default();
        assert_eq!(session.submit("const R radius = 1"), SubmitOutcome::Emitted(
            crate::compile_program_output("const R radius = 1\n").unwrap()
        ));

        let outcome = session.submit(&format!("/export {}", path.display()));

        assert!(matches!(outcome, SubmitOutcome::Exported(_)));
        assert!(fs::read_to_string(&path)
            .unwrap()
            .contains("const float radius = 1.0f;"));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn file_commands_require_a_filename() {
        let mut session = ReplSession::default();

        assert_eq!(
            session.submit("/save"),
            SubmitOutcome::Error("usage: /save <filename>".to_string())
        );
        assert_eq!(
            session.submit("/save   "),
            SubmitOutcome::Error("usage: /save <filename>".to_string())
        );
        assert_eq!(
            session.submit("/export   "),
            SubmitOutcome::Error("usage: /export <filename>".to_string())
        );
    }

    #[test]
    fn slash_command_only_works_when_it_starts_the_input_block() {
        let mut session = ReplSession::default();
        assert_eq!(session.submit("/help\nR radius = 1"), SubmitOutcome::Error("unknown shell command '/help\nR radius = 1'".to_string()));
        assert!(matches!(
            session.submit("R radius = 1\n/help"),
            SubmitOutcome::Error(_)
        ));
    }

    #[test]
    fn session_tracks_next_source_line_number() {
        let mut session = ReplSession::default();
        assert_eq!(session.next_line_number(), 1);

        assert_eq!(session.submit("R radius = 1"), SubmitOutcome::Accepted);
        assert_eq!(session.next_line_number(), 2);

        assert!(matches!(
            session.submit("const Object broken = Missing("),
            SubmitOutcome::Error(_)
        ));
        assert_eq!(session.next_line_number(), 2);
    }

    #[test]
    fn consecutive_lane_submissions_share_one_feed_box() {
        let mut app = App::new();
        app.transcript.clear();

        let first_group = app.push_submitted_input("R radius = 1".to_string(), Some(1), true);
        let second_group =
            app.push_submitted_input("R diameter = radius * 2".to_string(), Some(2), true);

        assert_eq!(first_group, second_group);
        assert_eq!(app.transcript.len(), 1);
        assert_eq!(
            app.transcript[0].text,
            "R radius = 1\nR diameter = radius * 2"
        );
        assert_eq!(app.transcript[0].line_start, Some(1));
    }

    #[test]
    fn app_loads_persisted_input_history() {
        let history_file = temp_history_file();
        fs::write(
            &history_file,
            "R radius = 1\n\nconst Object output = Ball3D(r=radius)\n",
        )
        .unwrap();

        let app = App::new_with_history_file(Some(history_file.clone()));

        assert_eq!(
            app.input_history,
            vec![
                "R radius = 1".to_string(),
                "const Object output = Ball3D(r=radius)".to_string()
            ]
        );
        let _ = fs::remove_file(history_file);
    }

    #[test]
    fn submitting_input_persists_history_to_disk() {
        let history_file = temp_history_file();
        let mut app = App::new_with_history_file(Some(history_file.clone()));

        app.input = "R radius = 1".to_string();
        app.submit_input();
        app.input = "const R radius2 = 2".to_string();
        app.submit_input();

        let stored = fs::read_to_string(&history_file).unwrap();
        assert_eq!(stored, "R radius = 1\nconst R radius2 = 2\n");
        let _ = fs::remove_file(history_file);
    }

    #[test]
    fn up_arrow_recalls_previous_submitted_input() {
        let mut app = App::new();

        app.input = "R radius = 1".to_string();
        app.submit_input();
        app.input = "const R diameter = radius * 2".to_string();
        app.submit_input();

        app.handle_key(KeyEvent::from(KeyCode::Up));
        assert_eq!(app.input, "const R diameter = radius * 2");

        app.handle_key(KeyEvent::from(KeyCode::Up));
        assert_eq!(app.input, "R radius = 1");

        app.handle_key(KeyEvent::from(KeyCode::Up));
        assert_eq!(app.input, "R radius = 1");
    }

    #[test]
    fn down_arrow_moves_forward_and_restores_draft() {
        let mut app = App::new();

        app.input = "R radius = 1".to_string();
        app.submit_input();
        app.input = "const R diameter = radius * 2".to_string();
        app.submit_input();
        app.input = "R draft = ".to_string();

        app.handle_key(KeyEvent::from(KeyCode::Up));
        assert_eq!(app.input, "const R diameter = radius * 2");
        app.handle_key(KeyEvent::from(KeyCode::Up));
        assert_eq!(app.input, "R radius = 1");
        app.handle_key(KeyEvent::from(KeyCode::Down));
        assert_eq!(app.input, "const R diameter = radius * 2");
        app.handle_key(KeyEvent::from(KeyCode::Down));
        assert_eq!(app.input, "R draft = ");
    }

    #[test]
    fn editing_recalled_input_starts_a_new_history_navigation() {
        let mut app = App::new();

        app.input = "R radius = 1".to_string();
        app.submit_input();
        app.handle_key(KeyEvent::from(KeyCode::Up));
        app.handle_key(KeyEvent::from(KeyCode::Char('0')));

        assert_eq!(app.input, "R radius = 10");
        app.handle_key(KeyEvent::from(KeyCode::Down));
        assert_eq!(app.input, "R radius = 10");
    }

    #[test]
    fn empty_input_shows_gray_placeholder_text() {
        let mut app = App::new();

        let text = app.input_text();

        assert_eq!(text.lines.len(), 3);
        assert_eq!(text.lines[1].spans[0].content.as_ref(), " ");
        assert_eq!(text.lines[1].spans[1].content.as_ref(), "    ");
        assert_eq!(text.lines[1].spans[2].content.as_ref(), INPUT_PLACEHOLDER);
        assert_eq!(text.lines[1].spans[2].style.fg, Some(INPUT_PLACEHOLDER_FG));
    }

    #[test]
    fn typed_input_replaces_placeholder_text() {
        let mut app = App::new();
        app.input = "R radius = 1".to_string();

        let text = app.input_text();
        let rendered = text
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .map(|span| span.content.as_ref())
            .collect::<String>();

        assert!(rendered.contains("R radius = 1"));
        assert!(!rendered.contains(INPUT_PLACEHOLDER));
    }

    #[test]
    fn current_input_uses_blank_gutter_to_align_with_numbered_lane_source() {
        let mut app = App::new();
        app.input = "R radius = 1".to_string();

        let text = app.input_text();

        assert_eq!(text.lines[1].spans[0].content.as_ref(), " ");
        assert_eq!(text.lines[1].spans[1].content.as_ref(), "    ");
        assert_eq!(
            app.current_input_gutter_width(),
            line_number_gutter_width(1)
        );
    }

    #[test]
    fn current_input_gutter_widens_with_next_source_line_number() {
        let mut app = App::new();
        app.session.source = (1..=9)
            .map(|line| format!("R r{line} = {line}"))
            .collect::<Vec<_>>()
            .join("\n");
        app.input = "R radius = 1".to_string();

        let text = app.input_text();
        let numbered = numbered_lane_text(Text::from("R radius = 1"), 10);

        assert_eq!(text.lines[1].spans[0].content.as_ref(), " ");
        assert_eq!(text.lines[1].spans[1].content.as_ref(), "     ");
        assert_eq!(
            text.lines[1].spans[1].content.chars().count(),
            numbered.lines[0].spans[0].content.chars().count()
        );
    }

    #[test]
    fn empty_input_placeholder_uses_next_source_line_gutter_width() {
        let mut app = App::new();
        app.session.source = (1..=9)
            .map(|line| format!("R r{line} = {line}"))
            .collect::<Vec<_>>()
            .join("\n");

        let text = app.input_text();

        assert_eq!(text.lines[1].spans[0].content.as_ref(), " ");
        assert_eq!(text.lines[1].spans[1].content.as_ref(), "     ");
        assert_eq!(text.lines[1].spans[2].content.as_ref(), INPUT_PLACEHOLDER);
    }

    #[test]
    fn tab_completes_current_input_from_lsp_items() {
        let mut app = App::new();
        set_app_input(&mut app, "const Object output = Bal");

        app.handle_key(KeyEvent::from(KeyCode::Tab));

        assert_eq!(app.input, "const Object output = Ball");
        assert!(app
            .completion_matches
            .iter()
            .any(|item| item.label == "Ball3D"));
    }

    #[test]
    fn tab_completes_repl_commands_when_input_starts_with_slash() {
        let mut app = App::new();
        set_app_input(&mut app, "/he");

        app.handle_key(KeyEvent::from(KeyCode::Tab));

        assert_eq!(app.input, "/help");
        assert!(app
            .completion_matches
            .iter()
            .any(|item| item.label == "/help"));
    }

    #[test]
    fn tab_does_not_extend_beyond_ambiguous_command_prefix() {
        let mut app = App::new();
        set_app_input(&mut app, "/s");

        app.handle_key(KeyEvent::from(KeyCode::Tab));

        assert_eq!(app.input, "/s");
        assert!(app
            .completion_matches
            .iter()
            .any(|item| item.label == "/show"));
        assert!(app
            .completion_matches
            .iter()
            .any(|item| item.label == "/split"));
    }

    #[test]
    fn input_shows_gray_completion_hint_without_inserting_it() {
        let mut app = App::new();
        set_app_input(&mut app, "const Object output = Bal");

        let text = app.input_text();
        let hint = text.lines[1]
            .spans
            .last()
            .expect("expected completion hint");

        assert_eq!(app.input, "const Object output = Bal");
        assert_eq!(hint.content.as_ref(), "l2D");
        assert_eq!(hint.style.fg, Some(COMPLETION_HINT_FG));
    }

    #[test]
    fn completion_hint_uses_selected_tab_completion_candidate() {
        let mut app = App::new();
        set_app_input(&mut app, "const Object output = Bal");
        app.completion_matches = lane::lane_completion_items()
            .into_iter()
            .filter(|item| matches!(item.label.as_str(), "Ball2D" | "Ball3D"))
            .collect();
        app.completion_index = app
            .completion_matches
            .iter()
            .position(|item| item.label == "Ball3D")
            .expect("expected Ball3D completion");

        let text = app.input_text();
        let hint = text.lines[1]
            .spans
            .last()
            .expect("expected completion hint");

        assert_eq!(hint.content.as_ref(), "l3D");
        assert_eq!(hint.style.fg, Some(COMPLETION_HINT_FG));
    }

    #[test]
    fn slash_input_shows_command_completion_hint() {
        let mut app = App::new();
        set_app_input(&mut app, "/sp");

        let text = app.input_text();
        let hint = text.lines[1]
            .spans
            .last()
            .expect("expected completion hint");

        assert_eq!(hint.content.as_ref(), "lit");
        assert_eq!(hint.style.fg, Some(COMPLETION_HINT_FG));
    }

    #[test]
    fn ctrl_f_formats_current_input() {
        let mut app = App::new();
        set_app_input(
            &mut app,
            "R radius = 1   \n\n\nconst R diameter = radius * 2   ",
        );

        app.handle_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL));

        assert_eq!(app.input, "R radius = 1\n\nconst R diameter = radius * 2");
    }

    #[test]
    fn shift_enter_inserts_newline_without_submitting() {
        let mut app = App::new();
        app.transcript.clear();
        set_app_input(&mut app, "R radius = 1");

        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT));

        assert_eq!(app.input, "R radius = 1\n");
        assert!(app.transcript.is_empty());
    }

    #[test]
    fn alt_enter_inserts_newline_without_submitting() {
        let mut app = App::new();
        app.transcript.clear();
        set_app_input(&mut app, "R radius = 1");

        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::ALT));

        assert_eq!(app.input, "R radius = 1\n");
        assert!(app.transcript.is_empty());
    }

    #[test]
    fn ctrl_enter_submits_input() {
        let mut app = App::new();
        app.transcript.clear();
        app.input = "R radius = 1".to_string();

        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL));

        assert!(app.input.is_empty());
        assert_eq!(app.transcript.len(), 1);
        assert!(matches!(app.transcript[0].kind, TranscriptKind::Lane));
    }

    #[test]
    fn lane_submission_after_message_starts_new_feed_box() {
        let mut app = App::new();
        app.transcript.clear();

        app.push_submitted_input("R radius = 1".to_string(), Some(1), true);
        app.transcript
            .push(TranscriptEntry::system("Session note."));
        app.push_submitted_input("R diameter = radius * 2".to_string(), Some(2), true);

        assert_eq!(app.transcript.len(), 3);
        assert_eq!(app.transcript[0].text, "R radius = 1");
        assert_eq!(app.transcript[2].text, "R diameter = radius * 2");
    }

    #[test]
    fn bottom_to_top_layout_places_first_rendered_entry_at_bottom() {
        let entries = vec![
            TranscriptEntry::system("old"),
            TranscriptEntry::submitted("R radius = 1".to_string(), Some(0), Some(1)),
            TranscriptEntry::glsl("float radius = 1.0;".to_string(), Some(0)),
        ];
        let mut layout = TranscriptLayout::default();
        layout.record_bottom_to_top(Rect::new(0, 0, 20, 11), &[2, 1, 0], &entries);

        assert_eq!(layout.entry_at(0, 10), Some(2));
        assert_eq!(layout.entry_at(0, 7), None);
        assert_eq!(layout.entry_at(0, 6), Some(1));
        assert_eq!(layout.entry_at(0, 3), None);
        assert_eq!(layout.entry_at(0, 2), Some(0));
    }

    #[test]
    fn transcript_area_matches_input_left_padding() {
        let frame_area = Rect::new(0, 0, 20, 12);

        assert_eq!(transcript_area(frame_area).x, input_area(frame_area).x);
        assert_eq!(
            transcript_area(frame_area).width,
            input_area(frame_area).width
        );
    }

    #[test]
    fn input_area_leaves_one_blank_row_above_and_below_the_input_block() {
        let frame_area = Rect::new(0, 0, 20, 12);

        assert_eq!(input_area(frame_area).y, 8);
        assert_eq!(input_area(frame_area).height, FEED_ENTRY_MIN_HEIGHT);
        assert_eq!(input_top_gap(frame_area), 1);
        assert_eq!(
            input_area(frame_area)
                .y
                .saturating_add(input_area(frame_area).height),
            frame_area
                .y
                .saturating_add(frame_area.height)
                .saturating_sub(INPUT_BOTTOM_GAP)
        );
        assert_eq!(
            transcript_area(frame_area).height,
            input_area(frame_area)
                .y
                .saturating_sub(frame_area.y)
                .saturating_sub(INPUT_TOP_GAP)
        );
    }

    #[test]
    fn transcript_entries_reserve_input_height() {
        let single_line = TranscriptEntry::system("Session restarted.");
        let multi_line = TranscriptEntry::help("Enter submits.\n/exit leaves.");

        assert_eq!(single_line.line_count(), FEED_ENTRY_MIN_HEIGHT);
        assert_eq!(multi_line.line_count(), 4);
    }

    #[test]
    fn command_entries_render_as_plain_text_without_box_margins() {
        let command = TranscriptEntry::command("/info".to_string(), None);
        let mut app = App::new();

        assert_eq!(command.line_count(), 1);
        assert_eq!(app.render_entry(&command).height(), 1);
    }

    #[test]
    fn welcome_entry_renders_as_plain_text_without_box_margins() {
        let welcome = TranscriptEntry::welcome("Lane 0.1.0");
        let mut app = App::new();

        assert_eq!(welcome.line_count(), 1);
        assert_eq!(app.render_entry(&welcome).height(), 1);
    }

    #[test]
    fn command_entries_leave_one_blank_row_before_user_code_blocks() {
        let command = TranscriptEntry::command("/info".to_string(), None);
        let lane = TranscriptEntry::submitted("R radius = 1".to_string(), Some(0), Some(1));
        let mut app = App::new();

        let items = spaced_transcript_items(vec![
            (app.render_entry(&lane), lane.kind),
            (app.render_entry(&command), command.kind),
        ]);
        assert_eq!(items.len(), 3);
        assert_eq!(items[1].height(), 1);
    }

    #[test]
    fn command_entries_stay_attached_to_command_responses() {
        let command = TranscriptEntry::command("/info".to_string(), None);
        let response = TranscriptEntry::system("Loaded modules:\n  std");
        let mut app = App::new();

        let items = spaced_transcript_items(vec![
            (app.render_entry(&response), response.kind),
            (app.render_entry(&command), command.kind),
        ]);
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn command_entries_leave_one_blank_row_before_error_boxes() {
        let command = TranscriptEntry::command("/wat".to_string(), None);
        let error = TranscriptEntry::error("unknown shell command '/wat'".to_string(), None);
        let mut app = App::new();

        let items = spaced_transcript_items(vec![
            (app.render_entry(&error), error.kind),
            (app.render_entry(&command), command.kind),
        ]);
        assert_eq!(items.len(), 3);
        assert_eq!(items[1].height(), 1);
    }

    #[test]
    fn invalid_commands_render_only_as_errors_not_green_commands() {
        let mut app = App::new();
        app.transcript.clear();
        app.input = "/wat".to_string();

        app.submit_input();

        assert_eq!(app.transcript.len(), 1);
        assert!(matches!(app.transcript[0].kind, TranscriptKind::Error));
        assert!(app.transcript[0].text.contains("unknown shell command '/wat'"));
    }

    #[test]
    fn failed_submission_marks_submitted_lane_block_as_error() {
        let mut app = App::new();
        app.transcript.clear();
        app.input = "const Object output = Missing3D(r=1)".to_string();

        app.submit_input();

        assert!(app.transcript[0].errored);
        assert!(matches!(app.transcript[0].kind, TranscriptKind::Lane));
        assert_eq!(app.transcript[0].base_style().bg, Some(ERROR_BG));
        assert!(app.transcript[0].error.is_some());
        assert_eq!(app.transcript.len(), 1);
    }

    #[test]
    fn failed_submission_does_not_mark_previous_successful_lane_block_as_error() {
        let mut app = App::new();
        app.transcript.clear();
        app.input = "R radius = 1".to_string();
        app.submit_input();

        app.input = "const Object output = Missing3D(r=1)".to_string();
        app.submit_input();

        assert_eq!(app.transcript.len(), 2);
        assert!(matches!(app.transcript[0].kind, TranscriptKind::Lane));
        assert!(!app.transcript[0].errored);
        assert_eq!(app.transcript[0].base_style().bg, Some(USER_BG));
        assert!(
            app.transcript[1]
                .error
                .as_deref()
                .is_some_and(|error| !error.is_empty())
        );
        assert!(app.transcript[1].errored);
    }

    #[test]
    fn consecutive_failed_submissions_do_not_merge() {
        let mut app = App::new();
        app.transcript.clear();
        app.input = "const Object output = Missing3D(r=1)".to_string();
        app.submit_input();
        app.input = "const Object output = Missing3D(r=2)".to_string();
        app.submit_input();

        assert_eq!(app.transcript.len(), 2);
        assert!(app.transcript[0].errored);
        assert!(app.transcript[1].errored);
    }

    #[test]
    fn failed_submission_marks_latest_lane_entry_when_group_is_shared() {
        let mut app = App::new();
        app.transcript.clear();
        app.push_submitted_input("R radius = 1".to_string(), Some(1), true);
        app.push_submitted_input("const Object output = Missing3D(r=1)".to_string(), Some(2), true);

        assert_eq!(app.transcript.len(), 1);
        let shared_group = app.transcript[0].group.expect("expected lane group");
        app.attach_group_error(shared_group, "unknown object Missing3D".to_string());
        assert_eq!(
            app.transcript[0].error.as_deref(),
            Some("unknown object Missing3D")
        );
        assert!(app.transcript[0].errored);
        assert_eq!(
            app.transcript[0].text,
            "R radius = 1\nconst Object output = Missing3D(r=1)"
        );
    }

    #[test]
    fn selecting_one_submission_group_does_not_highlight_other_groups() {
        let lane_a = TranscriptEntry::submitted("const R a = 1".to_string(), Some(0), Some(1));
        let glsl_a = TranscriptEntry::glsl("float a = 1.0;".to_string(), Some(0));
        let lane_b = TranscriptEntry::submitted("const R b = 2".to_string(), Some(1), Some(2));
        let glsl_b = TranscriptEntry::glsl("float b = 2.0;".to_string(), Some(1));

        let lane_a_style = lane_a.style(Some(0));
        let glsl_a_style = glsl_a.style(Some(0));
        let lane_b_style = lane_b.style(Some(0));
        let glsl_b_style = glsl_b.style(Some(0));

        assert_eq!(lane_a_style.bg, Some(SELECTED_USER_BG));
        assert_eq!(glsl_a_style.bg, Some(SELECTED_OUTPUT_BG));
        assert_eq!(lane_b_style.bg, Some(USER_BG));
        assert_eq!(glsl_b_style.bg, Some(OUTPUT_BG));
    }

    #[test]
    fn right_click_copy_uses_raw_transcript_entry_text() {
        let mut app = App::new();
        app.transcript = vec![TranscriptEntry::submitted(
            "const R radius = 1".to_string(),
            Some(0),
            Some(1),
        )];
        app.layout.entries = vec![RenderedEntry {
            index: 0,
            area: Rect::new(4, 5, 30, 3),
        }];

        assert_eq!(
            app.copyable_text_at(10, 6).as_deref(),
            Some("const R radius = 1")
        );
        assert_eq!(app.copyable_text_at(1, 1), None);
    }

    #[test]
    fn welcome_entry_is_not_copied() {
        let mut app = App::new();
        app.layout.entries = vec![RenderedEntry {
            index: 0,
            area: Rect::new(0, 0, 20, 1),
        }];

        assert_eq!(app.copyable_text_at(0, 0), None);
    }

    #[test]
    fn osc52_clipboard_payload_base64_encodes_text() {
        let mut output = Vec::new();

        write_osc52_clipboard(&mut output, "R radius = 1").unwrap();

        assert_eq!(
            String::from_utf8(output).unwrap(),
            "\x1b]52;c;UiByYWRpdXMgPSAx\x07"
        );
    }

    #[test]
    fn error_entries_reserve_vertical_box_padding() {
        let single_line = TranscriptEntry::error("unknown shell command '/wat'".to_string(), None);
        let multi_line = TranscriptEntry::error("line 1: bad\nline 2: worse".to_string(), None);

        assert_eq!(single_line.line_count(), 3);
        assert_eq!(multi_line.line_count(), 4);
    }

    #[test]
    fn error_box_text_has_blank_rows_above_and_below() {
        let text = error_box_text("line 1: bad\nline 2: worse");

        assert_eq!(text.lines.len(), 4);
        assert!(text.lines[0].spans.is_empty());
        assert_eq!(text.lines[1].spans[0].content.as_ref(), "line 1: bad");
        assert_eq!(text.lines[2].spans[0].content.as_ref(), "line 2: worse");
        assert!(text.lines[3].spans.is_empty());
    }

    #[test]
    fn left_padded_text_adds_one_inner_column_to_each_line() {
        let text = left_padded_text(Text::from("first\nsecond"));

        assert_eq!(text.lines[0].spans[0].content.as_ref(), " ");
        assert_eq!(text.lines[0].spans[1].content.as_ref(), "first");
        assert_eq!(text.lines[1].spans[0].content.as_ref(), " ");
        assert_eq!(text.lines[1].spans[1].content.as_ref(), "second");
    }

    #[test]
    fn numbered_lane_text_prefixes_each_source_line() {
        let text = Text::from("R radius = 1\nconst R diameter = 2");
        let text = numbered_lane_text(text, 9);

        assert_eq!(text.lines[0].spans[0].content.as_ref(), " 9 | ");
        assert_eq!(text.lines[1].spans[0].content.as_ref(), "10 | ");
        assert_eq!(text.lines[0].spans[1].content.as_ref(), "R radius = 1");
        assert_eq!(
            text.lines[1].spans[1].content.as_ref(),
            "const R diameter = 2"
        );
    }

    #[test]
    fn errored_lane_text_marks_submitted_source_with_error_symbol() {
        let text = Text::from("const Object output = Missing3D(r=1)");
        let text = errored_lane_text(text, 7, "unknown object Missing3D");

        assert_eq!(text.lines[0].spans[0].content.as_ref(), "    ");
        assert_eq!(
            text.lines[0].spans[1].content.as_ref(),
            "unknown object Missing3D"
        );
        assert_eq!(text.lines[1].spans[0].content.as_ref(), " | ");
        assert_eq!(
            text.lines[1].spans[1].content.as_ref(),
            "const Object output = Missing3D(r=1)"
        );
        assert_eq!(text.lines[0].spans[1].style.fg, Some(ERROR_FG));
        assert_eq!(text.lines[0].spans[1].style.bg, Some(ERROR_BG));
    }

    #[test]
    fn errored_lane_text_marks_multiline_submissions_with_error_symbol() {
        let text = Text::from("R a = 1\nR b = 2\nconst Object output = Missing3D(r=a+b)");
        let text = errored_lane_text(text, 12, "unknown object Missing3D");

        assert_eq!(text.lines[0].spans[0].content.as_ref(), "     ");
        assert_eq!(text.lines[1].spans[0].content.as_ref(), "  | ");
        assert_eq!(text.lines[2].spans[0].content.as_ref(), "  | ");
        assert_eq!(text.lines[3].spans[0].content.as_ref(), "  | ");
    }

    #[test]
    fn errored_lane_text_strips_error_line_references() {
        let text = Text::from("const Object output = Missing3D(r=1)");
        let text = errored_lane_text(
            text,
            7,
            "line 7: unknown object Missing3D\nline 8: expected declaration",
        );

        assert_eq!(
            text.lines[0].spans[1].content.as_ref(),
            "unknown object Missing3D"
        );
        assert_eq!(
            text.lines[1].spans[1].content.as_ref(),
            "expected declaration"
        );
        assert_eq!(text.lines[2].spans[0].content.as_ref(), " | ");
    }
}
