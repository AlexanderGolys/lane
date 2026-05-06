use lane::{
    compile_program as compile_program_with_float_suffixes, compile_program_from_path,
    known_builtin_object, known_builtin_objects, known_preregistered_objects, known_primitive,
    known_primitives, known_primitives_by_dimension, preregistered_object, Error,
    PreregisteredObjectKind, ShapeDimension,
};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

fn compile_program(source: &str) -> Result<String, Error> {
    compile_program_with_float_suffixes(source).map(|glsl| strip_glsl_float_suffixes(&glsl))
}

fn unique_temp_dir(name: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("lane_{name}_{nanos}"))
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
fn lists_known_primitives_with_lane_types() {
    let primitives = known_primitives();
    let ball = primitives
        .iter()
        .find(|primitive| primitive.name == "Ball3D")
        .unwrap();
    let ball2 = primitives
        .iter()
        .find(|primitive| primitive.name == "Ball2D")
        .unwrap();
    let box3 = primitives
        .iter()
        .find(|primitive| primitive.name == "Box3D")
        .unwrap();
    let segment3 = primitives
        .iter()
        .find(|primitive| primitive.name == "Segment3D")
        .unwrap();
    let plane3 = primitives
        .iter()
        .find(|primitive| primitive.name == "Plane3D")
        .unwrap();
    let line3 = primitives
        .iter()
        .find(|primitive| primitive.name == "Line3D")
        .unwrap();
    let triangle3 = primitives
        .iter()
        .find(|primitive| primitive.name == "Triangle3D")
        .unwrap();
    let polygon = primitives
        .iter()
        .find(|primitive| primitive.name == "Polygon2D")
        .unwrap();
    let quad2 = primitives
        .iter()
        .find(|primitive| primitive.name == "Quad2D")
        .unwrap();
    let quad3 = primitives
        .iter()
        .find(|primitive| primitive.name == "Quad3D")
        .unwrap();

    assert_eq!(ball.dimension, ShapeDimension::D3);
    assert_eq!(ball.parameter_space, "ParamBall3D");
    assert_eq!(ball.fields[0].name, "r");
    assert_eq!(ball.fields[0].domain, "R");
    assert!(ball
        .type_body
        .as_deref()
        .unwrap()
        .contains("struct ParamBall3D"));
    assert!(ball
        .function_body
        .contains("float sdf0_Ball3D(vec3 p, ParamBall3D params)"));

    assert_eq!(ball2.dimension, ShapeDimension::D2);
    assert_eq!(ball2.parameter_space, "ParamBall2D");
    assert_eq!(ball2.fields[0].name, "r");
    assert_eq!(ball2.fields[0].domain, "R");
    assert!(ball2
        .type_body
        .as_deref()
        .unwrap()
        .contains("struct ParamBall2D"));
    assert!(ball2
        .function_body
        .contains("float sdf0_Ball2D(vec2 p, ParamBall2D params)"));

    assert_eq!(box3.dimension, ShapeDimension::D3);
    assert_eq!(box3.parameter_space, "ParamBox3D");
    assert_eq!(box3.fields[0].name, "a");
    assert_eq!(box3.fields[0].domain, "R");
    assert_eq!(box3.fields[1].name, "b");
    assert_eq!(box3.fields[1].domain, "R");
    assert_eq!(box3.fields[2].name, "c");
    assert_eq!(box3.fields[2].domain, "R");
    assert!(box3
        .type_body
        .as_deref()
        .unwrap()
        .contains("struct ParamBox3D"));
    assert!(box3
        .function_body
        .contains("float sdf0_Box3D(vec3 p, ParamBox3D params)"));

    assert_eq!(segment3.dimension, ShapeDimension::D3);
    assert_eq!(segment3.parameter_space, "ParamSegment3D");
    assert_eq!(segment3.fields[0].name, "a");
    assert_eq!(segment3.fields[0].domain, "R3");
    assert_eq!(segment3.fields[1].name, "b");
    assert_eq!(segment3.fields[1].domain, "R3");
    assert!(segment3
        .type_body
        .as_deref()
        .unwrap()
        .contains("struct ParamSegment3D"));
    assert!(segment3
        .function_body
        .contains("float sdf0_Segment3D(vec3 p, ParamSegment3D params)"));

    assert_eq!(plane3.dimension, ShapeDimension::D3);
    assert_eq!(plane3.parameter_space, "ParamPlane3D");
    assert_eq!(plane3.fields[0].name, "n");
    assert_eq!(plane3.fields[0].domain, "R3");
    assert_eq!(plane3.fields[1].name, "origin");
    assert_eq!(plane3.fields[1].domain, "R3");
    assert!(plane3.type_body.as_deref().unwrap().contains("float h;"));

    assert_eq!(line3.dimension, ShapeDimension::D3);
    assert_eq!(line3.parameter_space, "ParamLine3D");
    assert_eq!(line3.fields[0].name, "x0");
    assert_eq!(line3.fields[0].domain, "R3");
    assert_eq!(line3.fields[1].name, "dir");
    assert_eq!(line3.fields[1].domain, "R3");

    assert_eq!(triangle3.dimension, ShapeDimension::D3);
    assert_eq!(triangle3.parameter_space, "ParamTriangle3D");
    assert_eq!(triangle3.fields[0].name, "p1");
    assert_eq!(triangle3.fields[1].name, "p2");
    assert_eq!(triangle3.fields[2].name, "p3");
    assert!(triangle3
        .function_body
        .contains("float sdf0_Triangle3D(vec3 p, ParamTriangle3D params)"));

    assert_eq!(polygon.dimension, ShapeDimension::D2);
    assert_eq!(polygon.parameter_space, "{ points: R2 list }");
    assert_eq!(polygon.fields[0].name, "points");
    assert_eq!(polygon.fields[0].domain, "R2 list");
    assert_eq!(polygon.type_body, None);
    assert!(polygon
        .function_body
        .contains("float sdf0_Polygon2D(vec2 p"));

    assert_eq!(quad2.dimension, ShapeDimension::D2);
    assert_eq!(quad2.parameter_space, "ParamQuad2D");
    assert_eq!(quad2.fields.len(), 4);
    assert!(quad2
        .function_body
        .contains("float sdf0_Quad2D(vec2 p, ParamQuad2D params)"));

    assert_eq!(quad3.dimension, ShapeDimension::D3);
    assert_eq!(quad3.parameter_space, "ParamQuad3D");
    assert_eq!(quad3.fields.len(), 4);
    assert!(quad3
        .function_body
        .contains("float sdf0_Quad3D(vec3 p, ParamQuad3D params)"));
}

#[test]
fn looks_up_known_primitive_by_name() {
    let primitive = known_primitive("Box2D").unwrap();

    assert_eq!(primitive.dimension, ShapeDimension::D2);
    assert_eq!(primitive.parameter_space, "ParamBox2D");
    assert!(primitive
        .type_body
        .as_deref()
        .unwrap()
        .contains("struct ParamBox2D"));
    assert!(primitive.type_body.as_deref().unwrap().contains("float a;"));
    assert!(primitive.type_body.as_deref().unwrap().contains("float b;"));
    assert!(primitive
        .function_body
        .contains("float sdf0_Box2D(vec2 p, ParamBox2D params)"));
}

#[test]
fn looks_up_ball2d_by_name() {
    let primitive = known_primitive("Ball2D").unwrap();

    assert_eq!(primitive.dimension, ShapeDimension::D2);
    assert_eq!(primitive.parameter_space, "ParamBall2D");
    assert!(primitive
        .type_body
        .as_deref()
        .unwrap()
        .contains("struct ParamBall2D"));
    assert!(primitive.type_body.as_deref().unwrap().contains("float r;"));
    assert!(primitive
        .function_body
        .contains("float sdf0_Ball2D(vec2 p, ParamBall2D params)"));
}

#[test]
fn filters_known_primitives_by_dimension() {
    let primitives_2d = known_primitives_by_dimension(ShapeDimension::D2);
    let primitives_3d = known_primitives_by_dimension(ShapeDimension::D3);

    assert!(primitives_2d
        .iter()
        .all(|primitive| primitive.dimension == ShapeDimension::D2));
    assert!(primitives_3d
        .iter()
        .all(|primitive| primitive.dimension == ShapeDimension::D3));
    assert!(primitives_2d
        .iter()
        .all(|primitive| primitive.name.ends_with("2D")));
    assert!(primitives_3d
        .iter()
        .all(|primitive| primitive.name.ends_with("3D")));
    assert!(primitives_2d
        .iter()
        .any(|primitive| primitive.name == "Polygon2D"));
    assert!(primitives_2d
        .iter()
        .any(|primitive| primitive.name == "Ball2D"));
    assert!(primitives_3d
        .iter()
        .any(|primitive| primitive.name == "Ball3D"));
}

#[test]
fn lists_preregistered_functions_and_types() {
    let objects = known_preregistered_objects();

    assert!(objects.iter().any(|object| {
        object.kind == PreregisteredObjectKind::Function && object.name == "sdf0_Ball3D"
    }));
    assert!(objects.iter().any(|object| {
        object.kind == PreregisteredObjectKind::Function && object.name == "_op_smooth_union"
    }));
    assert!(objects.iter().any(|object| {
        object.kind == PreregisteredObjectKind::Function && object.name == "_op_union"
    }));
    assert!(objects.iter().any(|object| {
        object.kind == PreregisteredObjectKind::Function && object.name == "_op_intersection"
    }));
    assert!(objects.iter().any(|object| {
        object.kind == PreregisteredObjectKind::Function && object.name == "_op_difference"
    }));
    assert!(objects.iter().any(|object| {
        object.kind == PreregisteredObjectKind::Function && object.name == "_op_xor"
    }));
    assert!(objects.iter().any(|object| {
        object.kind == PreregisteredObjectKind::Function && object.name == "_op_smooth_intersection"
    }));
    assert!(objects.iter().any(|object| {
        object.kind == PreregisteredObjectKind::Function && object.name == "_op_smooth_difference"
    }));
    assert!(objects.iter().any(|object| {
        object.kind == PreregisteredObjectKind::Function && object.name == "_op_smooth_xor"
    }));
    assert!(objects.iter().any(|object| {
        object.kind == PreregisteredObjectKind::Function && object.name == "pow2"
    }));
    assert!(objects.iter().any(|object| {
        object.kind == PreregisteredObjectKind::Function && object.name == "exp"
    }));
    assert!(!objects.iter().any(|object| {
        object.kind == PreregisteredObjectKind::Function && object.name == "cexp"
    }));
    assert!(objects.iter().any(|object| {
        object.kind == PreregisteredObjectKind::Type && object.name == "ParamBall3D"
    }));
}

#[test]
fn lists_builtin_lane_objects() {
    let objects = known_builtin_objects();

    assert!(objects
        .iter()
        .any(|object| object.name == "DivRing" && object.ty == "Cat"));
    assert!(objects
        .iter()
        .any(|object| object.name == "VectR" && object.ty == "Cat"));
    assert!(objects
        .iter()
        .any(|object| object.name == "Bool" && object.ty == "DivRing"));
    assert!(objects
        .iter()
        .any(|object| object.name == "C" && object.ty == "DivRing, RAlg"));
    assert!(objects
        .iter()
        .any(|object| object.name == "H" && object.ty == "DivRing, RAlg"));
    assert!(objects
        .iter()
        .any(|object| object.name == "Isom2" && object.ty == "Grp"));
    assert!(objects
        .iter()
        .any(|object| object.name == "Isom3" && object.ty == "Grp"));
    assert!(objects
        .iter()
        .any(|object| object.name == "pow2" && object.ty == "Hom(R, R)"));
    assert!(objects.iter().any(|object| {
        object.name == "pow"
            && object
                .ty
                .starts_with("Hom(Z × Mon, Mon) | Hom(Rn × Rn, Rn)")
            && object.ty.contains("Hom(C × C, C)")
    }));
    assert!(objects
        .iter()
        .any(|object| object.name == "not" && object.ty == "Hom(Bool, Bool)"));
    assert!(objects
        .iter()
        .any(|object| object.name == "and" && object.ty == "Hom(Bool × Bool, Bool)"));
    assert!(objects
        .iter()
        .any(|object| object.name == "or" && object.ty == "Hom(Bool × Bool, Bool)"));
    assert!(objects
        .iter()
        .any(|object| object.name == "xor" && object.ty == "Hom(Bool × Bool, Bool)"));
    assert!(!objects.iter().any(|object| object.name == "boolNot"));
    assert!(!objects.iter().any(|object| object.name == "cexp"));
    assert!(objects
        .iter()
        .any(|object| { object.name == "union" && object.ty == "Hom(Object × Object, Object)" }));
    assert!(objects.iter().any(|object| {
        object.name == "smoothUnion" && object.ty == "Hom(R, Hom(Object × Object, Object))"
    }));
    assert!(objects.iter().any(|object| {
        object.name == "revolution" && object.ty == "Hom(R, Hom(Object2D, Object))"
    }));
    assert!(objects
        .iter()
        .any(|object| { object.name == "extrude" && object.ty == "Hom(R, Hom(Object, Object))" }));
    assert!(objects
        .iter()
        .any(|object| { object.name == "rot" && object.ty == "Hom(R3 × R3 × R, Isom3)" }));
    assert!(objects
        .iter()
        .any(|object| { object.name == "rot2D" && object.ty == "Hom(R2 × R, Isom2)" }));
    assert!(objects
        .iter()
        .any(|object| { object.name == "derivative" && object.ty == "Hom(Hom(R, R), Hom(R, R))" }));
    assert!(objects.iter().any(|object| {
        object.name == "gradient" && object.ty == "Hom(Hom(R3, R), Hom(R3, R3))"
    }));
    assert!(objects
        .iter()
        .any(|object| object.name == "dfdx" && object.ty == "Hom(Hom(R3, R), Hom(R3, R))"));
    assert!(objects
        .iter()
        .any(|object| object.name == "dfdy" && object.ty == "Hom(Hom(R3, R), Hom(R3, R))"));
    assert!(objects
        .iter()
        .any(|object| object.name == "dfdz" && object.ty == "Hom(Hom(R3, R), Hom(R3, R))"));
    assert!(objects
        .iter()
        .any(|object| object.name == "dfdw" && object.ty == "Hom(Hom(R4, R), Hom(R4, R))"));
    assert!(!objects.iter().any(|object| object.name == "partialX"));
    assert!(!objects.iter().any(|object| object.name == "partialY"));
    assert!(!objects.iter().any(|object| object.name == "partialZ"));
    assert!(!objects
        .iter()
        .any(|object| object.name == "directionalDerivative"));
    assert!(objects.iter().any(|object| {
        object.name == "divergence" && object.ty == "Hom(Hom(R3, R3), Hom(R3, R))"
    }));
    assert!(objects
        .iter()
        .any(|object| object.name == "sin" && object.ty == "Hom(Rn, Rn) | Hom(C, C)"));
    assert!(objects
        .iter()
        .any(|object| object.name == "clamp" && object.ty.contains("Hom(Rn × R × R, Rn)")));
    assert!(objects
        .iter()
        .any(|object| object.name == "reflect" && object.ty == "Hom(Rn × Rn, Rn)"));
    assert!(objects.iter().any(|object| {
        object.name == "matrixCompMult" && object.ty == "Hom(Mat{n}x{m} × Mat{n}x{m}, Mat{n}x{m})"
    }));
}

