use super::*;

impl Default for Registry {
    fn default() -> Self {
        let primitives = HashMap::from([
            (
                "Ball3D",
                PrimitiveDef {
                    kind: PrimitiveKind::ParamStruct("ParamBall3D"),
                    fields: vec![PrimitiveFieldDef {
                        name: "r",
                        kind: PrimitiveFieldKind::Value(Type::Float),
                    }],
                    support_glsl: "struct ParamBall3D {\n    float r;\n};\n\nfloat sdf0_Ball3D(vec3 p, ParamBall3D params) {\n    return length(p) - params.r;\n}",
                },
            ),
            (
                "Box3D",
                PrimitiveDef {
                    kind: PrimitiveKind::ParamStruct("ParamBox3D"),
                    fields: vec![
                        PrimitiveFieldDef {
                            name: "a",
                            kind: PrimitiveFieldKind::Value(Type::Float),
                        },
                        PrimitiveFieldDef {
                            name: "b",
                            kind: PrimitiveFieldKind::Value(Type::Float),
                        },
                        PrimitiveFieldDef {
                            name: "c",
                            kind: PrimitiveFieldKind::Value(Type::Float),
                        },
                    ],
                    support_glsl: "struct ParamBox3D {\n    float a;\n    float b;\n    float c;\n};\n\nfloat sdf0_Box3D(vec3 p, ParamBox3D params) {\n    vec3 d = abs(p) - vec3(params.a, params.b, params.c);\n    return length(max(d, 0.0)) + min(max(d.x, max(d.y, d.z)), 0.0);\n}",
                },
            ),
            (
                "Triangle3D",
                PrimitiveDef {
                    kind: PrimitiveKind::ParamStruct("ParamTriangle3D"),
                    fields: vec![
                        PrimitiveFieldDef {
                            name: "p1",
                            kind: PrimitiveFieldKind::Value(Type::Vec3),
                        },
                        PrimitiveFieldDef {
                            name: "p2",
                            kind: PrimitiveFieldKind::Value(Type::Vec3),
                        },
                        PrimitiveFieldDef {
                            name: "p3",
                            kind: PrimitiveFieldKind::Value(Type::Vec3),
                        },
                    ],
                    support_glsl: "struct ParamTriangle3D {\n    vec3 p1;\n    vec3 p2;\n    vec3 p3;\n};\n\nfloat sdf0_Triangle3D(vec3 p, ParamTriangle3D params) {\n    vec3 ba = params.p2 - params.p1;\n    vec3 pa = p - params.p1;\n    vec3 cb = params.p3 - params.p2;\n    vec3 pb = p - params.p2;\n    vec3 ac = params.p1 - params.p3;\n    vec3 pc = p - params.p3;\n    vec3 nor = cross(ba, ac);\n    return sqrt((sign(dot(cross(ba, nor), pa)) + sign(dot(cross(cb, nor), pb)) + sign(dot(cross(ac, nor), pc)) < 2.0) ? min(min(dot((ba * clamp(dot(ba, pa) / dot(ba, ba), 0.0, 1.0)) - pa, (ba * clamp(dot(ba, pa) / dot(ba, ba), 0.0, 1.0)) - pa), dot((cb * clamp(dot(cb, pb) / dot(cb, cb), 0.0, 1.0)) - pb, (cb * clamp(dot(cb, pb) / dot(cb, cb), 0.0, 1.0)) - pb)), dot((ac * clamp(dot(ac, pc) / dot(ac, ac), 0.0, 1.0)) - pc, (ac * clamp(dot(ac, pc) / dot(ac, ac), 0.0, 1.0)) - pc)) : dot(nor, pa) * dot(nor, pa) / dot(nor, nor));\n}",
                },
            ),
            (
                "Simplex3D",
                PrimitiveDef {
                    kind: PrimitiveKind::ParamStruct("ParamSimplex3D"),
                    fields: vec![
                        PrimitiveFieldDef {
                            name: "p0",
                            kind: PrimitiveFieldKind::Value(Type::Vec3),
                        },
                        PrimitiveFieldDef {
                            name: "p1",
                            kind: PrimitiveFieldKind::Value(Type::Vec3),
                        },
                        PrimitiveFieldDef {
                            name: "p2",
                            kind: PrimitiveFieldKind::Value(Type::Vec3),
                        },
                        PrimitiveFieldDef {
                            name: "p3",
                            kind: PrimitiveFieldKind::Value(Type::Vec3),
                        },
                    ],
                    support_glsl: "struct ParamSimplex3D {\n    vec3 p0;\n    vec3 p1;\n    vec3 p2;\n    vec3 p3;\n};\n\nfloat sdf0_Simplex3D(vec3 p, ParamSimplex3D params) {\n    vec3 vertices[4] = vec3[4](params.p0, params.p1, params.p2, params.p3);\n    ivec3 faces[4] = ivec3[4](ivec3(0, 1, 2), ivec3(0, 3, 1), ivec3(0, 2, 3), ivec3(1, 3, 2));\n    float max_plane = -1e30;\n    for (int i = 0; i < 4; i++) {\n        ivec3 face = faces[i];\n        vec3 a = vertices[face.x];\n        vec3 b = vertices[face.y];\n        vec3 c = vertices[face.z];\n        vec3 n = normalize(cross(b - a, c - a));\n        int opposite = 6 - face.x - face.y - face.z;\n        if (dot(n, vertices[opposite] - a) > 0.0) {\n            n = -n;\n        }\n        max_plane = max(max_plane, dot(n, p - a));\n    }\n    return max_plane;\n}",
                },
            ),
            (
                "Plane3D",
                PrimitiveDef {
                    kind: PrimitiveKind::ParamStruct("ParamPlane3D"),
                    fields: vec![
                        PrimitiveFieldDef {
                            name: "n",
                            kind: PrimitiveFieldKind::Value(Type::Vec3),
                        },
                        PrimitiveFieldDef {
                            name: "origin",
                            kind: PrimitiveFieldKind::Value(Type::Vec3),
                        },
                    ],
                    support_glsl: "struct ParamPlane3D {\n    vec3 n;\n    float h;\n};\n\nfloat sdf0_Plane3D(vec3 p, ParamPlane3D params) {\n    return dot(normalize(params.n), p) + params.h;\n}",
                },
            ),
            (
                "Halfspace3D",
                PrimitiveDef {
                    kind: PrimitiveKind::ParamStruct("ParamHalfspace3D"),
                    fields: vec![
                        PrimitiveFieldDef {
                            name: "n",
                            kind: PrimitiveFieldKind::Value(Type::Vec3),
                        },
                        PrimitiveFieldDef {
                            name: "h",
                            kind: PrimitiveFieldKind::Value(Type::Float),
                        },
                    ],
                    support_glsl: "struct ParamHalfspace3D {\n    vec3 n;\n    float h;\n};\n\nfloat sdf0_Halfspace3D(vec3 p, ParamHalfspace3D params) {\n    return dot(p, normalize(params.n)) + params.h;\n}",
                },
            ),
            (
                "Line3D",
                PrimitiveDef {
                    kind: PrimitiveKind::ParamStruct("ParamLine3D"),
                    fields: vec![
                        PrimitiveFieldDef {
                            name: "x0",
                            kind: PrimitiveFieldKind::Value(Type::Vec3),
                        },
                        PrimitiveFieldDef {
                            name: "dir",
                            kind: PrimitiveFieldKind::Value(Type::Vec3),
                        },
                    ],
                    support_glsl: "struct ParamLine3D {\n    vec3 x0;\n    vec3 dir;\n};\n\nfloat sdf0_Line3D(vec3 p, ParamLine3D params) {\n    vec3 delta = p - params.x0;\n    vec3 direction = normalize(params.dir);\n    return length(delta - (direction * dot(delta, direction)));\n}",
                },
            ),
            (
                "Segment3D",
                PrimitiveDef {
                    kind: PrimitiveKind::ParamStruct("ParamSegment3D"),
                    fields: vec![
                        PrimitiveFieldDef {
                            name: "a",
                            kind: PrimitiveFieldKind::Value(Type::Vec3),
                        },
                        PrimitiveFieldDef {
                            name: "b",
                            kind: PrimitiveFieldKind::Value(Type::Vec3),
                        },
                    ],
                    support_glsl: "struct ParamSegment3D {\n    vec3 a;\n    vec3 b;\n};\n\nfloat sdf0_Segment3D(vec3 p, ParamSegment3D params) {\n    vec3 pa = p - params.a;\n    vec3 ba = params.b - params.a;\n    float h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);\n    return length(pa - (ba * h));\n}",
                },
            ),
            (
                "Torus3D",
                PrimitiveDef {
                    kind: PrimitiveKind::ParamStruct("ParamTorus3D"),
                    fields: vec![
                        PrimitiveFieldDef {
                            name: "major",
                            kind: PrimitiveFieldKind::Value(Type::Float),
                        },
                        PrimitiveFieldDef {
                            name: "minor",
                            kind: PrimitiveFieldKind::Value(Type::Float),
                        },
                    ],
                    support_glsl: "struct ParamTorus3D {\n    float major;\n    float minor;\n};\n\nfloat sdf0_Torus3D(vec3 p, ParamTorus3D params) {\n    vec2 q = vec2(length(p.xz) - params.major, p.y);\n    return length(q) - params.minor;\n}",
                },
            ),
            (
                "Ball2D",
                PrimitiveDef {
                    kind: PrimitiveKind::ParamStruct("ParamBall2D"),
                    fields: vec![PrimitiveFieldDef {
                        name: "r",
                        kind: PrimitiveFieldKind::Value(Type::Float),
                    }],
                    support_glsl: "struct ParamBall2D {\n    float r;\n};\n\nfloat sdf0_Ball2D(vec2 p, ParamBall2D params) {\n    return length(p) - params.r;\n}",
                },
            ),
            (
                "Quad2D",
                PrimitiveDef {
                    kind: PrimitiveKind::ParamStruct("ParamQuad2D"),
                    fields: vec![
                        PrimitiveFieldDef {
                            name: "p1",
                            kind: PrimitiveFieldKind::Value(Type::Vec2),
                        },
                        PrimitiveFieldDef {
                            name: "p2",
                            kind: PrimitiveFieldKind::Value(Type::Vec2),
                        },
                        PrimitiveFieldDef {
                            name: "p3",
                            kind: PrimitiveFieldKind::Value(Type::Vec2),
                        },
                        PrimitiveFieldDef {
                            name: "p4",
                            kind: PrimitiveFieldKind::Value(Type::Vec2),
                        },
                    ],
                    support_glsl: "struct ParamQuad2D {\n    vec2 p1;\n    vec2 p2;\n    vec2 p3;\n    vec2 p4;\n};\n\nfloat sdf0_Quad2D(vec2 p, ParamQuad2D params) {\n    vec2 vertices[4] = vec2[4](params.p1, params.p2, params.p3, params.p4);\n    float d = dot(p - vertices[0], p - vertices[0]);\n    float s = 1.0;\n    for (int i = 0, j = 3; i < 4; j = i, i++) {\n        vec2 e = vertices[j] - vertices[i];\n        vec2 w = p - vertices[i];\n        vec2 b = w - (e * clamp(dot(w, e) / dot(e, e), 0.0, 1.0));\n        d = min(d, dot(b, b));\n        bvec3 c = bvec3(p.y >= vertices[i].y, p.y < vertices[j].y, (e.x * w.y) > (e.y * w.x));\n        if (all(c) || all(not(c))) {\n            s *= -1.0;\n        }\n    }\n    return s * sqrt(d);\n}",
                },
            ),
            (
                "Box2D",
                PrimitiveDef {
                    kind: PrimitiveKind::ParamStruct("ParamBox2D"),
                    fields: vec![
                        PrimitiveFieldDef {
                            name: "a",
                            kind: PrimitiveFieldKind::Value(Type::Float),
                        },
                        PrimitiveFieldDef {
                            name: "b",
                            kind: PrimitiveFieldKind::Value(Type::Float),
                        },
                    ],
                    support_glsl: "struct ParamBox2D {\n    float a;\n    float b;\n};\n\nfloat sdf0_Box2D(vec2 p, ParamBox2D params) {\n    vec2 d = abs(p) - vec2(params.a, params.b);\n    return length(max(d, 0.0)) + min(max(d.x, d.y), 0.0);\n}",
                },
            ),
            (
                "Segment2D",
                PrimitiveDef {
                    kind: PrimitiveKind::ParamStruct("ParamSegment2D"),
                    fields: vec![
                        PrimitiveFieldDef {
                            name: "a",
                            kind: PrimitiveFieldKind::Value(Type::Vec2),
                        },
                        PrimitiveFieldDef {
                            name: "b",
                            kind: PrimitiveFieldKind::Value(Type::Vec2),
                        },
                    ],
                    support_glsl: "struct ParamSegment2D {\n    vec2 a;\n    vec2 b;\n};\n\nfloat sdf0_Segment2D(vec2 p, ParamSegment2D params) {\n    vec2 pa = p - params.a;\n    vec2 ba = params.b - params.a;\n    float h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);\n    return length(pa - (ba * h));\n}",
                },
            ),
            (
                "Triangle2D",
                PrimitiveDef {
                    kind: PrimitiveKind::ParamStruct("ParamTriangle2D"),
                    fields: vec![
                        PrimitiveFieldDef {
                            name: "p0",
                            kind: PrimitiveFieldKind::Value(Type::Vec2),
                        },
                        PrimitiveFieldDef {
                            name: "p1",
                            kind: PrimitiveFieldKind::Value(Type::Vec2),
                        },
                        PrimitiveFieldDef {
                            name: "p2",
                            kind: PrimitiveFieldKind::Value(Type::Vec2),
                        },
                    ],
                    support_glsl: "struct ParamTriangle2D {\n    vec2 p0;\n    vec2 p1;\n    vec2 p2;\n};\n\nfloat sdf0_Triangle2D(vec2 p, ParamTriangle2D params) {\n    vec2 e0 = params.p1 - params.p0;\n    vec2 e1 = params.p2 - params.p1;\n    vec2 e2 = params.p0 - params.p2;\n    vec2 v0 = p - params.p0;\n    vec2 v1 = p - params.p1;\n    vec2 v2 = p - params.p2;\n    vec2 pq0 = v0 - (e0 * clamp(dot(v0, e0) / dot(e0, e0), 0.0, 1.0));\n    vec2 pq1 = v1 - (e1 * clamp(dot(v1, e1) / dot(e1, e1), 0.0, 1.0));\n    vec2 pq2 = v2 - (e2 * clamp(dot(v2, e2) / dot(e2, e2), 0.0, 1.0));\n    float s = sign((e0.x * e2.y) - (e0.y * e2.x));\n    vec2 d = min(min(vec2(dot(pq0, pq0), s * ((v0.x * e0.y) - (v0.y * e0.x))), vec2(dot(pq1, pq1), s * ((v1.x * e1.y) - (v1.y * e1.x)))), vec2(dot(pq2, pq2), s * ((v2.x * e2.y) - (v2.y * e2.x))));\n    return -sqrt(d.x) * sign(d.y);\n}",
                },
            ),
            (
                "Quad3D",
                PrimitiveDef {
                    kind: PrimitiveKind::ParamStruct("ParamQuad3D"),
                    fields: vec![
                        PrimitiveFieldDef {
                            name: "p1",
                            kind: PrimitiveFieldKind::Value(Type::Vec3),
                        },
                        PrimitiveFieldDef {
                            name: "p2",
                            kind: PrimitiveFieldKind::Value(Type::Vec3),
                        },
                        PrimitiveFieldDef {
                            name: "p3",
                            kind: PrimitiveFieldKind::Value(Type::Vec3),
                        },
                        PrimitiveFieldDef {
                            name: "p4",
                            kind: PrimitiveFieldKind::Value(Type::Vec3),
                        },
                    ],
                    support_glsl: "struct ParamQuad3D {\n    vec3 p1;\n    vec3 p2;\n    vec3 p3;\n    vec3 p4;\n};\n\nfloat sdf0_Quad3D(vec3 p, ParamQuad3D params) {\n    vec3 ba = params.p2 - params.p1;\n    vec3 pa = p - params.p1;\n    vec3 cb = params.p3 - params.p2;\n    vec3 pb = p - params.p2;\n    vec3 dc = params.p4 - params.p3;\n    vec3 pc = p - params.p3;\n    vec3 ad = params.p1 - params.p4;\n    vec3 pd = p - params.p4;\n    vec3 nor = cross(ba, ad);\n    return sqrt((sign(dot(cross(ba, nor), pa)) + sign(dot(cross(cb, nor), pb)) + sign(dot(cross(dc, nor), pc)) + sign(dot(cross(ad, nor), pd)) < 3.0) ? min(min(min(dot((ba * clamp(dot(ba, pa) / dot(ba, ba), 0.0, 1.0)) - pa, (ba * clamp(dot(ba, pa) / dot(ba, ba), 0.0, 1.0)) - pa), dot((cb * clamp(dot(cb, pb) / dot(cb, cb), 0.0, 1.0)) - pb, (cb * clamp(dot(cb, pb) / dot(cb, cb), 0.0, 1.0)) - pb)), dot((dc * clamp(dot(dc, pc) / dot(dc, dc), 0.0, 1.0)) - pc, (dc * clamp(dot(dc, pc) / dot(dc, dc), 0.0, 1.0)) - pc)), dot((ad * clamp(dot(ad, pd) / dot(ad, ad), 0.0, 1.0)) - pd, (ad * clamp(dot(ad, pd) / dot(ad, ad), 0.0, 1.0)) - pd)) : dot(nor, pa) * dot(nor, pa) / dot(nor, nor));\n}",
                },
            ),
            (
                "Polygon2D",
                PrimitiveDef {
                    kind: PrimitiveKind::Polygon2D,
                    fields: vec![PrimitiveFieldDef {
                        name: "points",
                        kind: PrimitiveFieldKind::Vec2List,
                    }],
                    support_glsl: "const int POLYGON2D_MAX_VERTICES = 16;\n\nfloat sdf0_Polygon2D(vec2 p, vec2 vertices[POLYGON2D_MAX_VERTICES], int count) {\n    float d = dot(p - vertices[0], p - vertices[0]);\n    float s = 1.0;\n    for (int i = 0, j = count - 1; i < count; j = i, i++) {\n        vec2 e = vertices[j] - vertices[i];\n        vec2 w = p - vertices[i];\n        vec2 b = w - (e * clamp(dot(w, e) / dot(e, e), 0.0, 1.0));\n        d = min(d, dot(b, b));\n        bvec3 c = bvec3(p.y >= vertices[i].y, p.y < vertices[j].y, (e.x * w.y) > (e.y * w.x));\n        if (all(c) || all(not(c))) {\n            s *= -1.0;\n        }\n    }\n    return s * sqrt(d);\n}",
                },
            ),
            (
                "Point2D",
                PrimitiveDef {
                    kind: PrimitiveKind::ParamStruct("ParamPoint2D"),
                    fields: vec![PrimitiveFieldDef {
                        name: "at",
                        kind: PrimitiveFieldKind::Value(Type::Vec2),
                    }],
                    support_glsl: "struct ParamPoint2D {\n    vec2 at;\n};\n\nfloat sdf0_Point2D(vec2 p, ParamPoint2D params) {\n    return length(p - params.at);\n}",
                },
            ),
        ]);

        let object_ops = HashMap::from([
            (
                "SmoothUnion",
                ObjectOpDef {
                    name: "SmoothUnion",
                    value_arg_types: vec![Type::Float],
                    object_arg_count: 2,
                    associative_binary: false,
                    glsl_name: "op_smooth_union",
                    support_glsl: "float op_smooth_union_min(float a, float b, float k) {\n    k *= 1.0 / (1.0 - sqrt(0.5));\n    float h = max(k - abs(a - b), 0.0) / k;\n    return min(a, b) - (k * 0.5 * (1.0 + h - sqrt(1.0 - (h * (h - 2.0)))));\n}\n\nfloat op_smooth_union(float a, float b, float k) {\n    return op_smooth_union_min(a, b, k);\n}",
                },
            ),
            (
                "Union",
                ObjectOpDef {
                    name: "Union",
                    value_arg_types: vec![],
                    object_arg_count: 2,
                    associative_binary: true,
                    glsl_name: "op_union",
                    support_glsl: "float op_union(float a, float b) {\n    return min(a, b);\n}",
                },
            ),
            (
                "Intersection",
                ObjectOpDef {
                    name: "Intersection",
                    value_arg_types: vec![],
                    object_arg_count: 2,
                    associative_binary: true,
                    glsl_name: "op_intersection",
                    support_glsl: "float op_intersection(float a, float b) {\n    return max(a, b);\n}",
                },
            ),
            (
                "Difference",
                ObjectOpDef {
                    name: "Difference",
                    value_arg_types: vec![],
                    object_arg_count: 2,
                    associative_binary: false,
                    glsl_name: "op_difference",
                    support_glsl: "float op_difference(float a, float b) {\n    return max(a, -b);\n}",
                },
            ),
            (
                "Xor",
                ObjectOpDef {
                    name: "Xor",
                    value_arg_types: vec![],
                    object_arg_count: 2,
                    associative_binary: true,
                    glsl_name: "op_xor",
                    support_glsl: "float op_xor(float a, float b) {\n    return max(min(a, b), -max(a, b));\n}",
                },
            ),
            (
                "SmoothIntersection",
                ObjectOpDef {
                    name: "SmoothIntersection",
                    value_arg_types: vec![Type::Float],
                    object_arg_count: 2,
                    associative_binary: false,
                    glsl_name: "op_smooth_intersection",
                    support_glsl: "float op_smooth_intersection_min(float a, float b, float k) {\n    k *= 1.0 / (1.0 - sqrt(0.5));\n    float h = max(k - abs(a - b), 0.0) / k;\n    return min(a, b) - (k * 0.5 * (1.0 + h - sqrt(1.0 - (h * (h - 2.0)))));\n}\n\nfloat op_smooth_intersection_max(float a, float b, float k) {\n    return -op_smooth_intersection_min(-a, -b, k);\n}\n\nfloat op_smooth_intersection(float a, float b, float k) {\n    return op_smooth_intersection_max(a, b, k);\n}",
                },
            ),
            (
                "SmoothDifference",
                ObjectOpDef {
                    name: "SmoothDifference",
                    value_arg_types: vec![Type::Float],
                    object_arg_count: 2,
                    associative_binary: false,
                    glsl_name: "op_smooth_difference",
                    support_glsl: "float op_smooth_difference_min(float a, float b, float k) {\n    k *= 1.0 / (1.0 - sqrt(0.5));\n    float h = max(k - abs(a - b), 0.0) / k;\n    return min(a, b) - (k * 0.5 * (1.0 + h - sqrt(1.0 - (h * (h - 2.0)))));\n}\n\nfloat op_smooth_difference_max(float a, float b, float k) {\n    return -op_smooth_difference_min(-a, -b, k);\n}\n\nfloat op_smooth_difference(float a, float b, float k) {\n    return op_smooth_difference_max(a, -b, k);\n}",
                },
            ),
            (
                "SmoothXor",
                ObjectOpDef {
                    name: "SmoothXor",
                    value_arg_types: vec![Type::Float],
                    object_arg_count: 2,
                    associative_binary: false,
                    glsl_name: "op_smooth_xor",
                    support_glsl: "float op_smooth_xor_min(float a, float b, float k) {\n    k *= 1.0 / (1.0 - sqrt(0.5));\n    float h = max(k - abs(a - b), 0.0) / k;\n    return min(a, b) - (k * 0.5 * (1.0 + h - sqrt(1.0 - (h * (h - 2.0)))));\n}\n\nfloat op_smooth_xor_max(float a, float b, float k) {\n    return -op_smooth_xor_min(-a, -b, k);\n}\n\nfloat op_smooth_xor(float a, float b, float k) {\n    return op_smooth_xor_max(op_smooth_xor_min(a, b, k), -op_smooth_xor_max(a, b, k), k);\n}",
                },
            ),
            (
                "Revolution",
                ObjectOpDef {
                    name: "Revolution",
                    value_arg_types: vec![Type::Float],
                    object_arg_count: 1,
                    associative_binary: false,
                    glsl_name: "op_revolution",
                    support_glsl: "vec3 op_revolution_point(vec3 p, float offset) {\n    return vec3(length(p.xz) - offset, p.y, 0.0);\n}",
                },
            ),
            (
                "Extrusion",
                ObjectOpDef {
                    name: "Extrusion",
                    value_arg_types: vec![Type::Float],
                    object_arg_count: 1,
                    associative_binary: false,
                    glsl_name: "op_extrusion",
                    support_glsl: "float op_extrusion(float base_distance, float z, float height) {\n    vec2 w = vec2(base_distance, abs(z) - height);\n    return min(max(w.x, w.y), 0.0) + length(max(w, 0.0));\n}",
                },
            ),
        ]);

        let value_funcs = HashMap::from([
            (
                "ops_C",
                ValueFuncDef {
                    ty: Type::Func(Box::new(Type::Complex), Box::new(Type::Complex)),
                    support_glsl: Some(
                        "vec2 mult_C(vec2 a, vec2 b) {\n    return vec2((a.x * b.x) - (a.y * b.y), (a.x * b.y) + (a.y * b.x));\n}\n\nvec2 div_C(vec2 a, vec2 b) {\n    return mult_C(a, vec2(b.x, -b.y) / dot(b, b));\n}",
                    ),
                    listed: false,
                },
            ),
            (
                "ops_H",
                ValueFuncDef {
                    ty: Type::Func(Box::new(Type::Quat), Box::new(Type::Quat)),
                    support_glsl: Some(
                        "vec4 mult_H(vec4 a, vec4 b) {\n    return vec4(\n        a.x * b.x - a.y * b.y - a.z * b.z - a.w * b.w,\n        a.x * b.y + a.y * b.x + a.z * b.w - a.w * b.z,\n        a.x * b.z - a.y * b.w + a.z * b.x + a.w * b.y,\n        a.x * b.w + a.y * b.z - a.z * b.y + a.w * b.x\n    );\n}\n\nvec4 inv_H(vec4 q) {\n    return vec4(q.x, -q.y, -q.z, -q.w) / dot(q, q);\n}\n\nvec4 div_H(vec4 a, vec4 b) {\n    return mult_H(a, inv_H(b));\n}",
                    ),
                    listed: false,
                },
            ),
            (
                "sin",
                ValueFuncDef {
                    ty: Type::func(Type::Float, Type::Float),
                    support_glsl: None,
                    listed: false,
                },
            ),
            (
                "cos",
                ValueFuncDef {
                    ty: Type::func(Type::Float, Type::Float),
                    support_glsl: None,
                    listed: false,
                },
            ),
            (
                "pow2",
                ValueFuncDef {
                    ty: Type::func(Type::Float, Type::Float),
                    support_glsl: Some(
                        "float pow2(float x) {\n    return x * x;\n}",
                    ),
                    listed: true,
                },
            ),
            (
                "cinv",
                ValueFuncDef {
                    ty: Type::func(Type::Complex, Type::Complex),
                    support_glsl: Some(
                        "vec2 cinv(vec2 z) {\n    return vec2(z.x, -z.y) / dot(z, z);\n}",
                    ),
                    listed: true,
                },
            ),
            (
                "cexp",
                ValueFuncDef {
                    ty: Type::func(Type::Complex, Type::Complex),
                    support_glsl: Some(
                        "vec2 cexp(vec2 z) {\n    float scale = exp(z.x);\n    return scale * vec2(cos(z.y), sin(z.y));\n}",
                    ),
                    listed: true,
                },
            ),
            (
                "clog",
                ValueFuncDef {
                    ty: Type::func(Type::Complex, Type::Complex),
                    support_glsl: Some(
                        "vec2 clog(vec2 z) {\n    return vec2(log(length(z)), atan(z.y, z.x));\n}",
                    ),
                    listed: true,
                },
            ),
            (
                "csqrt",
                ValueFuncDef {
                    ty: Type::func(Type::Complex, Type::Complex),
                    support_glsl: Some(
                        "vec2 csqrt(vec2 z) {\n    float r = length(z);\n    float a = sqrt(max((r + z.x) * 0.5, 0.0));\n    float b = sqrt(max((r - z.x) * 0.5, 0.0));\n    return vec2(a, sign(z.y) * b);\n}",
                    ),
                    listed: true,
                },
            ),
            (
                "csin",
                ValueFuncDef {
                    ty: Type::func(Type::Complex, Type::Complex),
                    support_glsl: Some(
                        "vec2 csin(vec2 z) {\n    return vec2(sin(z.x) * cosh(z.y), cos(z.x) * sinh(z.y));\n}",
                    ),
                    listed: true,
                },
            ),
            (
                "ccos",
                ValueFuncDef {
                    ty: Type::func(Type::Complex, Type::Complex),
                    support_glsl: Some(
                        "vec2 ccos(vec2 z) {\n    return vec2(cos(z.x) * cosh(z.y), -sin(z.x) * sinh(z.y));\n}",
                    ),
                    listed: true,
                },
            ),
            (
                "ctan",
                ValueFuncDef {
                    ty: Type::func(Type::Complex, Type::Complex),
                    support_glsl: Some(
                        "vec2 ctan(vec2 z) {\n    float d = cos(2.0 * z.x) + cosh(2.0 * z.y);\n    return vec2(sin(2.0 * z.x), sinh(2.0 * z.y)) / d;\n}",
                    ),
                    listed: true,
                },
            ),
            (
                "csinh",
                ValueFuncDef {
                    ty: Type::func(Type::Complex, Type::Complex),
                    support_glsl: Some(
                        "vec2 csinh(vec2 z) {\n    return vec2(sinh(z.x) * cos(z.y), cosh(z.x) * sin(z.y));\n}",
                    ),
                    listed: true,
                },
            ),
            (
                "ccosh",
                ValueFuncDef {
                    ty: Type::func(Type::Complex, Type::Complex),
                    support_glsl: Some(
                        "vec2 ccosh(vec2 z) {\n    return vec2(cosh(z.x) * cos(z.y), sinh(z.x) * sin(z.y));\n}",
                    ),
                    listed: true,
                },
            ),
            (
                "ctanh",
                ValueFuncDef {
                    ty: Type::func(Type::Complex, Type::Complex),
                    support_glsl: Some(
                        "vec2 ctanh(vec2 z) {\n    float d = cosh(2.0 * z.x) + cos(2.0 * z.y);\n    return vec2(sinh(2.0 * z.x), sin(2.0 * z.y)) / d;\n}",
                    ),
                    listed: true,
                },
            ),
            (
                "derivative",
                ValueFuncDef {
                    ty: Type::func(
                        Type::Float,
                        Type::func(
                            Type::func(Type::Float, Type::Float),
                            Type::func(Type::Float, Type::Float),
                        ),
                    ),
                    support_glsl: None,
                    listed: false,
                },
            ),
            (
                "partialX",
                ValueFuncDef {
                    ty: Type::func(
                        Type::Float,
                        Type::func(
                            Type::func(Type::Vec3, Type::Float),
                            Type::func(Type::Vec3, Type::Float),
                        ),
                    ),
                    support_glsl: None,
                    listed: false,
                },
            ),
            (
                "partialY",
                ValueFuncDef {
                    ty: Type::func(
                        Type::Float,
                        Type::func(
                            Type::func(Type::Vec3, Type::Float),
                            Type::func(Type::Vec3, Type::Float),
                        ),
                    ),
                    support_glsl: None,
                    listed: false,
                },
            ),
            (
                "partialZ",
                ValueFuncDef {
                    ty: Type::func(
                        Type::Float,
                        Type::func(
                            Type::func(Type::Vec3, Type::Float),
                            Type::func(Type::Vec3, Type::Float),
                        ),
                    ),
                    support_glsl: None,
                    listed: false,
                },
            ),
            (
                "directionalDerivative",
                ValueFuncDef {
                    ty: Type::func(
                        Type::Float,
                        Type::func(
                            Type::Vec3,
                            Type::func(
                                Type::func(Type::Vec3, Type::Float),
                                Type::func(Type::Vec3, Type::Float),
                            ),
                        ),
                    ),
                    support_glsl: None,
                    listed: false,
                },
            ),
            (
                "gradient",
                ValueFuncDef {
                    ty: Type::func(
                        Type::Float,
                        Type::func(
                            Type::func(Type::Vec3, Type::Float),
                            Type::func(Type::Vec3, Type::Vec3),
                        ),
                    ),
                    support_glsl: None,
                    listed: false,
                },
            ),
            (
                "divergence",
                ValueFuncDef {
                    ty: Type::func(
                        Type::Float,
                        Type::func(
                            Type::func(Type::Vec3, Type::Vec3),
                            Type::func(Type::Vec3, Type::Float),
                        ),
                    ),
                    support_glsl: None,
                    listed: false,
                },
            ),
        ]);

        Self {
            primitives,
            object_ops,
            value_funcs,
        }
    }
}

