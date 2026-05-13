use super::{color, format_error, highlight_builtin_object_line, highlight_glsl, COLOR_ERROR};

/// Highlights GLSL keywords, types, identifiers, and numbers.
#[test]
fn highlights_glsl_keywords_types_and_numbers() {
    let highlighted = highlight_glsl("float sdf_output(vec3 p) { return 1.0f-2e-3f; }");

    assert!(highlighted.contains("\x1b[35mfloat\x1b[0m"));
    assert!(highlighted.contains("\x1b[34mvec3\x1b[0m"));
    assert!(highlighted.contains("\x1b[33msdf_output\x1b[0m"));
    assert!(highlighted.contains("\x1b[36m1.0f\x1b[0m-\x1b[36m2e-3f\x1b[0m"));
}

/// Highlights builtin object declarations with expected color classes.
#[test]
fn highlights_builtin_object_names_and_lane_types() {
    let highlighted = highlight_builtin_object_line(
        "union",
        "Hom(Object × Object, Object)",
        lane::KnownBuiltinObjectKind::Function,
    );

    assert!(highlighted.contains("\x1b[34munion\x1b[0m"));
    assert!(highlighted.contains("\x1b[97m:\x1b[0m"));
    assert!(highlighted.contains("\x1b[35mHom\x1b[0m"));
    assert!(highlighted.contains("\x1b[33mObject\x1b[0m"));
    assert!(highlighted.contains("\x1b[97m(\x1b[0m"));
    assert!(highlighted.contains("\x1b[97m,\x1b[0m"));
    assert!(highlighted.contains("\x1b[35m×\x1b[0m"));
}

/// Verifies built-in types are colorized as type tokens.
#[test]
fn highlights_builtin_type_names_as_types() {
    let highlighted =
        highlight_builtin_object_line("H", "RDivAlg", lane::KnownBuiltinObjectKind::Type);

    assert!(highlighted.contains("\x1b[33mH\x1b[0m"));
    assert!(highlighted.contains("\x1b[92mRDivAlg\x1b[0m"));
    assert!(!highlighted.contains("\x1b[33mRDivAlg\x1b[0m"));
}

/// Verifies categories and metatype names receive distinct styling.
#[test]
fn highlights_categories_as_bright_yellow_and_cat_as_white() {
    let highlighted =
        highlight_builtin_object_line("DivRing", "Cat", lane::KnownBuiltinObjectKind::Category);

    assert!(highlighted.contains("\x1b[92mDivRing\x1b[0m"));
    assert!(highlighted.contains("\x1b[38;2;255;255;255mCat\x1b[0m"));
    assert!(!highlighted.contains("\x1b[33mDivRing\x1b[0m"));
    assert!(!highlighted.contains("\x1b[92mCat\x1b[0m"));
}

/// Verifies metatype `Type` is treated as type token, not category.
#[test]
fn highlights_type_metatype_as_type_not_category() {
    let highlighted =
        highlight_builtin_object_line("Object", "Type", lane::KnownBuiltinObjectKind::Type);

    assert!(highlighted.contains("\x1b[33mObject\x1b[0m"));
    assert!(highlighted.contains("\x1b[33mType\x1b[0m"));
    assert!(!highlighted.contains("\x1b[92mType\x1b[0m"));
}

/// Ensures compiler errors print their tagged error source.
#[test]
fn formats_lane_errors_with_error_type() {
    let err = lane::compile_program("const Object output = Unknown3D(r=1)\n").unwrap_err();

    assert!(format_error(&err).contains("lane::Error: line 1: unknown primitive 'Unknown3D'"));
}

/// Ensures ANSI color path wraps terminal errors in expected red code.
#[test]
fn colors_error_messages_red() {
    assert_eq!(
        color(COLOR_ERROR, "lane::Error: bad"),
        "\x1b[31mlane::Error: bad\x1b[0m"
    );
}
