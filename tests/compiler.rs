use lane::{
    compile_program, known_preregistered_objects, known_primitive, known_primitives,
    known_primitives_by_dimension, preregistered_object, PreregisteredObjectKind, ShapeDimension,
};

#[test]
fn lists_known_primitives_with_lane_types() {
    let primitives = known_primitives();
    let ball = primitives
        .iter()
        .find(|primitive| primitive.name == "Ball3D")
        .unwrap();
    let polygon = primitives
        .iter()
        .find(|primitive| primitive.name == "Polygon2D")
        .unwrap();

    assert_eq!(ball.dimension, ShapeDimension::D3);
    assert_eq!(ball.parameter_space, "ParamBall3D");
    assert_eq!(ball.fields[0].name, "r");
    assert_eq!(ball.fields[0].domain, "float");
    assert!(ball
        .type_body
        .as_deref()
        .unwrap()
        .contains("struct ParamBall3D"));
    assert!(ball
        .function_body
        .contains("float sdf0_Ball3D(vec3 p, ParamBall3D params)"));

    assert_eq!(polygon.dimension, ShapeDimension::D2);
    assert_eq!(polygon.parameter_space, "{ points: vec2 list }");
    assert_eq!(polygon.fields[0].name, "points");
    assert_eq!(polygon.fields[0].domain, "vec2 list");
    assert_eq!(polygon.type_body, None);
    assert!(polygon
        .function_body
        .contains("float sdf0_Polygon2D(vec2 p"));
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
        object.kind == PreregisteredObjectKind::Function && object.name == "op_smooth_union"
    }));
    assert!(objects.iter().any(|object| {
        object.kind == PreregisteredObjectKind::Function && object.name == "op_union"
    }));
    assert!(objects.iter().any(|object| {
        object.kind == PreregisteredObjectKind::Function && object.name == "op_intersection"
    }));
    assert!(objects.iter().any(|object| {
        object.kind == PreregisteredObjectKind::Function && object.name == "op_difference"
    }));
    assert!(objects.iter().any(|object| {
        object.kind == PreregisteredObjectKind::Function && object.name == "op_xor"
    }));
    assert!(objects.iter().any(|object| {
        object.kind == PreregisteredObjectKind::Function && object.name == "op_smooth_intersection"
    }));
    assert!(objects.iter().any(|object| {
        object.kind == PreregisteredObjectKind::Function && object.name == "op_smooth_difference"
    }));
    assert!(objects.iter().any(|object| {
        object.kind == PreregisteredObjectKind::Function && object.name == "op_smooth_xor"
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
fn supports_full_line_comments() {
    let source = "// input animation\nin: float time\n// object body\nout: Ball3D(r=1 + time)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float scene_sdf(vec3 p, float time) {"));
    assert!(glsl.contains("ParamBall3D((1.0 + time))"));
}

#[test]
fn supports_trailing_line_comments() {
    let source = "in: float time // animation clock\nObj3 A = Ball3D(r=1) + (1, 0, 0) // translated sphere\nout: A // final object\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float scene_sdf(vec3 p, float time) {"));
    assert!(glsl.contains("sdf0_Ball3D((p - vec3(1.0, 0.0, 0.0)), ParamBall3D(1.0))"));
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
    let source = "Obj3 shape = Box2D(a=2, b=1)\nout: shape\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("struct ParamBox2D"));
    assert!(glsl.contains("float a;"));
    assert!(glsl.contains("float b;"));
    assert!(glsl.contains("float sdf0_Box2D(vec2 p, ParamBox2D params)"));
    assert!(glsl.contains("vec2(params.a, params.b)"));
    assert!(glsl.contains("sdf0_Box2D((p).xy, ParamBox2D(2.0, 1.0))"));
}

#[test]
fn emits_simplex3d_primitive() {
    let source = "Obj3 shape = Simplex3D(p0=(0, 0, 0), p1=(1, 0, 0), p2=(0, 1, 0), p3=(0, 0, 1))\nout: shape\n";
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
    assert!(glsl.contains("float sdf0_Segment2D(vec2 p, ParamSegment2D params)"));
    assert!(glsl.contains("sdf0_Segment2D((p).xy, ParamSegment2D(vec2(0.0, 0.0), vec2(2.0, 1.0)))"));
}

#[test]
fn emits_triangle_primitive() {
    let source = "Obj3 shape = Triangle2D(p0=(0, 0), p1=(2, 0), p2=(0, 2))\nout: shape\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("struct ParamTriangle2D"));
    assert!(glsl.contains("float sdf0_Triangle2D(vec2 p, ParamTriangle2D params)"));
    assert!(glsl.contains(
        "sdf0_Triangle2D((p).xy, ParamTriangle2D(vec2(0.0, 0.0), vec2(2.0, 0.0), vec2(0.0, 2.0)))"
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
    assert!(glsl.contains("float sdf0_Point2D(vec2 p, ParamPoint2D params)"));
    assert!(glsl.contains("sdf0_Point2D((p).xy, ParamPoint2D(vec2(3.0, 4.0)))"));
}

#[test]
fn emits_translation_action_from_addition_sugar() {
    let source = "out: Ball3D(r=1) + (1, 2, 3)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("sdf0_Ball3D((p - vec3(1.0, 2.0, 3.0)), ParamBall3D(1.0))"));
}

#[test]
fn emits_rotation_action_from_mat3_input() {
    let source = "in: mat3 R\nout: R * Ball3D(r=1)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float scene_sdf(vec3 p, mat3 R) {"));
    assert!(glsl.contains("sdf0_Ball3D((transpose(R) * p), ParamBall3D(1.0))"));
}