impl Registry {
    pub(super) fn known_primitives(&self) -> Vec<KnownPrimitive> {
        let mut names: Vec<_> = self.primitives.keys().copied().collect();
        names.sort_unstable();
        names
            .into_iter()
            .map(|name| {
                let primitive = &self.primitives[name];
                KnownPrimitive {
                    name: name.to_string(),
                    dimension: shape_dimension(name),
                    parameter_space: primitive.parameter_space(),
                    fields: primitive
                        .fields
                        .iter()
                        .map(KnownPrimitiveField::from_def)
                        .collect(),
                    type_body: primitive.type_body(),
                    function_body: primitive.function_body(name),
                }
            })
            .collect()
    }

    pub(super) fn known_primitive(&self, name: &str) -> Option<KnownPrimitive> {
        self.known_primitives()
            .into_iter()
            .find(|primitive| primitive.name == name)
    }

    pub(super) fn known_builtin_objects(&self) -> Vec<KnownBuiltinObject> {
        let mut objects = Vec::new();

        for def in ALGEBRAIC_CATEGORY_DEFS.iter() {
            objects.push(KnownBuiltinObject {
                name: def.name.to_string(),
                ty: CATEGORY_METATYPE_NAME.to_string(),
                kind: KnownBuiltinObjectKind::Category,
            });
        }

        for (name, _) in BUILTIN_TYPE_DETAILS {
            objects.push(KnownBuiltinObject {
                name: name.to_string(),
                ty: type_category_signature(name).unwrap_or_else(|| TYPE_METATYPE_NAME.to_string()),
                kind: KnownBuiltinObjectKind::Type,
            });
        }

        let mut value_func_names: Vec<_> = self
            .value_funcs
            .iter()
            .filter_map(|(name, func)| func.listed.then_some(*name))
            .collect();
        value_func_names.sort_unstable();
        for name in value_func_names {
            objects.push(KnownBuiltinObject {
                name: name.to_string(),
                ty: format_object_type(&self.value_funcs[name].ty),
                kind: KnownBuiltinObjectKind::Function,
            });
        }

        let mut op_names: Vec<_> = self.object_ops.keys().copied().collect();
        op_names.sort_unstable();
        for name in op_names {
            let op = &self.object_ops[name];
            objects.push(KnownBuiltinObject {
                name: op.name.to_string(),
                ty: format_object_type(&object_op_type(op)),
                kind: KnownBuiltinObjectKind::Function,
            });
        }

        objects
    }

