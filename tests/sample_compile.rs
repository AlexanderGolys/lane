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
provided func(Float -> Float) rB
provided func(Float -> Vec3) centerA

    func(Float -> Float) hardness = pow2 @ sin + .5
    func(Float -> Vec3) centerB = (1, sin, cos) + centerA / 2

    Object A = Ball3D(r=3) + centerA(time)
    Object B = Ball3D(r=rB(time)) + centerB(time)
    Object C = smoothUnion(hardness(time))(A, B)

generate C
"#;

    let glsl = compile_program(source).unwrap();

    let expected = r#"struct ParamBall3D {
    float r;
};

float sdf0_Ball3D(vec3 p, ParamBall3D params) {
    return length(p) - params.r;
}

float pow2(float x) {
    return x * x;
}

float op_smooth_union_min(float a, float b, float k) {
    k *= 1.0 / (1.0 - sqrt(0.5));
    float h = max(k - abs(a - b), 0.0) / k;
    return min(a, b) - (k * 0.5 * (1.0 + h - sqrt(1.0 - (h * (h - 2.0)))));
}

float op_smooth_union(float a, float b, float k) {
    return op_smooth_union_min(a, b, k);
}

float dsl_hardness(float t) {
    return (pow2(sin(t)) + 0.5);
}

vec3 dsl_centerB(float t) {
    return (vec3(1.0, sin(t), cos(t)) + (centerA(t) / 2.0));
}

float scene_sdf(vec3 p, float time) {
    return op_smooth_union(sdf0_Ball3D((p - centerA(time)), ParamBall3D(3.0)), sdf0_Ball3D((p - dsl_centerB(time)), ParamBall3D(rB(time))), dsl_hardness(time));
}

vec3 scene_grad(vec3 p, float time) {
    float eps = 0.0005;
    return normalize(vec3(((scene_sdf(p + vec3(eps, 0.0, 0.0), time) - scene_sdf(p - vec3(eps, 0.0, 0.0), time)) / (2.0 * eps)), ((scene_sdf(p + vec3(0.0, eps, 0.0), time) - scene_sdf(p - vec3(0.0, eps, 0.0), time)) / (2.0 * eps)), ((scene_sdf(p + vec3(0.0, 0.0, eps), time) - scene_sdf(p - vec3(0.0, 0.0, eps), time)) / (2.0 * eps))));
}"#;

    assert_eq!(glsl, expected);
}

#[test]
fn compiles_showcase_program_to_glsl() {
    let source = fs::read_to_string("showcase.lane").unwrap();
    let glsl = compile_program(&source).unwrap();

    assert!(glsl.contains("float op_union(float a, float b) {"));
    assert!(glsl.contains("float op_smooth_xor(float a, float b, float k) {"));
    assert!(glsl.contains("mat3 dsl_spin(float t) {"));
    assert!(glsl.contains("float scene_sdf(vec3 p, float time, mat3 frame) {"));
    assert!(glsl.contains("vec3 scene_grad(vec3 p, float time, mat3 frame) {"));
}

#[test]
fn compiles_example1_orbit_scene_to_glsl() {
    let source = fs::read_to_string("example1.lane").unwrap();
    let glsl = compile_program(&source).unwrap();

    assert!(glsl.contains("struct E3"));
    assert!(glsl.contains("E3 rot(vec3 binormal, vec3 anchor, float angle)"));
    assert!(glsl.contains("E3 r2 = rot(cross(c, p1), c, (time * v2));"));
    assert!(glsl.contains("vec3 p = act_E3(r2, p1);"));
    assert!(glsl.contains("float scene_sdf(vec3 p_"));
}
