use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;

pub fn compile_program(source: &str) -> Result<String, Error> {
    let registry = Registry::default();
    let program = Parser::new(source).parse_program()?;
    let typed = TypedProgram::from_program(&program, &registry)?;
    Ok(typed.emit_glsl(&registry))
}

pub fn known_primitives() -> Vec<KnownPrimitive> {
    let registry = Registry::default();
    registry.known_primitives()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    message: String,
}

impl Error {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KnownPrimitive {
    pub name: String,
    pub sdf_name: String,
    pub domain: String,
    pub parameter_type: Option<String>,
    pub fields: Vec<KnownPrimitiveField>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KnownPrimitiveField {
    pub name: String,
    pub domain: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Type {
    Float,
    Vec2,
    Vec3,
    Obj3,
    Func(Box<Type>, Box<Type>),
}

impl Type {
    fn func(input: Type, output: Type) -> Self {
        Self::Func(Box::new(input), Box::new(output))
    }

    fn glsl_name(&self) -> &'static str {
        match self {
            Self::Float => "float",
            Self::Vec2 => "vec2",
            Self::Vec3 => "vec3",
            Self::Obj3 | Self::Func(_, _) => "",
        }
    }

    fn surface_name(&self) -> &'static str {
        match self {
            Self::Float => "float",
            Self::Vec2 => "vec2",
            Self::Vec3 => "vec3",
            Self::Obj3 => "Obj3",
            Self::Func(_, _) => "func",
        }
    }
}

#[derive(Clone, Debug)]
struct InputDecl {
    name: String,
    ty: Type,
}

#[derive(Clone, Debug)]
struct FuncDecl {
    name: String,
    ty: Type,
    expr: Expr,
}

#[derive(Clone, Debug)]
struct BindingDecl {
    name: String,
    ty: Type,
    expr: Expr,
}

#[derive(Clone, Debug)]
struct OutputDecl {
    expr: Expr,
}

#[derive(Clone, Debug)]
struct Program {
    inputs: Vec<InputDecl>,
    funcs: Vec<FuncDecl>,
    bindings: Vec<BindingDecl>,
    output: OutputDecl,
}

#[derive(Clone, Debug)]
enum Decl {
    Input(InputDecl),
    Func(FuncDecl),
    Binding(BindingDecl),
    Output(OutputDecl),
}

#[derive(Clone, Debug)]
enum Expr {
    Number(f64),
    Ident(String),
    Tuple(Vec<Expr>),
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },
    Binary {
        op: BinOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Constructor {
        name: String,
        fields: Vec<(String, Expr)>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Compose,
}

#[derive(Clone, Debug)]
enum ValueExpr {
    Float(f64),
    Var(String, Type),
    Call {
        func: String,
        args: Vec<ValueExpr>,
        ty: Type,
    },
    Binary {
        op: BinOp,
        left: Box<ValueExpr>,
        right: Box<ValueExpr>,
        ty: Type,
    },
    Vec2(Box<ValueExpr>, Box<ValueExpr>),
    Vec3(Box<ValueExpr>, Box<ValueExpr>, Box<ValueExpr>),
}

impl ValueExpr {
    fn ty(&self) -> Type {
        match self {
            Self::Float(_) => Type::Float,
            Self::Var(_, ty) => ty.clone(),
            Self::Call { ty, .. } => ty.clone(),
            Self::Binary { ty, .. } => ty.clone(),
            Self::Vec2(_, _) => Type::Vec2,
            Self::Vec3(_, _, _) => Type::Vec3,
        }
    }
}

#[derive(Clone, Debug)]
enum PrimitiveArgExpr {
    Value(ValueExpr),
    Vec2List(Vec<ValueExpr>),
}

#[derive(Clone, Debug)]
enum ObjectExpr {
    Var(String),
    Primitive {
        name: String,
        kind: PrimitiveKind,
        fields: Vec<(String, PrimitiveArgExpr)>,
    },
    Shift {
        object: Box<ObjectExpr>,
        offset: ValueExpr,
    },
    RegisteredOp {
        name: String,
        glsl_name: String,
        value_args: Vec<ValueExpr>,
        object_args: Vec<ObjectExpr>,
    },
}

#[derive(Clone, Debug)]
struct TypedFunc {
    name: String,
    output: Type,
    expr: ValueExpr,
}

#[derive(Clone, Debug)]
struct TypedBinding {
    name: String,
    expr: ObjectExpr,
}

#[derive(Clone, Debug)]
struct TypedProgram {
    inputs: Vec<InputDecl>,
    funcs: Vec<TypedFunc>,
    bindings: Vec<TypedBinding>,
    output: ObjectExpr,
}

impl TypedProgram {
    fn from_program(program: &Program, registry: &Registry) -> Result<Self, Error> {
        let mut env = Env::new(registry);

        for input in &program.inputs {
            env.insert(input.name.clone(), input.ty.clone())?;
        }

        for func in &program.funcs {
            env.insert(func.name.clone(), func.ty.clone())?;
        }

        for binding in &program.bindings {
            env.insert(binding.name.clone(), binding.ty.clone())?;
        }

        let mut typed_funcs = Vec::new();
        for func in &program.funcs {
            let (input_ty, output_ty) = match &func.ty {
                Type::Func(input, output) => ((**input).clone(), (**output).clone()),
                other => {
                    return Err(Error::new(format!(
                        "function '{}' must have a function type, got {}",
                        func.name,
                        format_type(other)
                    )))
                }
            };
            if input_ty != Type::Float {
                return Err(Error::new(format!(
                    "function '{}' currently only supports float inputs",
                    func.name
                )));
            }
            if output_ty != Type::Float && output_ty != Type::Vec2 && output_ty != Type::Vec3 {
                return Err(Error::new(format!(
                    "function '{}' currently only supports float, vec2, or vec3 outputs",
                    func.name
                )));
            }

            let expr = infer_value_expr(&func.expr, &env, Some("t"))?;
            ensure_type(&expr.ty(), &output_ty, &format!("function '{}'", func.name))?;
            typed_funcs.push(TypedFunc {
                name: func.name.clone(),
                output: output_ty,
                expr,
            });
        }

        let mut typed_bindings = Vec::new();
        for binding in &program.bindings {
            ensure_type(
                &binding.ty,
                &Type::Obj3,
                &format!("binding '{}'", binding.name),
            )?;
            let expr = infer_object_expr(&binding.expr, &env)?;
            typed_bindings.push(TypedBinding {
                name: binding.name.clone(),
                expr,
            });
        }

        let output = infer_object_expr(&program.output.expr, &env)?;

        Ok(Self {
            inputs: program.inputs.clone(),
            funcs: typed_funcs,
            bindings: typed_bindings,
            output,
        })
    }

    fn emit_glsl(&self, registry: &Registry) -> String {
        let mut lines = Vec::new();

        for support in self.support_blocks(registry) {
            lines.extend(support.lines().map(str::to_string));
            lines.push(String::new());
        }

        for func in &self.funcs {
            lines.push(format!(
                "{} {}(float t) {{",
                func.output.glsl_name(),
                helper_name(&func.name)
            ));
            lines.push(format!(
                "    return {};",
                emit_value_expr(&func.expr, &self.func_names())
            ));
            lines.push("}".to_string());
            lines.push(String::new());
        }

        let mut signature = vec!["vec3 p".to_string()];
        for input in &self.inputs {
            match input.ty {
                Type::Float | Type::Vec2 | Type::Vec3 => {
                    signature.push(format!("{} {}", input.ty.glsl_name(), input.name));
                }
                Type::Obj3 | Type::Func(_, _) => {}
            }
        }

        lines.push(format!("float scene_sdf({}) {{", signature.join(", ")));
        let mut object_names = BTreeMap::new();
        let helper_names = self.func_names();
        for binding in &self.bindings {
            let temp_name = format!("obj_{}", binding.name);
            let expr = emit_object_expr(&binding.expr, "p", &object_names, &helper_names);
            lines.push(format!("    float {} = {};", temp_name, expr));
            object_names.insert(binding.name.clone(), temp_name);
        }
        let output = emit_object_expr(&self.output, "p", &object_names, &helper_names);
        lines.push(format!("    return {};", output));
        lines.push("}".to_string());

        lines.join("\n")
    }

    fn support_blocks(&self, registry: &Registry) -> Vec<&'static str> {
        let mut names = BTreeSet::new();
        for binding in &self.bindings {
            collect_object_support(&binding.expr, &mut names);
        }
        collect_object_support(&self.output, &mut names);

        let mut blocks = Vec::new();
        for name in names {
            if let Some(primitive) = registry.primitives.get(name) {
                blocks.push(primitive.support_glsl);
                continue;
            }
            if let Some(op) = registry.object_ops.get(name) {
                blocks.push(op.support_glsl);
            }
        }
        blocks
    }

    fn func_names(&self) -> HashMap<String, String> {
        self.funcs
            .iter()
            .map(|func| (func.name.clone(), helper_name(&func.name)))
            .collect()
    }
}

#[derive(Clone, Debug)]
struct PrimitiveDef {
    kind: PrimitiveKind,
    fields: Vec<PrimitiveFieldDef>,
    support_glsl: &'static str,
}

#[derive(Clone, Debug)]
struct PrimitiveFieldDef {
    name: &'static str,
    kind: PrimitiveFieldKind,
}

#[derive(Clone, Debug)]
enum PrimitiveFieldKind {
    Value(Type),
    Vec2List,
}

#[derive(Clone, Debug)]
enum PrimitiveKind {
    ParamStruct(&'static str),
    Polygon2D,
}

#[derive(Clone, Debug)]
struct ObjectOpDef {
    name: &'static str,
    value_arg_types: Vec<Type>,
    object_arg_count: usize,
    glsl_name: &'static str,
    support_glsl: &'static str,
}

#[derive(Clone, Debug)]
struct Registry {
    primitives: HashMap<&'static str, PrimitiveDef>,
    object_ops: HashMap<&'static str, ObjectOpDef>,
    builtins: HashMap<&'static str, Type>,
}

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
                "Simplex3D",
                PrimitiveDef {
                    kind: PrimitiveKind::ParamStruct("ParamSimplex3D"),
                    fields: vec![PrimitiveFieldDef {
                        name: "size",
                        kind: PrimitiveFieldKind::Value(Type::Float),
                    }],
                    support_glsl: "struct ParamSimplex3D {\n    float size;\n};\n\nfloat sdf0_Simplex3D(vec3 p, ParamSimplex3D params) {\n    float k = sqrt(2.0);\n    vec3 q = p;\n    q.x = abs(q.x);\n    q.z = abs(q.z);\n    if (q.z > q.x) {\n        q.xz = q.zx;\n    }\n    q.x -= params.size;\n    q.y += params.size / k;\n    if ((q.x + (k * q.y)) > 0.0) {\n        q.xy = vec2(q.x - (k * q.y), (-k * q.x) - q.y) * 0.5;\n    }\n    q.x -= clamp(q.x, -2.0 * params.size, 0.0);\n    return -length(q) * sign(q.y);\n}",
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
                "Box2D",
                PrimitiveDef {
                    kind: PrimitiveKind::ParamStruct("ParamBox2D"),
                    fields: vec![PrimitiveFieldDef {
                        name: "b",
                        kind: PrimitiveFieldKind::Value(Type::Vec2),
                    }],
                    support_glsl: "struct ParamBox2D {\n    vec2 b;\n};\n\nfloat sdf0_Box2D(vec3 p, ParamBox2D params) {\n    vec2 d = abs(p.xy) - params.b;\n    return length(max(d, 0.0)) + min(max(d.x, d.y), 0.0);\n}",
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
                    support_glsl: "struct ParamSegment2D {\n    vec2 a;\n    vec2 b;\n};\n\nfloat sdf0_Segment2D(vec3 p, ParamSegment2D params) {\n    vec2 pa = p.xy - params.a;\n    vec2 ba = params.b - params.a;\n    float h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);\n    return length(pa - (ba * h));\n}",
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
                    support_glsl: "struct ParamTriangle2D {\n    vec2 p0;\n    vec2 p1;\n    vec2 p2;\n};\n\nfloat sdf0_Triangle2D(vec3 p, ParamTriangle2D params) {\n    vec2 e0 = params.p1 - params.p0;\n    vec2 e1 = params.p2 - params.p1;\n    vec2 e2 = params.p0 - params.p2;\n    vec2 v0 = p.xy - params.p0;\n    vec2 v1 = p.xy - params.p1;\n    vec2 v2 = p.xy - params.p2;\n    vec2 pq0 = v0 - (e0 * clamp(dot(v0, e0) / dot(e0, e0), 0.0, 1.0));\n    vec2 pq1 = v1 - (e1 * clamp(dot(v1, e1) / dot(e1, e1), 0.0, 1.0));\n    vec2 pq2 = v2 - (e2 * clamp(dot(v2, e2) / dot(e2, e2), 0.0, 1.0));\n    float s = sign((e0.x * e2.y) - (e0.y * e2.x));\n    vec2 d = min(min(vec2(dot(pq0, pq0), s * ((v0.x * e0.y) - (v0.y * e0.x))), vec2(dot(pq1, pq1), s * ((v1.x * e1.y) - (v1.y * e1.x)))), vec2(dot(pq2, pq2), s * ((v2.x * e2.y) - (v2.y * e2.x))));\n    return -sqrt(d.x) * sign(d.y);\n}",
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
                    support_glsl: "struct ParamPoint2D {\n    vec2 at;\n};\n\nfloat sdf0_Point2D(vec3 p, ParamPoint2D params) {\n    return length(p.xy - params.at);\n}",
                },
            ),
        ]);

        let object_ops = HashMap::from([(
            "SmoothUnion",
            ObjectOpDef {
                name: "SmoothUnion",
                value_arg_types: vec![Type::Float],
                object_arg_count: 2,
                glsl_name: "op_smooth_union",
                support_glsl: "float op_smooth_union(float a, float b, float k) {\n    float h = max(k - abs(a - b), 0.0) / k;\n    return min(a, b) - ((h * h) * k * 0.25);\n}",
            },
        )]);

        let builtins = HashMap::from([
            ("sin", Type::func(Type::Float, Type::Float)),
            ("cos", Type::func(Type::Float, Type::Float)),
            ("pow2", Type::func(Type::Float, Type::Float)),
        ]);

        Self {
            primitives,
            object_ops,
            builtins,
        }
    }
}

