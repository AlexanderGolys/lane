use lane::{
    compile_program, known_builtin_objects, known_preregistered_objects, known_primitive,
    known_primitives, known_primitives_by_dimension, preregistered_object, PreregisteredObjectKind,
    ShapeDimension,
};

#[test]
fn lists_known_primitives_with_lane_types() {
    let primitives = known_primitives();
    let ball = primitives
        .iter()
        .find(|primitive| primitive.name == "Ball3D")
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
        object.kind == PreregisteredObjectKind::Function && object.name == "pow2"
    }));
    assert!(objects.iter().any(|object| {
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
        .any(|object| object.name == "pow2" && object.ty == "Hom(R, R)"));
    assert!(objects
        .iter()
        .any(|object| object.name == "cexp" && object.ty == "Hom(R2, R2)"));
    assert!(objects
        .iter()
        .any(|object| { object.name == "Union" && object.ty == "Hom(Obj3 × Obj3, Obj3)" }));
    assert!(objects.iter().any(|object| {
        object.name == "SmoothUnion" && object.ty == "Hom(R, Hom(Obj3 × Obj3, Obj3))"
    }));
    assert!(!objects.iter().any(|object| object.name == "sin"));
    assert!(!objects.iter().any(|object| object.name == "gradient"));
}

#[test]
fn supports_new_type_syntax_aliases() {
    let source = "provided R time\nprovided Hom(R3, R) density\nprovided End(R) loop\ngenerate Ball3D(r=1)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float scene_sdf(vec3 p, float time) {"));
}

#[test]
fn rejects_removed_constraint_type_alias() {
    let source = "provided C(R3) potential\ngenerate Ball3D(r=1)\n";
    let error = compile_program(source).unwrap_err().to_string();

    assert!(error.contains("unsupported type 'C(R3)'"));
}

#[test]
fn rejects_lowercase_builtin_type_names() {
    let source = "provided float time\ngenerate Ball3D(r=1)\n";
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
    let source = "func(Float -> Float) wobble = sin @ sin\ngenerate Ball3D(r=wobble(0))\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float dsl_wobble(float t) {"));
    assert!(glsl.contains("return sin(sin(t));"));
}

#[test]
fn supports_derivative_operator_in_function_bodies() {
    let source =
        "func(Float -> Float) slope = derivative(0.01)(sin)\ngenerate Ball3D(r=slope(0))\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float dsl_slope(float t) {"));
    assert!(glsl.contains("(sin((t + 0.01)) - sin((t - 0.01))) / (2.0 * 0.01)"));
}

#[test]
fn emits_support_for_custom_complex_functions() {
    let source =
        "Vec2 seed = (1, 0)\nfunc(Float -> Vec2) orbit = cexp(seed)\ngenerate Ball3D(r=1)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("vec2 cexp(vec2 z) {"));
    assert!(glsl.contains("vec2 seed = vec2(1.0, 0.0);"));
    assert!(glsl.contains("return cexp(seed);"));
}

#[test]
fn rejects_invalid_function_composition() {
    let source = "provided func(Float -> Vec3) center\nfunc(Float -> Float) wobble = sin @ center\ngenerate Ball3D(r=1)\n";
    let error = compile_program(source).unwrap_err().to_string();

    assert!(error.contains("cannot compose sin @ center"));
}

#[test]
fn supports_generate_alias() {
    let source = "gen Ball3D(r=1)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float scene_sdf(vec3 p) {"));
}

#[test]
fn supports_construct_alias() {
    let source = "const Obj3 shell = Ball3D(r=2)\ngenerate shell\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float sdf_shell(vec3 p) {"));
}

#[test]
fn supports_full_line_comments() {
    let source =
        "// input animation\nprovided Float time\n// object body\ngenerate Ball3D(r=1 + time)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float scene_sdf(vec3 p, float time) {"));
    assert!(glsl.contains("ParamBall3D((1.0 + time))"));
}