    pub(super) fn known_builtin_object(&self, name: &str) -> Option<KnownBuiltinObjectDetail> {
        if category_by_name(name).is_some() {
            return Some(KnownBuiltinObjectDetail {
                name: name.to_string(),
                ty: CATEGORY_METATYPE_NAME.to_string(),
                kind: KnownBuiltinObjectKind::Category,
                body: String::new(),
            });
        }

        if let Some((name, body)) = BUILTIN_TYPE_DETAILS
            .iter()
            .find(|(candidate, _)| *candidate == name)
        {
            return Some(KnownBuiltinObjectDetail {
                name: (*name).to_string(),
                ty: type_category_signature(name).unwrap_or_else(|| TYPE_METATYPE_NAME.to_string()),
                kind: KnownBuiltinObjectKind::Type,
                body: (*body).to_string(),
            });
        }

        if let Some(func) = self.value_funcs.get(name) {
            if !func.listed {
                return None;
            }
            let body = func.support_glsl?;
            return Some(KnownBuiltinObjectDetail {
                name: name.to_string(),
                ty: format_object_type(&func.ty),
                kind: KnownBuiltinObjectKind::Function,
                body: body.to_string(),
            });
        }

        let op = self.object_ops.get(name)?;
        Some(KnownBuiltinObjectDetail {
            name: op.name.to_string(),
            ty: format_object_type(&object_op_type(op)),
            kind: KnownBuiltinObjectKind::Function,
            body: op.support_glsl.to_string(),
        })
    }

