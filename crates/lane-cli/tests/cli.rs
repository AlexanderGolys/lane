use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

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
        .args(["list", "2d"])
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
        .args(["list", "3d"])
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
    assert!(stderr.contains("error:"));
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
        .arg("list")
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("DivRing: Cat"));
    assert!(stdout.contains("RVect: Cat"));
    assert!(stdout.contains("RDivAlg: Cat"));
    assert!(stdout.contains("Bool: DivRing"));
    assert!(stdout.contains("Isom2: Grp"));
    assert!(stdout.contains("pow2: Hom(R, R)"));
    assert!(stdout.contains("pow: Hom(Z × Mon, Mon) | Hom(Rn × Rn, Rn)"));
    assert!(stdout.contains("not: Hom(Bool, Bool)"));
    assert!(stdout.contains("and: Hom(Bool × Bool, Bool)"));
    assert!(stdout.contains("or: Hom(Bool × Bool, Bool)"));
    assert!(stdout.contains("xor: Hom(Bool × Bool, Bool)"));
    assert!(!stdout.contains("boolNot: Hom(Bool, Bool)"));
    assert!(!stdout.contains("cexp: Hom(C, C)"));
    assert!(stdout.contains("union: Hom(Object × Object, Object)"));
    assert!(stdout.contains("smoothUnion: Hom(R, Hom(Object × Object, Object))"));
    assert!(stdout.contains("revolution: Hom(R, Hom(Object2D, Object))"));
    assert!(stdout.contains("extrude: Hom(R, Hom(Object, Object))"));
    assert!(stdout.contains("rot: Hom(R3 × R3 × R, Isom3)"));
    assert!(stdout.contains("rot2D: Hom(R2 × R, Isom2)"));
    assert!(!stdout.contains("derivative: Hom(Hom(R, R), Hom(R, R))"));
    assert!(!stdout.contains("gradient: Hom(Hom(R3, R), Hom(R3, R3))"));
    assert!(!stdout.contains("dfdx: Hom(Hom(R3, R), Hom(R3, R))"));
    assert!(!stdout.contains("dfdy: Hom(Hom(R3, R), Hom(R3, R))"));
    assert!(!stdout.contains("dfdz: Hom(Hom(R3, R), Hom(R3, R))"));
    assert!(!stdout.contains("dfdw: Hom(Hom(R4, R), Hom(R4, R))"));
    assert!(!stdout.contains("divergence: Hom(Hom(R3, R3), Hom(R3, R))"));
    assert!(stdout.contains("sin: Hom(Rn, Rn)"));
    assert!(!stdout.contains("inv: Hom(C, C)"));
    assert!(stdout.contains("clamp: Hom(Rn × Rn × Rn, Rn) | Hom(Rn × R × R, Rn)"));
    assert!(stdout.contains("reflect: Hom(Rn × Rn, Rn)"));
    assert!(stdout.contains("transpose: Hom(Mat{n}x{m}, Mat{m}x{n})"));
    assert!(stdout.contains("determinant: Hom(Mat{n}, R)"));
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
    assert!(stdout.contains("Isom3: Grp"));
    assert!(stdout.contains("RAlg: Cat"));
    assert!(!stdout.contains("partialX: Hom(R, Hom(Hom(R3, R), Hom(R3, R)))"));
    assert!(!stdout.contains("directionalDerivative"));
    assert!(!stdout.contains("ctanh: Hom(C, C)"));
}

#[test]
fn lists_all_builtin_items_from_command() {
    let output = Command::new(env!("CARGO_BIN_EXE_lane"))
        .args(["list", "all"])
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Ball3D: {r: R}"));
    assert!(stdout.contains("Box2D: {a: R, b: R}"));
    assert!(stdout.contains("Polygon2D: { points: R2 list }"));
    assert!(stdout.contains("DivRing: Cat"));
    assert!(stdout.contains("Bool: DivRing"));
    assert!(stdout.contains("sin: Hom(Rn, Rn)"));
    assert!(stdout.contains("clamp: Hom(Rn × Rn × Rn, Rn) | Hom(Rn × R × R, Rn)"));
    assert!(stdout.contains("min: Hom(Rn × Rn, Rn) | Hom(Rn × R, Rn) | Hom(R × Rn, Rn)"));
    assert!(stdout.contains("transpose: Hom(Mat{n}x{m}, Mat{m}x{n})"));
    assert!(stdout.contains("inverse: Hom(Mat{n}, Mat{n})"));
    assert!(stdout.contains("union: Hom(Object × Object, Object)"));
    assert!(!stdout.contains("matrixCompMult:"));
    assert!(!stdout.contains("sdf0_Ball3D"));
    assert!(!stdout.contains("struct ParamBall3D"));
}

