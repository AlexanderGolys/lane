use std::process::Command;

#[test]
fn lists_known_primitives_from_cli() {
    let output = Command::new(env!("CARGO_BIN_EXE_lane"))
        .arg("--list")
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Ball2D\nstruct ParamBall2D {\n    float r;\n};"));
    assert!(stdout.contains("Ball3D\nstruct ParamBall3D {\n    float r;\n};"));
    assert!(stdout
        .contains("\n\nBox3D\nstruct ParamBox3D {\n    float a;\n    float b;\n    float c;\n};"));
    assert!(stdout.contains("Plane3D\nstruct ParamPlane3D"));
    assert!(stdout.contains("Line3D\nstruct ParamLine3D"));
    assert!(stdout.contains("Triangle3D\nstruct ParamTriangle3D"));
    assert!(stdout.contains("Segment3D\nstruct ParamSegment3D"));
    assert!(stdout.contains("Box2D\nstruct ParamBox2D {\n    float a;\n    float b;\n};"));
    assert!(stdout.contains("Quad2D\nstruct ParamQuad2D"));
    assert!(stdout.contains("Quad3D\nstruct ParamQuad3D"));
    assert!(stdout.contains("Polygon2D\n{ points: R2 list }"));
    assert!(!stdout.contains("Ball3D: {r: R}"));
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
    assert!(stdout.contains("Ball2D\nstruct ParamBall2D"));
    assert!(stdout.contains("Ball3D\nstruct ParamBall3D"));
    assert!(stdout.contains("Box3D\nstruct ParamBox3D"));
    assert!(stdout.contains("Plane3D\nstruct ParamPlane3D"));
    assert!(stdout.contains("Segment3D\nstruct ParamSegment3D"));
}

#[test]
fn lists_only_2d_primitives_from_cli() {
    let output = Command::new(env!("CARGO_BIN_EXE_lane"))
        .arg("--list2d")
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Ball2D\nstruct ParamBall2D"));
    assert!(stdout.contains("Box2D\nstruct ParamBox2D"));
    assert!(stdout.contains("Polygon2D\n{ points: R2 list }"));
    assert!(stdout.contains("Point2D\nstruct ParamPoint2D"));
    assert!(stdout.contains("Quad2D\nstruct ParamQuad2D"));
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
    assert!(stdout.contains("Ball2D\nstruct ParamBall2D"));
    assert!(stdout.contains("Point2D\nstruct ParamPoint2D"));
    assert!(stdout.contains("Quad2D\nstruct ParamQuad2D"));
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
    assert!(stdout.contains("Ball3D\nstruct ParamBall3D"));
    assert!(stdout.contains("Box3D\nstruct ParamBox3D"));
    assert!(stdout.contains("Plane3D\nstruct ParamPlane3D"));
    assert!(stdout.contains("Line3D\nstruct ParamLine3D"));
    assert!(stdout.contains("Segment3D\nstruct ParamSegment3D"));
    assert!(stdout.contains("Simplex3D\nstruct ParamSimplex3D"));
    assert!(stdout.contains("Triangle3D\nstruct ParamTriangle3D"));
    assert!(stdout.contains("Torus3D\nstruct ParamTorus3D"));
    assert!(stdout.contains("Quad3D\nstruct ParamQuad3D"));
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
    assert!(stdout.contains("Torus3D\nstruct ParamTorus3D"));
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
fn shows_ball2d_primitive_detail_from_cli() {
    let output = Command::new(env!("CARGO_BIN_EXE_lane"))
        .args(["--list", "Ball2D"])
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Ball2D: {r: R}"));
    assert!(stdout.contains("struct ParamBall2D"));
    assert!(stdout.contains("float sdf0_Ball2D(vec2 p, ParamBall2D params)"));
}

#[test]
fn shows_known_primitive_detail_body_from_cli() {
    let output = Command::new(env!("CARGO_BIN_EXE_lane"))
        .args(["--list", "Ball3D"])
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Ball3D: {r: R}"));
    assert!(stdout.contains("ParamBall3D"));
    assert!(stdout.contains("struct ParamBall3D"));
    assert!(stdout.contains("float r;"));
    assert!(stdout.contains("float sdf0_Ball3D(vec3 p, ParamBall3D params)"));
}

#[test]
fn lists_known_builtin_objects_from_cli() {
    let output = Command::new(env!("CARGO_BIN_EXE_lane"))
        .arg("--list-objects")
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Field: Cat"));
    assert!(stdout.contains("VectR: Cat"));
    assert!(stdout.contains("C: Field × Grp × AlgR × VectR"));
    assert!(stdout.contains("H: Field × Grp × AlgR × VectR"));
    assert!(stdout.contains("E2: VectR"));
    assert!(stdout.contains("pow2: Hom(R, R)"));
    assert!(stdout.contains("cexp: Hom(C, C)"));
    assert!(stdout.contains("Union: Hom(Solid × Solid, Solid)"));
    assert!(stdout.contains("SmoothUnion: Hom(R, Hom(Solid × Solid, Solid))"));
    assert!(stdout.contains("Revolution: Hom(R, Hom(Solid, Solid))"));
    assert!(stdout.contains("Extrusion: Hom(R, Hom(Solid, Solid))"));
    assert!(!stdout.contains(" sin"));
    assert!(!stdout.contains("sdf0_Ball3D"));
}

#[test]
fn lists_known_builtin_objects_from_short_flag() {
    let output = Command::new(env!("CARGO_BIN_EXE_lane"))
        .arg("-lo")
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("ctanh: Hom(C, C)"));
    assert!(stdout.contains("E3: VectR"));
    assert!(stdout.contains("AlgR: Cat"));
}

