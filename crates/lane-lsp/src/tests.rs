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

#[test]
fn emits_document_symbols_for_top_level_declarations() {
    let symbols = document_symbols::symbols(
        "#module\n\
         provided R time, scale\n\
         provided distance : R3 -> R\n\
         Set Material<roughness> = R\n\
         const Object output = Ball3D(r=scale)\n\
         shape = output\n",
    );

    let names = symbols
        .iter()
        .map(|symbol| symbol.name.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        names,
        vec!["#module", "time", "scale", "distance", "Material", "output", "shape"]
    );
    assert_eq!(symbols[0].kind, tower_lsp::lsp_types::SymbolKind::MODULE);
    assert_eq!(symbols[3].kind, tower_lsp::lsp_types::SymbolKind::FUNCTION);
    assert_eq!(symbols[4].kind, tower_lsp::lsp_types::SymbolKind::STRUCT);
    assert_eq!(symbols[5].kind, tower_lsp::lsp_types::SymbolKind::CONSTANT);
    assert_eq!(symbols[6].kind, tower_lsp::lsp_types::SymbolKind::VARIABLE);
    assert_eq!(symbols[5].selection_range.start, Position::new(4, 13));
}

#[test]
fn links_imports_to_resolved_modules() {
    let base_dir =
        std::env::temp_dir().join(format!("lane-lsp-import-links-{}", std::process::id()));
    let modules_dir = base_dir.join("modules");
    std::fs::create_dir_all(&modules_dir).unwrap();
    std::fs::write(modules_dir.join("helpers.lane"), "#module\n").unwrap();

    let links = Backend::import_links("  #import helpers\nconst R radius = 1\n", &base_dir);

    assert_eq!(links.len(), 1);
    assert_eq!(links[0].range.start, Position::new(0, 10));
    assert_eq!(links[0].range.end, Position::new(0, 17));
    assert_eq!(
        links[0].target.as_ref().unwrap().to_file_path().unwrap(),
        modules_dir.join("helpers.lane")
    );

    std::fs::remove_dir_all(base_dir).unwrap();
}

#[test]
fn links_quoted_nested_imports() {
    let base_dir = std::env::temp_dir().join(format!(
        "lane-lsp-quoted-import-links-{}",
        std::process::id()
    ));
    let modules_dir = base_dir.join("modules").join("math");
    std::fs::create_dir_all(&modules_dir).unwrap();
    std::fs::write(modules_dir.join("helpers.lane"), "#module\n").unwrap();

    let links = Backend::import_links("#import \"math/helpers\"\n", &base_dir);

    assert_eq!(links.len(), 1);
    assert_eq!(links[0].range.start, Position::new(0, 9));
    assert_eq!(links[0].range.end, Position::new(0, 21));
    assert_eq!(
        links[0].target.as_ref().unwrap().to_file_path().unwrap(),
        modules_dir.join("helpers.lane")
    );

    std::fs::remove_dir_all(base_dir).unwrap();
}