#[test]
fn looks_up_builtin_object_detail() {
    let revolution = known_builtin_object("revolution").unwrap();
    let pow2 = known_builtin_object("pow2").unwrap();
    let bool_ty = known_builtin_object("Bool").unwrap();
    let bool_xor = known_builtin_object("xor").unwrap();
    let complex = known_builtin_object("C").unwrap();
    let quat = known_builtin_object("H").unwrap();
    let field = known_builtin_object("DivRing").unwrap();
    let gradient = known_builtin_object("gradient").unwrap();

    assert_eq!(revolution.ty, "Hom(R, Hom(Object2D, Object))");
    assert!(revolution
        .body
        .contains("vec3 _op_revolution_point(vec3 _p, float _offset)"));
    let rot = known_builtin_object("rot").unwrap();
    assert_eq!(rot.ty, "Hom(R3 × R3 × R, Isom3)");
    assert_eq!(rot.body, "");
    assert_eq!(pow2.ty, "Hom(R, R)");
    assert!(pow2.body.contains("float pow2(float x)"));
    let pow = known_builtin_object("pow").unwrap();
    assert!(pow.ty.starts_with("Hom(Z × Mon, Mon) | Hom(Rn × Rn, Rn)"));
    assert!(pow.ty.contains("Hom(C × C, C)"));
    assert_eq!(bool_ty.ty, "DivRing");
    assert_eq!(bool_ty.body, "");
    assert_eq!(bool_xor.ty, "Hom(Bool × Bool, Bool)");
    assert!(bool_xor.body.contains("bool xor(bool a, bool b)"));
    assert_eq!(complex.ty, "DivRing, RAlg");
    assert!(complex.body.contains("#define Complex vec2"));
    assert!(complex.body.contains("vec2 mult_C(vec2 a, vec2 b)"));
    assert_eq!(quat.ty, "DivRing, RAlg");
    assert!(quat.body.contains("#define H vec4"));
    assert!(quat.body.contains("vec4 mult_H(vec4 a, vec4 b)"));
    let e2 = known_builtin_object("Isom2").unwrap();
    assert_eq!(e2.ty, "Grp");
    assert!(e2.body.contains("struct Isom2"));
    assert!(e2.body.contains("Isom2 mult_Isom2(Isom2 a, Isom2 b)"));
    assert!(e2.body.contains("Isom2 inv_Isom2(Isom2 g)"));
    let e3 = known_builtin_object("Isom3").unwrap();
    assert_eq!(e3.ty, "Grp");
    assert!(e3.body.contains("struct Isom3"));
    assert!(e3.body.contains("mat3 A"));
    assert!(e3.body.contains("vec3 t"));
    assert!(e3.body.contains("vec3 act_Isom3(Isom3 g, vec3 p)"));
    assert!(e3.body.contains("Isom3 mult_Isom3(Isom3 a, Isom3 b)"));
    assert!(e3.body.contains("Isom3 inv_Isom3(Isom3 g)"));
    assert!(e3
        .body
        .contains("Isom3 rot(vec3 binormal, vec3 anchor, float angle)"));
    assert_eq!(field.ty, "Cat");
    assert_eq!(field.body, "");
    assert_eq!(gradient.ty, "Hom(Hom(R3, R), Hom(R3, R3))");
    assert_eq!(gradient.body, "");
    let clamp = known_builtin_object("clamp").unwrap();
    assert!(clamp.ty.contains("Hom(Rn × Rn × Rn, Rn)"));
    assert!(clamp.ty.contains("Hom(Rn × R × R, Rn)"));
    assert_eq!(clamp.body, "");
}

#[test]
fn supports_new_type_syntax_aliases() {
    let source = "provided R time\nprovided C z\nprovided Hom(R3, R) density\nprovided End(R) loop\nconst Object output = Ball3D(r=1)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float scene_sdf(vec3 p) {"));
}

#[test]
fn rejects_removed_constraint_type_alias() {
    let source = "provided C(R3) potential\nconst Object output = Ball3D(r=1)\n";
    let error = compile_program(source).unwrap_err().to_string();

    assert!(error.contains("unsupported type 'C(R3)'"));
}

#[test]
fn rejects_legacy_arrow_function_type_syntax() {
    let source = "func(Float -> Float) wobble = sin\nconst Object output = Ball3D(r=1)\n";
    let error = compile_program(source).unwrap_err().to_string();

    assert!(error.contains("unsupported type 'func(Float -> Float)'"));
}

#[test]
fn rejects_lowercase_builtin_type_names() {
    let source = "provided float time\nconst Object output = Ball3D(r=1)\n";
    let error = compile_program(source).unwrap_err().to_string();

    assert!(error.contains("unsupported type 'float'"));
}

#[test]
fn looks_up_preregistered_body_by_name() {
    let param_body = preregistered_object("ParamBall3D").unwrap();
    let sdf_body = preregistered_object("sdf0_Ball3D").unwrap();

    assert_eq!(param_body.kind, PreregisteredObjectKind::Type);
    assert!(param_body.body.contains("struct ParamBall3D"));

    assert_eq!(sdf_body.kind, PreregisteredObjectKind::Function);
    assert!(sdf_body
        .body
        .contains("float sdf0_Ball3D(vec3 p, ParamBall3D params)"));
}

#[test]
fn composes_unary_functions_in_function_bodies() {
    let source =
        "Func(Float, Float) wobble = sin @ sin\nconst Object output = Ball3D(r=wobble(0))\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float wobble(float _t) {"));
    assert!(glsl.contains("return sin(sin(_t));"));
}

#[test]
fn supports_explicit_product_closure_parameters() {
    let source =
        "const Hom(R x R, R) g = (x, y) -> sin(x + y)\nconst Object output = Ball3D(r=g(1, 2))\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float g(float _t0, float _t1) {"));
    assert!(glsl.contains("float _x = _t0;"));
    assert!(glsl.contains("float _y = _t1;"));
    assert!(glsl.contains("return sin((_x + _y));"));
}

#[test]
fn preserves_explicit_scalar_product_function_domains() {
    let source = "const Hom(R x R x R, R) g = v -> v.x + v.x1 + v.z\nprovided R a\nprovided R b\nprovided R c\nconst Object output = Ball3D(r=g(a, b, c))\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float g(float _t0, float _t1, float _t2) {"));
    assert!(glsl.contains("float __lane_product_param_0 = _t0;"));
    assert!(glsl.contains("float __lane_product_param_1 = _t1;"));
    assert!(glsl.contains("float __lane_product_param_2 = _t2;"));
    assert!(glsl.contains(
        "return ((__lane_product_param_0 + __lane_product_param_1) + __lane_product_param_2);"
    ));
    assert!(glsl.contains("g(a, b, c)"));
}

#[test]
fn supports_single_vector_closure_parameter() {
    let source = "const Hom(R2, R) g = v -> v.x + v.y\nconst Object output = Ball3D(r=g((1, 2)))\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float g(vec2 _t) {"));
    assert!(glsl.contains("vec2 _v = _t;"));
    assert!(glsl.contains("return ((_v).x + (_v).y);"));
}

#[test]
fn supports_generic_vector_function_calls_and_composition() {
    let source = "provided Hom(R{n}, R) measure\nprovided Hom(R3, R3) warp\nprovided R3 p\nconst Hom(R3, R) h = measure @ warp\nconst Object output = Ball3D(r=h(p))\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float h(vec3 _t)"));
    assert!(glsl.contains("return measure(warp(_t));"));
}

#[test]
fn supports_generic_matrix_dimensions() {
    let source = "provided Hom(Mat{3}x{2}, R) measure\nprovided Mat{2}x{3} a\nMat3x2 b = transpose(a)\nR radius = measure(b)\nconst Object output = Ball3D(r=radius)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("mat2x3 b = transpose(a);"));
    assert!(glsl.contains("float radius = measure(b);"));
}

#[test]
fn supports_matrix_identity_and_basis_literals() {
    let source = "Mat3 eye = I{3}\nMat3 also_eye = eye{3}\nMat3 axis = E{1}{3}\nMat3 alias = E13\nR3 unit = e{3}{2}\nconst Object output = Ball3D(r=determinant(eye + also_eye + axis + alias) + length(unit))\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("mat3 eye = mat3(1.0);"));
    assert!(glsl.contains("mat3 also_eye = mat3(1.0);"));
    assert!(glsl.contains("mat3 axis = mat3(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0);"));
    assert!(glsl.contains("mat3 alias = mat3(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0);"));
    assert!(glsl.contains("vec3 unit = vec3(0.0, 1.0, 0.0);"));
}

#[test]
fn rejects_bare_i_identity_alias() {
    let error = compile_program("Mat3 m = I\nconst Object output = Ball3D(r=1)\n")
        .unwrap_err()
        .to_string();

    assert!(error.contains("function 'I' needs an explicit call outside function bodies"));
}

#[test]
fn supports_delimited_double_digit_matrix_basis_names() {
    let source = "provided Hom(Mat{12}x{3}, R) measure\nMat12x3 basis = E12_3\nR radius = measure(basis)\nconst Object output = Ball3D(r=radius)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("mat3x12 basis = mat3x12("));
    assert!(glsl.contains("float radius = measure(basis);"));
}

#[test]
fn rejects_inconsistent_generic_matrix_dimensions() {
    let source = "provided Hom(Mat{n}x{n}, R) trace_generic\nprovided Mat2x3 a\nR radius = trace_generic(a)\nconst Object output = Ball3D(r=radius)\n";
    let error = compile_program(source).unwrap_err().to_string();

    assert!(error.contains("no overload of 'trace_generic' matches provided argument(s)"));
}

#[test]
fn rejects_reserved_underscore_names() {
    let error = compile_program("R _x = 1\nconst Object output = Ball3D(r=1)\n")
        .unwrap_err()
        .to_string();

    assert!(error.contains("names cannot start with '_'"));
}

#[test]
fn rejects_reserved_underscore_closure_parameters() {
    let error =
        compile_program("const Hom(R, R) f = _t -> _t\nconst Object output = Ball3D(r=1)\n")
            .unwrap_err()
            .to_string();

    assert!(error.contains("closure parameter names cannot start with '_'"));
}

#[test]
fn emits_glsl_float_literals_with_f_suffixes() {
    let source =
        "R radius = .5\nconst Object output = smoothUnion(0.25)(Ball3D(r=radius), Ball3D(r=1))\n";
    let glsl = compile_program_with_float_suffixes(source).unwrap();

    assert!(glsl.contains("k *= 1.0f / (1.0f - sqrt(0.5f));"));
    assert!(glsl.contains("float radius = 0.5f;"));
    assert!(glsl.contains("ParamBall3D(1.0f)"));
    assert!(!glsl.contains("1.0 /"));
    assert!(!glsl.contains("0.5);"));
}

#[test]
fn supports_derivative_operator_in_function_bodies() {
    let source =
        "Func(Float, Float) slope = derivative(sin)\nconst Object output = Ball3D(r=slope(0))\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float slope(float _t) {"));
    assert!(glsl.contains("(sin((_t + 0.01)) - sin((_t - 0.01))) / (2.0 * 0.01)"));
}

#[test]
fn supports_partial_derivative_aliases() {
    let source = "provided Hom(R3, R) density\nprovided R3 p\nR dx = dfdx(density)(p)\nR dy = dfdy(density)(p)\nR dz = dfdz(density)(p)\nconst Object output = Ball3D(r=dx + dy + dz)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float dx = ((density((p + vec3(0.01, 0.0, 0.0)))"));
    assert!(glsl.contains("float dy = ((density((p + vec3(0.0, 0.01, 0.0)))"));
    assert!(glsl.contains("float dz = ((density((p + vec3(0.0, 0.0, 0.01)))"));
}