#[test]
fn shows_known_builtin_object_detail_from_cli() {
    let output = Command::new(env!("CARGO_BIN_EXE_lane"))
        .args(["--list-objects", "Revolution"])
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Revolution: Hom(R, Hom(Solid, Solid))"));
    assert!(stdout.contains("vec3 op_revolution_point(vec3 p, float offset)"));
}

#[test]
fn shows_builtin_type_detail_from_cli() {
    let output = Command::new(env!("CARGO_BIN_EXE_lane"))
        .args(["--list-objects", "H"])
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("H: Field × Grp × AlgR × VectR"));
    assert!(stdout.contains("#define H vec4"));
}

#[test]
fn shows_builtin_category_detail_from_cli() {
    let output = Command::new(env!("CARGO_BIN_EXE_lane"))
        .args(["--list-objects", "Field"])
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout, "Field: Cat\n");
}

#[test]
fn rejects_unknown_builtin_object_detail_from_cli() {
    let output = Command::new(env!("CARGO_BIN_EXE_lane"))
        .args(["--list-objects", "gradient"])
        .output()
        .unwrap();

    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("unknown builtin object 'gradient'"));
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
    assert!(stdout.contains("Ab Mon Grp Ring Field VectR AlgR"));
    assert!(stdout.contains("C E2 E3"));
    assert!(stdout.contains("Difference"));
    assert!(!stdout.contains("Complex Difference E2"));
    assert!(stdout.contains("--print-completion"));
    assert!(stdout.contains("-pc"));
    assert!(stdout.contains("--list"));
    assert!(stdout.contains("-l2"));
    assert!(stdout.contains("--list-objects"));
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
    assert!(stdout.contains("lane -l, --list [NAME]"));
    assert!(stdout.contains("lane -l2, --list2d"));
    assert!(stdout.contains("lane -l3, --list3d"));
    assert!(stdout.contains("lane -lo, --list-objects [NAME]"));
    assert!(stdout.contains("lane -pc, --print-completion <bash|zsh|fish>"));
    assert!(stdout.contains("lane -h, --help"));
    assert!(!stdout.contains("lane --list [NAME]"));
    assert!(!stdout.contains("lane -l [NAME]"));
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

#[test]
fn wraps_output_in_fragment_shader_when_directive_is_present() {
    let output = Command::new(env!("CARGO_BIN_EXE_lane"))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            child
                .stdin
                .as_mut()
                .unwrap()
                .write_all(b"// fragment-shader: #version 330 core\ngenerate Ball3D(r=1)\n")?;
            child.wait_with_output()
        })
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.starts_with("#version 330 core"));
    assert!(stdout.contains("uniform vec2 resolution;"));
    assert!(stdout.contains("float d = scene_sdf(vec3(uv, 0.0));"));
}

#[test]
fn rejects_fragment_shader_wrapper_for_lane_inputs() {
    let output = Command::new(env!("CARGO_BIN_EXE_lane"))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            child
                .stdin
                .as_mut()
                .unwrap()
                .write_all(b"// fragment-shader: #version 330 core\nprovided Float time\ngenerate Ball3D(r=time)\n")?;
            child.wait_with_output()
        })
        .unwrap();

    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("fragment shader wrapper currently requires"));
}