    pub(super) fn preregistered_objects(&self) -> Vec<PreregisteredObject> {
        let mut objects = Vec::new();
        let mut primitive_names: Vec<_> = self.primitives.keys().copied().collect();
        primitive_names.sort_unstable();
        for name in primitive_names {
            objects.extend(self.primitives[name].preregistered_objects(name));
        }

        let mut op_names: Vec<_> = self.object_ops.keys().copied().collect();
        op_names.sort_unstable();
        for name in op_names {
            let op = &self.object_ops[name];
            objects.push(PreregisteredObject {
                name: op.glsl_name.to_string(),
                kind: PreregisteredObjectKind::Function,
                body: op.support_glsl.to_string(),
            });
        }

        let mut value_func_names: Vec<_> = self
            .value_funcs
            .iter()
            .filter_map(|(name, func)| func.support_glsl.map(|_| *name))
            .collect();
        value_func_names.sort_unstable();
        for name in value_func_names {
            objects.push(PreregisteredObject {
                name: name.to_string(),
                kind: PreregisteredObjectKind::Function,
                body: self.value_funcs[name].support_glsl.unwrap().to_string(),
            });
        }

        objects.sort_by(|left, right| left.kind.cmp(&right.kind).then(left.name.cmp(&right.name)));
        objects
    }

