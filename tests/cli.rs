use std::process::Command;

#[test]
fn lists_known_primitives_from_cli() {
    let output = Command::new(env!("CARGO_BIN_EXE_sdf-dsl"))
        .arg("--list-primitives")
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Ball3D: sdf0_Ball3D(vec3 p, ParamBall3D params) -> float"));
    assert!(stdout.contains("params ParamBall3D {r: float}"));
    assert!(stdout.contains("Polygon2D: sdf0_Polygon2D(vec2 p, vec2 vertices[POLYGON2D_MAX_VERTICES], int count) -> float"));
    assert!(stdout.contains("fields {points: vec2 list}"));
}
