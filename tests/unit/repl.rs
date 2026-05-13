use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

/// Build a deterministic-ish temporary history file path for a test case.
fn temp_history_file() -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("lane-repl-history-test-{unique}.txt"))
}

/// Seed app input and move cursor to the end for concise test setup.
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
    assert!(glsl.text.contains("sdf0_Ball3D(p, ParamBall3D(radius))"));
}

#[test]
fn later_const_emits_only_lines_after_previous_output() {
    let mut session = ReplSession::default();
    let SubmitOutcome::Emitted(first_glsl) = session.submit("const R radius = 1") else {
        panic!("expected first GLSL output");
    };
    assert!(first_glsl.text.contains("const float radius = 1.0f;"));
    let outcome = session.submit("const Object output = Ball3D(r=radius)");
    let SubmitOutcome::Emitted(glsl) = outcome else {
        panic!("expected GLSL output");
    };
    assert!(glsl.text.contains("struct ParamBall3D"));
    assert!(glsl.text.contains("float sdf_output(vec3 p)"));
    assert!(!glsl.text.contains("const float radius = 1.0f;"));
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

    assert!(second.text.contains("struct Triple"));
    assert!(second
        .text
        .contains("const Triple triple = Triple(1.0f, 2.0f, 3.0f);"));
    assert!(!second.text.contains("struct Pair"));
    assert!(!second.text.contains("const Pair pair"));
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

    assert!(added.text.contains("struct Triple"));
    assert!(added.text.contains("const Triple triple = Triple(1.0);"));
    assert!(!added.text.contains("struct Pair"));
    assert!(!added.text.contains("const Pair pair"));
    assert_eq!(added.line_start, 5);
}