#[test]
fn derivative_of_vector_field_returns_matrix() {
    let source = "provided Hom(R2, R3) field\nprovided Hom(Mat2x3, R) measure\nprovided R2 p\nMat2x3 jacobian = derivative(field)(p)\nconst Object output = Ball3D(r=measure(jacobian))\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("mat3x2 jacobian = transpose(mat2x3("));
    assert!(glsl.contains("field((p + vec2(0.01, 0.0)))"));
    assert!(glsl.contains("field((p - vec2(0.0, 0.01)))"));
}

#[test]
fn divergence_accepts_same_dimensional_vector_fields() {
    let source = "provided Hom(R2, R2) flow\nprovided R2 p\nR outflow = divergence(flow)(p)\nconst Object output = Ball3D(r=outflow)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float outflow = (((flow((p + vec2(0.01, 0.0)))).x"));
    assert!(glsl.contains("+ ((flow((p + vec2(0.0, 0.01)))).y"));
    assert!(!glsl.contains(".z -"));
}

#[test]
fn supports_default_gradient_operator_for_scalar_functions() {
    let source = "Func(Float, Float) slope = grad(sin)\nconst Object output = Ball3D(r=slope(0))\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float slope(float _t) {"));
    assert!(glsl.contains("(sin((_t + 0.01)) - sin((_t - 0.01))) / (2.0 * 0.01)"));
}

#[test]
fn supports_default_gradient_operator_for_scalar_fields() {
    let source =
        "provided Hom(R3, R) density\nprovided Hom(R3, R) measure\nprovided R3 p\nR3 normal = gradient(density)(p)\nconst Object output = Ball3D(r=measure(normal))\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("vec3 normal = vec3("));
    assert!(glsl.contains("density((p + vec3(0.01, 0.0, 0.0)))"));
    assert!(glsl.contains("density((p - vec3(0.0, 0.0, 0.01)))"));
}

#[test]
fn emits_support_for_custom_complex_functions() {
    let source = "Complex seed = (1, 0)\nconst Func(Float, C) orbit = exp(seed)\nconst Object output = Ball3D(r=1)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("vec2 exp(vec2 z) {"));
    assert!(glsl.contains("vec2 seed = vec2(1.0, 0.0);"));
    assert!(glsl.contains("return exp(seed);"));
}

#[test]
fn supports_same_domain_function_products() {
    let source = "provided Hom(R2, R) f\nprovided Hom(R2, R) g\nHom(R2, R2) h = (f, g)\nprovided R2 uv\nconst Object output = Ball3D(r=length(h(uv)))\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("vec2 h(vec2 _t)"));
    assert!(glsl.contains("return vec2(f(_t), g(_t));"));
    assert!(glsl.contains("length(h(uv))"));
}

#[test]
fn supports_tensor_function_products() {
    let source = "Hom(R2, R2) h = sin x cos\nprovided R2 uv\nconst Object output = Ball3D(r=length(h(uv)))\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("vec2 h(vec2 _t)"));
    assert!(glsl.contains("return vec2(sin(_t[0]), cos(_t[1]));"));
}

#[test]
fn supports_operator_references_as_value_calls() {
    let source = "provided R x\nprovided R y\nR sum = &+(x, y)\nBool ordered = &<(x, y)\nconst Object output = Ball3D(r=sum + if(ordered) 1 else 0)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float sum = (x + y);"));
    assert!(glsl.contains("bool ordered = (x < y);"));
}

#[test]
fn supports_operator_references_as_function_expressions() {
    let source = "const Hom(R x R, R) h = sin @ &+\nconst Hom(R x R, Bool) ordered = &<\nprovided R x\nprovided R y\nconst Object output = Ball3D(r=h(x, y) + if(ordered(x, y)) 1 else 0)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float h(float _t0, float _t1)"));
    assert!(glsl.contains("return sin((_t0 + _t1));"));
    assert!(glsl.contains("bool ordered(float _t0, float _t1)"));
    assert!(glsl.contains("return (_t0 < _t1);"));
}

#[test]
fn supports_pointwise_function_arithmetic() {
    let source = "provided Hom(R2, R) f\nprovided Hom(R x R, R) g\nHom(R2, R) h = f + g\nprovided R2 uv\nconst Object output = Ball3D(r=h(uv))\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float h(vec2 _t)"));
    assert!(glsl.contains("return (f(_t) + g(_t));"));
}

#[test]
fn supports_pointwise_function_arithmetic_with_value_constants() {
    let source = "provided Hom(R2, R) f\nconst Hom(R2, R) h = f + 1\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float h(vec2 _t)"));
    assert!(glsl.contains("return (f(_t) + 1.0);"));
}

#[test]
fn lifts_value_calls_over_function_arguments() {
    let source =
        "#2D\nconst Object2D rect = Box2D(a=1, b=2)\nconst Hom(R2, R) m = max(rect.sdf, 0.01)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float m(vec2 _t)"));
    assert!(glsl.contains("return max(sdf_rect(_t), 0.01);"));
}

#[test]
fn supports_pointwise_bool_masks_for_vector_functions() {
    let source = "#2D\nprovided R time\nconst Object2D rect = Box2D(a=1, b=2)\nconst Object2D ball = Ball2D(r=1.2)\nconst Object2D scene = union(rect, ball)\nHom(R2, R4) blend = (max(rect.sdf, 0.01) * (0.9, 0.5, 0.5, 1) + max(ball.sdf, 0.01) * (0.5, 0.5, 0.9, 1)) / (max(rect.sdf, 0.01) + max(ball.sdf, 0.01))\nconst Hom(R2, R4) color = blend * (scene.sdf > 0)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("vec4 blend(vec2 _t)"));
    assert!(glsl.contains("max(sdf_rect(_t), 0.01)"));
    assert!(glsl.contains("vec4 color(vec2 _t)"));
    assert!(glsl.contains("return (blend(_t) * ((sdf_scene(_t) > 0.0) ? 1.0 : 0.0));"));
}

#[test]
fn emits_functions_and_object_helpers_in_source_order() {
    let source = "#2D\nconst Object2D rect = Box2D(a=1, b=2)\nconst Object2D scene = rect\nconst Hom(R2, R4) color = if(scene.sdf > 0) (1, 0, 0, 1)\n";
    let glsl = compile_program(source).unwrap();

    let rect_def = glsl.find("float sdf_rect(vec2 p) {").unwrap();
    let sdf_def = glsl.find("float sdf_scene(vec2 p) {").unwrap();
    let color_def = glsl.find("vec4 color(vec2 _t) {").unwrap();
    assert!(rect_def < sdf_def);
    assert!(sdf_def < color_def);
    assert!(glsl.contains("return ((sdf_scene(_t) > 0.0) ? vec4(1.0, 0.0, 0.0, 1.0) : vec4(0.0));"));
}

#[test]
fn typed_declarations_can_reference_inferred_bindings() {
    let source = "#2D\nprovided R time\ncolor1 = (.5, .5, .9, 1)\ncolor2 = (.9, .5, .5, 1)\nconst rect = Box2D(a=1, b=2)\nHom(R, R2) center = (sin * 2, cos * 2)\nconst ball = Ball2D(r=1.2) + center(time)\nblend = (max(rect.sdf, 0.01) * color2 + max(ball.sdf, 0.01) * color1) / (max(rect.sdf, 0.01) + max(ball.sdf, 0.01))\nconst scene = union(rect, ball)\nconst Hom(R2, R4) color = blend * (scene.sdf > 0)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("vec4 blend(vec2 _t)"));
    assert!(glsl.contains("vec4 color(vec2 _t)"));
    assert!(glsl.contains("return (blend(_t) * ((sdf_scene(_t) > 0.0) ? 1.0 : 0.0));"));
}

