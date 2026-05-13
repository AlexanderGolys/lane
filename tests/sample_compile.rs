use lane::{compile_program as compile_program_with_float_suffixes, Error};
use std::fs;

fn compile_program(source: &str) -> Result<String, Error> {
    compile_program_with_float_suffixes(source).map(|glsl| strip_glsl_float_suffixes(&glsl))
}

fn strip_glsl_float_suffixes(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let chars = source.chars().collect::<Vec<_>>();
    let mut index = 0;

    while index < chars.len() {
        if index > 0
            && chars[index] == 'f'
            && chars[index - 1].is_ascii_digit()
            && (index + 1 == chars.len()
                || !(chars[index + 1].is_ascii_alphanumeric() || chars[index + 1] == '_'))
        {
            index += 1;
            continue;
        }
        out.push(chars[index]);
        index += 1;
    }

    out
}

#[test]
fn compiles_sample_program_to_glsl() {
    let source = r#"
provided Float time
provided Hom(Float, Float) rB
provided Hom(Float, Vec3) centerA

Hom(Float, Float) hardness = pow2 @ sin + .5
Hom(Float, Vec3) centerB = (1, sin, cos) + centerA / 2

Object A = Ball3D(r=3) + centerA(time)
Object B = Ball3D(r=rB(time)) + centerB(time)
Object C = smoothUnion(hardness(time))(A, B)

const Object output = C
"#;

    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("struct ParamBall3D"));
    assert!(glsl.contains("float sdf0_Ball3D(vec3 p, ParamBall3D params)"));
    assert!(glsl.contains("float pow2(float x)"));
    assert!(glsl.contains("float _op_smooth_union(float _a, float _b, float _k)"));
    assert!(glsl.contains("float hardness(float _t);"));
    assert!(glsl.contains("vec3 centerB(float _t);"));
    assert!(glsl.contains("float hardness(float _t)"));
    assert!(glsl.contains("vec3 centerB(float _t)"));
    assert!(glsl.contains("float sdf_output(vec3 p)"));
    assert!(glsl.contains("vec3 grad_sdf_output(vec3 p)"));
}

#[test]
fn compiles_showcase_program_to_glsl() {
    let source = fs::read_to_string("examples/showcase.lane").unwrap();
    let glsl = compile_program(&source).unwrap();

    assert!(glsl.contains("float _op_union(float _a, float _b) {"));
    assert!(glsl.contains("float _op_smooth_xor(float _a, float _b, float _k) {"));
    assert!(glsl.contains("mat3 spin(float _t) {"));
    assert!(glsl.contains("float sdf_output(vec3 p) {"));
    assert!(glsl.contains("vec3 grad_sdf_output(vec3 p) {"));
}

#[test]
fn compiles_example1_orbit_scene_to_glsl() {
    let source = fs::read_to_string("examples/example1.lane").unwrap();
    let glsl = compile_program(&source).unwrap();

    assert!(glsl.contains("struct Isom3"));
    assert!(glsl.contains("Isom3 rot(vec3 binormal, vec3 anchor, float angle)"));
    assert!(glsl.contains("Isom3 r2 = rot(cross(c, p1), c, (time * v2));"));
    assert!(glsl.contains("vec3 p = act_Isom3(r2, p1);"));
    assert!(glsl.contains("float sdf_output(vec3 p_"));
}

#[test]
fn compiles_all_features_samples_to_glsl() {
    let source = fs::read_to_string("examples/all_features.lane").unwrap();
    let glsl = compile_program(&source).unwrap();

    assert!(glsl.contains("struct G"));
    assert!(glsl.contains("G __inv(G value)"));
    assert!(glsl.contains("G __mult(G a, G b)"));
    assert!(!glsl.contains("sdf_output"));

    let source_2d = fs::read_to_string("examples/all_features_2d.lane").unwrap();
    let glsl_2d = compile_program(&source_2d).unwrap();

    assert!(glsl_2d.contains("float sdf_output(vec2"));
    assert!(!glsl_2d.contains("scene_grad("));
}