#[test]
fn first_glsl_emission_starts_at_line_one() {
    let mut session = ReplSession::default();
    let SubmitOutcome::Emitted(glsl) = session.submit("const R radius = 1") else {
        panic!("expected emitted GLSL");
    };

    assert_eq!(glsl.line_start, 1);
    assert!(glsl.text.contains("const float radius = 1.0f;"));
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
    app.transcript.push(TranscriptEntry::glsl(
        "float radius = 1.0;".to_string(),
        Some(0),
        None,
    ));
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
fn split_scroll_offsets_move_independently() {
    let mut app = App::new();
    app.split_mode = true;
    app.transcript = vec![
        TranscriptEntry::submitted("R old = 1".to_string(), Some(0), Some(1)),
        TranscriptEntry::glsl("float old = 1.0;".to_string(), Some(0), None),
        TranscriptEntry::submitted("R new = 2".to_string(), Some(1), Some(2)),
        TranscriptEntry::glsl("float new = 2.0;".to_string(), Some(1), None),
    ];

    app.scroll_split_pane_up(SplitPane::Glsl);
    assert_eq!(app.split_user_scroll, 0);
    assert_eq!(app.split_glsl_scroll, 1);

    app.scroll_split_pane_up(SplitPane::User);
    app.scroll_split_pane_down(SplitPane::Glsl);
    assert_eq!(app.split_user_scroll, 1);
    assert_eq!(app.split_glsl_scroll, 0);
}

#[test]
fn split_mouse_wheel_scrolls_pane_under_pointer() {
    let backend = ratatui::backend::TestBackend::new(80, 12);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::new();
    app.split_mode = true;
    app.transcript = vec![
        TranscriptEntry::submitted("R old = 1".to_string(), Some(0), Some(1)),
        TranscriptEntry::glsl("float old = 1.0;".to_string(), Some(0), None),
        TranscriptEntry::submitted("R new = 2".to_string(), Some(1), Some(2)),
        TranscriptEntry::glsl("float new = 2.0;".to_string(), Some(1), None),
    ];

    terminal.draw(|frame| app.draw(frame)).unwrap();

    app.scroll_at(45, 1, ScrollDirection::Up);
    assert_eq!(app.split_user_scroll, 0);
    assert_eq!(app.split_glsl_scroll, 1);

    app.scroll_at(5, 1, ScrollDirection::Up);
    assert_eq!(app.split_user_scroll, 1);
    assert_eq!(app.split_glsl_scroll, 1);
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
    assert_eq!(
        session.submit("const R radius = 1"),
        SubmitOutcome::Emitted(GlslOutput {
            text: crate::compile_program_output("const R radius = 1\n").unwrap(),
            line_start: 1,
        })
    );

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
    assert_eq!(
        session.submit("/help\nR radius = 1"),
        SubmitOutcome::Error("unknown shell command '/help\nR radius = 1'".to_string())
    );
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
fn lane_submissions_do_not_merge_across_generated_glsl() {
    let mut app = App::new();
    app.transcript.clear();
    app.input = "const R radius = 1".to_string();
    app.submit_input();
    app.input = "R diameter = radius * 2".to_string();
    app.submit_input();

    assert_eq!(app.transcript.len(), 3);
    assert!(matches!(app.transcript[0].kind, TranscriptKind::Lane));
    assert!(matches!(app.transcript[1].kind, TranscriptKind::Glsl));
    assert!(matches!(app.transcript[2].kind, TranscriptKind::Lane));
    assert_eq!(app.transcript[0].text, "const R radius = 1");
    assert_eq!(app.transcript[2].text, "R diameter = radius * 2");
    assert_eq!(app.transcript[0].line_start, Some(1));
    assert_eq!(app.transcript[0].group, app.transcript[1].group);
    assert_ne!(app.transcript[0].group, app.transcript[2].group);
}

#[test]
fn generated_glsl_outputs_do_not_merge_across_lane_blocks() {
    let mut app = App::new();
    app.transcript.clear();
    app.split_mode = true;
    app.input = "const R radius = 1".to_string();
    app.submit_input();
    app.input = "const R diameter = radius * 2".to_string();
    app.submit_input();

    assert_eq!(app.transcript.len(), 4);
    assert!(matches!(app.transcript[0].kind, TranscriptKind::Lane));
    assert!(matches!(app.transcript[1].kind, TranscriptKind::Glsl));
    assert!(matches!(app.transcript[2].kind, TranscriptKind::Lane));
    assert!(matches!(app.transcript[3].kind, TranscriptKind::Glsl));
    assert!(app.transcript[1]
        .text
        .contains("const float radius = 1.0f;"));
    assert!(app.transcript[3]
        .text
        .contains("const float diameter = (radius * 2.0f);"));
}

#[test]
fn adjacent_glsl_outputs_share_one_feed_box() {
    let mut app = App::new();
    app.transcript.clear();
    app.push_glsl_output(
        GlslOutput {
            text: "float radius = 1.0;".to_string(),
            line_start: 1,
        },
        Some(0),
    );
    app.push_glsl_output(
        GlslOutput {
            text: "float diameter = radius * 2.0;".to_string(),
            line_start: 2,
        },
        Some(0),
    );

    assert_eq!(app.transcript.len(), 1);
    assert!(matches!(app.transcript[0].kind, TranscriptKind::Glsl));
    assert_eq!(
        app.transcript[0].text,
        "float radius = 1.0;\nfloat diameter = radius * 2.0;"
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
        TranscriptEntry::glsl("float radius = 1.0;".to_string(), Some(0), None),
    ];
    let mut layout = TranscriptLayout::default();
    layout.record_bottom_to_top(Rect::new(0, 0, 20, 11), &[2, 1, 0], &entries, 0);

    assert_eq!(layout.entry_at(0, 10), Some(2));
    assert_eq!(layout.entry_at(0, 7), None);
    assert_eq!(layout.entry_at(0, 6), Some(1));
    assert_eq!(layout.entry_at(0, 3), None);
    assert_eq!(layout.entry_at(0, 2), Some(0));
}

#[test]
fn bottom_to_top_layout_scrolls_by_rows_not_entries() {
    let entries = vec![
        TranscriptEntry::submitted("R first = 1".to_string(), Some(0), Some(1)),
        TranscriptEntry::submitted("R second = 2".to_string(), Some(1), Some(2)),
        TranscriptEntry::submitted("R third = 3".to_string(), Some(2), Some(3)),
    ];
    let mut layout = TranscriptLayout::default();
    layout.record_bottom_to_top(Rect::new(0, 0, 20, 7), &[2, 1, 0], &entries, 1);

    assert_eq!(layout.entry_at(0, 6), Some(2));
    assert_eq!(layout.entry_at(0, 4), None);
    assert_eq!(layout.entry_at(0, 3), Some(1));
}

#[test]
fn split_layout_keeps_gap_between_lane_entries_separated_by_glsl() {
    let entries = vec![
        TranscriptEntry::submitted("R radius = 1".to_string(), Some(0), Some(1)),
        TranscriptEntry::glsl("float radius = 1.0;".to_string(), Some(0), None),
        TranscriptEntry::submitted("R diameter = 2".to_string(), Some(1), Some(2)),
    ];
    let mut layout = TranscriptLayout::default();
    layout.record_bottom_to_top(Rect::new(0, 0, 20, 7), &[2, 0], &entries, 0);

    assert_eq!(layout.entry_at(0, 6), Some(2));
    assert_eq!(layout.entry_at(0, 3), None);
    assert_eq!(layout.entry_at(0, 2), Some(0));
}

#[test]
fn split_layout_keeps_gap_between_glsl_entries_separated_by_lane() {
    let entries = vec![
        TranscriptEntry::submitted("const R radius = 1".to_string(), Some(0), Some(1)),
        TranscriptEntry::glsl("float radius = 1.0;".to_string(), Some(0), None),
        TranscriptEntry::submitted("const R diameter = 2".to_string(), Some(1), Some(2)),
        TranscriptEntry::glsl("float diameter = 2.0;".to_string(), Some(1), None),
    ];
    let mut layout = TranscriptLayout::default();
    layout.record_bottom_to_top(Rect::new(0, 0, 20, 7), &[3, 1], &entries, 0);

    assert_eq!(layout.entry_at(0, 6), Some(3));
    assert_eq!(layout.entry_at(0, 3), None);
    assert_eq!(layout.entry_at(0, 2), Some(1));
}

#[test]
fn oversized_glsl_entry_still_renders_visible_lines() {
    let backend = ratatui::backend::TestBackend::new(60, 8);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::new();
    app.transcript.clear();
    app.transcript.push(TranscriptEntry::glsl(
        (0..20)
            .map(|index| format!("glsl_line_{index}"))
            .collect::<Vec<_>>()
            .join("\n"),
        Some(0),
        None,
    ));

    terminal.draw(|frame| app.draw(frame)).unwrap();
    let buffer_text = terminal_buffer_text(terminal.backend().buffer());

    assert!(buffer_text.contains("glsl_line_0"));
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
    assert!(app.transcript[0]
        .text
        .contains("unknown shell command '/wat'"));
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
    assert!(app.transcript[1]
        .error
        .as_deref()
        .is_some_and(|error| !error.is_empty()));
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
    app.push_submitted_input(
        "const Object output = Missing3D(r=1)".to_string(),
        Some(2),
        true,
    );

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
    let glsl_a = TranscriptEntry::glsl("float a = 1.0;".to_string(), Some(0), None);
    let lane_b = TranscriptEntry::submitted("const R b = 2".to_string(), Some(1), Some(2));
    let glsl_b = TranscriptEntry::glsl("float b = 2.0;".to_string(), Some(1), None);

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
fn right_click_copy_includes_error_message_for_failed_lane_blocks() {
    let mut app = App::new();
    let mut entry = TranscriptEntry::submitted("const R radius = *".to_string(), Some(0), Some(1));
    entry.errored = true;
    entry.error = Some("line 1: unexpected token '*' in expression".to_string());
    app.transcript = vec![entry];
    app.layout.entries = vec![RenderedEntry {
        index: 0,
        area: Rect::new(4, 5, 40, 4),
    }];

    assert_eq!(
        app.copyable_text_at(10, 6).as_deref(),
        Some("unexpected token '*' in expression\nconst R radius = *")
    );
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
fn glsl_entry_rendering_prefixes_each_generated_line() {
    let mut app = App::new();
    let entry = TranscriptEntry::glsl(
        "float radius = 1.0;\nfloat diameter = 2.0;".to_string(),
        Some(0),
        Some(41),
    );

    let text = app.render_entry_text(&entry);

    assert_eq!(text.lines[1].spans[0].content.as_ref(), " ");
    assert_eq!(text.lines[1].spans[1].content.as_ref(), "41 | ");
    assert_eq!(text.lines[2].spans[0].content.as_ref(), " ");
    assert_eq!(text.lines[2].spans[1].content.as_ref(), "42 | ");
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
    assert_eq!(text.lines[0].spans[1].style.bg, None);
}

#[test]
fn selected_failed_lane_error_message_uses_entry_background() {
    let mut entry = TranscriptEntry::submitted(
        "const Object output = Missing3D(r=1)".to_string(),
        Some(0),
        Some(7),
    );
    entry.errored = true;
    entry.error = Some("unknown object Missing3D".to_string());
    let text = errored_lane_text(
        Text::from(entry.text.clone()),
        7,
        entry.error.as_deref().unwrap(),
    );

    assert_eq!(entry.style(Some(0)).bg, Some(SELECTED_ERROR_BG));
    assert_eq!(entry.style(Some(0)).fg, Some(ERROR_FG));
    assert!(text
        .lines
        .iter()
        .flat_map(|line| line.spans.iter())
        .all(|span| span.style.bg != Some(ERROR_BG)));
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

/// Flatten a ratatui buffer into plain text for snapshot-style assertions.
fn terminal_buffer_text(buffer: &ratatui::buffer::Buffer) -> String {
    let area = buffer.area;
    let mut text = String::new();
    for y in area.y..area.y.saturating_add(area.height) {
        for x in area.x..area.x.saturating_add(area.width) {
            text.push_str(buffer[(x, y)].symbol());
        }
        text.push('\n');
    }
    text
}
