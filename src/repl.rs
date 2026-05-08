use std::io::{self, Stdout};
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
const COMMAND_FG: Color = Color::LightGreen;
const INPUT_PLACEHOLDER_FG: Color = Color::DarkGray;
const LINE_NUMBER_FG: Color = Color::DarkGray;
const FEED_X_OFFSET: u16 = 3;
const FEED_ENTRY_MIN_HEIGHT: u16 = 3;
const FEED_ENTRY_GAP: u16 = 1;
const TEXT_BOX_INNER_LEFT_PADDING: u16 = 1;
const CURRENT_INPUT_GUTTER: &str = "    ";
const CURRENT_INPUT_GUTTER_WIDTH: u16 = 4;
const INPUT_PLACEHOLDER: &str = "Type Lane code...";

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
                crate::run_preview_source(&source)?;
                app.transcript
                    .push(TranscriptEntry::system("Preview closed."));
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
    input_history: Vec<String>,
    history_position: Option<usize>,
    history_draft: String,
    completion_matches: Vec<lane::LaneCompletionItem>,
    completion_index: usize,
    transcript: Vec<TranscriptEntry>,
    highlighter: SyntaxHighlighter,
    split_mode: bool,
    selected_group: Option<usize>,
    next_group: usize,
    layout: TranscriptLayout,
}

impl App {
    fn new() -> Self {
        Self {
            session: ReplSession::default(),
            input: String::new(),
            input_history: Vec::new(),
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
            (KeyCode::Enter, modifiers) if modifiers.contains(KeyModifiers::CONTROL) => {
                self.clear_history_navigation();
                self.clear_completion();
                self.input.push('\n');
            }
            (KeyCode::Enter, _) => return self.submit_input(),
            (KeyCode::Tab, _) => self.apply_completion(),
            (KeyCode::Up, _) => self.recall_older_input(),
            (KeyCode::Down, _) => self.recall_newer_input(),
            (KeyCode::Backspace, _) => {
                self.clear_history_navigation();
                self.clear_completion();
                self.input.pop();
            }
            (KeyCode::Char(ch), _) => {
                self.clear_history_navigation();
                self.clear_completion();
                self.input.push(ch);
            }
            _ => {}
        }
        None
    }