#[test]
fn supports_trailing_line_comments() {
    let source = "provided Float time // animation clock\nObj3 A = Ball3D(r=1) + (1, 0, 0) // translated sphere\ngenerate A // final object\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float scene_sdf(vec3 p, float time) {"));
    assert!(glsl.contains("sdf0_Ball3D((p - vec3(1.0, 0.0, 0.0)), ParamBall3D(1.0))"));
}

#[test]
fn emits_generated_object_helpers() {
    let source = "construct Obj3 shell = Ball3D(r=2) + (1, 0, 0)\ngenerate shell\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float sdf_shell(vec3 p) {"));
    assert!(glsl.contains("vec3 grad_sdf_shell(vec3 p) {"));
    assert!(glsl.contains("return sdf0_Ball3D((p - vec3(1.0, 0.0, 0.0)), ParamBall3D(2.0));"));
}

#[test]
fn generated_helpers_capture_scene_inputs_in_their_signatures() {
    let source = "provided Float time\nconstruct Obj3 shell = Ball3D(r=1 + time)\ngenerate shell\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float sdf_shell(vec3 p, float time) {"));
    assert!(glsl.contains("vec3 grad_sdf_shell(vec3 p, float time) {"));
    assert!(glsl.contains("float dx = sdf_shell(p + vec3(eps, 0.0, 0.0), time) - sdf_shell(p - vec3(eps, 0.0, 0.0), time);"));
}

#[test]
fn renames_generated_locals_on_name_conflicts() {
    let source = "provided Float p\nprovided Float eps\ngenerate Ball3D(r=eps) + (p, 0, 0)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float scene_sdf(vec3 p_r"));
    assert!(glsl.contains(", float p, float eps)"));
    assert!(glsl.contains("float eps_r"));
    assert!(glsl.contains("scene_sdf(p_r"));
    assert!(glsl.contains("vec3(eps_r"));
}

#[test]
fn plain_object_bindings_do_not_export_helpers() {
    let source = "Obj3 shell = Ball3D(r=2)\ngenerate shell\n";
    let glsl = compile_program(source).unwrap();

    assert!(!glsl.contains("float sdf_shell("));
    assert!(!glsl.contains("vec3 grad_sdf_shell("));
}

#[test]
fn reports_the_offending_token_for_expression_parse_errors() {
    let source = "generate Ball3D(r=1) + *\n";
    let error = compile_program(source).unwrap_err().to_string();

    assert!(error.contains("unexpected token '*' in expression"));
}

#[test]
fn emits_only_used_support_code() {
    let source = "Obj3 A = Ball3D(r=3)\ngenerate A\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("struct ParamBall3D"));
    assert!(glsl.contains("float sdf0_Ball3D"));
    assert!(!glsl.contains("op_smooth_union"));
    assert!(glsl.contains("vec3 scene_grad(vec3 p) {"));
}

#[test]
fn rejects_unknown_primitive_field() {
    let source = "Obj3 A = Ball3D(radius=3)\ngenerate A\n";
    let error = compile_program(source).unwrap_err().to_string();

    assert!(error.contains("missing field 'r'"));
}

#[test]
fn rejects_old_binding_syntax() {
    let source = "A : Obj3 = Ball3D(r=3)\ngenerate A\n";
    let error = compile_program(source).unwrap_err().to_string();

    assert!(error.contains("use 'type name = value'"));
}

#[test]
fn rejects_construct_on_non_object_bindings() {
    let source = "construct R radius = 2\ngenerate Ball3D(r=radius)\n";
    let error = compile_program(source).unwrap_err().to_string();

    assert!(error.contains("'construct' currently only supports Obj3 bindings"));
}

#[test]
fn rejects_old_out_syntax() {
    let source = "Obj3 A = Ball3D(r=3)\ngenerate Obj3 = A\n";
    let error = compile_program(source).unwrap_err().to_string();

    assert!(error.contains("use 'generate value'"));
}

#[test]
fn emits_box_primitive() {
    let source = "Obj3 shape = Box2D(a=2, b=1)\ngenerate shape\n";
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
    let source = "Obj3 shape = Box3D(a=2, b=1, c=3)\ngenerate shape\n";
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
    let source = "Obj3 shape = Box3D(2, 1, 3)\ngenerate shape\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("sdf0_Box3D(p, ParamBox3D(2.0, 1.0, 3.0))"));
}

