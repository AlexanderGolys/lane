use super::*;

#[test]
/// Verifies that the word extractor follows current cursor character position.
fn finds_word_at_lsp_position() {
    let text = "const Object scene = Ball3D(r=1)\n";
    let position = Position::new(0, 24);

    assert_eq!(
        position::word_at_position(text, position).as_deref(),
        Some("Ball3D")
    );
}

#[test]
/// Checks hover text exists for a known primitive name.
fn hovers_known_primitive() {
    let hover = Backend::hover_for_word("Ball3D").unwrap();

    assert!(hover.contains("Ball3D"));
    assert!(hover.contains("r: R"));
}

#[test]
/// Confirms completion list contains new syntax and skips legacy type placeholders.
fn completes_new_language_surface() {
    let labels = Backend::completion_items()
        .into_iter()
        .map(|item| item.label)
        .collect::<Vec<_>>();

    assert!(labels.iter().any(|label| label == "#import"));
    assert!(labels.iter().any(|label| label == "raytracing"));
    assert!(labels.iter().any(|label| label == "Ball3D"));
    assert!(labels.iter().any(|label| label == "Mat{n}"));
    assert!(labels.iter().any(|label| label == "Mat{n}x{m}"));
    assert!(!labels.iter().any(|label| label == "R{n}"));
}

#[test]
/// Parses call context when cursor sits on a function call argument list.
fn finds_call_context_at_lsp_position() {
    let text = "const R y = mix(a, clamp(b, 0, 1), 0.5)\n";
    let context = signature::call_context_at_position(text, Position::new(0, 38)).unwrap();

    assert_eq!(
        context,
        signature::CallContext {
            name: "mix".to_string(),
            active_parameter: 2,
        }
    );
}

#[test]
/// Exercises UTF-16 column handling in cursor-to-byte conversion.
fn call_context_uses_lsp_utf16_columns() {
    let text = "const Object output = 💡 Ball3D(r=1)\n";
    let character = text[..text.find("r=1").unwrap() + 1].encode_utf16().count() as u32;
    let context = signature::call_context_at_position(text, Position::new(0, character)).unwrap();

    assert_eq!(context.name, "Ball3D");
    assert_eq!(context.active_parameter, 0);
}

#[test]
/// Builds signature help for a builtin primitive and validates parameter labels.
fn provides_primitive_signature_help() {
    let context = signature::CallContext {
        name: "Ball3D".to_string(),
        active_parameter: 0,
    };
    let help = signature::signature_help_for_context(&context).unwrap();

    assert_eq!(help.active_parameter, Some(0));
    assert_eq!(help.signatures[0].label, "Ball3D(r: R)");
    assert_eq!(
        help.signatures[0].parameters.as_ref().unwrap()[0].label,
        tower_lsp::lsp_types::ParameterLabel::Simple("r: R".to_string())
    );
}

#[test]
/// Builds signature help for a builtin function with overload-like args.
fn provides_builtin_function_signature_help() {
    let context = signature::CallContext {
        name: "mix".to_string(),
        active_parameter: 1,
    };
    let help = signature::signature_help_for_context(&context).unwrap();

    assert_eq!(help.active_parameter, Some(1));
    assert!(help.signatures[0].label.starts_with("mix("));
    assert!(help.signatures[0].label.contains("->"));
    assert!(help.signatures[0].parameters.as_ref().unwrap().len() >= 3);
}

#[test]
/// Confirms document range helper spans the full parsed source.
fn formats_whole_document_range() {
    let range = formatting::whole_document_range("R radius = 1\nconst R diameter = 2\n");

    assert_eq!(range.start, Position::new(0, 0));
    assert_eq!(range.end.line, 3);
}

#[test]
/// Validates symbol extraction for module, declarations, and provided inputs.
fn emits_document_symbols_for_top_level_declarations() {
    let symbols = document_symbols::symbols(
        "#module\n\
         provided R time, scale\n\
         provided distance : R3 -> R\n\
         Ab Distance = R {0: zero_R, -: neg_R, +: add_R}\n\
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
        vec!["#module", "time", "scale", "distance", "Distance", "Material", "output", "shape"]
    );
    assert_eq!(symbols[0].kind, tower_lsp::lsp_types::SymbolKind::MODULE);
    assert_eq!(symbols[3].kind, tower_lsp::lsp_types::SymbolKind::FUNCTION);
    assert_eq!(symbols[4].kind, tower_lsp::lsp_types::SymbolKind::STRUCT);
    assert_eq!(symbols[5].kind, tower_lsp::lsp_types::SymbolKind::STRUCT);
    assert_eq!(symbols[6].kind, tower_lsp::lsp_types::SymbolKind::CONSTANT);
    assert_eq!(symbols[7].kind, tower_lsp::lsp_types::SymbolKind::VARIABLE);
    assert_eq!(symbols[6].selection_range.start, Position::new(5, 13));
}

#[test]
/// Resolves bare imports into sibling module files.
fn links_imports_to_resolved_modules() {
    let base_dir =
        std::env::temp_dir().join(format!("lane-lsp-import-links-{}", std::process::id()));
    let modules_dir = base_dir.join("modules");
    std::fs::create_dir_all(&modules_dir).unwrap();
    std::fs::write(modules_dir.join("helpers.lane"), "#module\n").unwrap();

    let links = links::import_links("  #import helpers\nconst R radius = 1\n", &base_dir);

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
/// Resolves quoted module imports into nested directories.
fn links_quoted_nested_imports() {
    let base_dir = std::env::temp_dir().join(format!(
        "lane-lsp-quoted-import-links-{}",
        std::process::id()
    ));
    let modules_dir = base_dir.join("modules").join("math");
    std::fs::create_dir_all(&modules_dir).unwrap();
    std::fs::write(modules_dir.join("helpers.lane"), "#module\n").unwrap();

    let links = links::import_links("#import \"math/helpers\"\n", &base_dir);

    assert_eq!(links.len(), 1);
    assert_eq!(links[0].range.start, Position::new(0, 9));
    assert_eq!(links[0].range.end, Position::new(0, 21));
    assert_eq!(
        links[0].target.as_ref().unwrap().to_file_path().unwrap(),
        modules_dir.join("helpers.lane")
    );

    std::fs::remove_dir_all(base_dir).unwrap();
}
