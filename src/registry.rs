use super::*;

impl Default for Registry {
    /// Builds the built-in registry with primitives, operations, and functions as defaults.
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
                "smoothUnion",
                ObjectOpDef {
                    name: "smoothUnion",
                    value_arg_types: vec![Type::Float],
                    object_arg_count: 2,
                    associative_binary: false,
                    glsl_name: "_op_smooth_union",
                    support_glsl: "float _op_smooth_union(float _a, float _b, float _k) {\n    _k *= 1.0 / (1.0 - sqrt(0.5));\n    float _h = max(_k - abs(_a - _b), 0.0) / _k;\n    return min(_a, _b) - (_k * 0.5 * (1.0 + _h - sqrt(1.0 - (_h * (_h - 2.0)))));\n}",
                },
            ),
            (
                "union",
                ObjectOpDef {
                    name: "union",
                    value_arg_types: vec![],
                    object_arg_count: 2,
                    associative_binary: true,
                    glsl_name: "_op_union",
                    support_glsl: "float _op_union(float _a, float _b) {\n    return min(_a, _b);\n}",
                },
            ),
            (
                "intersect",
                ObjectOpDef {
                    name: "intersect",
                    value_arg_types: vec![],
                    object_arg_count: 2,
                    associative_binary: true,
                    glsl_name: "_op_intersection",
                    support_glsl: "float _op_intersection(float _a, float _b) {\n    return max(_a, _b);\n}",
                },
            ),
            (
                "diff",
                ObjectOpDef {
                    name: "diff",
                    value_arg_types: vec![],
                    object_arg_count: 2,
                    associative_binary: false,
                    glsl_name: "_op_difference",
                    support_glsl: "float _op_difference(float _a, float _b) {\n    return max(_a, -_b);\n}",
                },
            ),
            (
                "xor",
                ObjectOpDef {
                    name: "xor",
                    value_arg_types: vec![],
                    object_arg_count: 2,
                    associative_binary: true,
                    glsl_name: "_op_xor",
                    support_glsl: "float _op_xor(float _a, float _b) {\n    return max(min(_a, _b), -max(_a, _b));\n}",
                },
            ),
            (
                "smoothIntersect",
                ObjectOpDef {
                    name: "smoothIntersect",
                    value_arg_types: vec![Type::Float],
                    object_arg_count: 2,
                    associative_binary: false,
                    glsl_name: "_op_smooth_intersection",
                    support_glsl: "float _op_smooth_intersection_min(float _a, float _b, float _k) {\n    _k *= 1.0 / (1.0 - sqrt(0.5));\n    float _h = max(_k - abs(_a - _b), 0.0) / _k;\n    return min(_a, _b) - (_k * 0.5 * (1.0 + _h - sqrt(1.0 - (_h * (_h - 2.0)))));\n}\n\nfloat _op_smooth_intersection_max(float _a, float _b, float _k) {\n    return -_op_smooth_intersection_min(-_a, -_b, _k);\n}\n\nfloat _op_smooth_intersection(float _a, float _b, float _k) {\n    return _op_smooth_intersection_max(_a, _b, _k);\n}",
                },
            ),
            (
                "smoothDiff",
                ObjectOpDef {
                    name: "smoothDiff",
                    value_arg_types: vec![Type::Float],
                    object_arg_count: 2,
                    associative_binary: false,
                    glsl_name: "_op_smooth_difference",
                    support_glsl: "float _op_smooth_difference_min(float _a, float _b, float _k) {\n    _k *= 1.0 / (1.0 - sqrt(0.5));\n    float _h = max(_k - abs(_a - _b), 0.0) / _k;\n    return min(_a, _b) - (_k * 0.5 * (1.0 + _h - sqrt(1.0 - (_h * (_h - 2.0)))));\n}\n\nfloat _op_smooth_difference_max(float _a, float _b, float _k) {\n    return -_op_smooth_difference_min(-_a, -_b, _k);\n}\n\nfloat _op_smooth_difference(float _a, float _b, float _k) {\n    return _op_smooth_difference_max(_a, -_b, _k);\n}",
                },
            ),
            (
                "smoothXor",
                ObjectOpDef {
                    name: "smoothXor",
                    value_arg_types: vec![Type::Float],
                    object_arg_count: 2,
                    associative_binary: false,
                    glsl_name: "_op_smooth_xor",
                    support_glsl: "float _op_smooth_xor_min(float _a, float _b, float _k) {\n    _k *= 1.0 / (1.0 - sqrt(0.5));\n    float _h = max(_k - abs(_a - _b), 0.0) / _k;\n    return min(_a, _b) - (_k * 0.5 * (1.0 + _h - sqrt(1.0 - (_h * (_h - 2.0)))));\n}\n\nfloat _op_smooth_xor_max(float _a, float _b, float _k) {\n    return -_op_smooth_xor_min(-_a, -_b, _k);\n}\n\nfloat _op_smooth_xor(float _a, float _b, float _k) {\n    return _op_smooth_xor_max(_op_smooth_xor_min(_a, _b, _k), -_op_smooth_xor_max(_a, _b, _k), _k);\n}",
                },
            ),
            (
                "revolution",
                ObjectOpDef {
                    name: "revolution",
                    value_arg_types: vec![Type::Float],
                    object_arg_count: 1,
                    associative_binary: false,
                    glsl_name: "_op_revolution",
                    support_glsl: "vec3 _op_revolution_point(vec3 _p, float _offset) {\n    return vec3(length(_p.xz) - _offset, _p.y, 0.0);\n}",
                },
            ),
            (
                "extrude",
                ObjectOpDef {
                    name: "extrude",
                    value_arg_types: vec![Type::Float],
                    object_arg_count: 1,
                    associative_binary: false,
                    glsl_name: "_op_extrusion",
                    support_glsl: "float _op_extrusion(float _base_distance, float _z, float _height) {\n    vec2 _w = vec2(_base_distance, abs(_z) - _height);\n    return min(max(_w.x, _w.y), 0.0) + length(max(_w, 0.0));\n}",
                },
            ),
            (
                "rot",
                ObjectOpDef {
                    name: "rot",
                    value_arg_types: vec![Type::Vec3, Type::Vec3, Type::Float],
                    object_arg_count: 1,
                    associative_binary: false,
                    glsl_name: "_op_rot",
                    support_glsl: "mat3 _op_rot_matrix(vec3 _binormal, float _angle) {\n    vec3 _axis = normalize(_binormal);\n    float _c = cos(_angle);\n    float _s = sin(_angle);\n    float _oc = 1.0 - _c;\n    return mat3(\n        vec3((_axis.x * _axis.x * _oc) + _c, (_axis.y * _axis.x * _oc) + (_axis.z * _s), (_axis.z * _axis.x * _oc) - (_axis.y * _s)),\n        vec3((_axis.x * _axis.y * _oc) - (_axis.z * _s), (_axis.y * _axis.y * _oc) + _c, (_axis.z * _axis.y * _oc) + (_axis.x * _s)),\n        vec3((_axis.x * _axis.z * _oc) + (_axis.y * _s), (_axis.y * _axis.z * _oc) - (_axis.x * _s), (_axis.z * _axis.z * _oc) + _c)\n    );\n}\n\nvec3 _op_rot_inverse_point(vec3 _p, vec3 _binormal, vec3 _anchor, float _angle) {\n    mat3 _r = _op_rot_matrix(_binormal, _angle);\n    return _anchor + (transpose(_r) * (_p - _anchor));\n}",
                },
            ),
            (
                "rot2D",
                ObjectOpDef {
                    name: "rot2D",
                    value_arg_types: vec![Type::Vec2, Type::Float],
                    object_arg_count: 1,
                    associative_binary: false,
                    glsl_name: "_op_rot2D",
                    support_glsl: "mat2 _op_rot2D_matrix(float _angle) {\n    float _c = cos(_angle);\n    float _s = sin(_angle);\n    return mat2(vec2(_c, _s), vec2(-_s, _c));\n}\n\nvec3 _op_rot2D_inverse_point(vec3 _p, vec2 _anchor, float _angle) {\n    mat2 _r = _op_rot2D_matrix(_angle);\n    return vec3(_anchor + (transpose(_r) * (_p.xy - _anchor)), _p.z);\n}",
                },
            ),
            (
                "withMaterial",
                ObjectOpDef {
                    name: "withMaterial",
                    value_arg_types: vec![Type::Generic("Material".to_string())],
                    object_arg_count: 1,
                    associative_binary: false,
                    glsl_name: "_op_with_material",
                    support_glsl: "",
                },
            ),
            (
                "withBounds",
                ObjectOpDef {
                    name: "withBounds",
                    value_arg_types: vec![Type::Generic("Bounds".to_string())],
                    object_arg_count: 1,
                    associative_binary: false,
                    glsl_name: "_op_with_bounds",
                    support_glsl: "",
                },
            ),
            (
                "withLipschitz",
                ObjectOpDef {
                    name: "withLipschitz",
                    value_arg_types: vec![Type::Float],
                    object_arg_count: 1,
                    associative_binary: false,
                    glsl_name: "_op_with_lipschitz",
                    support_glsl: "",
                },
            ),
            (
                "withBlend",
                ObjectOpDef {
                    name: "withBlend",
                    value_arg_types: vec![Type::Generic("Blend".to_string())],
                    object_arg_count: 1,
                    associative_binary: false,
                    glsl_name: "_op_with_blend",
                    support_glsl: "",
                },
            ),
        ]);

        let value_funcs = HashMap::from([
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
                "not",
                ValueFuncDef {
                    ty: Type::func(Type::Bool, Type::Bool),
                    support_glsl: Some("bool not(bool x) {\n    return !x;\n}"),
                    listed: true,
                },
            ),
            (
                "and",
                ValueFuncDef {
                    ty: Type::func(Type::Product(vec![Type::Bool, Type::Bool]), Type::Bool),
                    support_glsl: Some("bool and(bool a, bool b) {\n    return a && b;\n}"),
                    listed: true,
                },
            ),
            (
                "or",
                ValueFuncDef {
                    ty: Type::func(Type::Product(vec![Type::Bool, Type::Bool]), Type::Bool),
                    support_glsl: Some("bool or(bool a, bool b) {\n    return a || b;\n}"),
                    listed: true,
                },
            ),
            (
                "xor",
                ValueFuncDef {
                    ty: Type::func(Type::Product(vec![Type::Bool, Type::Bool]), Type::Bool),
                    support_glsl: Some("bool xor(bool a, bool b) {\n    return a != b;\n}"),
                    listed: true,
                },
            ),
            (
                "rot",
                ValueFuncDef {
                    ty: Type::func(
                        Type::Product(vec![Type::Vec3, Type::Vec3, Type::Float]),
                        Type::Isom3,
                    ),
                    support_glsl: None,
                    listed: true,
                },
            ),
            (
                "rot2D",
                ValueFuncDef {
                    ty: Type::func(Type::Product(vec![Type::Vec2, Type::Float]), Type::Isom2),
                    support_glsl: Some(
                        "mat2 rot2D_Isom2_matrix(float angle) {\n    float c = cos(angle);\n    float s = sin(angle);\n    return mat2(vec2(c, s), vec2(-s, c));\n}\n\nIsom2 rot2D(vec2 anchor, float angle) {\n    mat2 A = rot2D_Isom2_matrix(angle);\n    return Isom2(A, anchor - (A * anchor));\n}",
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
                    listed: false,
                },
            ),
            (
                "cexp",
                ValueFuncDef {
                    ty: Type::func(Type::Complex, Type::Complex),
                    support_glsl: Some(
                        "vec2 cexp(vec2 z) {\n    float scale = exp(z.x);\n    return scale * vec2(cos(z.y), sin(z.y));\n}",
                    ),
                    listed: false,
                },
            ),
            (
                "clog",
                ValueFuncDef {
                    ty: Type::func(Type::Complex, Type::Complex),
                    support_glsl: Some(
                        "vec2 clog(vec2 z) {\n    return vec2(log(length(z)), atan(z.y, z.x));\n}",
                    ),
                    listed: false,
                },
            ),
            (
                "csqrt",
                ValueFuncDef {
                    ty: Type::func(Type::Complex, Type::Complex),
                    support_glsl: Some(
                        "vec2 csqrt(vec2 z) {\n    float r = length(z);\n    float a = sqrt(max((r + z.x) * 0.5, 0.0));\n    float b = sqrt(max((r - z.x) * 0.5, 0.0));\n    return vec2(a, sign(z.y) * b);\n}",
                    ),
                    listed: false,
                },
            ),
            (
                "csin",
                ValueFuncDef {
                    ty: Type::func(Type::Complex, Type::Complex),
                    support_glsl: Some(
                        "vec2 csin(vec2 z) {\n    return vec2(sin(z.x) * cosh(z.y), cos(z.x) * sinh(z.y));\n}",
                    ),
                    listed: false,
                },
            ),
            (
                "ccos",
                ValueFuncDef {
                    ty: Type::func(Type::Complex, Type::Complex),
                    support_glsl: Some(
                        "vec2 ccos(vec2 z) {\n    return vec2(cos(z.x) * cosh(z.y), -sin(z.x) * sinh(z.y));\n}",
                    ),
                    listed: false,
                },
            ),
            (
                "ctan",
                ValueFuncDef {
                    ty: Type::func(Type::Complex, Type::Complex),
                    support_glsl: Some(
                        "vec2 ctan(vec2 z) {\n    float d = cos(2.0 * z.x) + cosh(2.0 * z.y);\n    return vec2(sin(2.0 * z.x), sinh(2.0 * z.y)) / d;\n}",
                    ),
                    listed: false,
                },
            ),
            (
                "csinh",
                ValueFuncDef {
                    ty: Type::func(Type::Complex, Type::Complex),
                    support_glsl: Some(
                        "vec2 csinh(vec2 z) {\n    return vec2(sinh(z.x) * cos(z.y), cosh(z.x) * sin(z.y));\n}",
                    ),
                    listed: false,
                },
            ),
            (
                "ccosh",
                ValueFuncDef {
                    ty: Type::func(Type::Complex, Type::Complex),
                    support_glsl: Some(
                        "vec2 ccosh(vec2 z) {\n    return vec2(cosh(z.x) * cos(z.y), sinh(z.x) * sin(z.y));\n}",
                    ),
                    listed: false,
                },
            ),
            (
                "ctanh",
                ValueFuncDef {
                    ty: Type::func(Type::Complex, Type::Complex),
                    support_glsl: Some(
                        "vec2 ctanh(vec2 z) {\n    float d = cosh(2.0 * z.x) + cos(2.0 * z.y);\n    return vec2(sinh(2.0 * z.x), sin(2.0 * z.y)) / d;\n}",
                    ),
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
    /// Classifies every Rust-defined object so migration work has an explicit
    /// checklist instead of an implicit grab bag of compiler special cases.
    pub(super) fn rust_defined_objects(&self) -> Vec<RustDefinedObject> {
        let mut objects = Vec::new();
        for category in CATEGORY_DEFS.iter() {
            objects.push(RustDefinedObject {
                name: category.name.to_string(),
                kind: RustDefinedObjectKind::Category,
                role: RustDefinedObjectRole::CoreSyntax,
                reason: "category names are part of the current type/category checker".to_string(),
            });
        }
        for ty in BUILTIN_TYPE_DEFS.iter() {
            objects.push(RustDefinedObject {
                name: ty.display_name.to_string(),
                kind: RustDefinedObjectKind::Type,
                role: rust_defined_type_role(ty),
                reason: rust_defined_type_reason(ty).to_string(),
            });
        }
        for name in self.primitives.keys() {
            objects.push(RustDefinedObject {
                name: (*name).to_string(),
                kind: RustDefinedObjectKind::Primitive,
                role: RustDefinedObjectRole::StdMovable,
                reason: "SDF constructor with backend support; should become a std definition once Lane can express primitive objects".to_string(),
            });
        }
        for op in self.object_ops.values() {
            objects.push(RustDefinedObject {
                name: op.name.to_string(),
                kind: RustDefinedObjectKind::ObjectOperator,
                role: RustDefinedObjectRole::StdMovable,
                reason: "object combinator; not syntactically special after importable definitions exist".to_string(),
            });
        }
        for (name, func) in &self.value_funcs {
            objects.push(RustDefinedObject {
                name: (*name).to_string(),
                kind: RustDefinedObjectKind::Function,
                role: RustDefinedObjectRole::StdMovable,
                reason: if func.support_glsl.is_some() {
                    "Rust currently owns support GLSL; migrate behind std/raw-GLSL definitions where possible"
                } else {
                    "pre-registered ordinary function overload; should be provided by std unless it is true backend syntax"
                }
                .to_string(),
            });
        }
        for (name, _) in glsl_builtin_value_func_overloads() {
            objects.push(RustDefinedObject {
                name: name.to_string(),
                kind: RustDefinedObjectKind::Function,
                role: RustDefinedObjectRole::StdMovable,
                reason: "GLSL builtin exposed as Lane function; keep only as backend hook or import through std".to_string(),
            });
        }
        objects.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then_with(|| format!("{:?}", left.kind).cmp(&format!("{:?}", right.kind)))
        });
        objects.dedup_by(|left, right| left.name == right.name && left.kind == right.kind);
        objects
    }
}

fn rust_defined_type_role(ty: &BuiltinTypeDef) -> RustDefinedObjectRole {
    match &ty.ty {
        Type::Complex | Type::Quat | Type::Isom2 | Type::Isom3 => RustDefinedObjectRole::StdMovable,
        _ => RustDefinedObjectRole::CoreSyntax,
    }
}

fn rust_defined_type_reason(ty: &BuiltinTypeDef) -> &'static str {
    match &ty.ty {
        Type::Complex | Type::Quat | Type::Isom2 | Type::Isom3 => {
            "concrete algebraic type with Rust GLSL/category support; should become std Lane code"
        }
        Type::Object | Type::Object2D => {
            "core SDF object type used by the object/typechecker boundary"
        }
        Type::Bool | Type::Float | Type::Int | Type::Vec2 | Type::Vec3 | Type::Vec4 => {
            "core scalar/vector backend type"
        }
        Type::Unit
        | Type::Mat(_, _)
        | Type::Func(_, _)
        | Type::Product(_)
        | Type::Custom { .. }
        | Type::Generic(_)
        | Type::VecGeneric(_)
        | Type::MatGeneric(_, _)
        | Type::Power(_, _)
        | Type::Array(_) => "core typechecker representation",
    }
}