impl Registry {
    fn known_primitives(&self) -> Vec<KnownPrimitive> {
        let mut names: Vec<_> = self.primitives.keys().copied().collect();
        names.sort_unstable();
        names
            .into_iter()
            .map(|name| {
                let primitive = &self.primitives[name];
                KnownPrimitive {
                    name: name.to_string(),
                    sdf_name: format!("sdf0_{name}"),
                    domain: primitive.domain(name),
                    parameter_type: primitive.parameter_type(),
                    fields: primitive
                        .fields
                        .iter()
                        .map(KnownPrimitiveField::from_def)
                        .collect(),
                }
            })
            .collect()
    }
}

impl PrimitiveDef {
    fn domain(&self, name: &str) -> String {
        match &self.kind {
            PrimitiveKind::ParamStruct(param_type) => {
                format!("sdf0_{name}(vec3 p, {param_type} params) -> float")
            }
            PrimitiveKind::Polygon2D => {
                "sdf0_Polygon2D(vec2 p, vec2 vertices[POLYGON2D_MAX_VERTICES], int count) -> float"
                    .to_string()
            }
        }
    }

    fn parameter_type(&self) -> Option<String> {
        match &self.kind {
            PrimitiveKind::ParamStruct(param_type) => Some((*param_type).to_string()),
            PrimitiveKind::Polygon2D => None,
        }
    }
}