    fn handle_mouse(&mut self, mouse: MouseEvent) {
        if !matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
            return;
        }
        self.selected_group = self
            .layout
            .entry_at(mouse.column, mouse.row)
            .and_then(|index| self.transcript.get(index))
            .and_then(|entry| entry.group);
    }

    fn submit_input(&mut self) -> Option<ReplAction> {
        self.clear_history_navigation();
        self.clear_completion();
        let input = std::mem::take(&mut self.input);
        if input.trim().is_empty() {
            return None;
        }

        self.record_input_history(&input);
        let line_start = (!input.starts_with('\\')).then(|| self.session.next_line_number());
        let group = self.push_submitted_input(input.clone(), line_start);
        match self.session.submit(&input) {
            SubmitOutcome::Accepted => {}
            SubmitOutcome::Emitted(glsl) => {
                if glsl.is_empty() {
                    return None;
                }
                self.transcript.push(TranscriptEntry::glsl(glsl, group));
            }
            SubmitOutcome::Cleared => {
                self.transcript.clear();
                self.selected_group = None;
            }
            SubmitOutcome::Restarted => {
                self.transcript.clear();
                self.selected_group = None;
                self.next_group = 0;
                self.transcript
                    .push(TranscriptEntry::system("Session restarted."));
            }
            SubmitOutcome::Help => self.transcript.push(TranscriptEntry::help(help_text())),
            SubmitOutcome::Info(info) => self.transcript.push(TranscriptEntry::system(info)),
            SubmitOutcome::Show(source) => return Some(ReplAction::Show(source)),
            SubmitOutcome::ToggleSplit => self.toggle_split(),
            SubmitOutcome::Exit => return Some(ReplAction::Exit),
            SubmitOutcome::Error(error) => {
                if let Some(group) = group {
                    self.mark_group_error(group);
                }
                self.transcript.push(TranscriptEntry::error(error, group))
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
    }

    fn apply_completion(&mut self) {
        let Some((start, prefix)) = completion_token(&self.input) else {
            self.clear_completion();
            return;
        };
        if prefix.is_empty() {
            self.clear_completion();
            return;
        }
        if self.completion_matches.is_empty()
            || self
                .completion_matches
                .get(self.completion_index)
                .is_none_or(|item| item.label != prefix)
        {
            self.completion_matches = lane::lane_completion_items()
                .into_iter()
                .filter(|item| item.label.starts_with(&prefix))
                .collect();
            self.completion_index = 0;
        } else if !self.completion_matches.is_empty() {
            self.completion_index = (self.completion_index + 1) % self.completion_matches.len();
        }
        let Some(item) = self.completion_matches.get(self.completion_index) else {
            return;
        };
        self.input.replace_range(start.., &item.label);
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

    fn push_submitted_input(&mut self, input: String, line_start: Option<usize>) -> Option<usize> {
        if input.starts_with('\\') {
            self.transcript.push(TranscriptEntry::command(input, None));
            return None;
        }
        if let Some(entry) = self.transcript.last_mut() {
            if let Some(group) = entry.append_lane_input(&input, line_start) {
                return Some(group);
            }
        }
        let group = self.allocate_group();
        self.transcript
            .push(TranscriptEntry::submitted(input, Some(group), line_start));
        Some(group)
    }

    fn mark_group_error(&mut self, group: usize) {
        for entry in &mut self.transcript {
            if entry.group == Some(group) && matches!(entry.kind, TranscriptKind::Lane) {
                entry.errored = true;
            }
        }
    }

    fn toggle_split(&mut self) {
        self.split_mode = !self.split_mode;
        let mode = if self.split_mode {
            "split"
        } else {
            "linear transcript"
        };
        self.transcript
            .push(TranscriptEntry::system(format!("Display mode: {mode}.")));
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
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            let glsl_entries = self
                .transcript
                .iter()
                .enumerate()
                .rev()
                .filter(|(_, entry)| matches!(entry.kind, TranscriptKind::Glsl))
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

        let last_line_width = self
            .input
            .lines()
            .last()
            .map_or(0, |line| line.chars().count() as u16);
        let cursor_column = last_line_width
            .saturating_add(TEXT_BOX_INNER_LEFT_PADDING)
            .saturating_add(CURRENT_INPUT_GUTTER_WIDTH)
            .min(area.width.saturating_sub(1));
        let cursor_x = area.x.saturating_add(cursor_column);
        let cursor_y = if self.input.contains('\n') {
            area.y.saturating_add(area.height.saturating_sub(1))
        } else {
            area.y.saturating_add(1.min(area.height.saturating_sub(1)))
        };
        frame.set_cursor_position(Position::new(cursor_x, cursor_y));
    }

    fn input_text(&mut self) -> Text<'static> {
        if self.input.is_empty() {
            return placeholder_input_text();
        }

        let mut visible = self.input.lines().rev().take(3).collect::<Vec<_>>();
        visible.reverse();

        if self.input.starts_with('\\') {
            let mut lines = visible
                .iter()
                .map(|line| {
                    Line::from(Span::styled(
                        (*line).to_string(),
                        Style::default().fg(COMMAND_FG).add_modifier(Modifier::BOLD),
                    ))
                })
                .collect::<Vec<_>>();
            if lines.len() <= 1 {
                lines.insert(0, Line::raw(""));
                lines.push(self.input_status_line());
            } else {
                while lines.len() < 3 {
                    lines.insert(0, Line::raw(""));
                }
            }
            return current_input_text(Text::from(lines));
        }

        let source = if visible.len() <= 1 {
            self.input.clone()
        } else {
            visible.join("\n")
        };
        let mut text = self.highlighter.highlight_lane(&source);
        if visible.len() <= 1 {
            text.lines.insert(0, Line::raw(""));
            text.lines.push(self.input_status_line());
        } else {
            while text.lines.len() < 3 {
                text.lines.insert(0, Line::raw(""));
            }
        }
        current_input_text(text)
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

    fn render_entry(&mut self, entry: &TranscriptEntry) -> ListItem<'static> {
        let text = match entry.kind {
            TranscriptKind::Lane => {
                let text = self.highlighter.highlight_lane(&entry.text);
                if let Some(line_start) = entry.line_start {
                    numbered_lane_text(text, line_start)
                } else {
                    text
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
        let text = if matches!(entry.kind, TranscriptKind::Error | TranscriptKind::Command) {
            text
        } else {
            padded_feed_text(text)
        };
        let text = left_padded_text(text);
        ListItem::new(text).style(entry.style(self.selected_group))
    }
}

enum ReplAction {
    Exit,
    Show(String),
}

fn transcript_area(area: Rect) -> Rect {
    transcript_feed_area(Rect {
        height: area.height.saturating_sub(FEED_ENTRY_MIN_HEIGHT),
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
    Rect {
        x: area.x.saturating_add(x_offset),
        y: area
            .y
            .saturating_add(area.height.saturating_sub(FEED_ENTRY_MIN_HEIGHT)),
        width: area.width.saturating_sub(x_offset),
        height: area.height.min(FEED_ENTRY_MIN_HEIGHT),
    }
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
    matches!(previous, TranscriptKind::Command) || matches!(current, TranscriptKind::Command)
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

fn current_input_text(mut text: Text<'static>) -> Text<'static> {
    for line in &mut text.lines {
        line.spans.insert(0, Span::raw(CURRENT_INPUT_GUTTER));
        line.spans.insert(0, Span::raw(" "));
    }
    text
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

fn is_completion_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == '#'
}

fn placeholder_input_text() -> Text<'static> {
    current_input_text(Text::from(vec![
        Line::raw(""),
        Line::from(Span::styled(
            INPUT_PLACEHOLDER,
            Style::default().fg(INPUT_PLACEHOLDER_FG),
        )),
        Line::raw(""),
    ]))
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

#[derive(Clone)]
struct TranscriptEntry {
    kind: TranscriptKind,
    text: String,
    group: Option<usize>,
    line_start: Option<usize>,
    errored: bool,
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
        }
    }

    fn submitted(text: String, group: Option<usize>, line_start: Option<usize>) -> Self {
        if text.starts_with('\\') {
            Self::command(text, group)
        } else {
            Self {
                kind: TranscriptKind::Lane,
                text,
                group,
                line_start,
                errored: false,
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
        }
    }

    fn error(text: String, group: Option<usize>) -> Self {
        Self {
            kind: TranscriptKind::Error,
            text,
            group,
            line_start: None,
            errored: false,
        }
    }

    fn system(text: impl Into<String>) -> Self {
        Self {
            kind: TranscriptKind::System,
            text: text.into(),
            group: None,
            line_start: None,
            errored: false,
        }
    }

    fn welcome(text: impl Into<String>) -> Self {
        Self {
            kind: TranscriptKind::Welcome,
            text: text.into(),
            group: None,
            line_start: None,
            errored: false,
        }
    }

    fn help(text: impl Into<String>) -> Self {
        Self {
            kind: TranscriptKind::Help,
            text: text.into(),
            group: None,
            line_start: None,
            errored: false,
        }
    }

    fn line_count(&self) -> u16 {
        let lines = self.text.lines().count().max(1) as u16;
        if matches!(self.kind, TranscriptKind::Command) {
            return lines;
        }
        lines.saturating_add(2)
    }

    fn append_lane_input(&mut self, input: &str, line_start: Option<usize>) -> Option<usize> {
        if !matches!(self.kind, TranscriptKind::Lane) || self.next_line_start() != line_start {
            return None;
        }
        if !self.text.ends_with('\n') {
            self.text.push('\n');
        }
        self.text.push_str(input.trim_end_matches(['\r', '\n']));
        self.group
    }

    fn next_line_start(&self) -> Option<usize> {
        self.line_start
            .map(|line_start| line_start.saturating_add(self.text.lines().count()))
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
    emitted_glsl_lines: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum SubmitOutcome {
    Accepted,
    Emitted(String),
    Cleared,
    Restarted,
    Help,
    Info(String),
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
        if line.starts_with('\\') {
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
                    let emitted = glsl
                        .lines()
                        .skip(self.emitted_glsl_lines)
                        .collect::<Vec<_>>()
                        .join("\n");
                    self.emitted_glsl_lines = glsl.lines().count();
                    SubmitOutcome::Emitted(emitted)
                } else {
                    SubmitOutcome::Accepted
                }
            }
            Err(err) => SubmitOutcome::Error(err.to_string()),
        }
    }

    fn run_command(&mut self, command: &str) -> SubmitOutcome {
        match command {
            "\\clear" => SubmitOutcome::Cleared,
            "\\help" => SubmitOutcome::Help,
            "\\info" => match lane::program_info(&self.source) {
                Ok(info) => SubmitOutcome::Info(format_program_info(&info)),
                Err(err) => SubmitOutcome::Error(err.to_string()),
            },
            "\\show" => SubmitOutcome::Show(self.source.clone()),
            "\\split" => SubmitOutcome::ToggleSplit,
            "\\restart" => {
                self.source.clear();
                self.emitted_glsl_lines = 0;
                SubmitOutcome::Restarted
            }
            "\\exit" => SubmitOutcome::Exit,
            _ => SubmitOutcome::Error(format!("unknown shell command '{command}'")),
        }
    }
}

fn help_text() -> String {
    [
        "Enter submits.",
        "Ctrl-Enter inserts a newline when supported by the terminal.",
        "Up and Down recall submitted input history.",
        "Tab completes the current word with Lane LSP items.",
        "Ctrl-F formats the current input.",
        "\\info shows loaded modules, used directives, and provided objects.",
        "\\show opens a native preview window for the current session.",
        "\\split toggles split mode.",
        "\\clear clears the transcript but keeps the session.",
        "\\restart starts from an empty session.",
        "\\exit leaves.",
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
            let mut rest = line;
            while let Some(index) = rest.find('\\') {
                let (before, after_before) = rest.split_at(index);
                if !before.is_empty() {
                    spans.push(Span::raw(before.to_string()));
                }
                let command_len = after_before
                    .chars()
                    .take_while(|ch| *ch == '\\' || ch.is_ascii_alphabetic())
                    .map(char::len_utf8)
                    .sum::<usize>();
                let (command, after_command) = after_before.split_at(command_len);
                spans.push(Span::styled(
                    command.to_string(),
                    Style::default().fg(COMMAND_FG).add_modifier(Modifier::BOLD),
                ));
                rest = after_command;
            }
            if !rest.is_empty() {
                spans.push(Span::raw(rest.to_string()));
            }
            Line::from(spans)
        })
        .collect::<Vec<_>>();
    Text::from(lines)
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
        let outcome = session.submit("const Object output = Ball3D(r=radius)");
        let SubmitOutcome::Emitted(glsl) = outcome else {
            panic!("expected GLSL output");
        };
        let full_glsl = crate::compile_program_output(
            "const R radius = 1\nconst Object output = Ball3D(r=radius)\n",
        )
        .unwrap();
        let expected = full_glsl
            .lines()
            .skip(first_glsl.lines().count())
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(glsl, expected);
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
        assert_eq!(session.submit("\\restart"), SubmitOutcome::Restarted);
        let outcome = session.submit("const Object output = Ball3D(r=radius)");
        assert!(matches!(outcome, SubmitOutcome::Error(_)));
    }

    #[test]
    fn split_command_toggles_split_mode() {
        let mut session = ReplSession::default();
        assert_eq!(session.submit("\\split"), SubmitOutcome::ToggleSplit);
    }

    #[test]
    fn help_command_prints_help() {
        let mut session = ReplSession::default();
        assert_eq!(session.submit("\\help"), SubmitOutcome::Help);
    }

    #[test]
    fn info_command_reports_session_metadata() {
        let mut session = ReplSession::default();
        assert_eq!(session.submit("#import std"), SubmitOutcome::Accepted);
        assert_eq!(session.submit("#2D"), SubmitOutcome::Accepted);
        assert_eq!(session.submit("#prec 0.002"), SubmitOutcome::Accepted);
        assert_eq!(session.submit("provided R time"), SubmitOutcome::Accepted);

        let SubmitOutcome::Info(info) = session.submit("\\info") else {
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
            session.submit("\\info"),
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
            session.submit("\\show"),
            SubmitOutcome::Show("R radius = 1\n".to_string())
        );
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

        let first_group = app.push_submitted_input("R radius = 1".to_string(), Some(1));
        let second_group = app.push_submitted_input("R diameter = radius * 2".to_string(), Some(2));

        assert_eq!(first_group, second_group);
        assert_eq!(app.transcript.len(), 1);
        assert_eq!(
            app.transcript[0].text,
            "R radius = 1\nR diameter = radius * 2"
        );
        assert_eq!(app.transcript[0].line_start, Some(1));
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
        assert_eq!(
            text.lines[1].spans[1].content.as_ref(),
            CURRENT_INPUT_GUTTER
        );
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
        assert_eq!(
            text.lines[1].spans[1].content.as_ref(),
            CURRENT_INPUT_GUTTER
        );
        assert_eq!(CURRENT_INPUT_GUTTER.chars().count() as u16, 4);
    }

    #[test]
    fn tab_completes_current_input_from_lsp_items() {
        let mut app = App::new();
        app.input = "const Object output = Bal".to_string();

        app.handle_key(KeyEvent::from(KeyCode::Tab));

        assert_eq!(app.input, "const Object output = Ball2D");
        assert!(app
            .completion_matches
            .iter()
            .any(|item| item.label == "Ball3D"));
    }

    #[test]
    fn ctrl_f_formats_current_input() {
        let mut app = App::new();
        app.input = "R radius = 1   \n\n\nconst R diameter = radius * 2   ".to_string();

        app.handle_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL));

        assert_eq!(app.input, "R radius = 1\n\nconst R diameter = radius * 2");
    }

    #[test]
    fn lane_submission_after_message_starts_new_feed_box() {
        let mut app = App::new();
        app.transcript.clear();

        app.push_submitted_input("R radius = 1".to_string(), Some(1));
        app.transcript
            .push(TranscriptEntry::system("Session note."));
        app.push_submitted_input("R diameter = radius * 2".to_string(), Some(2));

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
    fn transcript_entries_reserve_input_height() {
        let single_line = TranscriptEntry::system("Session restarted.");
        let multi_line = TranscriptEntry::help("Enter submits.\n\\exit leaves.");

        assert_eq!(single_line.line_count(), FEED_ENTRY_MIN_HEIGHT);
        assert_eq!(multi_line.line_count(), 4);
    }

    #[test]
    fn command_entries_render_as_plain_text_without_box_margins() {
        let command = TranscriptEntry::command("\\info".to_string(), None);
        let lane = TranscriptEntry::submitted("R radius = 1".to_string(), Some(0), Some(1));
        let mut app = App::new();

        assert_eq!(command.line_count(), 1);
        assert_eq!(app.render_entry(&command).height(), 1);

        let items = spaced_transcript_items(vec![
            (app.render_entry(&lane), lane.kind),
            (app.render_entry(&command), command.kind),
        ]);
        assert_eq!(items.len(), 2);
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
        assert!(matches!(app.transcript[1].kind, TranscriptKind::Error));
    }

    #[test]
    fn error_entries_reserve_vertical_box_padding() {
        let single_line = TranscriptEntry::error("unknown shell command '\\wat'".to_string(), None);
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
}