#[test]
fn lists_all_builtin_items_from_flag() {
    let output = Command::new(env!("CARGO_BIN_EXE_lane"))
        .arg("--list-all")
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Ball3D: {r: R}"));
    assert!(stdout.contains("sin: Hom(Rn, Rn)"));
}

#[test]
fn lists_all_builtin_items_from_short_flag() {
    let output = Command::new(env!("CARGO_BIN_EXE_lane"))
        .arg("-la")
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Ball3D: {r: R}"));
    assert!(stdout.contains("transpose: Hom(Mat{n}x{m}, Mat{m}x{n})"));
    assert!(stdout.contains("determinant: Hom(Mat{n}, R)"));
}

#[test]
fn shows_known_builtin_object_detail_from_cli() {
    let output = Command::new(env!("CARGO_BIN_EXE_lane"))
        .args(["list", "revolution"])
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("revolution: Hom(R, Hom(Object2D, Object))"));
    assert!(stdout.contains("vec3 _op_revolution_point(vec3 _p, float _offset)"));
}

#[test]
fn shows_builtin_type_detail_from_cli() {
    let output = Command::new(env!("CARGO_BIN_EXE_lane"))
        .args(["list", "Isom2"])
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("struct Isom2"));
    assert!(stdout.contains("mult_Isom2"));
}

#[test]
fn shows_builtin_category_detail_from_cli() {
    let output = Command::new(env!("CARGO_BIN_EXE_lane"))
        .args(["list", "DivRing"])
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout, "DivRing: Cat\n");
}

#[test]
fn no_longer_lists_std_differential_objects_as_builtins() {
    let output = Command::new(env!("CARGO_BIN_EXE_lane"))
        .args(["list", "gradient"])
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
    assert!(stdout.contains("Ab Mon Grp Ring DivRing RVect RAlg RDivAlg"));
    assert!(stdout.contains("Isom2 Isom3"));
    assert!(stdout.contains("diff"));
    assert!(stdout.contains("--print-completion"));
    assert!(stdout.contains("-pc"));
    assert!(stdout.contains("preview repl list"));
    assert!(stdout.contains("all 2d 3d"));
    assert!(!stdout.contains("--list"));
    assert!(!stdout.contains("-l2"));
    assert!(!stdout.contains("--list-objects"));
    assert!(!stdout.contains("--list-all"));
    assert!(!stdout.contains("list-all"));
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
    assert!(stdout.contains("lane [SOURCE [TARGET]] [--show]"));
    assert!(stdout.contains("lane repl"));
    assert!(stdout.contains("lane list [NAME]"));
    assert!(stdout.contains("lane list 2d"));
    assert!(stdout.contains("lane list 3d"));
    assert!(stdout.contains("lane list all"));
    assert!(stdout.contains("lane -pc, --print-completion <bash|zsh|fish>"));
    assert!(stdout.contains("lane -h, --help"));
    assert!(stdout.contains("opens the interactive shell when stdin is a terminal"));
    assert!(stdout.contains("`lane repl` opens the same shell explicitly"));
    assert!(stdout.contains("right-clicking a transcript block copies"));
    assert!(stdout.contains("When TARGET is present"));
    assert!(stdout.contains("Use --show or -s with SOURCE TARGET"));
    assert!(!stdout.contains("lane -l, --list [NAME]"));
    assert!(!stdout.contains("lane list-all"));
}

#[test]
fn list_command_lists_builtin_objects() {
    let output = Command::new(env!("CARGO_BIN_EXE_lane"))
        .arg("list")
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("DivRing: Cat"));
    assert!(!stdout.contains("gradient: Hom(Hom(R3, R), Hom(R3, R3))"));
}

#[test]
fn rejects_show_without_source_and_target() {
    let output = Command::new(env!("CARGO_BIN_EXE_lane"))
        .args(["--show", "ParamBall3D"])
        .output()
        .unwrap();

    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--show requires SOURCE and TARGET"));
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
fn writes_compiled_output_to_target_path() {
    let temp_dir = unique_temp_dir("lane-cli-target");
    std::fs::create_dir(&temp_dir).unwrap();
    let source_path = temp_dir.join("scene.lane");
    let target_path = temp_dir.join("scene.glsl");
    std::fs::write(&source_path, "const Object output = Ball3D(r=1)\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_lane"))
        .arg(&source_path)
        .arg(&target_path)
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(output.stdout.is_empty());

    let glsl = std::fs::read_to_string(&target_path).unwrap();
    assert!(glsl.contains("float sdf_output(vec3 p)"));
    assert!(glsl.contains("sdf0_Ball3D(p, ParamBall3D(1.0f))"));

    std::fs::remove_dir_all(temp_dir).unwrap();
}