impl Registry {
    /// Collects all known built-in primitives into LSP-friendly metadata.
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

    /// Looks up a primitive by name and returns its public descriptor.
    pub(super) fn known_primitive(&self, name: &str) -> Option<KnownPrimitive> {
        self.known_primitives()
            .into_iter()
            .find(|primitive| primitive.name == name)
    }

    /// Enumerates built-in objects including categories, types, and functions.
    pub(super) fn known_builtin_objects(&self) -> Vec<KnownBuiltinObject> {
        let mut objects = Vec::new();

        for def in CATEGORY_DEFS.iter() {
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
        value_func_names.extend(
            glsl_builtin_value_func_overloads()
                .into_iter()
                .map(|(name, _)| name),
        );
        value_func_names.extend(COMPLEX_OVERLOAD_NAMES);
        value_func_names.sort_unstable();
        value_func_names.dedup();
        for name in value_func_names {
            let Some(ty) = listed_builtin_function_signature(name, &self.value_funcs) else {
                debug_assert!(
                    false,
                    "known_builtin_objects expected function signature for '{name}'"
                );
                continue;
            };
            objects.push(KnownBuiltinObject {
                name: name.to_string(),
                ty,
                kind: KnownBuiltinObjectKind::Function,
            });
        }

        let mut _op_names: Vec<_> = self.object_ops.keys().copied().collect();
        _op_names.sort_unstable();
        for name in _op_names {
            if self.value_funcs.get(name).is_some_and(|func| func.listed)
                || listed_builtin_value_func_overloads(name).is_some()
            {
                continue;
            }
            let op = &self.object_ops[name];
            objects.push(KnownBuiltinObject {
                name: op.name.to_string(),
                ty: format_object_type(&object_op_type(op)),
                kind: KnownBuiltinObjectKind::Function,
            });
        }

        objects
    }

    /// Returns object detail (source signature + optional body) for a known built-in name.
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
            let body = match (body.is_empty(), builtin_type_support_glsl(name)) {
                (true, Some(support_glsl)) => support_glsl.to_string(),
                (false, Some(support_glsl)) => format!("{body}\n\n{support_glsl}"),
                (_, None) => (*body).to_string(),
            };
            return Some(KnownBuiltinObjectDetail {
                name: (*name).to_string(),
                ty: type_category_signature(name).unwrap_or_else(|| TYPE_METATYPE_NAME.to_string()),
                kind: KnownBuiltinObjectKind::Type,
                body: suffix_glsl_float_literals(&body),
            });
        }

