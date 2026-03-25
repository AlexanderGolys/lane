use lane::compile_program;
use std::fs;

#[test]
fn compiles_sample_program_to_glsl() {
    let source = r#"
in: float time
in: func(float -> float) rB
in: func(float -> vec3) centerA

    func(float -> float) hardness = pow2 @ sin + .5
    func(float -> vec3) centerB = (1, sin, cos) + centerA / 2

    Obj3 A = Ball3D(r=3) + centerA(time)
    Obj3 B = Ball3D(r=rB(time)) + centerB(time)
    Obj3 C = SmoothUnion(hardness(time))(A, B)

out: C
"#;

    let glsl = compile_program(source).unwrap();

    let expected = r#"struct ParamBall3D {
    float r;
};

float sdf0_Ball3D(vec3 p, ParamBall3D params) {
    return length(p) - params.r;
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
    float obj_A = sdf0_Ball3D((p - centerA(time)), ParamBall3D(3.0));
    float obj_B = sdf0_Ball3D((p - dsl_centerB(time)), ParamBall3D(rB(time)));
    float obj_C = op_smooth_union(obj_A, obj_B, dsl_hardness(time));
    return obj_C;
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
}
