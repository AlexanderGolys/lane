use std::io::{self, Stdout};
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
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
const COMMAND_FG: Color = Color::LightGreen;

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
        execute!(stdout, EnterAlternateScreen)?;
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
        execute!(self.terminal.backend_mut(), LeaveAlternateScreen)?;
        self.terminal.show_cursor()?;
        self.active = false;
        Ok(())
    }
}

impl Drop for ReplTerminal {
    fn drop(&mut self) {
        if self.active {
            let _ = disable_raw_mode();
            let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
            let _ = self.terminal.show_cursor();
        }
    }
}

struct App {
    session: ReplSession,
    input: String,
    transcript: Vec<TranscriptEntry>,
    highlighter: SyntaxHighlighter,
    split_mode: bool,
}

impl App {
    fn new() -> Self {
        Self {
            session: ReplSession::default(),
            input: String::new(),
            transcript: vec![TranscriptEntry::welcome(format!(
                "Lane {} started.",
                env!("CARGO_PKG_VERSION")
            ))],
            highlighter: SyntaxHighlighter::new(),
            split_mode: false,
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
            if let Event::Key(key) = event::read()? {
                if let Some(action) = self.handle_key(key) {
                    return Ok(action);
                }
            }
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Option<ReplAction> {
        match (key.code, key.modifiers) {
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => return Some(ReplAction::Exit),
            (KeyCode::Enter, modifiers) if modifiers.contains(KeyModifiers::CONTROL) => {
                self.input.push('\n')
            }
            (KeyCode::Enter, _) => return self.submit_input(),
            (KeyCode::Backspace, _) => {
                self.input.pop();
            }
            (KeyCode::Char(ch), _) => self.input.push(ch),
            _ => {}
        }
        None
    }

    fn submit_input(&mut self) -> Option<ReplAction> {
        let input = std::mem::take(&mut self.input);
        if input.trim().is_empty() {
            return None;
        }

        self.transcript
            .push(TranscriptEntry::submitted(input.clone()));
        match self.session.submit(&input) {
            SubmitOutcome::Accepted => {}
            SubmitOutcome::Emitted(glsl) => {
                if glsl.is_empty() {
                    return None;
                }
                self.transcript.push(TranscriptEntry::glsl(glsl));
            }
            SubmitOutcome::Cleared => self.transcript.clear(),
            SubmitOutcome::Restarted => {
                self.transcript.clear();
                self.transcript
                    .push(TranscriptEntry::system("Session restarted."));
            }
            SubmitOutcome::Help => self.transcript.push(TranscriptEntry::help(help_text())),
            SubmitOutcome::Show(source) => return Some(ReplAction::Show(source)),
            SubmitOutcome::ToggleSplit => self.toggle_split(),
            SubmitOutcome::Exit => return Some(ReplAction::Exit),
            SubmitOutcome::Error(error) => self.transcript.push(TranscriptEntry::error(error)),
        }
        None
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
                .filter(|entry| !matches!(entry.kind, TranscriptKind::Glsl))
                .cloned()
                .collect::<Vec<_>>();
            let glsl_entries = self
                .transcript
                .iter()
                .filter(|entry| matches!(entry.kind, TranscriptKind::Glsl))
                .cloned()
                .collect::<Vec<_>>();
            let user_items = user_entries
                .iter()
                .map(|entry| self.render_entry(entry))
                .collect::<Vec<_>>();
            let glsl_items = glsl_entries
                .iter()
                .map(|entry| self.render_entry(entry))
                .collect::<Vec<_>>();
            let user_transcript = List::new(user_items).direction(ListDirection::BottomToTop);
            let glsl_transcript = List::new(glsl_items).direction(ListDirection::BottomToTop);
            frame.render_widget(user_transcript, user_area);
            frame.render_widget(glsl_transcript, split[1]);
            self.render_input(frame, input_area);
        } else {
            let entries = self.transcript.clone();
            let items = entries
                .iter()
                .map(|entry| self.render_entry(entry))
                .collect::<Vec<_>>();
            let transcript = List::new(items).direction(ListDirection::BottomToTop);
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
        let cursor_x = area
            .x
            .saturating_add(last_line_width.min(area.width.saturating_sub(1)));
        let cursor_y = if self.input.contains('\n') {
            area.y.saturating_add(area.height.saturating_sub(1))
        } else {
            area.y.saturating_add(1.min(area.height.saturating_sub(1)))
        };
        frame.set_cursor_position(Position::new(cursor_x, cursor_y));
    }

    fn input_text(&mut self) -> Text<'static> {
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
                lines.push(Line::raw(""));
            } else {
                while lines.len() < 3 {
                    lines.insert(0, Line::raw(""));
                }
            }
            return Text::from(lines);
        }