    pub(super) fn preregistered_object(&self, name: &str) -> Option<PreregisteredObject> {
        self.preregistered_objects()
            .into_iter()
            .find(|object| object.name == name)
    }
}

pub(crate) fn shape_dimension(name: &str) -> ShapeDimension {
    if name.ends_with("2D") {
        return ShapeDimension::D2;
    }
    if name.ends_with("3D") {
        return ShapeDimension::D3;
    }
    panic!("primitive '{name}' is missing a dimensional suffix")
}

impl PrimitiveDef {
    fn parameter_space(&self) -> String {
        match &self.kind {
            PrimitiveKind::ParamStruct(param_type) => (*param_type).to_string(),
            PrimitiveKind::Polygon2D => format!("{{ {} }}", self.field_summary()),
        }
    }

    fn type_body(&self) -> Option<String> {
        match &self.kind {
            PrimitiveKind::ParamStruct(_) => self
                .support_glsl
                .split_once("\n\n")
                .map(|(struct_body, _)| struct_body.to_string()),
            PrimitiveKind::Polygon2D => None,
        }
    }

    fn function_body(&self, name: &str) -> String {
        match &self.kind {
            PrimitiveKind::ParamStruct(_) => self
                .support_glsl
                .split_once("\n\n")
                .map(|(_, function_body)| function_body.to_string())
                .unwrap_or_else(|| format!("float sdf0_{name}(...) {{}}")),
            PrimitiveKind::Polygon2D => self.support_glsl.to_string(),
        }
    }