#[test]
fn imports_local_modules_and_mangles_private_names() {
    let dir = unique_temp_dir("module_private");
    fs::create_dir_all(dir.join("modules")).unwrap();
    fs::write(
        dir.join("modules").join("helpers.lane"),
        "#module\nR secret = 2\nconst R exported = secret + 1\n",
    )
    .unwrap();
    let source_path = dir.join("scene.lane");
    fs::write(
        &source_path,
        "#import helpers\nR secret = 5\nconst Object output = Ball3D(r=exported + secret)\n",
    )
    .unwrap();

    let glsl = strip_glsl_float_suffixes(&compile_program_from_path(&source_path).unwrap());

    assert!(glsl.contains("__lane_mod_helpers_secret"));
    assert!(glsl.contains("float secret = 5.0;"));
    assert!(glsl.contains("float exported = (__lane_mod_helpers_secret + 1.0);"));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn imports_module_raw_glsl_functions() {
    let dir = unique_temp_dir("module_raw");
    fs::create_dir_all(dir.join("modules")).unwrap();
    fs::write(
        dir.join("modules").join("raw.lane"),
        "#module\nconst Hom(R, R) twice = \"float twice(float x) { return x * 2.0; }\"\n",
    )
    .unwrap();
    let source_path = dir.join("scene.lane");
    fs::write(
        &source_path,
        "#import raw\nconst Object output = Ball3D(r=twice(2))\n",
    )
    .unwrap();

    let glsl = strip_glsl_float_suffixes(&compile_program_from_path(&source_path).unwrap());

    assert!(glsl.contains("float twice(float x) { return x * 2.0; }"));
    assert!(glsl.contains("ParamBall3D(twice(2.0))"));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn raw_glsl_placeholders_resolve_private_helpers_and_locals() {
    let dir = unique_temp_dir("module_raw_placeholders");
    fs::create_dir_all(dir.join("modules")).unwrap();
    fs::write(
        dir.join("modules").join("raw.lane"),
        "#module\nHom(R, R) helper = sin\nconst Hom(R, R) wrapped = \"float wrapped(float x) { float _value = ${helper}(x); return _value; }\"\n",
    )
    .unwrap();
    let source_path = dir.join("scene.lane");
    fs::write(
        &source_path,
        "#import raw\nconst Object output = Ball3D(r=wrapped(1))\n",
    )
    .unwrap();

    let glsl = strip_glsl_float_suffixes(&compile_program_from_path(&source_path).unwrap());

    assert!(glsl.contains("float __lane_mod_raw_helper(float _t)"));
    assert!(glsl.contains("__lane_mod_raw_helper(x)"));
    assert!(glsl.contains("float _value = __lane_mod_raw_helper(x);"));
    assert!(!glsl.contains("${"));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn rejects_raw_glsl_functions_outside_modules() {
    let source = "const Hom(R, R) twice = \"return _t * 2.0;\"\n";
    let error = compile_program(source).unwrap_err().to_string();

    assert!(error.contains("raw GLSL function bodies are only valid in modules"));
}

#[test]
fn rejects_module_import_cycles() {
    let dir = unique_temp_dir("module_cycle");
    fs::create_dir_all(dir.join("modules")).unwrap();
    fs::write(dir.join("modules").join("a.lane"), "#module\n#import b\n").unwrap();
    fs::write(dir.join("modules").join("b.lane"), "#module\n#import a\n").unwrap();
    let source_path = dir.join("scene.lane");
    fs::write(
        &source_path,
        "#import a\nconst Object output = Ball3D(r=1)\n",
    )
    .unwrap();

    let error = compile_program_from_path(&source_path)
        .unwrap_err()
        .to_string();

    assert!(error.contains("module import cycle"));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn imports_raytracing_module() {
    let source = "#import raytracing\nprovided R3 cameraPosition\nprovided R3 cameraForward\nprovided R3 cameraGlobalUp\nprovided R2 resolution\nprovided R3 ambientColor\nconst Camera camera = Camera(cameraPosition, cameraForward, cameraGlobalUp, resolution)\nconst Object scene = Ball3D(r=1)\nconst Material material = Material((0.8, 0.6, 0.4), (0, 0, 0), 0.2)\nconst Hom(R3, Material) scene_material = (x, y, z) -> material\nconst Hom(R2, Ray) scene_camera_ray = camera_ray(camera)\nconst Hom(Ray, Hit) scene_raytrace = raytrace_with(default_raytrace_config, scene)\nconst Hom(Hit, R3) scene_material_color = hit -> material_color(scene_material(hit.position))\nconst Hom(Hit, R3) scene_material_emission = hit -> material_emission(scene_material(hit.position))\nconst Hom(Hit, R) scene_material_reflectiveness = hit -> material_reflectiveness(scene_material(hit.position))\nconst Hom(Ray, R3) scene_raycolor = raycolor_from_hit_with(default_raycolor_config, ambientColor, scene_raytrace, scene_material_color, scene_material_emission, scene_material_reflectiveness)\nconst Hom(R2, R4) scene_shade = shade(scene_camera_ray, scene_raycolor)\nconst Hom(*, *) main = fragment_main(scene_shade)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("struct Ray"));
    assert!(glsl.contains("struct Hit"));
    assert!(glsl.contains("struct Material"));
    assert!(glsl.contains("struct Camera"));
    assert!(glsl.contains("vec2 camera_uv(Camera _t0, vec2 _t1)"));
    assert!(glsl.contains("vec3 camera_right(Camera _t)"));
    assert!(glsl.contains("vec3 camera_up(Camera _t)"));
    assert!(!glsl.contains("Ray camera_ray(vec2 fragCoord)"));
    assert!(glsl.contains("Ray scene_camera_ray(vec2 _t)"));
    assert!(glsl.contains("vec2 _v = _t;"));
    assert!(glsl.contains("normalize(cross((_camera).forward, (_camera).global_up))"));
    assert!(glsl.contains(
        "normalize(cross(cross((_camera).forward, (_camera).global_up), (_camera).forward))"
    ));
    assert!(glsl.contains("return Ray((camera).position, normalize("));
    assert!(glsl.contains("camera_uv(camera, _v)"));
    assert!(glsl.contains("camera_right(camera)"));
    assert!(glsl.contains("camera_up(camera)"));
    assert!(!glsl.contains("cross(normalize((camera).forward), (camera).global_up)"));
    assert!(!glsl.contains("cross(normalize(cross("));
    assert!(!glsl.contains("Hit raytrace(Ray ray)"));
    assert!(glsl.contains("Hit scene_raytrace(Ray _t)"));
    assert!(glsl.contains("float _d = sdf_scene(_p);"));
    assert!(glsl.contains("vec3 _n = grad_sdf_scene(_p);"));
    assert!(glsl.contains("default_raytrace_config.max_steps"));
    assert!(glsl.contains("default_raytrace_config.hit_threshold"));
    assert!(glsl.contains("default_raytrace_config.max_travel"));
    assert!(!glsl.contains("vec3 raycolor(Ray initialRay)"));
    assert!(glsl.contains("vec3 scene_raycolor(Ray _t)"));
    assert!(glsl.contains("default_raycolor_config.max_bounces"));
    assert!(glsl.contains("default_raycolor_config.throughput_threshold"));
    assert!(glsl.contains("default_raycolor_config.ray_bias"));
    assert!(glsl.contains("vec3 _surface_color = scene_material_color(_hit);"));
    assert!(glsl.contains("vec3 _surface_emission = scene_material_emission(_hit);"));
    assert!(glsl
        .contains("float _reflectiveness = clamp(scene_material_reflectiveness(_hit), 0.0, 1.0);"));
    assert!(!glsl.contains("vec4 shade(vec2 fragCoord)"));
    assert!(glsl.contains("return vec4(scene_raycolor(scene_camera_ray(_v)), 1.0"));
    assert!(glsl.contains("vec4 scene_shade(vec2 _t)"));
    assert!(glsl.contains("void main()"));
    assert!(glsl.contains("outColor = scene_shade(gl_FragCoord.xy);"));
    assert!(glsl.contains("_radiance += _throughput * (_surface_emission"));
    assert!(glsl.contains("_throughput *= _surface_color * _reflectiveness;"));
    assert!(glsl.contains("vec3 _dir = reflect(_ray.dir, _hit.normal);"));
}

#[test]
fn raytracing_raycolor_accepts_custom_material_types() {
    let source = "#import raytracing\nconst VectR FancyMaterial = R3 x R3 x R x R <albedo, glow, roughness, metallic>\nprovided Hom(Ray, Hit) hit\nprovided Hom(R3, FancyMaterial) material\nprovided R3 ambient\nconst Hom(FancyMaterial, R3) color = m -> m.albedo\nconst Hom(FancyMaterial, R3) emission = m -> m.glow\nconst Hom(FancyMaterial, R) reflectiveness = m -> m.metallic\nconst Hom(Hit, R3) color_at = color @ material @ hit_position\nconst Hom(Hit, R3) emission_at = emission @ material @ hit_position\nconst Hom(Hit, R) reflectiveness_at = reflectiveness @ material @ hit_position\nconst Hom(Ray, R3) color_ray = raycolor_from_hit_with(default_raycolor_config, ambient, hit, color_at, emission_at, reflectiveness_at)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("struct FancyMaterial"));
    assert!(glsl.contains("vec3 color_ray(Ray _t)"));
    assert!(glsl.contains("vec3 _surface_color = color_at(_hit);"));
    assert!(glsl.contains("vec3 _surface_emission = emission_at(_hit);"));
    assert!(glsl.contains("float _reflectiveness = clamp(reflectiveness_at(_hit), 0.0, 1.0);"));
    assert!(!glsl.contains("_material.reflectiveness"));
}

#[test]
fn imports_std_module() {
    let source = "#import std\nprovided R2 uv\nR radius = projection_1(uv) + diagonal3(1).z\nconst Object output = Ball3D(r=radius)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float projection_1(vec2 _t)"));
    assert!(glsl.contains("return (_v).y;"));
    assert!(glsl.contains("vec3 diagonal3(float _t)"));
    assert!(glsl.contains("return vec3(_x, _x, _x);"));
}

#[test]
fn raw_glsl_unit_templates_require_main_binding() {
    let dir = unique_temp_dir("module_raw_main");
    fs::create_dir_all(dir.join("modules")).unwrap();
    fs::write(
        dir.join("modules").join("preview.lane"),
        "#module\nconst Hom(Hom(R2, R4), Hom(*, *)) preview = shade -> \"outColor = ${shade}(gl_FragCoord.xy);\"\n",
    )
    .unwrap();
    let source_path = dir.join("scene.lane");
    fs::write(
        &source_path,
        "#import preview\nprovided Hom(R2, R4) shade\nconst Hom(*, *) entry = preview(shade)\n",
    )
    .unwrap();

    let error = compile_program_from_path(&source_path)
        .unwrap_err()
        .to_string();

    assert!(error.contains("shader entry function 'entry' must be named 'main'"));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn raw_glsl_placeholders_accept_glsl_nameable_builtins() {
    let dir = unique_temp_dir("module_raw_builtin_placeholder");
    fs::create_dir_all(dir.join("modules")).unwrap();
    fs::write(
        dir.join("modules").join("raw.lane"),
        "#module\nconst Hom(R, R) wave = \"float wave(float x) { return ${sin}(x); }\"\n",
    )
    .unwrap();
    let source_path = dir.join("scene.lane");
    fs::write(
        &source_path,
        "#import raw\nconst Object output = Ball3D(r=wave(1))\n",
    )
    .unwrap();

    let glsl = strip_glsl_float_suffixes(&compile_program_from_path(&source_path).unwrap());

    assert!(glsl.contains("return sin(x);"));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn raw_glsl_placeholders_reject_non_nameable_function_operators() {
    let dir = unique_temp_dir("module_raw_bad_placeholder");
    fs::create_dir_all(dir.join("modules")).unwrap();
    fs::write(
        dir.join("modules").join("raw.lane"),
        "#module\nconst Hom(R, R) bad = \"float bad(float x) { return ${gradient}(x); }\"\n",
    )
    .unwrap();
    let source_path = dir.join("scene.lane");
    fs::write(
        &source_path,
        "#import raw\nconst Object output = Ball3D(r=1)\n",
    )
    .unwrap();

    let error = compile_program_from_path(&source_path)
        .unwrap_err()
        .to_string();

    assert!(error.contains("cannot be rendered as a GLSL reference"));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn module_can_provide_its_own_product_type() {
    let dir = unique_temp_dir("module_provided_product");
    fs::create_dir_all(dir.join("modules")).unwrap();
    fs::write(
        dir.join("modules").join("materials.lane"),
        "#module\nprovided VectR Material = R3 x R3 x R <color, emission, reflectiveness>\n",
    )
    .unwrap();
    let source_path = dir.join("scene.lane");
    fs::write(
        &source_path,
        "#import materials\nprovided Material material\nconst Hom(R, Material) copiedMaterial = material\nconst Object output = Ball3D(r=1)\n",
    )
    .unwrap();
    let glsl = compile_program_from_path(&source_path).unwrap();

    assert!(!glsl.contains("struct Material"));
    assert!(glsl.contains("Material copiedMaterial(float _t)"));
    assert!(glsl.contains("return material;"));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn provided_product_type_is_declared_but_not_emitted() {
    let source = "provided VectR External = R3 x R <color, weight>\nprovided External external\nconst Hom(R, External) copied = external\n";
    let glsl = compile_program(source).unwrap();

    assert!(!glsl.contains("struct External"));
    assert!(glsl.contains("External copied(float _t)"));
    assert!(glsl.contains("return external;"));
}

#[test]
fn supports_pointwise_function_arithmetic_support_dependencies() {
    let source = "provided Hom(R, C) f\nprovided Hom(R, C) g\nHom(R, C) h = f * g\nconst Object output = Ball3D(r=length(h(1)))\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("vec2 mult_C(vec2 a, vec2 b)"));
    assert!(glsl.contains("vec2 h(float _t)"));
    assert!(glsl.contains("return mult_C(f(_t), g(_t));"));
}

#[test]
fn supports_complex_pow_overload() {
    let source = "provided C z\nprovided C w\nconst C y = pow(z, w)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("vec2 mult_C(vec2 a, vec2 b)"));
    assert!(glsl.contains("vec2 exp(vec2 z)"));
    assert!(glsl.contains("vec2 log(vec2 z)"));
    assert!(glsl.contains("vec2 pow(vec2 z, vec2 w)"));
    assert!(glsl.contains("return exp(mult_C(w, log(z)));"));
}

#[test]
fn supports_monoid_pow_for_product_types() {
    let source = "Mon Pair = R x Z\nprovided Pair p\nconst Pair q = pow(3, p)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("Pair one_Pair = Pair(1.0, 1);"));
    assert!(glsl.contains("Pair mult_Pair(Pair a, Pair b)"));
    assert!(glsl.contains("Pair pow_monoid_Pair(int exponent, Pair value)"));
    assert!(glsl.contains("result = mult_Pair(result, factor);"));
}

#[test]
fn supports_monoid_pow_for_provided_category_types() {
    let source = "provided Grp G\nprovided G g\nprovided Hom(G, R) measure\nR radius = measure(pow(4, g))\nconst Object output = Ball3D(r=radius)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("G pow_monoid_G(int exponent, G value)"));
    assert!(glsl.contains("G result = e_G;"));
    assert!(glsl.contains("result = mult_G(result, factor);"));
    assert!(glsl.contains("float radius = measure(pow_monoid_G(4, g));"));
}

#[test]
fn emits_glsl_builtin_scalar_vector_and_geometric_calls() {
    let source = "provided R3 v\nR3 n = normalize(v)\nR len = length(n)\nR angle = atan(len, 1)\nR3 bounced = reflect(v, n)\nR3 blended = mix(clamp(cross(n, bounced), -1, 1), refract(v, n, 0.5), 0.25)\nconst Object output = Ball3D(r=len + distance(blended, n) + dot(blended, n) + angle)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("vec3 n = normalize(v);"));
    assert!(glsl.contains("float len = length(n);"));
    assert!(glsl.contains("float angle = atan(len, 1.0);"));
    assert!(glsl.contains("vec3 bounced = reflect(v, n);"));
    assert!(glsl.contains(
        "vec3 blended = mix(clamp(cross(n, bounced), (-1.0), 1.0), refract(v, n, 0.5), 0.25);"
    ));
    assert!(glsl.contains("distance(blended, n)"));
    assert!(glsl.contains("dot(blended, n)"));
}

#[test]
fn emits_glsl_builtin_matrix_calls() {
    let source = "provided Mat3 frame\nMat3 m = matrixCompMult(frame, inverse(transpose(frame)))\nR d = determinant(m)\nconst Object output = Ball3D(r=d)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("mat3 m = matrixCompMult(frame, inverse(transpose(frame)));"));
    assert!(glsl.contains("float d = determinant(m);"));
}

#[test]
fn emits_scalar_first_min_max_as_valid_glsl_vector_calls() {
    let source = "provided R edge\nprovided R2 uv\nR2 hi = max(edge, uv)\nR2 lo = min(edge, uv)\nconst Object output = Ball3D(r=length(hi) + length(lo))\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("vec2 hi = max(uv, edge);"));
    assert!(glsl.contains("vec2 lo = min(uv, edge);"));
}

#[test]
fn lowers_complex_ring_operations_through_category_helpers() {
    let source = "provided C z\nC product = z * z\nC shifted = 1 - (z / z)\nconst Object output = Ball3D(r=1)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("vec2 mult_C(vec2 a, vec2 b)"));
    assert!(glsl.contains("vec2 div_C(vec2 a, vec2 b)"));
}

#[test]
fn lowers_quaternion_field_operations_through_category_helpers() {
    let source = "provided H p\nprovided H q\nH product = p * q\nH ratio = 1 / q\nH literal = (1, 0, 0, 0)\nconst Object output = Ball3D(r=1)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("vec4 mult_H(vec4 a, vec4 b)"));
    assert!(glsl.contains("vec4 div_H(vec4 a, vec4 b)"));
}

#[test]
fn lowers_e2_group_operations_through_category_helpers() {
    let source = "provided Isom2 a\nprovided Isom2 b\nprovided R2 p\nprovided Hom(R2, R) measure\nIsom2 product = a * b\nR2 moved = product * p\nR radius = measure(moved)\nconst Object output = Ball3D(r=radius)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("struct Isom2"));
    assert!(glsl.contains("Isom2 mult_Isom2(Isom2 a, Isom2 b)"));
    assert!(!glsl.contains("Isom2 div_Isom2(Isom2 a, Isom2 b)"));
    assert!(glsl.contains("Isom2 product = mult_Isom2(a, b);"));
    assert!(glsl.contains("vec2 moved = act_Isom2(product, p);"));
}

#[test]
fn supports_provided_group_category_types() {
    let source = "provided Grp G\nprovided G a\nprovided G b\nprovided Hom(G, R) measure\nR radius = measure(a * b)\nconst Object output = Ball3D(r=radius)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float scene_sdf(vec3 p) {"));
    assert!(glsl.contains("float radius = measure(mult_G(a, b));"));
}

#[test]
fn supports_product_group_types_with_named_fields() {
    let source = "Grp G = Isom3 x Isom2 <m, n>\nprovided G a\nprovided G b\nprovided Hom(G, R) measure\nR radius = measure(a * b)\nconst Object output = Ball3D(r=radius)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("struct G {\n    Isom3 m;\n    Isom2 n;\n};"));
    assert!(glsl.contains(
        "G mult_G(G a, G b) {\n    return G(mult_Isom3(a.m, b.m), mult_Isom2(a.n, b.n));\n}"
    ));
    assert!(glsl.contains("float radius = measure(mult_G(a, b));"));
}

#[test]
fn supports_product_value_constructors_with_default_fields() {
    let source = "Ab Pair = R2 x R3\nPair p = Pair((1, 2), 0)\nprovided Hom(Pair, R) measure\nR radius = measure(p + p)\nconst Object output = Ball3D(r=radius)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("struct Pair {\n    vec2 x;\n    vec3 y;\n};"));
    assert!(glsl.contains("Pair p = Pair(vec2(1.0, 2.0), vec3(0.0));"));
    assert!(glsl.contains(
        "Pair add_Pair(Pair a, Pair b) {\n    return Pair((a.x + b.x), (a.y + b.y));\n}"
    ));
}

#[test]
fn supports_positional_field_aliases_for_default_product_fields() {
    let source = "Set Four = R x R x R x R\nSet Five = C x R x R x R x R\nFour a = Four(1, 2, 3, 4)\nFive b = Five((5, 6), 7, 8, 9, 10)\nR radius = a.x0 + a.x + a.x3 + a.w + b.x.x + b.x0.x + b.y + b.x1 + b.w + b.x3\nconst Object output = Ball3D(r=radius)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float radius = ((((((((((a).x + (a).x) + (a).w) + (a).w) + ((b).x0).x) + ((b).x0).x) + (b).x1) + (b).x1) + (b).x3) + (b).x3);"));
}

#[test]
fn supports_numeric_field_aliases_for_vectors() {
    let source = "provided R4 v\nR radius = v.x0 + v.x + v.x3 + v.w\nconst Object output = Ball3D(r=radius)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float radius = ((((v).x + (v).x) + (v).w) + (v).w);"));
}

