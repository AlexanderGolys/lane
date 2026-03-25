use lane::{
    compile_program, known_preregistered_objects, known_primitives, preregistered_object,
    PreregisteredObjectKind,
};

#[test]
fn lists_known_primitives_with_domains() {
    let primitives = known_primitives();
    let ball = primitives
        .iter()
        .find(|primitive| primitive.name == "Ball3D")
        .unwrap();
    let polygon = primitives
        .iter()
        .find(|primitive| primitive.name == "Polygon2D")
        .unwrap();

    assert_eq!(ball.sdf_name, "sdf0_Ball3D");
    assert_eq!(
        ball.domain,
        "sdf0_Ball3D(vec3 p, ParamBall3D params) -> float"
    );
    assert_eq!(ball.parameter_type.as_deref(), Some("ParamBall3D"));
    assert_eq!(ball.fields[0].name, "r");
    assert_eq!(ball.fields[0].domain, "float");

    assert_eq!(
        polygon.domain,
        "sdf0_Polygon2D(vec2 p, vec2 vertices[POLYGON2D_MAX_VERTICES], int count) -> float"
    );
    assert_eq!(polygon.parameter_type, None);
    assert_eq!(polygon.fields[0].name, "points");
    assert_eq!(polygon.fields[0].domain, "vec2 list");
}

#[test]
fn lists_preregistered_functions_and_types() {
    let objects = known_preregistered_objects();

    assert!(objects.iter().any(|object| {
        object.kind == PreregisteredObjectKind::Function && object.name == "sdf0_Ball3D"
    }));
    assert!(objects.iter().any(|object| {
        object.kind == PreregisteredObjectKind::Function && object.name == "op_smooth_union"
    }));
    assert!(objects.iter().any(|object| {
        object.kind == PreregisteredObjectKind::Type && object.name == "ParamBall3D"
    }));
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
    let source = "func(float -> float) wobble = sin @ sin\nout: Ball3D(r=wobble(0))\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float dsl_wobble(float t) {"));
    assert!(glsl.contains("return sin(sin(t));"));
}

#[test]
fn rejects_invalid_function_composition() {
    let source = "in: func(float -> vec3) center\nfunc(float -> float) wobble = sin @ center\nout: Ball3D(r=1)\n";
    let error = compile_program(source).unwrap_err().to_string();

    assert!(error.contains("cannot compose sin @ center"));
}

#[test]
fn emits_only_used_support_code() {
    let source = "Obj3 A = Ball3D(r=3)\nout: A\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("struct ParamBall3D"));
    assert!(glsl.contains("float sdf0_Ball3D"));
    assert!(!glsl.contains("op_smooth_union"));
}

#[test]
fn rejects_unknown_primitive_field() {
    let source = "Obj3 A = Ball3D(radius=3)\nout: A\n";
    let error = compile_program(source).unwrap_err().to_string();

    assert!(error.contains("missing field 'r'"));
}

#[test]
fn rejects_old_binding_syntax() {
    let source = "A : Obj3 = Ball3D(r=3)\nout: A\n";
    let error = compile_program(source).unwrap_err().to_string();

    assert!(error.contains("use 'type name = value'"));
}

#[test]
fn rejects_old_out_syntax() {
    let source = "Obj3 A = Ball3D(r=3)\nout: Obj3 = A\n";
    let error = compile_program(source).unwrap_err().to_string();

    assert!(error.contains("use 'out: value'"));
}

#[test]
fn emits_box_primitive() {
    let source = "Obj3 shape = Box2D(b=(2, 1))\nout: shape\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("struct ParamBox2D"));
    assert!(glsl.contains("float sdf0_Box2D(vec3 p, ParamBox2D params)"));
    assert!(glsl.contains("sdf0_Box2D(p, ParamBox2D(vec2(2.0, 1.0)))"));
}

#[test]
fn emits_simplex3d_primitive() {
    let source = "Obj3 shape = Simplex3D(size=2)\nout: shape\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("struct ParamSimplex3D"));
    assert!(glsl.contains("float sdf0_Simplex3D(vec3 p, ParamSimplex3D params)"));
    assert!(glsl.contains("q.x -= params.size;"));
    assert!(glsl.contains("sdf0_Simplex3D(p, ParamSimplex3D(2.0))"));
}

#[test]
fn emits_halfspace3d_primitive() {
    let source = "Obj3 shape = Halfspace3D(n=(0, 1, 0), h=2)\nout: shape\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("struct ParamHalfspace3D"));
    assert!(glsl.contains("float sdf0_Halfspace3D(vec3 p, ParamHalfspace3D params)"));
    assert!(glsl.contains("return dot(p, normalize(params.n)) + params.h;"));
    assert!(glsl.contains("sdf0_Halfspace3D(p, ParamHalfspace3D(vec3(0.0, 1.0, 0.0), 2.0))"));
}

#[test]
fn emits_torus3d_primitive() {
    let source = "Obj3 shape = Torus3D(major=3, minor=.5)\nout: shape\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("struct ParamTorus3D"));
    assert!(glsl.contains("float sdf0_Torus3D(vec3 p, ParamTorus3D params)"));
    assert!(glsl.contains("vec2 q = vec2(length(p.xz) - params.major, p.y);"));
    assert!(glsl.contains("sdf0_Torus3D(p, ParamTorus3D(3.0, 0.5))"));
}

#[test]
fn emits_segment_primitive() {
    let source = "Obj3 shape = Segment2D(a=(0, 0), b=(2, 1))\nout: shape\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("struct ParamSegment2D"));
    assert!(glsl.contains("float sdf0_Segment2D(vec3 p, ParamSegment2D params)"));
    assert!(glsl.contains("sdf0_Segment2D(p, ParamSegment2D(vec2(0.0, 0.0), vec2(2.0, 1.0)))"));
}

#[test]
fn emits_triangle_primitive() {
    let source = "Obj3 shape = Triangle2D(p0=(0, 0), p1=(2, 0), p2=(0, 2))\nout: shape\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("struct ParamTriangle2D"));
    assert!(glsl.contains("float sdf0_Triangle2D(vec3 p, ParamTriangle2D params)"));
    assert!(glsl.contains(
        "sdf0_Triangle2D(p, ParamTriangle2D(vec2(0.0, 0.0), vec2(2.0, 0.0), vec2(0.0, 2.0)))"
    ));
}

#[test]
fn emits_polygon_primitive() {
    let source = "Obj3 shape = Polygon2D(points=((0, 0), (2, 0), (2, 1), (0, 1)))\nout: shape\n";
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
    let source = "Obj3 shape = Point2D(at=(3, 4))\nout: shape\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("struct ParamPoint2D"));
    assert!(glsl.contains("float sdf0_Point2D(vec3 p, ParamPoint2D params)"));
    assert!(glsl.contains("sdf0_Point2D(p, ParamPoint2D(vec2(3.0, 4.0)))"));
}