        if let Some(func) = self.value_funcs.get(name) {
            if func.listed {
                return Some(KnownBuiltinObjectDetail {
                    name: name.to_string(),
                    ty: format_object_type(&func.ty),
                    kind: KnownBuiltinObjectKind::Function,
                    body: suffix_glsl_float_literals(func.support_glsl.unwrap_or_default()),
                });
            }
        }

        if let Some(overloads) = listed_builtin_function_signature(name, &self.value_funcs) {
            return Some(KnownBuiltinObjectDetail {
                name: name.to_string(),
                ty: overloads,
                kind: KnownBuiltinObjectKind::Function,
                body: String::new(),
            });
        }

        let op = self.object_ops.get(name)?;
        Some(KnownBuiltinObjectDetail {
            name: op.name.to_string(),
            ty: format_object_type(&object_op_type(op)),
            kind: KnownBuiltinObjectKind::Function,
            body: suffix_glsl_float_literals(op.support_glsl),
        })
    }

    /// Returns all preregistered types/functions emitted in generated GLSL form.
    pub(super) fn preregistered_objects(&self) -> Vec<PreregisteredObject> {
        let mut objects = Vec::new();
        let mut primitive_names: Vec<_> = self.primitives.keys().copied().collect();
        primitive_names.sort_unstable();
        for name in primitive_names {
            objects.extend(self.primitives[name].preregistered_objects(name));
        }

        let mut _op_names: Vec<_> = self.object_ops.keys().copied().collect();
        _op_names.sort_unstable();
        for name in _op_names {
            let op = &self.object_ops[name];
            objects.push(PreregisteredObject {
                name: op.glsl_name.to_string(),
                kind: PreregisteredObjectKind::Function,
                body: suffix_glsl_float_literals(op.support_glsl),
            });
        }

        let mut value_func_names: Vec<_> = self
            .value_funcs
            .iter()
            .filter_map(|(name, func)| func.support_glsl.map(|_| *name))
            .filter(|name| complex_overload_name(name).is_none())
            .collect();
        value_func_names.sort_unstable();
        for name in value_func_names {
            let Some(body) = self
                .value_funcs
                .get(name)
                .and_then(|func| func.support_glsl)
            else {
                debug_assert!(
                    false,
                    "preregistered_objects expected GLSL support for value function '{name}'"
                );
                continue;
            };
            objects.push(PreregisteredObject {
                name: name.to_string(),
                kind: PreregisteredObjectKind::Function,
                body: suffix_glsl_float_literals(body),
            });
        }

        for name in COMPLEX_OVERLOAD_NAMES {
            if let Some(body) = complex_overload_support_glsl(name) {
                objects.push(PreregisteredObject {
                    name: name.to_string(),
                    kind: PreregisteredObjectKind::Function,
                    body: suffix_glsl_float_literals(body),
                });
            } else {
                debug_assert!(
                    false,
                    "preregistered_objects expected complex overload support for '{name}'"
                );
            }
        }

        for name in ["C", "H", "Isom2", "Isom3"] {
            if let Some(body) = builtin_type_support_glsl(name) {
                objects.push(PreregisteredObject {
                    name: name.to_string(),
                    kind: PreregisteredObjectKind::Type,
                    body: suffix_glsl_float_literals(body),
                });
            } else {
                debug_assert!(
                    false,
                    "preregistered_objects expected builtin type support for '{name}'"
                );
            }
        }

        objects.sort_by(|left, right| left.kind.cmp(&right.kind).then(left.name.cmp(&right.name)));
        objects
    }

    /// Resolves one preregistered object by name from the full preregistered catalog.
    pub(super) fn preregistered_object(&self, name: &str) -> Option<PreregisteredObject> {
        self.preregistered_objects()
            .into_iter()
            .find(|object| object.name == name)
    }
}