#[test]
fn supports_product_set_types_without_operations() {
    let source = "Set Pair = R x R3\nPair p = Pair(1, (0, 0, 0))\nprovided Hom(Pair, R) measure\nR radius = measure(p)\nconst Object output = Ball3D(r=radius)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("struct Pair {\n    float x;\n    vec3 y;\n};"));
    assert!(!glsl.contains("add_Pair"));
    assert!(!glsl.contains("mult_Pair"));
}

#[test]
fn emits_all_product_ops_for_const_product_types() {
    let source = "const Grp G = Isom3 x Isom2\nprovided G a\nprovided Hom(G, R) measure\nR radius = measure(a)\nconst Object output = Ball3D(r=radius)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("G e_G = G(Isom3(mat3(1.0), vec3(0.0)), Isom2(mat2(1.0), vec2(0.0)));"));
    assert!(glsl.contains("G mult_G(G a, G b)"));
    assert!(glsl.contains("G inv_G(G value)"));
}

#[test]
fn rejects_product_field_count_mismatches() {
    let source = "Grp G = Isom3 x Isom2 <m>\nconst Object output = Ball3D(r=1)\n";
    let err = compile_program(source).unwrap_err().to_string();

    assert!(err.contains("product type 'G' has 2 component(s) but 1 field name(s)"));
}

#[test]
fn rejects_duplicate_product_field_names() {
    let source = "Grp G = Isom3 x Isom2 <m, m>\nconst Object output = Ball3D(r=1)\n";
    let err = compile_program(source).unwrap_err().to_string();

    assert!(err.contains("product type 'G' has duplicate field name 'm'"));
}

#[test]
fn rejects_reserved_glsl_product_field_names() {
    let source = "Set Hit = R3 x R <position, distance>\nconst Object output = Ball3D(r=1)\n";
    let err = compile_program(source).unwrap_err().to_string();

    assert!(err.contains("product type 'Hit' field name 'distance' is reserved in GLSL"));
}

#[test]
fn rejects_product_field_types() {
    let source = "DivRing G = C x H\nconst Object output = Ball3D(r=1)\n";
    let err = compile_program(source).unwrap_err().to_string();

    assert!(err.contains("product type 'G' cannot be declared as DivRing"));
}

#[test]
fn rejects_product_components_outside_category() {
    let source = "Grp G = Isom3 x R3\nconst Object output = Ball3D(r=1)\n";
    let err = compile_program(source).unwrap_err().to_string();

    assert!(err.contains("product type 'G' component R3 does not satisfy Grp"));
}

#[test]
fn rejects_division_for_group_category_types() {
    let source = "provided Grp G\nprovided G a\nprovided G b\nprovided Hom(G, R) measure\nR radius = measure(a / b)\nconst Object output = Ball3D(r=radius)\n";
    let err = compile_program(source).unwrap_err().to_string();

    assert!(err.contains("unsupported operands for binary operator: G / G"));
}

#[test]
fn supports_provided_vector_space_category_types() {
    let source = "provided VectR V\nprovided V v\nprovided Hom(V, R) norm\nR radius = norm(2 * (v / 3))\nconst Object output = Ball3D(r=radius)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float scene_sdf(vec3 p) {"));
    assert!(glsl.contains("float radius = norm(scale_V(scale_V(v, (1.0 / 3.0)), 2.0));"));
}

#[test]
fn infers_local_value_and_object_binding_types() {
    let source = "provided R time\nradius = 1 + time\noffset = (1, 0, 0)\nshape = Ball3D(r=radius) + offset\nconst Object output = shape\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float radius = (1.0 + time);"));
    assert!(glsl.contains("vec3 offset = vec3(1.0, 0.0, 0.0);"));
    assert!(glsl.contains("return sdf0_Ball3D((p - offset), ParamBall3D(radius));"));
}

#[test]
fn casts_neutral_literals_to_expected_builtin_types() {
    let source =
        "R3 p = 0\nMat3 m = e\nIsom3 g = Isom3(e, 0)\nconst Object output = g * Ball3D(r=1) + p\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("vec3 p = vec3(0.0);"));
    assert!(glsl.contains("mat3 m = mat3(1.0);"));
    assert!(glsl.contains("Isom3 g = Isom3(mat3(1.0), vec3(0.0));"));
}

#[test]
fn supports_bool_field_values_and_builtins() {
    let source = "provided Hom(Bool, R) choose\nconst Bool sum = true + false\nconst Bool product = sum * not(false)\nconst Bool either = or(product, false)\nconst R radius = choose(either)\nconst Object output = Ball3D(r=radius)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("bool not(bool x)"));
    assert!(glsl.contains("bool or(bool a, bool b)"));
    assert!(glsl.contains("const bool sum = (true != false);"));
    assert!(glsl.contains("bool product = (sum && not(false));"));
    assert!(glsl.contains("bool either = or(product, false);"));
    assert!(glsl.contains("float radius = choose(either);"));
}

#[test]
fn casts_bool_values_to_expected_numeric_types() {
    let source = "provided Bool flag\nprovided Hom(R, Bool) pred\nprovided Hom(Z, R) measure\nprovided Z count\nconst R literal = true\nR variable = flag\nR call = pred(0)\nR mixed = 2 + flag\nZ total = count + pred(1)\nR radius = literal + variable + call + mixed + measure(total)\nconst Object output = Ball3D(r=radius)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("const float literal = (true ? 1.0 : 0.0);"));
    assert!(glsl.contains("float variable = (flag ? 1.0 : 0.0);"));
    assert!(glsl.contains("float call = (pred(0.0) ? 1.0 : 0.0);"));
    assert!(glsl.contains("float mixed = (2.0 + (flag ? 1.0 : 0.0));"));
    assert!(glsl.contains("int total = (count + (pred(1.0) ? 1 : 0));"));
    assert!(glsl.contains("measure(total)"));
}

#[test]
fn supports_conditional_value_expressions() {
    let source = "provided Bool flag\nconst R a = if(flag) 2 else 3\nconst R b = if(flag) 2\nconst Object output = Ball3D(r=a + b)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float a = (flag ? 2.0 : 3.0);"));
    assert!(glsl.contains("float b = (flag ? 2.0 : 0.0);"));
    assert!(glsl.contains("ParamBall3D((a + b))"));
}

#[test]
fn supports_product_conditional_values_without_glsl_ternaries() {
    let source = "const VectR Swatch = R3 x R <color, weight>\nprovided Bool flag\nconst Swatch warm = Swatch((1, 0, 0), 1)\nconst Swatch cool = Swatch((0, 0, 1), 2)\nconst Swatch selected = if(flag) warm else cool\nconst Object output = Ball3D(r=selected.weight)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains(
        "Swatch conditional_swatch(bool condition, Swatch then_value, Swatch else_value)"
    ));
    assert!(glsl.contains("Swatch selected = conditional_swatch(flag, warm, cool);"));
    assert!(!glsl.contains("? warm : cool"));
}

#[test]
fn supports_pointwise_conditional_functions() {
    let source = "#2D\nconst Object2D shape = Box2D(a=1, b=2)\nconst Hom(R2, R) clipped = if(shape.sdf > 0) shape.sdf\nconst Hom(R2, R4) color = if(shape.sdf > 0) (1, 0, 0, 1) else (0, 0, 1, 1)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float clipped(vec2 _t)"));
    assert!(glsl.contains("return ((sdf_shape(_t) > 0.0) ? sdf_shape(_t) : 0.0);"));
    assert!(glsl.contains("vec4 color(vec2 _t)"));
    assert!(glsl.contains(
        "return ((sdf_shape(_t) > 0.0) ? vec4(1.0, 0.0, 0.0, 1.0) : vec4(0.0, 0.0, 1.0, 1.0));"
    ));
}

#[test]
fn supports_comparison_operators() {
    let source = "provided Hom(Bool, R) choose\nprovided R time\nprovided Z count\nconst Bool scalar_order = 1 + time <= 3\nconst Bool int_order = count < 4\nconst Bool bool_equal = true == false\nconst Bool int_not_equal = count != 0\nconst R radius = choose(or(and(scalar_order, int_order), and(bool_equal, int_not_equal)))\nconst Object output = Ball3D(r=radius)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("bool scalar_order = ((1.0 + time) <= 3.0);"));
    assert!(glsl.contains("bool int_order = (count < 4);"));
    assert!(glsl.contains("const bool bool_equal = (true == false);"));
    assert!(glsl.contains("bool int_not_equal = (count != 0);"));
    assert!(glsl.contains(
        "float radius = choose(or(and(scalar_order, int_order), and(bool_equal, int_not_equal)));"
    ));
}

#[test]
fn rejects_ordering_non_ordered_values() {
    let source = "const Bool bad = true < false\nconst Object output = Ball3D(r=1)\n";
    let err = compile_program(source).unwrap_err().to_string();

    assert!(err.contains("unsupported operands for binary operator: Bool < Bool"));
}

#[test]
fn casts_bool_zero_and_one_neutral_literals() {
    let source = "const Bool zero = 0\nconst Bool one = 1\nconst Object output = Ball3D(r=1)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("const bool zero = false;"));
    assert!(glsl.contains("const bool one = true;"));
}

#[test]
fn casts_neutral_literals_to_provided_category_constants() {
    let source = "provided Grp G\nprovided Hom(G, R) measure\nG g = e\nR radius = measure(g)\nconst Object output = Ball3D(r=radius)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("G g = e_G;"));
}

#[test]
fn resolves_overloaded_calls_with_exact_numeric_match_before_neutral_casts() {
    let source = "provided Hom(C, C) f\nprovided Hom(C, R) norm\nR radius = sin(0)\nC z = f(0)\nconst Object output = Ball3D(r=radius + norm(z))\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float radius = sin(0.0);"));
    assert!(glsl.contains("vec2 z = f(vec2(0.0, 0.0));"));
}

#[test]
fn rejects_ambiguous_overloaded_neutral_casts() {
    let source =
        "provided Hom(C, C) f\nprovided Hom(H, H) f\na = f(0)\nconst Object output = Ball3D(r=1)\n";
    let err = compile_program(source).unwrap_err().to_string();

    assert!(err.contains("ambiguous overload for 'f'"));
}

#[test]
fn rejects_duplicate_overloads_with_same_domain() {
    let source = "provided Hom(C, C) f\nprovided Hom(C, R) f\nconst Object output = Ball3D(r=1)\n";
    let err = compile_program(source).unwrap_err().to_string();

    assert!(err.contains("duplicate overload for 'f' with domain C"));
}

#[test]
fn rejects_category_names_as_value_binding_types() {
    let source = "Grp g = g\nconst Object output = Ball3D(r=1)\n";
    let err = compile_program(source).unwrap_err();

    assert_eq!(
        err.to_string(),
        "line 1: category 'Grp' cannot be used as a type"
    );
}

#[test]
fn rejects_category_names_as_provided_type_names() {
    let source = "provided Grp DivRing\nconst Object output = Ball3D(r=1)\n";
    let err = compile_program(source).unwrap_err();

    assert_eq!(
        err.to_string(),
        "line 1: 'DivRing' cannot be used as a provided type name"
    );
}

#[test]
fn allows_vector_space_scaling_by_category() {
    let source =
        "provided R3 p\nR3 scaled = 2 * (p / 3)\nconst Object output = Ball3D(r=1) + scaled\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("vec3 scaled = (2.0 * (p / 3.0));"));
}

#[test]
fn rejects_invalid_function_composition() {
    let source = "provided Func(Float, Vec3) center\nFunc(Float, Float) wobble = pow2 @ center\nconst Object output = Ball3D(r=1)\n";
    let error = compile_program(source).unwrap_err().to_string();

    assert!(error.contains("cannot compose pow2 @ center"));
}

#[test]
fn supports_const_output_declaration() {
    let source = "const Object output = Ball3D(r=1)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float sdf_output(vec3 p) {"));
    assert!(glsl.contains("vec3 grad_sdf_output(vec3 p) {"));
    assert!(glsl.contains("float scene_sdf(vec3 p) {"));
}

#[test]
fn supports_multiple_const_object_declarations() {
    let source = "const Object a = Ball3D(r=1)\nconst Object b = Ball3D(r=2)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float sdf_a("));
    assert!(glsl.contains("float sdf_b("));
    assert!(glsl.contains("vec3 grad_sdf_a("));
    assert!(glsl.contains("vec3 grad_sdf_b("));
    assert!(glsl.contains("return sdf0_Ball3D(p, ParamBall3D(2.0));"));
    assert!(!glsl.contains("float scene_sdf("));
}

