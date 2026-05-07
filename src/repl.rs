use std::io::{self, Stdout};
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::Terminal;
use tree_sitter_highlight::{
    Error as HighlightError, Highlight, HighlightConfiguration, HighlightEvent, Highlighter,
};
use tree_sitter_language::LanguageFn;

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
    let mut terminal = ReplTerminal::enter()?;
    let result = App::new().run(&mut terminal.terminal);
    terminal.leave()?;
    result
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
        if !self.active {
            return;
        }
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}

#[derive(Default)]
pub(crate) struct ReplSession {
    source: String,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum SubmitOutcome {
    Accepted,
    Emitted(String),
    Cleared,
    Restarted,
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
                    SubmitOutcome::Emitted(glsl)
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
            "\\restart" => {
                self.source.clear();
                SubmitOutcome::Restarted
            }
            "\\exit" => SubmitOutcome::Exit,
            _ => SubmitOutcome::Error(format!("unknown shell command '{command}'")),
        }
    }
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

struct App {
    session: ReplSession,
    input: String,
    transcript: Vec<TranscriptEntry>,
    highlighter: SyntaxHighlighter,
}

impl App {
    fn new() -> Self {
        Self {
            session: ReplSession::default(),
            input: String::new(),
            transcript: vec![TranscriptEntry::system(
                "Lane shell. Enter submits, Ctrl-Enter inserts a newline, \\clear clears output, \\restart starts fresh, \\exit leaves.",
            )],
            highlighter: SyntaxHighlighter::new(),
        }
    }

    fn run(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        loop {
            terminal.draw(|frame| self.draw(frame))?;
            if !event::poll(Duration::from_millis(80))? {
                continue;
            }
            if let Event::Key(key) = event::read()? {
                if self.handle_key(key) {
                    break;
                }
            }
        }
        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        match (key.code, key.modifiers) {
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => return true,
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
        false
    }

    fn submit_input(&mut self) -> bool {
        let input = std::mem::take(&mut self.input);
        if input.trim().is_empty() {
            return false;
        }
        self.transcript.push(TranscriptEntry::lane(input.clone()));
        match self.session.submit(&input) {
            SubmitOutcome::Accepted => {}
            SubmitOutcome::Emitted(glsl) => self.transcript.push(TranscriptEntry::glsl(glsl)),
            SubmitOutcome::Cleared => self.transcript.clear(),
            SubmitOutcome::Restarted => {
                self.transcript.clear();
                self.transcript
                    .push(TranscriptEntry::system("Session restarted."));
            }
            SubmitOutcome::Exit => return true,
            SubmitOutcome::Error(error) => self.transcript.push(TranscriptEntry::error(error)),
        }
        false
    }

    fn draw(&mut self, frame: &mut ratatui::Frame) {
        let areas = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(5), Constraint::Length(7)])
            .split(frame.area());

        let entries = self.transcript.clone();
        let items = entries
            .iter()
            .map(|entry| self.render_entry(entry))
            .collect::<Vec<_>>();
        let transcript = List::new(items)
            .block(Block::default().title("Session").borders(Borders::ALL))
            .style(Style::default().bg(Color::Black));
        frame.render_widget(transcript, areas[0]);

        let input = Paragraph::new(self.highlighter.highlight_lane(&self.input))
            .block(Block::default().title("Lane input").borders(Borders::ALL))
            .style(Style::default().fg(Color::White).bg(Color::Rgb(18, 24, 28)))
            .wrap(Wrap { trim: false });
        frame.render_widget(input, areas[1]);
    }

    fn render_entry(&mut self, entry: &TranscriptEntry) -> ListItem<'static> {
        let text = match entry.kind {
            TranscriptKind::Lane => self.highlighter.highlight_lane(&entry.text),
            TranscriptKind::Glsl => self.highlighter.highlight_glsl(&entry.text),
            TranscriptKind::Error | TranscriptKind::System => Text::from(entry.text.clone()),
        };
        ListItem::new(text).style(entry.style())
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
    Glsl,
    Error,
    System,
}

impl TranscriptEntry {
    fn lane(text: String) -> Self {
        Self {
            kind: TranscriptKind::Lane,
            text,
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

    fn style(&self) -> Style {
        match self.kind {
            TranscriptKind::Lane => Style::default().bg(Color::Rgb(18, 24, 28)),
            TranscriptKind::Glsl => Style::default().bg(Color::Rgb(14, 32, 24)),
            TranscriptKind::Error => Style::default().fg(Color::Red).bg(Color::Rgb(36, 18, 18)),
            TranscriptKind::System => Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::ITALIC),
        }
    }
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
}