    fn field_summary(&self) -> String {
        self.fields
            .iter()
            .map(KnownPrimitiveField::from_def)
            .map(|field| format!("{}: {}", field.name, field.domain))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn preregistered_objects(&self, name: &str) -> Vec<PreregisteredObject> {
        let mut objects = Vec::new();
        match &self.kind {
            PrimitiveKind::ParamStruct(param_type) => {
                if let Some((struct_body, function_body)) = self.support_glsl.split_once("\n\n") {
                    objects.push(PreregisteredObject {
                        name: (*param_type).to_string(),
                        kind: PreregisteredObjectKind::Type,
                        body: struct_body.to_string(),
                    });
                    objects.push(PreregisteredObject {
                        name: format!("sdf0_{name}"),
                        kind: PreregisteredObjectKind::Function,
                        body: function_body.to_string(),
                    });
                }
            }
            PrimitiveKind::Polygon2D => {
                objects.push(PreregisteredObject {
                    name: "sdf0_Polygon2D".to_string(),
                    kind: PreregisteredObjectKind::Function,
                    body: self.support_glsl.to_string(),
                });
            }
        }
        objects
    }
}

impl KnownPrimitiveField {
    fn from_def(field: &PrimitiveFieldDef) -> Self {
        Self {
            name: field.name.to_string(),
            domain: match &field.kind {
                PrimitiveFieldKind::Value(ty) => ty.type_name().to_string(),
                PrimitiveFieldKind::Vec2List => "R2 list".to_string(),
            },
        }
    }
}