#[test]
fn writes_preview_fragment_and_vertex_shaders() {
    let temp_dir = unique_temp_dir("lane-cli-preview");
    std::fs::create_dir(&temp_dir).unwrap();
    let source_path = temp_dir.join("scene.lane");
    let frag_path = temp_dir.join("preview.frag");
    let vert_path = temp_dir.join("preview.vert");
    std::fs::write(
        &source_path,
        "const Object scene = Ball3D(r=1)\nconst Material material = Material((0.8, 0.6, 0.4), (0, 0, 0), 0.2)\nconst Hom(R3, Material) scene_material = (x, y, z) |-> material\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_lane"))
        .arg(&source_path)
        .arg(format!("--frag={}", frag_path.display()))
        .arg(format!("--vert={}", vert_path.display()))
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let frag = std::fs::read_to_string(&frag_path).unwrap();
    assert!(frag.starts_with("#version 300 es\n"));
    assert!(frag.contains("out vec4 outColor;"));
    assert!(frag.contains("uniform vec3 cameraPosition;"));
    assert!(!frag.contains("raytracingMaterial"));
    assert!(frag.contains("float sdf_scene(vec3 p)"));
    assert!(frag.contains("void main()"));
    assert!(frag.contains("Material scene_material(vec3 _t)"));
    assert!(frag.contains("outColor = preview_shade(gl_FragCoord.xy);"));

    let vert = std::fs::read_to_string(&vert_path).unwrap();
    assert!(vert.starts_with("#version 300 es\n"));
    assert!(vert.contains("gl_Position"));

    std::fs::remove_dir_all(temp_dir).unwrap();
}

#[test]
fn preview_shader_version_flag_splits_es_suffix() {
    let temp_dir = unique_temp_dir("lane-cli-preview-version");
    std::fs::create_dir(&temp_dir).unwrap();
    let source_path = temp_dir.join("scene.lane");
    let frag_path = temp_dir.join("preview.frag");
    std::fs::write(
        &source_path,
        "const Object scene = Ball3D(r=1)\nconst Material material = Material((0.8, 0.6, 0.4), (0, 0, 0), 0.2)\nconst Hom(R3, Material) scene_material = (x, y, z) |-> material\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_lane"))
        .arg(&source_path)
        .arg(format!("--frag={}", frag_path.display()))
        .arg("--version=310es")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let frag = std::fs::read_to_string(&frag_path).unwrap();
    assert!(frag.starts_with("#version 310 es\n"));

    std::fs::remove_dir_all(temp_dir).unwrap();
}

#[test]
fn writes_vulkan_preview_glsl_shaders() {
    let temp_dir = unique_temp_dir("lane-cli-preview-vulkan-glsl");
    std::fs::create_dir(&temp_dir).unwrap();
    let source_path = temp_dir.join("scene.lane");
    let frag_path = temp_dir.join("preview.frag");
    let vert_path = temp_dir.join("preview.vert");
    std::fs::write(
        &source_path,
        "provided R time\nconst Object scene = Ball3D(r=1 + sin(time))\nconst Material material = Material((0.8, 0.6, 0.4), (0, 0, 0), 0.2)\nconst Hom(R3, Material) scene_material = (x, y, z) |-> material\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_lane"))
        .arg(&source_path)
        .arg("--target=vulkan")
        .arg(format!("--frag={}", frag_path.display()))
        .arg(format!("--vert={}", vert_path.display()))
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let frag = std::fs::read_to_string(&frag_path).unwrap();
    assert!(frag.starts_with("#version 450\n"));
    assert!(frag.contains("layout(location = 0) out vec4 outColor;"));
    assert!(frag.contains("layout(std140, push_constant) uniform PreviewUniforms"));
    assert!(frag.contains("    vec3 cameraPosition;"));
    assert!(frag.contains("    float time;"));
    assert!(!frag.contains("uniform vec3 cameraPosition;"));

    let vert = std::fs::read_to_string(&vert_path).unwrap();
    assert!(vert.starts_with("#version 450\n"));
    assert!(vert.contains("gl_VertexIndex"));

    std::fs::remove_dir_all(temp_dir).unwrap();
}

#[test]
fn writes_vulkan_preview_spirv_shaders() {
    let temp_dir = unique_temp_dir("lane-cli-preview-vulkan-spv");
    std::fs::create_dir(&temp_dir).unwrap();
    let source_path = temp_dir.join("scene.lane");
    let frag_path = temp_dir.join("preview.frag.spv");
    let vert_path = temp_dir.join("preview.vert.spv");
    std::fs::write(
        &source_path,
        "const Object scene = Ball3D(r=1)\nconst Material material = Material((0.8, 0.6, 0.4), (0, 0, 0), 0.2)\nconst Hom(R3, Material) scene_material = (x, y, z) |-> material\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_lane"))
        .arg(&source_path)
        .arg(format!("--frag-spv={}", frag_path.display()))
        .arg(format!("--vert-spv={}", vert_path.display()))
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        std::fs::read(&frag_path).unwrap()[..4],
        [0x03, 0x02, 0x23, 0x07]
    );
    assert_eq!(
        std::fs::read(&vert_path).unwrap()[..4],
        [0x03, 0x02, 0x23, 0x07]
    );

    std::fs::remove_dir_all(temp_dir).unwrap();
}