#[test]
fn supports_negative_tuple_components() {
    let source = "Obj3 shape = Box3D(2, 1, 3) + (-1, -2, -3)\ngenerate shape\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("(p - vec3((-1.0), (-2.0), (-3.0)))"));
}

#[test]
fn supports_scientific_notation_literals() {
    let source = "Obj3 shape = Ball3D(r=1e-1) + (2e0, .5e+1, 3E-1)\ngenerate shape\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("ParamBall3D(0.1)"));
    assert!(glsl.contains("vec3(2.0, 5.0, 0.3)"));
}

#[test]
fn emits_primitive_with_positional_arguments() {
    let source = "Obj3 shape = Box2D(2, 1)\ngenerate shape\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("sdf0_Box2D((p).xy, ParamBox2D(2.0, 1.0))"));
}

#[test]
fn rejects_wrong_number_of_positional_primitive_arguments() {
    let source = "generate Ball3D()\n";
    let error = compile_program(source).unwrap_err().to_string();

    assert!(error.contains("primitive 'Ball3D' expects 1 field(s)"));
}

#[test]
fn emits_simplex3d_primitive() {
    let source = "Obj3 shape = Simplex3D(p0=(0, 0, 0), p1=(1, 0, 0), p2=(0, 1, 0), p3=(0, 0, 1))\ngenerate shape\n";
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
    let source = "Obj3 shape = Halfspace3D(n=(0, 1, 0), h=2)\ngenerate shape\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("struct ParamHalfspace3D"));
    assert!(glsl.contains("float sdf0_Halfspace3D(vec3 p, ParamHalfspace3D params)"));
    assert!(glsl.contains("return dot(p, normalize(params.n)) + params.h;"));
    assert!(glsl.contains("sdf0_Halfspace3D(p, ParamHalfspace3D(vec3(0.0, 1.0, 0.0), 2.0))"));
}

#[test]
fn emits_plane3d_primitive() {
    let source = "Obj3 shape = Plane3D(n=(0, 1, 0), origin=(0, 2, 0))\ngenerate shape\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("struct ParamPlane3D"));
    assert!(glsl.contains("float sdf0_Plane3D(vec3 p, ParamPlane3D params)"));
    assert!(glsl.contains("float h;"));
    assert!(glsl.contains("sdf0_Plane3D(p, ParamPlane3D(vec3(0.0, 1.0, 0.0), (-dot(normalize(vec3(0.0, 1.0, 0.0)), vec3(0.0, 2.0, 0.0)))))"));
}

#[test]
fn emits_line3d_primitive() {
    let source = "Obj3 shape = Line3D(x0=(0, 0, 0), dir=(2, 1, 3))\ngenerate shape\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("struct ParamLine3D"));
    assert!(glsl.contains("float sdf0_Line3D(vec3 p, ParamLine3D params)"));
    assert!(glsl.contains("vec3 direction = normalize(params.dir);"));
    assert!(glsl.contains("sdf0_Line3D(p, ParamLine3D(vec3(0.0, 0.0, 0.0), vec3(2.0, 1.0, 3.0)))"));
}

#[test]
fn emits_triangle3d_primitive() {
    let source =
        "Obj3 shape = Triangle3D(p1=(0, 0, 0), p2=(1, 0, 0), p3=(0, 1, 0))\ngenerate shape\n";
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
    let source = "Obj3 shape = Torus3D(major=3, minor=.5)\ngenerate shape\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("struct ParamTorus3D"));
    assert!(glsl.contains("float sdf0_Torus3D(vec3 p, ParamTorus3D params)"));
    assert!(glsl.contains("vec2 q = vec2(length(p.xz) - params.major, p.y);"));
    assert!(glsl.contains("sdf0_Torus3D(p, ParamTorus3D(3.0, 0.5))"));
}

