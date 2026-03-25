use std::process::Command;

#[test]
fn lists_known_primitives_from_cli() {
    let output = Command::new(env!("CARGO_BIN_EXE_lane"))
        .arg("--list")
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Ball3D: {r: float}"));
    assert!(stdout.contains("Box2D: {a: float, b: float}"));
    assert!(stdout.contains("Polygon2D: { points: vec2 list }"));
    assert!(!stdout.contains("ParamBall3D\n"));
    assert!(!stdout.contains("[3D]"));
    assert!(!stdout.contains("sdf0_Ball3D"));
}

#[test]
fn lists_known_primitives_from_short_flag() {
    let output = Command::new(env!("CARGO_BIN_EXE_lane"))
        .arg("-l")
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Ball3D: {r: float}"));
}

#[test]
fn lists_only_2d_primitives_from_cli() {
    let output = Command::new(env!("CARGO_BIN_EXE_lane"))
        .arg("--list2d")
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Box2D: {a: float, b: float}"));
    assert!(stdout.contains("Polygon2D: { points: vec2 list }"));
    assert!(stdout.contains("Point2D: {at: vec2}"));
    assert!(!stdout.contains("[2D]"));
    assert!(!stdout.contains("Ball3D"));
}

#[test]
fn lists_only_2d_primitives_from_short_flag() {
    let output = Command::new(env!("CARGO_BIN_EXE_lane"))
        .arg("-l2")
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Point2D: {at: vec2}"));
    assert!(!stdout.contains("Ball3D"));
}

#[test]
fn lists_only_3d_primitives_from_cli() {
    let output = Command::new(env!("CARGO_BIN_EXE_lane"))
        .arg("--list3d")
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Ball3D: {r: float}"));
    assert!(stdout.contains("Simplex3D: {p0: vec3, p1: vec3, p2: vec3, p3: vec3}"));
    assert!(stdout.contains("Torus3D: {major: float, minor: float}"));
    assert!(!stdout.contains("[3D]"));
    assert!(!stdout.contains("Box2D"));
}

#[test]
fn lists_only_3d_primitives_from_short_flag() {
    let output = Command::new(env!("CARGO_BIN_EXE_lane"))
        .arg("-l3")
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Torus3D: {major: float, minor: float}"));
    assert!(!stdout.contains("Box2D"));
}

#[test]
fn shows_known_primitive_detail_from_cli() {
    let output = Command::new(env!("CARGO_BIN_EXE_lane"))
        .args(["--list", "ParamBall3D"])
        .output()
        .unwrap();

    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("unknown primitive 'ParamBall3D'"));
}

#[test]
fn shows_known_primitive_detail_body_from_cli() {
    let output = Command::new(env!("CARGO_BIN_EXE_lane"))
        .args(["--list", "Ball3D"])
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Ball3D: {r: float}"));
    assert!(stdout.contains("ParamBall3D"));
    assert!(stdout.contains("struct ParamBall3D"));
    assert!(stdout.contains("float r;"));
    assert!(stdout.contains("float sdf0_Ball3D(vec3 p, ParamBall3D params)"));
}

#[test]
fn lists_known_predefined_functions_from_cli() {
    let output = Command::new(env!("CARGO_BIN_EXE_lane"))
        .arg("--list-functions")
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("op_union"));
    assert!(stdout.contains("op_smooth_union"));
    assert!(stdout.contains("sdf0_Ball3D"));
    assert!(!stdout.contains("ParamBall3D"));
}

#[test]
fn lists_known_predefined_functions_from_short_flag() {
    let output = Command::new(env!("CARGO_BIN_EXE_lane"))
        .arg("-lf")
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("op_union"));
}

#[test]
fn lists_known_predefined_types_from_cli() {
    let output = Command::new(env!("CARGO_BIN_EXE_lane"))
        .arg("--list-types")
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("ParamBall3D"));
    assert!(stdout.contains("ParamBox2D"));
    assert!(!stdout.contains("sdf0_Ball3D"));
}

#[test]
fn lists_known_predefined_types_from_short_flag() {
    let output = Command::new(env!("CARGO_BIN_EXE_lane"))
        .arg("-lt")
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("ParamBox2D"));
}

#[test]
fn prints_bash_completion_from_cli() {
    let output = Command::new(env!("CARGO_BIN_EXE_lane"))
        .args(["--print-completion", "bash"])
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("complete -F _lane lane"));
    assert!(stdout.contains("--print-completion"));
    assert!(stdout.contains("-pc"));
    assert!(stdout.contains("--list"));
    assert!(stdout.contains("-l2"));
}

#[test]
fn rejects_unknown_completion_shell() {
    let output = Command::new(env!("CARGO_BIN_EXE_lane"))
        .args(["--print-completion", "tcsh"])
        .output()
        .unwrap();

    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("unsupported shell 'tcsh'"));
}

#[test]
fn prints_help_from_cli() {
    let output = Command::new(env!("CARGO_BIN_EXE_lane"))
        .arg("--help")
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Usage:"));
    assert!(stdout.contains("lane --list [NAME]"));
    assert!(stdout.contains("lane -l [NAME]"));
    assert!(stdout.contains("lane --list2d"));
    assert!(stdout.contains("lane -l2"));
    assert!(stdout.contains("lane --list3d"));
    assert!(stdout.contains("lane -l3"));
    assert!(stdout.contains("lane --list-functions"));
    assert!(stdout.contains("lane -lf"));
    assert!(stdout.contains("lane --list-types"));
    assert!(stdout.contains("lane -lt"));
    assert!(stdout.contains("lane --print-completion <bash|zsh|fish>"));
    assert!(stdout.contains("lane -pc <bash|zsh|fish>"));
    assert!(stdout.contains("lane -h"));
}

#[test]
fn treats_old_list_command_as_input_path() {
    let output = Command::new(env!("CARGO_BIN_EXE_lane"))
        .arg("list")
        .output()
        .unwrap();

    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("No such file or directory"));
}

#[test]
fn treats_removed_show_flag_as_invalid_path() {
    let output = Command::new(env!("CARGO_BIN_EXE_lane"))
        .args(["--show", "ParamBall3D"])
        .output()
        .unwrap();

    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("unexpected arguments") || stderr.contains("No such file or directory")
    );
}

#[test]
fn treats_removed_list_preregistered_flag_as_input_path() {
    let output = Command::new(env!("CARGO_BIN_EXE_lane"))
        .arg("--list-preregistered")
        .output()
        .unwrap();

    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("No such file or directory"));
}

#[test]
fn prints_help_from_short_flag() {
    let output = Command::new(env!("CARGO_BIN_EXE_lane"))
        .arg("-h")
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Usage:"));
}

#[test]
fn treats_bare_help_as_input_path() {
    let output = Command::new(env!("CARGO_BIN_EXE_lane"))
        .arg("help")
        .output()
        .unwrap();

    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("No such file or directory"));
}