fn listed_builtin_function_signature(
    name: &str,
    value_funcs: &HashMap<&'static str, ValueFuncDef>,
) -> Option<String> {
    if let Some(func) = value_funcs.get(name).filter(|func| func.listed) {
        return Some(format_object_type(&func.ty));
    }
    listed_builtin_value_func_overloads(name)
}

/// Infers whether a built-in name is 2D or 3D from its suffix.
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
    /// Returns the GLSL type used for this primitive's parameter struct or polygon field summary.
    fn parameter_space(&self) -> String {
        match &self.kind {
            PrimitiveKind::ParamStruct(param_type) => (*param_type).to_string(),
            PrimitiveKind::Polygon2D => format!("{{ {} }}", self.field_summary()),
        }
    }

    /// Extracts and returns the type declaration body for this primitive, if any.
    fn type_body(&self) -> Option<String> {
        match &self.kind {
            PrimitiveKind::ParamStruct(_) => self
                .support_glsl
                .split_once("\n\n")
                .map(|(struct_body, _)| struct_body.to_string()),
            PrimitiveKind::Polygon2D => None,
        }
    }

    /// Extracts and returns the callable GLSL function body for this primitive.
    fn function_body(&self, name: &str) -> String {
        match &self.kind {
            PrimitiveKind::ParamStruct(_) => self
                .support_glsl
                .split_once("\n\n")
                .map(|(_, function_body)| suffix_glsl_float_literals(function_body))
                .unwrap_or_else(|| format!("float sdf0_{name}(...) {{}}")),
            PrimitiveKind::Polygon2D => self.support_glsl.to_string(),
        }
    }

    /// Formats primitive field declarations into a short field summary string.
    fn field_summary(&self) -> String {
        self.fields
            .iter()
            .map(KnownPrimitiveField::from_def)
            .map(|field| format!("{}: {}", field.name, field.domain))
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Emits preregistered entries (GLSL type and constructor/function) for a primitive.
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
                        body: suffix_glsl_float_literals(function_body),
                    });
                }
            }
            PrimitiveKind::Polygon2D => {
                objects.push(PreregisteredObject {
                    name: "sdf0_Polygon2D".to_string(),
                    kind: PreregisteredObjectKind::Function,
                    body: suffix_glsl_float_literals(self.support_glsl),
                });
            }
        }
        objects
    }
}

impl KnownPrimitiveField {
    /// Builds a public `KnownPrimitiveField` from internal field definition metadata.
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