#[test]
fn emits_mat3_helpers_and_uses_them_in_object_actions() {
    let source = "in: float time\nfunc(float -> mat3) spin = ((1, 0, 0), (0, 1, 0), (0, 0, 1))\nout: spin(time) * Ball3D(r=1)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("mat3 dsl_spin(float t) {"));
    assert!(glsl.contains(
        "return transpose(mat3(vec3(1.0, 0.0, 0.0), vec3(0.0, 1.0, 0.0), vec3(0.0, 0.0, 1.0)));"
    ));
    assert!(glsl.contains("sdf0_Ball3D((transpose(dsl_spin(time)) * p), ParamBall3D(1.0))"));
}

#[test]
fn emits_difference_operator() {
    let source =
        "Obj3 a = Ball3D(r=2)\nObj3 b = Ball3D(r=1) + (0.5, 0, 0)\nout: Difference(a, b)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float op_difference(float a, float b) {"));
    assert!(glsl.contains("return max(a, -b);"));
    assert!(glsl.contains("return op_difference(obj_a, obj_b);"));
}

#[test]
fn emits_union_operator() {
    let source = "Obj3 a = Ball3D(r=2)\nObj3 b = Ball3D(r=1) + (0.5, 0, 0)\nout: Union(a, b)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float op_union(float a, float b) {"));
    assert!(glsl.contains("return min(a, b);"));
    assert!(glsl.contains("return op_union(obj_a, obj_b);"));
}

#[test]
fn emits_intersection_operator() {
    let source =
        "Obj3 a = Ball3D(r=2)\nObj3 b = Ball3D(r=1) + (0.5, 0, 0)\nout: Intersection(a, b)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float op_intersection(float a, float b) {"));
    assert!(glsl.contains("return max(a, b);"));
    assert!(glsl.contains("return op_intersection(obj_a, obj_b);"));
}

#[test]
fn emits_xor_operator() {
    let source = "Obj3 a = Ball3D(r=2)\nObj3 b = Ball3D(r=1) + (0.5, 0, 0)\nout: Xor(a, b)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float op_xor(float a, float b) {"));
    assert!(glsl.contains("return max(min(a, b), -max(a, b));"));
    assert!(glsl.contains("return op_xor(obj_a, obj_b);"));
}

#[test]
fn emits_smooth_union_operator() {
    let source =
        "Obj3 a = Ball3D(r=2)\nObj3 b = Ball3D(r=1) + (0.5, 0, 0)\nout: SmoothUnion(0.25)(a, b)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float op_smooth_union_min(float a, float b, float k) {"));
    assert!(glsl.contains("k *= 1.0 / (1.0 - sqrt(0.5));"));
    assert!(glsl.contains("return op_smooth_union(obj_a, obj_b, 0.25);"));
}

#[test]
fn emits_smooth_intersection_operator() {
    let source = "Obj3 a = Ball3D(r=2)\nObj3 b = Ball3D(r=1) + (0.5, 0, 0)\nout: SmoothIntersection(0.25)(a, b)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float op_smooth_intersection(float a, float b, float k) {"));
    assert!(glsl.contains("return op_smooth_intersection_max(a, b, k);"));
    assert!(glsl.contains("return op_smooth_intersection(obj_a, obj_b, 0.25);"));
}

#[test]
fn emits_smooth_difference_operator() {
    let source = "Obj3 a = Ball3D(r=2)\nObj3 b = Ball3D(r=1) + (0.5, 0, 0)\nout: SmoothDifference(0.25)(a, b)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float op_smooth_difference(float a, float b, float k) {"));
    assert!(glsl.contains("return op_smooth_difference_max(a, -b, k);"));
    assert!(glsl.contains("return op_smooth_difference(obj_a, obj_b, 0.25);"));
}

#[test]
fn emits_smooth_xor_operator() {
    let source =
        "Obj3 a = Ball3D(r=2)\nObj3 b = Ball3D(r=1) + (0.5, 0, 0)\nout: SmoothXor(0.25)(a, b)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float op_smooth_xor(float a, float b, float k) {"));
    assert!(glsl.contains(
        "return op_smooth_xor_max(op_smooth_xor_min(a, b, k), -op_smooth_xor_max(a, b, k), k);"
    ));
    assert!(glsl.contains("return op_smooth_xor(obj_a, obj_b, 0.25);"));
}

#[test]
fn emits_only_used_object_operator_support() {
    let source = "Obj3 a = Ball3D(r=2)\nObj3 b = Ball3D(r=1)\nout: Difference(a, b)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("op_difference"));
    assert!(!glsl.contains("op_smooth_union"));
    assert!(!glsl.contains("op_union"));
    assert!(!glsl.contains("op_intersection"));
    assert!(!glsl.contains("op_xor"));
    assert!(!glsl.contains("op_smooth_intersection"));
    assert!(!glsl.contains("op_smooth_difference"));
    assert!(!glsl.contains("op_smooth_xor"));
}