#[test]
fn const_value_and_function_declarations_emit_even_when_unused() {
    let source = "const R radius = 1\nconst tint = (.5, .5, .9, 1)\nconst Hom(R, R) wave = sin\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("const float radius = 1.0;"));
    assert!(glsl.contains("const vec4 tint = vec4(0.5, 0.5, 0.9, 1.0);"));
    assert!(glsl.contains("float wave(float _t) {"));
    assert!(glsl.contains("return sin(_t);"));
}

#[test]
fn const_value_expressions_lift_object_getters_over_points() {
    let source = "#2D\nprovided R time\ncolor1 = (.5, .5, .9, 1)\ncolor2 = (.9, .5, .5, 1)\nconst rect = Box2D(a=1, b=2)\nHom(R, R2) center = (sin*2, cos*2)\nconst ball = Ball2D(r=1.2) + center(time)\nconst color = (rect.sdf*color1 + ball.sdf*color2)/(rect.sdf + ball.sdf + 1e-3)\nconst scene = union(rect, ball)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("vec4 color(vec2 _t) {"));
    assert!(glsl.contains("sdf_rect(_t) * color1"));
    assert!(glsl.contains("sdf_ball(_t) * color2"));
    assert!(glsl.contains("float sdf_scene(vec2 p) {"));
}

#[test]
fn omits_non_const_functions_not_used_by_const_outputs() {
    let source = "#2D\nprovided R time\ncolor1 = (.5, .5, .9, 1)\ncolor2 = (.9, .5, .5, 1)\nconst rect = Box2D(a=1, b=2)\nHom(R, R2) center = (sin*2, cos*2)\nconst ball = Ball2D(r=1.2) + center(time)\ncolor = (rect.sdf*color1 + ball.sdf*color2)/(rect.sdf + ball.sdf + 1e-3)\nconst scene = union(rect, ball)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("vec2 center(float _t)"));
    assert!(!glsl.contains("vec4 color(vec2 _t)"));
    assert!(glsl.contains("float sdf_scene(vec2 p) {"));
}

#[test]
fn object_declarations_do_not_require_an_explicit_scene() {
    let source = "Object a = Ball3D(r=1)\nObject b = Ball3D(r=2)\n";
    let glsl = compile_program(source).unwrap();

    assert!(!glsl.contains("float scene_sdf("));
    assert!(!glsl.contains("float sdf_a("));
    assert!(!glsl.contains("float sdf_b("));
}

#[test]
fn const_object2d_declarations_emit_only_sdf_helpers() {
    let source = "#2D\nconst Object2D shape = Box2D(a=2, b=1)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float sdf_shape(vec2 p) {"));
    assert!(!glsl.contains("grad_sdf_shape"));
    assert!(!glsl.contains("float scene_sdf("));
}

#[test]
fn supports_construct_object_helpers() {
    let source = "construct Object shell = Ball3D(r=2)\nconst Object output = shell\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float sdf_shell(vec3 p) {"));
}

#[test]
fn supports_object3d_type_alias() {
    let source = "Object3D shell = Ball3D(r=2)\nconst Object output = shell\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("sdf0_Ball3D(p, ParamBall3D(2.0))"));
}

#[test]
fn directive_2d_uses_object2d_ambient_space() {
    let source = "#2D\nObject shape = Box2D(a=2, b=1) + (1, 2)\nconst Object output = shape\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float scene_sdf(vec2 p) {"));
    assert!(glsl.contains("vec2 scene_grad(vec2 p) {"));
    assert!(glsl.contains("sdf0_Box2D((p - vec2(1.0, 2.0)), ParamBox2D(2.0, 1.0))"));
    assert!(!glsl.contains("scene_sdf(vec3 p)"));
}

#[test]
fn directive_prec_sets_default_differential_precision() {
    let source = "#prec 0.002\nprovided Hom(R3, R) density\nprovided R3 p\nFunc(R, R) slope = grad(sin)\nR3 normal = gradient(density)(p)\nconst Object output = Ball3D(r=slope(0) + density(normal))\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float eps = 0.002;"));
    assert!(glsl.contains("(sin((_t + 0.002)) - sin((_t - 0.002))) / (2.0 * 0.002)"));
    assert!(glsl.contains("density((p + vec3(0.002, 0.0, 0.0)))"));
    assert!(glsl.contains("density((p - vec3(0.0, 0.0, 0.002)))"));
}

#[test]
fn directive_prec_accepts_scientific_notation() {
    let source = "#prec 1e-3\nconst Object output = Ball3D(r=1)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float eps = 0.001;"));
}

#[test]
fn directive_prec_rejects_invalid_values() {
    let source = "#prec nope\nconst Object output = Ball3D(r=1)\n";
    let err = compile_program(source).unwrap_err().to_string();
    assert!(err.contains("invalid #prec value 'nope'"));

    let source = "#prec 0\nconst Object output = Ball3D(r=1)\n";
    let err = compile_program(source).unwrap_err().to_string();
    assert!(err.contains("#prec expects a positive float value"));
}

#[test]
fn directive_2d_supports_2d_isometry_actions() {
    let source = "#2D\nIsom2 g = Isom2(e, [1, 2])\nconst Object output = g * Box2D(a=2, b=1)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("struct Isom2"));
    assert!(glsl.contains("float scene_sdf(vec2 p) {"));
    assert!(glsl.contains("sdf0_Box2D(act_Isom2(inv_Isom2(g), p), ParamBox2D(2.0, 1.0))"));
}

#[test]
fn directive_2d_rejects_3d_primitives() {
    let source = "#2D\nconst Object output = Ball3D(r=1)\n";
    let err = compile_program(source).unwrap_err().to_string();

    assert!(err.contains("primitive 'Ball3D' is 3D but ambient space is 2D"));
}

#[test]
fn directive_2d_rejects_non_initial_directives() {
    let source = "const Object output = Box2D(a=2, b=1)\n#2D\n";
    let err = compile_program(source).unwrap_err().to_string();

    assert!(err.contains("directives must appear before declarations"));
}

#[test]
fn supports_full_line_comments() {
    let source =
        "// input animation\nprovided Float time\n// object body\nconst Object output = Ball3D(r=1 + time)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float scene_sdf(vec3 p) {"));
    assert!(glsl.contains("ParamBall3D((1.0 + time))"));
}

#[test]
fn supports_trailing_line_comments() {
    let source = "provided Float time // animation clock\nObject A = Ball3D(r=1) + (1, 0, 0) // translated sphere\nconst Object output = A // final object\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float scene_sdf(vec3 p) {"));
    assert!(glsl.contains("sdf0_Ball3D((p - vec3(1.0, 0.0, 0.0)), ParamBall3D(1.0))"));
}

#[test]
fn emits_generated_object_helpers() {
    let source = "construct Object shell = Ball3D(r=2) + (1, 0, 0)\nconst Object output = shell\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float sdf_shell(vec3 p) {"));
    assert!(glsl.contains("vec3 grad_sdf_shell(vec3 p) {"));
    assert!(glsl.contains("return sdf0_Ball3D((p - vec3(1.0, 0.0, 0.0)), ParamBall3D(2.0));"));
}

#[test]
fn generated_helpers_use_provided_inputs_as_globals() {
    let source =
        "provided Float time\nconstruct Object shell = Ball3D(r=1 + time)\nconst Object output = shell\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float sdf_shell(vec3 p) {"));
    assert!(glsl.contains("vec3 grad_sdf_shell(vec3 p) {"));
    assert!(glsl.contains("return normalize(vec3(((sdf_shell(p + vec3(eps, 0.0, 0.0)) - sdf_shell(p - vec3(eps, 0.0, 0.0))) / (2.0 * eps))"));
}

#[test]
fn object_sdf_and_grad_getters_emit_helpers_for_plain_bindings() {
    let source = "provided R3 q\nObject shell = Ball3D(r=2) + (1, 0, 0)\nR d = shell.sdf(q)\nR3 g = shell.grad(q)\nconst Object output = Ball3D(r=d + length(g))\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float sdf_shell(vec3 p) {"));
    assert!(glsl.contains("vec3 grad_sdf_shell(vec3 p) {"));
    assert!(glsl.contains("float d = sdf_shell(q);"));
    assert!(glsl.contains("vec3 g = grad_sdf_shell(q);"));
}

#[test]
fn object_sdf_getter_closure_uses_provided_inputs_as_globals() {
    let source = "provided R time\nObject shell = Ball3D(r=1 + time)\nR3 p0 = (0, 0, 0)\nR3 g = gradient(shell.sdf)(p0)\nconst Object output = Ball3D(r=length(g))\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float sdf_shell(vec3 p) {"));
    assert!(glsl.contains("return sdf0_Ball3D(p, ParamBall3D((1.0 + time)));"));
    assert!(glsl.contains("vec3 g = vec3("));
    assert!(glsl.contains("sdf_shell((p0 + vec3(0.01, 0.0, 0.0)))"));
}

#[test]
fn scene_sdf_reuses_generated_object_helpers() {
    let source = "construct Object a = Ball3D(r=1)\nconstruct Object b = Ball3D(r=2) + (1, 0, 0)\nconst Object output = union(a, b)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float sdf_a(vec3 p) {"));
    assert!(glsl.contains("float sdf_b(vec3 p) {"));
    assert!(glsl.contains("return _op_union(sdf_a(p), sdf_b(p));"));
    assert!(!glsl.contains("return _op_union(sdf0_Ball3D"));
}

#[test]
fn hoists_scene_invariant_value_bindings_to_global_consts() {
    let source = "provided R time\nR3 axis = (0, 0, 1)\nR3 start = (1, 0, 0)\nIsom3 r = rot(axis, axis * 0, time)\nR3 p = r * start\nconstruct Object b = Ball3D(r=1) + p\nconst Object output = b\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("const vec3 axis = vec3(0.0, 0.0, 1.0);"));
    assert!(glsl.contains("const vec3 start = vec3(1.0, 0.0, 0.0);"));
    assert!(glsl.contains("float sdf_b(vec3 p_"));
    assert!(glsl.contains("Isom3 r = rot(axis, (axis * 0.0), time);"));
    assert!(glsl.contains("vec3 p = act_Isom3(r, start);"));
    assert!(glsl.contains("float scene_sdf(vec3 p_"));
    assert!(glsl.contains("return sdf_b(p_"));
}

#[test]
fn renames_generated_locals_on_name_conflicts() {
    let source =
        "provided Float p\nprovided Float eps\nconst Object output = Ball3D(r=eps) + (p, 0, 0)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float scene_sdf(vec3 p_r"));
    assert!(!glsl.contains(", float p, float eps)"));
    assert!(glsl.contains("float eps_r"));
    assert!(glsl.contains("scene_sdf(p_r"));
    assert!(glsl.contains("vec3(eps_r"));
}

#[test]
fn plain_object_bindings_do_not_export_helpers() {
    let source = "Object shell = Ball3D(r=2)\nconst Object output = shell\n";
    let glsl = compile_program(source).unwrap();

    assert!(!glsl.contains("float sdf_shell("));
    assert!(!glsl.contains("vec3 grad_sdf_shell("));
}

#[test]
fn reports_the_offending_token_for_expression_parse_errors() {
    let source = "const Object output = Ball3D(r=1) + *\n";
    let error = compile_program(source).unwrap_err().to_string();

    assert!(error.contains("unexpected token '*' in expression"));
}

#[test]
fn reports_line_number_for_parse_errors() {
    let source = "Object ok = Ball3D(r=1)\nconst Object output = Ball3D(r=1) provided R time\n";
    let err = compile_program(source).unwrap_err();

    assert_eq!(err.line(), Some(2));
    assert!(err.to_string().contains("line 2:"));
    assert!(err.to_string().contains("identifier 'provided'"));
}

#[test]
fn reports_line_number_for_type_errors() {
    let source = "Object ok = Ball3D(r=1)\nR bad = (1, 2, 3)\nconst Object output = ok\n";
    let err = compile_program(source).unwrap_err();

    assert_eq!(err.line(), Some(2));
    assert!(err.to_string().contains("line 2:"));
    assert!(err.to_string().contains("binding 'bad'"));
}

#[test]
fn emits_only_used_support_code() {
    let source = "Object A = Ball3D(r=3)\nconst Object output = A\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("struct ParamBall3D"));
    assert!(glsl.contains("float sdf0_Ball3D"));
    assert!(!glsl.contains("_op_smooth_union"));
    assert!(glsl.contains("vec3 scene_grad(vec3 p) {"));
}

#[test]
fn rejects_unknown_primitive_field() {
    let source = "Object A = Ball3D(radius=3)\nconst Object output = A\n";
    let error = compile_program(source).unwrap_err().to_string();

    assert!(error.contains("missing field 'r'"));
}

#[test]
fn rejects_old_binding_syntax() {
    let source = "A : Object = Ball3D(r=3)\nconst Object output = A\n";
    let error = compile_program(source).unwrap_err().to_string();

    assert!(error.contains("use 'type name = value'"));
}

#[test]
fn rejects_construct_on_non_object_bindings() {
    let source = "construct R radius = 2\nconst Object output = Ball3D(r=radius)\n";
    let error = compile_program(source).unwrap_err().to_string();

    assert!(error.contains("'construct' currently only supports Object bindings"));
}

#[test]
fn rejects_generate_declarations() {
    let source = "Object A = Ball3D(r=3)\ngenerate A\n";
    let error = compile_program(source).unwrap_err().to_string();

    assert!(
        error.contains("generate declarations have been removed; use 'const Object name = value'")
    );
}