#[test]
fn emits_segment_primitive() {
    let source = "Obj3 shape = Segment2D(a=(0, 0), b=(2, 1))\ngenerate shape\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("struct ParamSegment2D"));
    assert!(glsl.contains("float sdf0_Segment2D(vec2 p, ParamSegment2D params)"));
    assert!(glsl.contains("sdf0_Segment2D((p).xy, ParamSegment2D(vec2(0.0, 0.0), vec2(2.0, 1.0)))"));
}

#[test]
fn emits_segment3d_primitive() {
    let source = "Obj3 shape = Segment3D(a=(0, 0, 0), b=(2, 1, 3))\ngenerate shape\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("struct ParamSegment3D"));
    assert!(glsl.contains("float sdf0_Segment3D(vec3 p, ParamSegment3D params)"));
    assert!(glsl
        .contains("sdf0_Segment3D(p, ParamSegment3D(vec3(0.0, 0.0, 0.0), vec3(2.0, 1.0, 3.0)))"));
}

#[test]
fn emits_triangle_primitive() {
    let source = "Obj3 shape = Triangle2D(p0=(0, 0), p1=(2, 0), p2=(0, 2))\ngenerate shape\n";
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
        "Obj3 shape = Quad2D(p1=(0, 0), p2=(2, 0), p3=(2, 1), p4=(0, 1))\ngenerate shape\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("struct ParamQuad2D"));
    assert!(glsl.contains("float sdf0_Quad2D(vec2 p, ParamQuad2D params)"));
    assert!(glsl.contains(
        "sdf0_Quad2D((p).xy, ParamQuad2D(vec2(0.0, 0.0), vec2(2.0, 0.0), vec2(2.0, 1.0), vec2(0.0, 1.0)))"
    ));
}

#[test]
fn emits_quad3d_primitive() {
    let source = "Obj3 shape = Quad3D(p1=(0, 0, 0), p2=(1, 0, 0), p3=(1, 1, 0), p4=(0, 1, 0))\ngenerate shape\n";
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
        "Obj3 shape = Polygon2D(points=((0, 0), (2, 0), (2, 1), (0, 1)))\ngenerate shape\n";
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
    let source = "Obj3 shape = Point2D(at=(3, 4))\ngenerate shape\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("struct ParamPoint2D"));
    assert!(glsl.contains("float sdf0_Point2D(vec2 p, ParamPoint2D params)"));
    assert!(glsl.contains("sdf0_Point2D((p).xy, ParamPoint2D(vec2(3.0, 4.0)))"));
}

#[test]
fn emits_translation_action_from_addition_sugar() {
    let source = "generate Ball3D(r=1) + (1, 2, 3)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("sdf0_Ball3D((p - vec3(1.0, 2.0, 3.0)), ParamBall3D(1.0))"));
}

#[test]
fn emits_rotation_action_from_mat3_input() {
    let source = "provided Mat3 R\ngenerate R * Ball3D(r=1)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float scene_sdf(vec3 p, mat3 R) {"));
    assert!(glsl.contains("sdf0_Ball3D((transpose(R) * p), ParamBall3D(1.0))"));
}

#[test]
fn emits_mat3_helpers_and_uses_them_in_object_actions() {
    let source = "provided Float time\nfunc(Float -> Mat3) spin = ((1, 0, 0), (0, 1, 0), (0, 0, 1))\ngenerate spin(time) * Ball3D(r=1)\n";
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
        "Obj3 a = Ball3D(r=2)\nObj3 b = Ball3D(r=1) + (0.5, 0, 0)\ngenerate Difference(a, b)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float op_difference(float a, float b) {"));
    assert!(glsl.contains("return max(a, -b);"));
    assert!(glsl.contains("return op_difference("));
}

#[test]
fn emits_union_operator() {
    let source = "Obj3 a = Ball3D(r=2)\nObj3 b = Ball3D(r=1) + (0.5, 0, 0)\ngenerate Union(a, b)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float op_union(float a, float b) {"));
    assert!(glsl.contains("return min(a, b);"));
    assert!(glsl.contains("return op_union("));
}