#[test]
fn preview_generation_reports_missing_scene_material_requirement() {
    let temp_dir = unique_temp_dir("lane-cli-preview-missing-scene-material");
    std::fs::create_dir(&temp_dir).unwrap();
    let source_path = temp_dir.join("scene.lane");
    let frag_path = temp_dir.join("preview.frag");
    std::fs::write(&source_path, "const Object scene = Ball3D(r=1)\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_lane"))
        .arg(&source_path)
        .arg(format!("--frag={}", frag_path.display()))
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("preview generation requirements were not met"));
    assert!(stderr.contains("scene_material"));
    assert!(!stderr.contains("alias"));

    std::fs::remove_dir_all(temp_dir).unwrap();
}

#[test]
fn preview_generation_reports_missing_scene_requirement() {
    let temp_dir = unique_temp_dir("lane-cli-preview-missing-scene");
    std::fs::create_dir(&temp_dir).unwrap();
    let source_path = temp_dir.join("scene.lane");
    let frag_path = temp_dir.join("preview.frag");
    std::fs::write(
        &source_path,
        "const Material material = Material((0.8, 0.6, 0.4), (0, 0, 0), 0.2)\nconst Hom(R3, Material) scene_material = (x, y, z) |-> material\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_lane"))
        .arg(&source_path)
        .arg(format!("--frag={}", frag_path.display()))
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("preview generation requirements were not met"));
    assert!(stderr.contains("const Object scene"));
    assert!(!stderr.contains("alias"));

    std::fs::remove_dir_all(temp_dir).unwrap();
}

#[test]
fn show_prints_compiled_output_while_writing_target_path() {
    let temp_dir = unique_temp_dir("lane-cli-show-target");
    std::fs::create_dir(&temp_dir).unwrap();
    let source_path = temp_dir.join("scene.lane");
    let target_path = temp_dir.join("scene.glsl");
    std::fs::write(&source_path, "const Object output = Ball3D(r=1)\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_lane"))
        .arg("--show")
        .arg(&source_path)
        .arg(&target_path)
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    let glsl = std::fs::read_to_string(&target_path).unwrap();
    assert_eq!(stdout, format!("{glsl}\n"));
    assert!(stdout.contains("sdf0_Ball3D(p, ParamBall3D(1.0f))"));

    std::fs::remove_dir_all(temp_dir).unwrap();
}

#[test]
fn short_show_flag_can_follow_source_and_target() {
    let temp_dir = unique_temp_dir("lane-cli-short-show-target");
    std::fs::create_dir(&temp_dir).unwrap();
    let source_path = temp_dir.join("scene.lane");
    let target_path = temp_dir.join("scene.glsl");
    std::fs::write(&source_path, "const Object output = Ball3D(r=2)\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_lane"))
        .arg(&source_path)
        .arg(&target_path)
        .arg("-s")
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    let glsl = std::fs::read_to_string(&target_path).unwrap();
    assert_eq!(stdout, format!("{glsl}\n"));
    assert!(stdout.contains("ParamBall3D(2.0f)"));

    std::fs::remove_dir_all(temp_dir).unwrap();
}

#[test]
fn treats_fragment_shader_directive_as_a_comment() {
    let output = Command::new(env!("CARGO_BIN_EXE_lane"))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            child.stdin.as_mut().unwrap().write_all(
                b"// fragment-shader: #version 330 core\nconst Object output = Ball3D(r=1)\n",
            )?;
            child.wait_with_output()
        })
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(!stdout.starts_with("#version 330 core"));
    assert!(!stdout.contains("uniform vec2 resolution;"));
    assert!(stdout.contains("float sdf_output(vec3 p)"));
}

#[test]
fn prefixes_interpreter_errors_with_error_type() {
    let output = Command::new(env!("CARGO_BIN_EXE_lane"))
        .stdin(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            child
                .stdin
                .as_mut()
                .unwrap()
                .write_all(b"const Object output = Missing3D(r=1)\n")?;
            child.wait_with_output()
        })
        .unwrap();

    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("lane::Error: line 1: unknown primitive 'Missing3D'"));
}

fn unique_temp_dir(name: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("{name}-{}-{nanos}", std::process::id()))
}