        let source = if visible.len() <= 1 {
            self.input.clone()
        } else {
            visible.join("\n")
        };
        let mut text = self.highlighter.highlight_lane(&source);
        if visible.len() <= 1 {
            text.lines.insert(0, Line::raw(""));
            text.lines.push(Line::raw(""));
        } else {
            while text.lines.len() < 3 {
                text.lines.insert(0, Line::raw(""));
            }
        }
        text
    }

    fn render_entry(&mut self, entry: &TranscriptEntry) -> ListItem<'static> {
        let text = match entry.kind {
            TranscriptKind::Lane => self.highlighter.highlight_lane(&entry.text),
            TranscriptKind::Command => Text::from(Line::from(Span::styled(
                entry.text.clone(),
                Style::default().fg(COMMAND_FG).add_modifier(Modifier::BOLD),
            ))),
            TranscriptKind::Glsl => self.highlighter.highlight_glsl(&entry.text),
            TranscriptKind::Help => highlight_help_text(&entry.text),
            TranscriptKind::Error | TranscriptKind::System | TranscriptKind::Welcome => {
                Text::from(entry.text.clone())
            }
        };
        ListItem::new(text).style(entry.style())
    }
}

enum ReplAction {
    Exit,
    Show(String),
}

fn transcript_area(area: Rect) -> Rect {
    Rect {
        height: area.height.saturating_sub(3),
        ..area
    }
}

fn input_area(area: Rect) -> Rect {
    let x_offset = 3.min(area.width);
    Rect {
        x: area.x.saturating_add(x_offset),
        y: area.y.saturating_add(area.height.saturating_sub(3)),
        width: area.width.saturating_sub(x_offset),
        height: area.height.min(3),
    }
}

#[derive(Clone)]
struct TranscriptEntry {
    kind: TranscriptKind,
    text: String,
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
    fn lane(text: String) -> Self {
        Self {
            kind: TranscriptKind::Lane,
            text,
        }
    }

    fn command(text: String) -> Self {
        Self {
            kind: TranscriptKind::Command,
            text,
        }
    }

    fn submitted(text: String) -> Self {
        if text.starts_with('\\') {
            Self::command(text)
        } else {
            Self::lane(text)
        }
    }

    fn glsl(text: String) -> Self {
        Self {
            kind: TranscriptKind::Glsl,
            text,
        }
    }

    fn error(text: String) -> Self {
        Self {
            kind: TranscriptKind::Error,
            text,
        }
    }

    fn system(text: impl Into<String>) -> Self {
        Self {
            kind: TranscriptKind::System,
            text: text.into(),
        }
    }

    fn welcome(text: impl Into<String>) -> Self {
        Self {
            kind: TranscriptKind::Welcome,
            text: text.into(),
        }
    }

    fn help(text: impl Into<String>) -> Self {
        Self {
            kind: TranscriptKind::Help,
            text: text.into(),
        }
    }

    fn style(&self) -> Style {
        match self.kind {
            TranscriptKind::Lane => Style::default().bg(USER_BG),
            TranscriptKind::Command => Style::default().fg(COMMAND_FG),
            TranscriptKind::Glsl => Style::default().bg(OUTPUT_BG),
            TranscriptKind::Error => Style::default().fg(Color::Red).bg(ERROR_BG),
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
    Show(String),
    ToggleSplit,
    Exit,
    Error(String),
}

impl ReplSession {
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
        "\\show opens a native preview window for the current session.",
        "\\split toggles split mode.",
        "\\clear clears the transcript but keeps the session.",
        "\\restart starts from an empty session.",
        "\\exit leaves.",
    ]
    .join("\n")
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
    fn show_command_returns_current_source() {
        let mut session = ReplSession::default();
        assert_eq!(session.submit("R radius = 1"), SubmitOutcome::Accepted);
        assert_eq!(
            session.submit("\\show"),
            SubmitOutcome::Show("R radius = 1\n".to_string())
        );
    }
}