#[test]
fn emits_box_primitive() {
    let source = "Object shape = Box2D(a=2, b=1)\nconst Object output = shape\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("struct ParamBox2D"));
    assert!(glsl.contains("float a;"));
    assert!(glsl.contains("float b;"));
    assert!(glsl.contains("float sdf0_Box2D(vec2 p, ParamBox2D params)"));
    assert!(glsl.contains("vec2(params.a, params.b)"));
    assert!(glsl.contains("sdf0_Box2D((p).xy, ParamBox2D(2.0, 1.0))"));
}

#[test]
fn emits_box3d_primitive() {
    let source = "Object shape = Box3D(a=2, b=1, c=3)\nconst Object output = shape\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("struct ParamBox3D"));
    assert!(glsl.contains("float a;"));
    assert!(glsl.contains("float b;"));
    assert!(glsl.contains("float c;"));
    assert!(glsl.contains("float sdf0_Box3D(vec3 p, ParamBox3D params)"));
    assert!(glsl.contains("vec3 d = abs(p) - vec3(params.a, params.b, params.c);"));
    assert!(glsl.contains("sdf0_Box3D(p, ParamBox3D(2.0, 1.0, 3.0))"));
}

#[test]
fn emits_box3d_from_flat_positional_arguments() {
    let source = "Object shape = Box3D(2, 1, 3)\nconst Object output = shape\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("sdf0_Box3D(p, ParamBox3D(2.0, 1.0, 3.0))"));
}

#[test]
fn supports_negative_tuple_components() {
    let source = "Object shape = Box3D(2, 1, 3) + (-1, -2, -3)\nconst Object output = shape\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("(p - vec3((-1.0), (-2.0), (-3.0)))"));
}

#[test]
fn supports_scientific_notation_literals() {
    let source =
        "Object shape = Ball3D(r=1e-1) + (2e0, .5e+1, 3E-1)\nconst Object output = shape\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("ParamBall3D(0.1)"));
    assert!(glsl.contains("vec3(2.0, 5.0, 0.3)"));
}

#[test]
fn emits_primitive_with_positional_arguments() {
    let source = "Object shape = Box2D(2, 1)\nconst Object output = shape\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("sdf0_Box2D((p).xy, ParamBox2D(2.0, 1.0))"));
}

#[test]
fn rejects_wrong_number_of_positional_primitive_arguments() {
    let source = "const Object output = Ball3D()\n";
    let error = compile_program(source).unwrap_err().to_string();

    assert!(error.contains("primitive 'Ball3D' expects 1 field(s)"));
}

#[test]
fn emits_simplex3d_primitive() {
    let source = "Object shape = Simplex3D(p0=(0, 0, 0), p1=(1, 0, 0), p2=(0, 1, 0), p3=(0, 0, 1))\nconst Object output = shape\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("struct ParamSimplex3D"));
    assert!(glsl.contains("vec3 p0;"));
    assert!(glsl.contains("vec3 p1;"));
    assert!(glsl.contains("vec3 p2;"));
    assert!(glsl.contains("vec3 p3;"));
    assert!(glsl.contains("float sdf0_Simplex3D(vec3 p, ParamSimplex3D params)"));
    assert!(
        glsl.contains("vec3 vertices[4] = vec3[4](params.p0, params.p1, params.p2, params.p3);")
    );
    assert!(glsl.contains("sdf0_Simplex3D(p, ParamSimplex3D(vec3(0.0, 0.0, 0.0), vec3(1.0, 0.0, 0.0), vec3(0.0, 1.0, 0.0), vec3(0.0, 0.0, 1.0)))"));
}

#[test]
fn emits_halfspace3d_primitive() {
    let source = "Object shape = Halfspace3D(n=(0, 1, 0), h=2)\nconst Object output = shape\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("struct ParamHalfspace3D"));
    assert!(glsl.contains("float sdf0_Halfspace3D(vec3 p, ParamHalfspace3D params)"));
    assert!(glsl.contains("return dot(p, normalize(params.n)) + params.h;"));
    assert!(glsl.contains("sdf0_Halfspace3D(p, ParamHalfspace3D(vec3(0.0, 1.0, 0.0), 2.0))"));
}

#[test]
fn emits_plane3d_primitive() {
    let source =
        "Object shape = Plane3D(n=(0, 1, 0), origin=(0, 2, 0))\nconst Object output = shape\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("struct ParamPlane3D"));
    assert!(glsl.contains("float sdf0_Plane3D(vec3 p, ParamPlane3D params)"));
    assert!(glsl.contains("float h;"));
    assert!(glsl.contains("sdf0_Plane3D(p, ParamPlane3D(vec3(0.0, 1.0, 0.0), (-dot(normalize(vec3(0.0, 1.0, 0.0)), vec3(0.0, 2.0, 0.0)))))"));
}

#[test]
fn emits_line3d_primitive() {
    let source =
        "Object shape = Line3D(x0=(0, 0, 0), dir=(2, 1, 3))\nconst Object output = shape\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("struct ParamLine3D"));
    assert!(glsl.contains("float sdf0_Line3D(vec3 p, ParamLine3D params)"));
    assert!(glsl.contains("vec3 direction = normalize(params.dir);"));
    assert!(glsl.contains("sdf0_Line3D(p, ParamLine3D(vec3(0.0, 0.0, 0.0), vec3(2.0, 1.0, 3.0)))"));
}

#[test]
fn emits_triangle3d_primitive() {
    let source =
        "Object shape = Triangle3D(p1=(0, 0, 0), p2=(1, 0, 0), p3=(0, 1, 0))\nconst Object output = shape\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("struct ParamTriangle3D"));
    assert!(glsl.contains("float sdf0_Triangle3D(vec3 p, ParamTriangle3D params)"));
    assert!(glsl.contains("vec3 nor = cross(ba, ac);"));
    assert!(glsl.contains(
        "sdf0_Triangle3D(p, ParamTriangle3D(vec3(0.0, 0.0, 0.0), vec3(1.0, 0.0, 0.0), vec3(0.0, 1.0, 0.0)))"
    ));
}

#[test]
fn emits_torus3d_primitive() {
    let source = "Object shape = Torus3D(major=3, minor=.5)\nconst Object output = shape\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("struct ParamTorus3D"));
    assert!(glsl.contains("float sdf0_Torus3D(vec3 p, ParamTorus3D params)"));
    assert!(glsl.contains("vec2 q = vec2(length(p.xz) - params.major, p.y);"));
    assert!(glsl.contains("sdf0_Torus3D(p, ParamTorus3D(3.0, 0.5))"));
}

#[test]
fn emits_segment_primitive() {
    let source = "Object shape = Segment2D(a=(0, 0), b=(2, 1))\nconst Object output = shape\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("struct ParamSegment2D"));
    assert!(glsl.contains("float sdf0_Segment2D(vec2 p, ParamSegment2D params)"));
    assert!(glsl.contains("sdf0_Segment2D((p).xy, ParamSegment2D(vec2(0.0, 0.0), vec2(2.0, 1.0)))"));
}

#[test]
fn emits_segment2d_length_constructor() {
    let source = "Object shape = Segment2D(length=2)\nconst Object output = shape\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("sdf0_Segment2D((p).xy, ParamSegment2D(vec2((-1.0 * (0.5 * 2.0)), 0.0), vec2((0.5 * 2.0), 0.0)))"));
}

#[test]
fn emits_segment3d_primitive() {
    let source =
        "Object shape = Segment3D(a=(0, 0, 0), b=(2, 1, 3))\nconst Object output = shape\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("struct ParamSegment3D"));
    assert!(glsl.contains("float sdf0_Segment3D(vec3 p, ParamSegment3D params)"));
    assert!(glsl
        .contains("sdf0_Segment3D(p, ParamSegment3D(vec3(0.0, 0.0, 0.0), vec3(2.0, 1.0, 3.0)))"));
}

#[test]
fn emits_segment3d_length_constructor() {
    let source = "Object shape = Segment3D(2)\nconst Object output = shape\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("sdf0_Segment3D(p, ParamSegment3D(vec3((-1.0 * (0.5 * 2.0)), 0.0, 0.0), vec3((0.5 * 2.0), 0.0, 0.0)))"));
}

#[test]
fn emits_triangle_primitive() {
    let source =
        "Object shape = Triangle2D(p0=(0, 0), p1=(2, 0), p2=(0, 2))\nconst Object output = shape\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("struct ParamTriangle2D"));
    assert!(glsl.contains("float sdf0_Triangle2D(vec2 p, ParamTriangle2D params)"));
    assert!(glsl.contains(
        "sdf0_Triangle2D((p).xy, ParamTriangle2D(vec2(0.0, 0.0), vec2(2.0, 0.0), vec2(0.0, 2.0)))"
    ));
}

#[test]
fn emits_quad2d_primitive() {
    let source =
        "Object shape = Quad2D(p1=(0, 0), p2=(2, 0), p3=(2, 1), p4=(0, 1))\nconst Object output = shape\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("struct ParamQuad2D"));
    assert!(glsl.contains("float sdf0_Quad2D(vec2 p, ParamQuad2D params)"));
    assert!(glsl.contains(
        "sdf0_Quad2D((p).xy, ParamQuad2D(vec2(0.0, 0.0), vec2(2.0, 0.0), vec2(2.0, 1.0), vec2(0.0, 1.0)))"
    ));
}

#[test]
fn emits_quad3d_primitive() {
    let source = "Object shape = Quad3D(p1=(0, 0, 0), p2=(1, 0, 0), p3=(1, 1, 0), p4=(0, 1, 0))\nconst Object output = shape\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("struct ParamQuad3D"));
    assert!(glsl.contains("float sdf0_Quad3D(vec3 p, ParamQuad3D params)"));
    assert!(glsl.contains("vec3 dc = params.p4 - params.p3;"));
    assert!(glsl.contains(
        "sdf0_Quad3D(p, ParamQuad3D(vec3(0.0, 0.0, 0.0), vec3(1.0, 0.0, 0.0), vec3(1.0, 1.0, 0.0), vec3(0.0, 1.0, 0.0)))"
    ));
}

#[test]
fn emits_polygon_primitive() {
    let source =
        "Object shape = Polygon2D(points=((0, 0), (2, 0), (2, 1), (0, 1)))\nconst Object output = shape\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("const int POLYGON2D_MAX_VERTICES = 16;"));
    assert!(glsl.contains(
        "float sdf0_Polygon2D(vec2 p, vec2 vertices[POLYGON2D_MAX_VERTICES], int count)"
    ));
    assert!(glsl.contains(
        "sdf0_Polygon2D(p.xy, vec2[16](vec2(0.0, 0.0), vec2(2.0, 0.0), vec2(2.0, 1.0), vec2(0.0, 1.0)"
    ));
}

#[test]
fn emits_point_primitive() {
    let source = "Object shape = Point2D(at=(3, 4))\nconst Object output = shape\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("struct ParamPoint2D"));
    assert!(glsl.contains("float sdf0_Point2D(vec2 p, ParamPoint2D params)"));
    assert!(glsl.contains("sdf0_Point2D((p).xy, ParamPoint2D(vec2(3.0, 4.0)))"));
}

#[test]
fn emits_translation_action_from_addition_sugar() {
    let source = "const Object output = Ball3D(r=1) + (1, 2, 3)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("sdf0_Ball3D((p - vec3(1.0, 2.0, 3.0)), ParamBall3D(1.0))"));
}

#[test]
fn emits_revolution_operator() {
    let source =
        "Object2D profile = Segment2D(a=(0, -1), b=(0, 1))\nconst Object output = revolution(1.5)(profile)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("vec3 _op_revolution_point(vec3 _p, float _offset)"));
    assert!(glsl.contains("sdf0_Segment2D((_op_revolution_point(p, 1.5)).xy, ParamSegment2D(vec2(0.0, (-1.0)), vec2(0.0, 1.0)))"));
}

#[test]
fn rejects_revolution_of_3d_object() {
    let source = "const Object output = revolution(1.5)(Ball3D(r=1))\n";
    let err = compile_program(source).unwrap_err().to_string();

    assert!(err.contains("operator 'revolution' expects an Object2D argument"));
}

#[test]
fn emits_extrusion_operator() {
    let source = "const Object output = extrude(.25)(Box2D(a=1, b=.5))\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float _op_extrusion(float _base_distance, float _z, float _height)"));
    assert!(glsl.contains(
        "_op_extrusion(sdf0_Box2D((vec3((p).xy, 0.0)).xy, ParamBox2D(1.0, 0.5)), (p).z, 0.25)"
    ));
}

#[test]
fn emits_rotation_action_from_mat3_input() {
    let source = "provided Mat3 R\nconst Object output = R * Ball3D(r=1)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float scene_sdf(vec3 p) {"));
    assert!(glsl.contains("sdf0_Ball3D((transpose(R) * p), ParamBall3D(1.0))"));
}

#[test]
fn emits_builtin_3d_rotation_operator() {
    let source = "const Object output = rot((0, 1, 0), (1, 0, 0), 0.5)(Ball3D(r=1))\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("mat3 _op_rot_matrix(vec3 _binormal, float _angle)"));
    assert!(glsl.contains(
        "vec3 _op_rot_inverse_point(vec3 _p, vec3 _binormal, vec3 _anchor, float _angle)"
    ));
    assert!(glsl.contains("sdf0_Ball3D(_op_rot_inverse_point(p, vec3(0.0, 1.0, 0.0), vec3(1.0, 0.0, 0.0), 0.5), ParamBall3D(1.0))"));
}