impl KnownPrimitiveField {
    fn from_def(field: &PrimitiveFieldDef) -> Self {
        Self {
            name: field.name.to_string(),
            domain: match &field.kind {
                PrimitiveFieldKind::Value(ty) => ty.surface_name().to_string(),
                PrimitiveFieldKind::Vec2List => "vec2 list".to_string(),
            },
        }
    }
}

#[derive(Clone)]
struct Env<'a> {
    registry: &'a Registry,
    types: HashMap<String, Type>,
}

impl<'a> Env<'a> {
    fn new(registry: &'a Registry) -> Self {
        let mut types = HashMap::new();
        for (name, ty) in &registry.builtins {
            types.insert((*name).to_string(), ty.clone());
        }
        for op in registry.object_ops.values() {
            let mut ty = Type::Obj3;
            for _ in 0..op.object_arg_count {
                ty = Type::func(Type::Obj3, ty);
            }
            for value_arg in op.value_arg_types.iter().rev() {
                ty = Type::func(value_arg.clone(), ty);
            }
            types.insert(op.name.to_string(), ty);
        }
        Self { registry, types }
    }

    fn insert(&mut self, name: String, ty: Type) -> Result<(), Error> {
        if self.types.contains_key(&name) {
            return Err(Error::new(format!("duplicate declaration for '{}'", name)));
        }
        self.types.insert(name, ty);
        Ok(())
    }

    fn get(&self, name: &str) -> Option<&Type> {
        self.types.get(name)
    }
}

