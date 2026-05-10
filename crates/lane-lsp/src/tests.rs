use super::*;

#[test]
fn finds_word_at_lsp_position() {
    let text = "const Object scene = Ball3D(r=1)\n";
    let position = Position::new(0, 24);

    assert_eq!(
        Backend::word_at_position(text, position).as_deref(),
        Some("Ball3D")
    );
}

#[test]
fn hovers_known_primitive() {
    let hover = Backend::hover_for_word("Ball3D").unwrap();

    assert!(hover.contains("Ball3D"));
    assert!(hover.contains("r: R"));
}

#[test]
fn completes_new_language_surface() {
    let labels = Backend::completion_items()
        .into_iter()
        .map(|item| item.label)
        .collect::<Vec<_>>();

    assert!(labels.iter().any(|label| label == "#import"));
    assert!(labels.iter().any(|label| label == "raytracing"));
    assert!(labels.iter().any(|label| label == "Ball3D"));
    assert!(labels.iter().any(|label| label == "Mat{n}x{m}"));
    assert!(!labels.iter().any(|label| label == "R{n}"));
}

#[test]
fn formats_whole_document_range() {
    let range = whole_document_range("R radius = 1\nconst R diameter = 2\n");

    assert_eq!(range.start, Position::new(0, 0));
    assert_eq!(range.end.line, 3);
}