#[test]
fn emits_associative_union_operator_with_four_args() {
    let source = "Obj3 a = Ball3D(r=4)\nObj3 b = Ball3D(r=3) + (1, 0, 0)\nObj3 c = Ball3D(r=2) + (2, 0, 0)\nObj3 d = Ball3D(r=1) + (3, 0, 0)\ngenerate Union(a, b, c, d)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("return op_union(op_union("));
    assert!(glsl.contains(", op_union("));
}

#[test]
fn emits_associative_union_operator_with_three_args() {
    let source = "Obj3 a = Ball3D(r=3)\nObj3 b = Ball3D(r=2) + (1, 0, 0)\nObj3 c = Ball3D(r=1) + (2, 0, 0)\ngenerate Union(a, b, c)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("return op_union(sdf0_Ball3D("));
    assert!(glsl.contains(", op_union("));
}

#[test]
fn emits_intersection_operator() {
    let source =
        "Obj3 a = Ball3D(r=2)\nObj3 b = Ball3D(r=1) + (0.5, 0, 0)\ngenerate Intersection(a, b)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float op_intersection(float a, float b) {"));
    assert!(glsl.contains("return max(a, b);"));
    assert!(glsl.contains("return op_intersection("));
}

#[test]
fn emits_xor_operator() {
    let source = "Obj3 a = Ball3D(r=2)\nObj3 b = Ball3D(r=1) + (0.5, 0, 0)\ngenerate Xor(a, b)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float op_xor(float a, float b) {"));
    assert!(glsl.contains("return max(min(a, b), -max(a, b));"));
    assert!(glsl.contains("return op_xor("));
}

#[test]
fn emits_smooth_union_operator() {
    let source =
        "Obj3 a = Ball3D(r=2)\nObj3 b = Ball3D(r=1) + (0.5, 0, 0)\ngenerate SmoothUnion(0.25)(a, b)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float op_smooth_union_min(float a, float b, float k) {"));
    assert!(glsl.contains("k *= 1.0 / (1.0 - sqrt(0.5));"));
    assert!(glsl.contains("return op_smooth_union("));
}

#[test]
fn emits_smooth_intersection_operator() {
    let source = "Obj3 a = Ball3D(r=2)\nObj3 b = Ball3D(r=1) + (0.5, 0, 0)\ngenerate SmoothIntersection(0.25)(a, b)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float op_smooth_intersection(float a, float b, float k) {"));
    assert!(glsl.contains("return op_smooth_intersection_max(a, b, k);"));
    assert!(glsl.contains("return op_smooth_intersection("));
}

#[test]
fn emits_smooth_difference_operator() {
    let source = "Obj3 a = Ball3D(r=2)\nObj3 b = Ball3D(r=1) + (0.5, 0, 0)\ngenerate SmoothDifference(0.25)(a, b)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float op_smooth_difference(float a, float b, float k) {"));
    assert!(glsl.contains("return op_smooth_difference_max(a, -b, k);"));
    assert!(glsl.contains("return op_smooth_difference("));
}

#[test]
fn emits_smooth_xor_operator() {
    let source =
        "Obj3 a = Ball3D(r=2)\nObj3 b = Ball3D(r=1) + (0.5, 0, 0)\ngenerate SmoothXor(0.25)(a, b)\n";
    let glsl = compile_program(source).unwrap();

    assert!(glsl.contains("float op_smooth_xor(float a, float b, float k) {"));
    assert!(glsl.contains(
        "return op_smooth_xor_max(op_smooth_xor_min(a, b, k), -op_smooth_xor_max(a, b, k), k);"
    ));
    assert!(glsl.contains("return op_smooth_xor("));
}

#[test]
fn emits_only_used_object_operator_support() {
    let source = "Obj3 a = Ball3D(r=2)\nObj3 b = Ball3D(r=1)\ngenerate Difference(a, b)\n";
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

#[test]
fn rejects_extra_arguments_for_non_associative_operator() {
    let source = "Obj3 a = Ball3D(r=3)\nObj3 b = Ball3D(r=2)\nObj3 c = Ball3D(r=1)\ngenerate Difference(a, b, c)\n";
    let err = compile_program(source).unwrap_err();

    assert_eq!(
        err.to_string(),
        "operator 'Difference' expects 2 argument(s), got 3"
    );
}