fn infer_object_expr(expr: &Expr, env: &Env<'_>) -> Result<ObjectExpr, Error> {
    match expr {
        Expr::Ident(name) => {
            let ty = env
                .get(name)
                .ok_or_else(|| Error::new(format!("unknown identifier '{}'", name)))?;
            ensure_type(ty, &Type::Obj3, &format!("identifier '{}'", name))?;
            Ok(ObjectExpr::Var(name.clone()))
        }
        Expr::Constructor { name, fields } => {
            let primitive = env
                .registry
                .primitives
                .get(name.as_str())
                .ok_or_else(|| Error::new(format!("unknown primitive '{}'", name)))?;
            if fields.len() != primitive.fields.len() {
                return Err(Error::new(format!(
                    "primitive '{}' expects {} field(s)",
                    name,
                    primitive.fields.len()
                )));
            }
            let mut typed_fields = Vec::new();
            for field in &primitive.fields {
                let value = fields
                    .iter()
                    .find(|(field_name, _)| field_name == field.name)
                    .ok_or_else(|| {
                        Error::new(format!(
                            "primitive '{}' is missing field '{}'",
                            name, field.name
                        ))
                    })?;
                let typed = match &field.kind {
                    PrimitiveFieldKind::Value(expected_ty) => {
                        let typed = infer_value_expr(&value.1, env, None)?;
                        ensure_type(
                            &typed.ty(),
                            expected_ty,
                            &format!("field '{}.{}'", name, field.name),
                        )?;
                        PrimitiveArgExpr::Value(typed)
                    }
                    PrimitiveFieldKind::Vec2List => {
                        PrimitiveArgExpr::Vec2List(infer_vec2_list_expr(
                            &value.1,
                            env,
                            &format!("field '{}.{}'", name, field.name),
                        )?)
                    }
                };
                typed_fields.push((field.name.to_string(), typed));
            }
            Ok(ObjectExpr::Primitive {
                name: name.clone(),
                kind: primitive.kind.clone(),
                fields: typed_fields,
            })
        }
        Expr::Binary {
            op: BinOp::Add,
            left,
            right,
        } => {
            let object = infer_object_expr(left, env)?;
            let offset = infer_value_expr(right, env, None)?;
            ensure_type(&offset.ty(), &Type::Vec3, "object shift")?;
            Ok(ObjectExpr::Shift {
                object: Box::new(object),
                offset,
            })
        }
        Expr::Call { .. } => infer_object_call(expr, env),
        Expr::Number(_) | Expr::Tuple(_) => Err(Error::new("expected an Obj3 expression")),
        Expr::Binary { .. } => Err(Error::new("unsupported object expression")),
    }
}

fn infer_object_call(expr: &Expr, env: &Env<'_>) -> Result<ObjectExpr, Error> {
    let (name, args) = flatten_call(expr)?;
    let op = env
        .registry
        .object_ops
        .get(name.as_str())
        .ok_or_else(|| Error::new(format!("unknown object operator '{}'", name)))?;
    let total_args = op.value_arg_types.len() + op.object_arg_count;
    if args.len() != total_args {
        return Err(Error::new(format!(
            "operator '{}' expects {} argument(s), got {}",
            name,
            total_args,
            args.len()
        )));
    }

    let mut value_args = Vec::new();
    for (index, expected_ty) in op.value_arg_types.iter().enumerate() {
        let value = infer_value_expr(args[index], env, None)?;
        ensure_type(
            &value.ty(),
            expected_ty,
            &format!("operator '{}' value argument {}", name, index + 1),
        )?;
        value_args.push(value);
    }

    let mut object_args = Vec::new();
    for expr in &args[op.value_arg_types.len()..] {
        object_args.push(infer_object_expr(expr, env)?);
    }

    Ok(ObjectExpr::RegisteredOp {
        name: op.name.to_string(),
        glsl_name: op.glsl_name.to_string(),
        value_args,
        object_args,
    })
}

fn collect_object_support<'a>(expr: &'a ObjectExpr, names: &mut BTreeSet<&'a str>) {
    match expr {
        ObjectExpr::Var(_) => {}
        ObjectExpr::Primitive { name, .. } => {
            names.insert(name.as_str());
        }
        ObjectExpr::Shift { object, .. } => collect_object_support(object, names),
        ObjectExpr::RegisteredOp {
            name, object_args, ..
        } => {
            names.insert(name.as_str());
            for arg in object_args {
                collect_object_support(arg, names);
            }
        }
    }
}

fn infer_value_expr(
    expr: &Expr,
    env: &Env<'_>,
    lift_param: Option<&str>,
) -> Result<ValueExpr, Error> {
    match expr {
        Expr::Number(value) => Ok(ValueExpr::Float(*value)),
        Expr::Ident(name) => infer_identifier_value(name, env, lift_param),
        Expr::Tuple(items) => infer_tuple_value_expr(items, env, lift_param),
        Expr::Call { callee, args } => {
            let name = match &**callee {
                Expr::Ident(name) => name,
                _ => return Err(Error::new("only named value functions are supported")),
            };
            let mut current_ty = env
                .get(name)
                .cloned()
                .ok_or_else(|| Error::new(format!("unknown function '{}'", name)))?;
            let mut typed_args = Vec::new();
            for arg in args {
                let (input_ty, output_ty) = match current_ty {
                    Type::Func(input, output) => (*input, *output),
                    _ => {
                        return Err(Error::new(format!(
                            "'{}' is not callable with more arguments",
                            name
                        )))
                    }
                };
                let typed_arg = infer_value_expr(arg, env, lift_param)?;
                ensure_type(&typed_arg.ty(), &input_ty, &format!("call '{}(...)'", name))?;
                typed_args.push(typed_arg);
                current_ty = output_ty;
            }

            match current_ty {
                Type::Float | Type::Vec2 | Type::Vec3 => Ok(ValueExpr::Call {
                    func: name.clone(),
                    args: typed_args,
                    ty: current_ty,
                }),
                Type::Obj3 | Type::Func(_, _) => Err(Error::new(format!(
                    "value expression '{}' does not return a value type",
                    name
                ))),
            }
        }
        Expr::Binary {
            op: BinOp::Compose,
            left,
            right,
        } => infer_composed_value_expr(left, right, env, lift_param),
        Expr::Binary { op, left, right } => {
            let left = infer_value_expr(left, env, lift_param)?;
            let right = infer_value_expr(right, env, lift_param)?;
            let ty = infer_binary_type(*op, &left.ty(), &right.ty())?;
            Ok(ValueExpr::Binary {
                op: *op,
                left: Box::new(left),
                right: Box::new(right),
                ty,
            })
        }
        Expr::Constructor { .. } => {
            Err(Error::new("primitive constructors are object expressions"))
        }
    }
}

