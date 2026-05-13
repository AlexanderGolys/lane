use super::format_lane_source;

#[test]
fn formats_ascii_product_separators_in_types() {
    let source = "\
provided Hom(R3 x R3, R3) cross
provided f: X x Y -> Z
const Hom(R x R, R) wave = sin x cos
";

    assert_eq!(
        format_lane_source(source),
        "\
provided Hom(R3 × R3, R3) cross
provided f: X × Y -> Z
const Hom(R × R, R) wave = sin x cos
"
    );
}

#[test]
fn formats_product_type_definition_without_renaming_x_bindings() {
    let source = "\
Set Pair<left, right> = R x R
const R x = 1
";

    assert_eq!(
        format_lane_source(source),
        "\
Set Pair<left, right> = R × R
const R x = 1
"
    );
}