#[test]
fn emits_value_level_3d_rotation() {
    let source = "provided R time\nR3 e3 = (0, 0, 1)\nIsom3 r = rot(e3, e3 * 0, time)\nR3 p = r * (1, 0, 0)\nconst Object output = Ball3D(r=1) + p\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("struct Isom3"));
    assert!(glsl.contains("Isom3 rot(vec3 binormal, vec3 anchor, float angle)"));
    assert!(glsl.contains("Isom3 r = rot(e3, (e3 * 0.0), time);"));
    assert!(glsl.contains("vec3 p = act_Isom3(r, vec3(1.0, 0.0, 0.0));"));
    assert!(glsl.contains("sdf0_Ball3D(("));
    assert!(glsl.contains(" - p), ParamBall3D(1.0))"));
}

#[test]
fn calls_product_domain_value_functions_with_multiple_arguments() {
    let source = "provided Hom(R3 x R3, R3) cross\nR3 a = (1, 0, 0)\nR3 b = (0, 1, 0)\nR3 c = cross(a, b)\nconst Object output = Ball3D(r=1) + c\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("vec3 c = cross(a, b);"));
    assert!(glsl.contains("sdf0_Ball3D((p - c), ParamBall3D(1.0))"));
}

#[test]
fn emits_builtin_rotation_operator_defaults() {
    let angle_glsl = compile_program("const Object output = rot(0.5)(Ball3D(r=1))\n").unwrap();
    let zero_arg_glsl = compile_program("const Object output = rot()(Ball3D(r=2))\n").unwrap();

    assert!(angle_glsl.contains("sdf0_Ball3D(_op_rot_inverse_point(p, vec3(0.0, 0.0, 1.0), vec3(0.0, 0.0, 0.0), 0.5), ParamBall3D(1.0))"));
    assert!(zero_arg_glsl.contains("sdf0_Ball3D(_op_rot_inverse_point(p, vec3(0.0, 0.0, 1.0), vec3(0.0, 0.0, 0.0), 0.0), ParamBall3D(2.0))"));
}

#[test]
fn emits_builtin_2d_rotation_operator() {
    let source = "const Object output = rot2D([1, 0], 0.5)(Box2D(a=1, b=.5))\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("mat2 _op_rot2D_matrix(float _angle)"));
    assert!(glsl.contains("vec3 _op_rot2D_inverse_point(vec3 _p, vec2 _anchor, float _angle)"));
    assert!(glsl.contains(
        "sdf0_Box2D((_op_rot2D_inverse_point(p, vec2(1.0, 0.0), 0.5)).xy, ParamBox2D(1.0, 0.5))"
    ));
}

#[test]
fn emits_builtin_2d_rotation_operator_defaults() {
    let angle_glsl =
        compile_program("const Object output = rot2D(0.5)(Box2D(a=1, b=.5))\n").unwrap();
    let zero_arg_glsl =
        compile_program("const Object output = rot2D()(Box2D(a=2, b=1))\n").unwrap();

    assert!(angle_glsl.contains(
        "sdf0_Box2D((_op_rot2D_inverse_point(p, vec2(0.0, 0.0), 0.5)).xy, ParamBox2D(1.0, 0.5))"
    ));
    assert!(zero_arg_glsl.contains(
        "sdf0_Box2D((_op_rot2D_inverse_point(p, vec2(0.0, 0.0), 0.0)).xy, ParamBox2D(2.0, 1.0))"
    ));
}

#[test]
fn emits_mat3_helpers_and_uses_them_in_object_actions() {
    let source = "provided Float time\nFunc(Float, Mat3) spin = ((1, 0, 0), (0, 1, 0), (0, 0, 1))\nconst Object output = spin(time) * Ball3D(r=1)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("mat3 spin(float _t) {"));
    assert!(glsl.contains(
        "return transpose(mat3(vec3(1.0, 0.0, 0.0), vec3(0.0, 1.0, 0.0), vec3(0.0, 0.0, 1.0)));"
    ));
    assert!(glsl.contains("sdf0_Ball3D((transpose(spin(time)) * p), ParamBall3D(1.0))"));
}

#[test]
fn emits_mat2_and_mat4_bracket_constructors() {
    let source = "Mat2 a = ((1, 2), (3, 4))\nMat4 b = ((1, 0, 0, 0), (0, 1, 0, 0), (0, 0, 1, 0), (0, 0, 0, 1))\nconst Object output = Ball3D(r=1)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("mat2 a = transpose(mat2(vec2(1.0, 2.0), vec2(3.0, 4.0)));"));
    assert!(glsl.contains("mat4 b = transpose(mat4(vec4(1.0, 0.0, 0.0, 0.0), vec4(0.0, 1.0, 0.0, 0.0), vec4(0.0, 0.0, 1.0, 0.0), vec4(0.0, 0.0, 0.0, 1.0)));"));
}

#[test]
fn emits_rectangular_matrix_bracket_constructors() {
    let source = "Mat2x3 wide = ((1, 2, 3), (4, 5, 6))\nMat3x2 tall = ((1, 2), (3, 4), (5, 6))\nconst Object output = Ball3D(r=1)\n";
    let glsl = compile_program(source).unwrap();

    assert!(
        glsl.contains("mat3x2 wide = transpose(mat2x3(vec3(1.0, 2.0, 3.0), vec3(4.0, 5.0, 6.0)));")
    );
    assert!(glsl.contains(
        "mat2x3 tall = transpose(mat3x2(vec2(1.0, 2.0), vec2(3.0, 4.0), vec2(5.0, 6.0)));"
    ));
}

#[test]
fn treats_square_matrices_as_rings_by_shape() {
    let source =
        "provided Mat3 a\nprovided Mat3 b\nMat3 c = (a * b) + a\nconst Object output = c * Ball3D(r=1)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float scene_sdf(vec3 p) {"));
    assert!(glsl.contains("mat3 c = ((a * b) + a);"));
}

#[test]
fn emits_difference_operator() {
    let source =
        "Object a = Ball3D(r=2)\nObject b = Ball3D(r=1) + (0.5, 0, 0)\nconst Object output = diff(a, b)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float _op_difference(float _a, float _b) {"));
    assert!(glsl.contains("return max(_a, -_b);"));
    assert!(glsl.contains("return _op_difference("));
}

#[test]
fn emits_union_operator() {
    let source =
        "Object a = Ball3D(r=2)\nObject b = Ball3D(r=1) + (0.5, 0, 0)\nconst Object output = union(a, b)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float _op_union(float _a, float _b) {"));
    assert!(glsl.contains("return min(_a, _b);"));
    assert!(glsl.contains("return _op_union("));
}

#[test]
fn emits_associative_union_operator_with_four_args() {
    let source = "Object a = Ball3D(r=4)\nObject b = Ball3D(r=3) + (1, 0, 0)\nObject c = Ball3D(r=2) + (2, 0, 0)\nObject d = Ball3D(r=1) + (3, 0, 0)\nconst Object output = union(a, b, c, d)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("return _op_union(_op_union("));
    assert!(glsl.contains(", _op_union("));
}

#[test]
fn emits_associative_union_operator_with_three_args() {
    let source = "Object a = Ball3D(r=3)\nObject b = Ball3D(r=2) + (1, 0, 0)\nObject c = Ball3D(r=1) + (2, 0, 0)\nconst Object output = union(a, b, c)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("return _op_union(sdf0_Ball3D("));
    assert!(glsl.contains(", _op_union("));
}

#[test]
fn emits_intersection_operator() {
    let source =
        "Object a = Ball3D(r=2)\nObject b = Ball3D(r=1) + (0.5, 0, 0)\nconst Object output = intersect(a, b)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float _op_intersection(float _a, float _b) {"));
    assert!(glsl.contains("return max(_a, _b);"));
    assert!(glsl.contains("return _op_intersection("));
}

#[test]
fn emits_xor_operator() {
    let source =
        "Object a = Ball3D(r=2)\nObject b = Ball3D(r=1) + (0.5, 0, 0)\nconst Object output = xor(a, b)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float _op_xor(float _a, float _b) {"));
    assert!(glsl.contains("return max(min(_a, _b), -max(_a, _b));"));
    assert!(glsl.contains("return _op_xor("));
}

#[test]
fn emits_smooth_union_operator() {
    let source =
        "Object a = Ball3D(r=2)\nObject b = Ball3D(r=1) + (0.5, 0, 0)\nconst Object output = smoothUnion(0.25)(a, b)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float _op_smooth_union(float _a, float _b, float _k) {"));
    assert!(!glsl.contains("_op_smooth_union_min"));
    assert!(glsl.contains("_k *= 1.0 / (1.0 - sqrt(0.5));"));
    assert!(glsl.contains("return _op_smooth_union("));
}

#[test]
fn emits_smooth_intersection_operator() {
    let source = "Object a = Ball3D(r=2)\nObject b = Ball3D(r=1) + (0.5, 0, 0)\nconst Object output = smoothIntersect(0.25)(a, b)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float _op_smooth_intersection(float _a, float _b, float _k) {"));
    assert!(glsl.contains("return _op_smooth_intersection_max(_a, _b, _k);"));
    assert!(glsl.contains("return _op_smooth_intersection("));
}

#[test]
fn emits_smooth_difference_operator() {
    let source = "Object a = Ball3D(r=2)\nObject b = Ball3D(r=1) + (0.5, 0, 0)\nconst Object output = smoothDiff(0.25)(a, b)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float _op_smooth_difference(float _a, float _b, float _k) {"));
    assert!(glsl.contains("return _op_smooth_difference_max(_a, -_b, _k);"));
    assert!(glsl.contains("return _op_smooth_difference("));
}

#[test]
fn emits_smooth_xor_operator() {
    let source =
        "Object a = Ball3D(r=2)\nObject b = Ball3D(r=1) + (0.5, 0, 0)\nconst Object output = smoothXor(0.25)(a, b)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float _op_smooth_xor(float _a, float _b, float _k) {"));
    assert!(glsl.contains(
        "return _op_smooth_xor_max(_op_smooth_xor_min(_a, _b, _k), -_op_smooth_xor_max(_a, _b, _k), _k);"
    ));
    assert!(glsl.contains("return _op_smooth_xor("));
}

#[test]
fn emits_only_used_object_operator_support() {
    let source =
        "Object a = Ball3D(r=2)\nObject b = Ball3D(r=1)\nconst Object output = diff(a, b)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("_op_difference"));
    assert!(!glsl.contains("_op_smooth_union"));
    assert!(!glsl.contains("_op_union"));
    assert!(!glsl.contains("_op_intersection"));
    assert!(!glsl.contains("_op_xor"));
    assert!(!glsl.contains("_op_smooth_intersection"));
    assert!(!glsl.contains("_op_smooth_difference"));
    assert!(!glsl.contains("_op_smooth_xor"));
}

#[test]
fn emits_array_literals_index_size_and_concat() {
    let source = "Array(R) a = Array(1, 2, 3)\nR x = a[1]\nZ n = size(a)\nArray(R) b = concat(a, Array(4, 5))\nconst Object output = Ball3D(r=x + b[n])\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float a[3] = float[3](1.0, 2.0, 3.0);"));
    assert!(glsl.contains("float x = a[1];"));
    assert!(glsl.contains("int n = 3;"));
    assert!(glsl.contains("float[5] concat_r_3_2(float[3] left, float[2] right) {"));
    assert!(glsl.contains("float b[5] = concat_r_3_2(a, float[2](4.0, 5.0));"));
}

#[test]
fn emits_array_types_for_inputs_and_function_returns() {
    let source = "provided Array(R) weights\nFunc(Float, Array(R)) pair = Array(sin, cos)\nR radius = weights[0] + pair(0)[1]\nconst Object output = Ball3D(r=radius)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float[] pair(float _t) {"));
    assert!(glsl.contains("return float[2](sin(_t), cos(_t));"));
    assert!(glsl.contains("float scene_sdf(vec3 p) {"));
    assert!(glsl.contains("float radius = (weights[0] + pair(0.0)[1]);"));
}

#[test]
fn rejects_empty_array_literal() {
    let source = "Array(R) a = Array()\nconst Object output = Ball3D(r=1)\n";
    let err = compile_program(source).unwrap_err();

    assert_eq!(
        err.to_string(),
        "line 1: Array(...) requires at least one element"
    );
}

#[test]
fn rejects_mixed_array_elements() {
    let source = "Array(R) a = Array(1, (1, 2))\nconst Object output = Ball3D(r=1)\n";
    let err = compile_program(source).unwrap_err();

    assert_eq!(
        err.to_string(),
        "line 1: array element 2 expected R, got R2"
    );
}

#[test]
fn rejects_non_integer_array_index() {
    let source = "Array(R) a = Array(1, 2)\nR x = a[0.5]\nconst Object output = Ball3D(r=x)\n";
    let err = compile_program(source).unwrap_err();

    assert_eq!(
        err.to_string(),
        "line 2: integer expression expected Z, got R"
    );
}

#[test]
fn rejects_indexing_non_array() {
    let source = "R x = 1[0]\nconst Object output = Ball3D(r=x)\n";
    let err = compile_program(source).unwrap_err();

    assert_eq!(err.to_string(), "line 1: indexing expected Array(T), got R");
}

#[test]
fn rejects_concat_element_mismatch() {
    let source = "Array(R) a = Array(1)\nArray(R2) b = Array([1, 2])\nArray(R) c = concat(a, b)\nconst Object output = Ball3D(r=1)\n";
    let err = compile_program(source).unwrap_err();

    assert_eq!(
        err.to_string(),
        "line 3: concat element type expected R, got R2"
    );
}

#[test]
fn rejects_extra_arguments_for_non_associative_operator() {
    let source = "Object a = Ball3D(r=3)\nObject b = Ball3D(r=2)\nObject c = Ball3D(r=1)\nconst Object output = diff(a, b, c)\n";
    let err = compile_program(source).unwrap_err();

    assert_eq!(
        err.to_string(),
        "line 4: operator 'diff' expects 2 argument(s), got 3"
    );
}