fn infer_tuple_value_expr(
    items: &[Expr],
    env: &Env<'_>,
    lift_param: Option<&str>,
) -> Result<ValueExpr, Error> {
    match items.len() {
        2 => {
            let x = infer_value_expr(&items[0], env, lift_param)?;
            let y = infer_value_expr(&items[1], env, lift_param)?;
            ensure_type(&x.ty(), &Type::Float, "vec2 element 1")?;
            ensure_type(&y.ty(), &Type::Float, "vec2 element 2")?;
            Ok(ValueExpr::Vec2(Box::new(x), Box::new(y)))
        }
        3 => {
            let x = infer_value_expr(&items[0], env, lift_param)?;
            let y = infer_value_expr(&items[1], env, lift_param)?;
            let z = infer_value_expr(&items[2], env, lift_param)?;
            ensure_type(&x.ty(), &Type::Float, "vec3 element 1")?;
            ensure_type(&y.ty(), &Type::Float, "vec3 element 2")?;
            ensure_type(&z.ty(), &Type::Float, "vec3 element 3")?;
            Ok(ValueExpr::Vec3(Box::new(x), Box::new(y), Box::new(z)))
        }
        _ => Err(Error::new(
            "only vec2 and vec3 tuples are supported in value expressions",
        )),
    }
}

fn infer_vec2_list_expr(
    expr: &Expr,
    env: &Env<'_>,
    context: &str,
) -> Result<Vec<ValueExpr>, Error> {
    let items = match expr {
        Expr::Tuple(items) => items,
        _ => {
            return Err(Error::new(format!(
                "{} expected a tuple of vec2 points",
                context
            )))
        }
    };
    if items.len() < 3 {
        return Err(Error::new(format!("{} needs at least 3 points", context)));
    }
    if items.len() > 16 {
        return Err(Error::new(format!(
            "{} supports at most 16 points",
            context
        )));
    }

    let mut points = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let point = infer_value_expr(item, env, None)?;
        ensure_type(
            &point.ty(),
            &Type::Vec2,
            &format!("{} point {}", context, index + 1),
        )?;
        points.push(point);
    }
    Ok(points)
}

fn infer_composed_value_expr(
    left: &Expr,
    right: &Expr,
    env: &Env<'_>,
    lift_param: Option<&str>,
) -> Result<ValueExpr, Error> {
    let param_name = lift_param.ok_or_else(|| {
        Error::new("function composition is only supported inside function bodies")
    })?;
    let composed = infer_function_expr(
        &Expr::Binary {
            op: BinOp::Compose,
            left: Box::new(left.clone()),
            right: Box::new(right.clone()),
        },
        env,
    )?;
    if composed.input != Type::Float {
        return Err(Error::new(
            "composed functions must accept float inputs in this compiler slice",
        ));
    }
    Ok(apply_function_expr(
        &composed,
        ValueExpr::Var(param_name.to_string(), Type::Float),
    ))
}

#[derive(Clone, Debug)]
struct FunctionExpr {
    input: Type,
    output: Type,
    kind: FunctionExprKind,
}

#[derive(Clone, Debug)]
enum FunctionExprKind {
    Named(String),
    Compose(Box<FunctionExpr>, Box<FunctionExpr>),
}

fn infer_function_expr(expr: &Expr, env: &Env<'_>) -> Result<FunctionExpr, Error> {
    match expr {
        Expr::Ident(name) => {
            let ty = env
                .get(name)
                .cloned()
                .ok_or_else(|| Error::new(format!("unknown identifier '{}'", name)))?;
            match ty {
                Type::Func(input, output) => Ok(FunctionExpr {
                    input: (*input).clone(),
                    output: (*output).clone(),
                    kind: FunctionExprKind::Named(name.clone()),
                }),
                Type::Float | Type::Vec2 | Type::Vec3 => {
                    Err(Error::new(format!("'{}' is a value, not a function", name)))
                }
                Type::Obj3 => Err(Error::new(format!(
                    "object '{}' is not a function expression",
                    name
                ))),
            }
        }
        Expr::Binary {
            op: BinOp::Compose,
            left,
            right,
        } => {
            let outer = infer_function_expr(left, env)?;
            let inner = infer_function_expr(right, env)?;
            if inner.output != outer.input {
                return Err(Error::new(format!(
                    "cannot compose {} @ {} because {} does not match {}",
                    format_function_expr(left),
                    format_function_expr(right),
                    format_type(&inner.output),
                    format_type(&outer.input)
                )));
            }
            Ok(FunctionExpr {
                input: inner.input.clone(),
                output: outer.output.clone(),
                kind: FunctionExprKind::Compose(Box::new(outer), Box::new(inner)),
            })
        }
        _ => Err(Error::new(
            "function composition currently only supports named unary functions",
        )),
    }
}

fn apply_function_expr(func: &FunctionExpr, arg: ValueExpr) -> ValueExpr {
    match &func.kind {
        FunctionExprKind::Named(name) => ValueExpr::Call {
            func: name.clone(),
            args: vec![arg],
            ty: func.output.clone(),
        },
        FunctionExprKind::Compose(outer, inner) => {
            let inner_value = apply_function_expr(inner, arg);
            apply_function_expr(outer, inner_value)
        }
    }
}

fn format_function_expr(expr: &Expr) -> String {
    match expr {
        Expr::Ident(name) => name.clone(),
        Expr::Binary {
            op: BinOp::Compose,
            left,
            right,
        } => format!(
            "({} @ {})",
            format_function_expr(left),
            format_function_expr(right)
        ),
        _ => "<function expression>".to_string(),
    }
}

fn infer_identifier_value(
    name: &str,
    env: &Env<'_>,
    lift_param: Option<&str>,
) -> Result<ValueExpr, Error> {
    let ty = env
        .get(name)
        .cloned()
        .ok_or_else(|| Error::new(format!("unknown identifier '{}'", name)))?;
    match ty {
        Type::Float | Type::Vec2 | Type::Vec3 => Ok(ValueExpr::Var(name.to_string(), ty)),
        Type::Func(input, output) => {
            if lift_param.is_none() {
                return Err(Error::new(format!(
                    "function '{}' needs an explicit call outside function bodies",
                    name
                )));
            }
            if *input != Type::Float {
                return Err(Error::new(format!(
                    "function '{}' cannot be lifted implicitly",
                    name
                )));
            }
            let param_name = lift_param.unwrap().to_string();
            Ok(ValueExpr::Call {
                func: name.to_string(),
                args: vec![ValueExpr::Var(param_name, Type::Float)],
                ty: (*output).clone(),
            })
        }
        Type::Obj3 => Err(Error::new(format!(
            "object '{}' is not a value expression",
            name
        ))),
    }
}

fn infer_binary_type(op: BinOp, left: &Type, right: &Type) -> Result<Type, Error> {
    match (op, left, right) {
        (BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div, Type::Float, Type::Float) => {
            Ok(Type::Float)
        }
        (BinOp::Add | BinOp::Sub, Type::Vec2, Type::Vec2) => Ok(Type::Vec2),
        (BinOp::Add | BinOp::Sub, Type::Vec3, Type::Vec3) => Ok(Type::Vec3),
        (BinOp::Mul | BinOp::Div, Type::Vec2, Type::Float) => Ok(Type::Vec2),
        (BinOp::Mul, Type::Float, Type::Vec2) => Ok(Type::Vec2),
        (BinOp::Mul | BinOp::Div, Type::Vec3, Type::Float) => Ok(Type::Vec3),
        (BinOp::Mul, Type::Float, Type::Vec3) => Ok(Type::Vec3),
        _ => Err(Error::new(format!(
            "unsupported operands for binary operator: {} {} {}",
            format_type(left),
            op.symbol(),
            format_type(right)
        ))),
    }
}

fn emit_value_expr(expr: &ValueExpr, helper_names: &HashMap<String, String>) -> String {
    match expr {
        ValueExpr::Float(value) => format_float(*value),
        ValueExpr::Var(name, _) => name.clone(),
        ValueExpr::Call { func, args, .. } => {
            let rendered_args = args
                .iter()
                .map(|arg| emit_value_expr(arg, helper_names))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "{}({})",
                emitted_function_name(func, helper_names),
                rendered_args
            )
        }
        ValueExpr::Binary {
            op, left, right, ..
        } => format!(
            "({} {} {})",
            emit_value_expr(left, helper_names),
            op.symbol(),
            emit_value_expr(right, helper_names)
        ),
        ValueExpr::Vec2(x, y) => format!(
            "vec2({}, {})",
            emit_value_expr(x, helper_names),
            emit_value_expr(y, helper_names)
        ),
        ValueExpr::Vec3(x, y, z) => format!(
            "vec3({}, {}, {})",
            emit_value_expr(x, helper_names),
            emit_value_expr(y, helper_names),
            emit_value_expr(z, helper_names)
        ),
    }
}

fn emit_object_expr(
    expr: &ObjectExpr,
    point_expr: &str,
    object_names: &BTreeMap<String, String>,
    helper_names: &HashMap<String, String>,
) -> String {
    match expr {
        ObjectExpr::Var(name) => object_names
            .get(name)
            .cloned()
            .unwrap_or_else(|| format!("obj_{}", name)),
        ObjectExpr::Primitive { name, kind, fields } => match kind {
            PrimitiveKind::ParamStruct(param_type) => {
                let rendered_fields = fields
                    .iter()
                    .map(|(_, expr)| match expr {
                        PrimitiveArgExpr::Value(expr) => emit_value_expr(expr, helper_names),
                        PrimitiveArgExpr::Vec2List(_) => unreachable!(),
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "sdf0_{}({}, {}({}))",
                    name, point_expr, param_type, rendered_fields
                )
            }
            PrimitiveKind::Polygon2D => {
                let vertices = fields
                    .iter()
                    .find_map(|(field_name, expr)| match (field_name.as_str(), expr) {
                        ("points", PrimitiveArgExpr::Vec2List(vertices)) => Some(vertices),
                        _ => None,
                    })
                    .unwrap();
                format!(
                    "sdf0_Polygon2D({}.xy, {}, {})",
                    point_expr,
                    emit_polygon_vertices(vertices, helper_names),
                    vertices.len()
                )
            }
        },
        ObjectExpr::Shift { object, offset } => {
            let offset = emit_value_expr(offset, helper_names);
            let shifted_point = format!("({} - {})", point_expr, offset);
            emit_object_expr(object, &shifted_point, object_names, helper_names)
        }
        ObjectExpr::RegisteredOp {
            name: _,
            glsl_name,
            value_args,
            object_args,
        } => {
            let mut args = object_args
                .iter()
                .map(|arg| emit_object_expr(arg, point_expr, object_names, helper_names))
                .collect::<Vec<_>>();
            args.extend(
                value_args
                    .iter()
                    .map(|arg| emit_value_expr(arg, helper_names)),
            );
            format!("{}({})", glsl_name, args.join(", "))
        }
    }
}

fn emit_polygon_vertices(vertices: &[ValueExpr], helper_names: &HashMap<String, String>) -> String {
    let mut rendered = vertices
        .iter()
        .map(|vertex| emit_value_expr(vertex, helper_names))
        .collect::<Vec<_>>();
    let fill = rendered
        .last()
        .cloned()
        .unwrap_or_else(|| "vec2(0.0, 0.0)".to_string());
    while rendered.len() < 16 {
        rendered.push(fill.clone());
    }
    format!("vec2[16]({})", rendered.join(", "))
}

fn emitted_function_name(name: &str, helper_names: &HashMap<String, String>) -> String {
    helper_names
        .get(name)
        .cloned()
        .unwrap_or_else(|| name.to_string())
}

fn helper_name(name: &str) -> String {
    format!("dsl_{}", name)
}

fn format_float(value: f64) -> String {
    if value.fract().abs() < f64::EPSILON {
        format!("{value:.1}")
    } else {
        value.to_string()
    }
}

fn ensure_type(actual: &Type, expected: &Type, context: &str) -> Result<(), Error> {
    if actual == expected {
        return Ok(());
    }
    Err(Error::new(format!(
        "{} expected {}, got {}",
        context,
        format_type(expected),
        format_type(actual)
    )))
}

fn format_type(ty: &Type) -> String {
    match ty {
        Type::Float => "float".to_string(),
        Type::Vec2 => "vec2".to_string(),
        Type::Vec3 => "vec3".to_string(),
        Type::Obj3 => "Obj3".to_string(),
        Type::Func(input, output) => {
            format!("func({} -> {})", format_type(input), format_type(output))
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Token {
    Ident(String),
    Number(String),
    LParen,
    RParen,
    Colon,
    Comma,
    Equal,
    Plus,
    Minus,
    Star,
    Slash,
    At,
    Arrow,
}

struct Parser<'a> {
    source: &'a str,
}

impl<'a> Parser<'a> {
    fn new(source: &'a str) -> Self {
        Self { source }
    }

    fn parse_program(&self) -> Result<Program, Error> {
        let mut inputs = Vec::new();
        let mut funcs = Vec::new();
        let mut bindings = Vec::new();
        let mut output = None;

        for raw_line in self.source.lines() {
            let line = raw_line.trim();
            if line.is_empty() {
                continue;
            }

            match self.parse_decl(line)? {
                Decl::Input(input) => inputs.push(input),
                Decl::Func(func) => funcs.push(func),
                Decl::Binding(binding) => bindings.push(binding),
                Decl::Output(out) => {
                    if output.is_some() {
                        return Err(Error::new("multiple out declarations are not supported"));
                    }
                    output = Some(out);
                }
            }
        }

        let output = output.ok_or_else(|| Error::new("missing out declaration"))?;

        Ok(Program {
            inputs,
            funcs,
            bindings,
            output,
        })
    }

    fn parse_decl(&self, line: &str) -> Result<Decl, Error> {
        if let Some(rest) = line.strip_prefix("in:") {
            let (ty, name) = split_type_name(rest.trim())?;
            return Ok(Decl::Input(InputDecl {
                name: name.to_string(),
                ty: parse_type(ty)?,
            }));
        }

        if let Some(rest) = line.strip_prefix("out:") {
            let expr_source = rest.trim();
            if let Some((left, _)) = expr_source.split_once('=') {
                if parse_type(left.trim()).is_ok() {
                    return Err(Error::new(
                        "use 'out: value' instead of 'out: type = value'",
                    ));
                }
            }
            let expr = ExprParser::new(expr_source).parse()?;
            return Ok(Decl::Output(OutputDecl { expr }));
        }

        let (left, expr_source) = split_once_required(line, '=')?;
        if left.contains(':') {
            return Err(Error::new(
                "use 'type name = value' for declarations instead of 'name : type = value'",
            ));
        }
        let (ty_source, name) = split_type_name(left.trim())?;
        let ty = parse_type(ty_source.trim())?;
        let expr = ExprParser::new(expr_source.trim()).parse()?;
        if matches!(ty, Type::Func(_, _)) {
            return Ok(Decl::Func(FuncDecl {
                name: name.to_string(),
                ty,
                expr,
            }));
        }
        Ok(Decl::Binding(BindingDecl {
            name: name.to_string(),
            ty,
            expr,
        }))
    }
}

struct ExprParser {
    tokens: Vec<Token>,
    index: usize,
}

impl ExprParser {
    fn new(source: &str) -> Self {
        Self {
            tokens: tokenize(source),
            index: 0,
        }
    }

    fn parse(mut self) -> Result<Expr, Error> {
        let expr = self.parse_add_sub()?;
        if self.peek().is_some() {
            return Err(Error::new("unexpected trailing tokens in expression"));
        }
        Ok(expr)
    }

    fn parse_add_sub(&mut self) -> Result<Expr, Error> {
        let mut expr = self.parse_mul_div()?;
        loop {
            let op = match self.peek() {
                Some(Token::Plus) => BinOp::Add,
                Some(Token::Minus) => BinOp::Sub,
                _ => break,
            };
            self.index += 1;
            let rhs = self.parse_mul_div()?;
            expr = Expr::Binary {
                op,
                left: Box::new(expr),
                right: Box::new(rhs),
            };
        }
        Ok(expr)
    }

    fn parse_mul_div(&mut self) -> Result<Expr, Error> {
        let mut expr = self.parse_compose()?;
        loop {
            let op = match self.peek() {
                Some(Token::Star) => BinOp::Mul,
                Some(Token::Slash) => BinOp::Div,
                _ => break,
            };
            self.index += 1;
            let rhs = self.parse_compose()?;
            expr = Expr::Binary {
                op,
                left: Box::new(expr),
                right: Box::new(rhs),
            };
        }
        Ok(expr)
    }

    fn parse_compose(&mut self) -> Result<Expr, Error> {
        let mut expr = self.parse_postfix()?;
        while matches!(self.peek(), Some(Token::At)) {
            self.index += 1;
            let rhs = self.parse_postfix()?;
            expr = Expr::Binary {
                op: BinOp::Compose,
                left: Box::new(expr),
                right: Box::new(rhs),
            };
        }
        Ok(expr)
    }

    fn parse_postfix(&mut self) -> Result<Expr, Error> {
        let mut expr = self.parse_primary()?;
        while matches!(self.peek(), Some(Token::LParen)) {
            let args = self.parse_positional_args()?;
            expr = Expr::Call {
                callee: Box::new(expr),
                args,
            };
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, Error> {
        match self.next() {
            Some(Token::Number(value)) => Ok(Expr::Number(value.parse::<f64>().unwrap())),
            Some(Token::Ident(name)) => {
                if matches!(self.peek(), Some(Token::LParen)) {
                    let start = self.index;
                    let args = self.parse_mixed_args()?;
                    if let Some(named) = args.named {
                        return Ok(Expr::Constructor {
                            name,
                            fields: named,
                        });
                    }
                    self.index = start;
                    let args = self.parse_positional_args()?;
                    return Ok(Expr::Call {
                        callee: Box::new(Expr::Ident(name)),
                        args,
                    });
                }
                Ok(Expr::Ident(name))
            }
            Some(Token::LParen) => self.parse_paren_or_tuple(),
            _ => Err(Error::new("unexpected token in expression")),
        }
    }

    fn parse_paren_or_tuple(&mut self) -> Result<Expr, Error> {
        let first = self.parse_add_sub()?;
        if !matches!(self.peek(), Some(Token::Comma)) {
            self.expect(Token::RParen)?;
            return Ok(first);
        }

        let mut items = vec![first];
        while matches!(self.peek(), Some(Token::Comma)) {
            self.index += 1;
            items.push(self.parse_add_sub()?);
        }
        self.expect(Token::RParen)?;
        Ok(Expr::Tuple(items))
    }

    fn parse_positional_args(&mut self) -> Result<Vec<Expr>, Error> {
        self.expect(Token::LParen)?;
        if matches!(self.peek(), Some(Token::RParen)) {
            self.index += 1;
            return Ok(Vec::new());
        }
        let mut args = Vec::new();
        loop {
            args.push(self.parse_add_sub()?);
            match self.peek() {
                Some(Token::Comma) => self.index += 1,
                Some(Token::RParen) => {
                    self.index += 1;
                    break;
                }
                _ => return Err(Error::new("expected ',' or ')' in argument list")),
            }
        }
        Ok(args)
    }

    fn parse_mixed_args(&mut self) -> Result<MixedArgs, Error> {
        self.expect(Token::LParen)?;
        if matches!(self.peek(), Some(Token::RParen)) {
            self.index += 1;
            return Ok(MixedArgs {
                named: Some(Vec::new()),
            });
        }

        let mut named = Vec::new();
        loop {
            let field_name = match (self.peek(), self.peek_n(1)) {
                (Some(Token::Ident(name)), Some(Token::Equal)) => name.clone(),
                _ => {
                    return Ok(MixedArgs { named: None });
                }
            };
            self.index += 2;
            let expr = self.parse_add_sub()?;
            named.push((field_name, expr));
            match self.peek() {
                Some(Token::Comma) => self.index += 1,
                Some(Token::RParen) => {
                    self.index += 1;
                    break;
                }
                _ => return Err(Error::new("expected ',' or ')' in named argument list")),
            }
        }
        Ok(MixedArgs { named: Some(named) })
    }

    fn expect(&mut self, expected: Token) -> Result<(), Error> {
        let token = self
            .next()
            .ok_or_else(|| Error::new("unexpected end of input"))?;
        if token == expected {
            return Ok(());
        }
        Err(Error::new("unexpected token"))
    }

    fn next(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.index).cloned();
        if token.is_some() {
            self.index += 1;
        }
        token
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.index)
    }

    fn peek_n(&self, offset: usize) -> Option<&Token> {
        self.tokens.get(self.index + offset)
    }
}

struct MixedArgs {
    named: Option<Vec<(String, Expr)>>,
}

fn tokenize(source: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = source.chars().collect();
    let mut index = 0;

    while index < chars.len() {
        let ch = chars[index];
        if ch.is_whitespace() {
            index += 1;
            continue;
        }
        if ch == '-' && chars.get(index + 1) == Some(&'>') {
            tokens.push(Token::Arrow);
            index += 2;
            continue;
        }
        if ch.is_ascii_digit()
            || (ch == '.' && chars.get(index + 1).is_some_and(|c| c.is_ascii_digit()))
        {
            let start = index;
            index += 1;
            while index < chars.len() && (chars[index].is_ascii_digit() || chars[index] == '.') {
                index += 1;
            }
            tokens.push(Token::Number(chars[start..index].iter().collect()));
            continue;
        }
        if ch.is_ascii_alphabetic() || ch == '_' {
            let start = index;
            index += 1;
            while index < chars.len()
                && (chars[index].is_ascii_alphanumeric() || chars[index] == '_')
            {
                index += 1;
            }
            tokens.push(Token::Ident(chars[start..index].iter().collect()));
            continue;
        }
        let token = match ch {
            '(' => Token::LParen,
            ')' => Token::RParen,
            ':' => Token::Colon,
            ',' => Token::Comma,
            '=' => Token::Equal,
            '+' => Token::Plus,
            '-' => Token::Minus,
            '*' => Token::Star,
            '/' => Token::Slash,
            '@' => Token::At,
            _ => panic!("unsupported token: {ch}"),
        };
        tokens.push(token);
        index += 1;
    }

    tokens
}

fn parse_type(source: &str) -> Result<Type, Error> {
    let source = source.trim();
    if source.starts_with("func(") && source.ends_with(')') {
        let inner = &source[5..source.len() - 1];
        let (input, output) = split_arrow(inner)?;
        return Ok(Type::func(parse_type(input)?, parse_type(output)?));
    }
    match source {
        "float" => Ok(Type::Float),
        "vec2" => Ok(Type::Vec2),
        "vec3" => Ok(Type::Vec3),
        "Obj3" => Ok(Type::Obj3),
        _ => Err(Error::new(format!("unsupported type '{}'", source))),
    }
}

fn split_arrow(source: &str) -> Result<(&str, &str), Error> {
    source
        .split_once("->")
        .map(|(left, right)| (left.trim(), right.trim()))
        .ok_or_else(|| Error::new("expected '->' in function type"))
}

fn split_type_name(source: &str) -> Result<(&str, &str), Error> {
    let index = source
        .rfind(' ')
        .ok_or_else(|| Error::new("expected '<type> <name>'"))?;
    Ok((&source[..index], source[index + 1..].trim()))
}

fn split_once_required(source: &str, ch: char) -> Result<(&str, &str), Error> {
    source
        .split_once(ch)
        .map(|(left, right)| (left, right))
        .ok_or_else(|| Error::new(format!("expected '{}'", ch)))
}

fn flatten_call<'a>(expr: &'a Expr) -> Result<(String, Vec<&'a Expr>), Error> {
    let mut args = Vec::new();
    let mut current = expr;
    loop {
        match current {
            Expr::Call {
                callee,
                args: call_args,
            } => {
                for arg in call_args.iter().rev() {
                    args.push(arg);
                }
                current = callee;
            }
            Expr::Ident(name) => {
                args.reverse();
                return Ok((name.clone(), args));
            }
            _ => return Err(Error::new("unsupported callable object expression")),
        }
    }
}

impl BinOp {
    fn symbol(self) -> &'static str {
        match self {
            Self::Add => "+",
            Self::Sub => "-",
            Self::Mul => "*",
            Self::Div => "/",
            Self::Compose => "@",
        }
    }
}
