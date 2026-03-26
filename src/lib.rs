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

pub fn known_primitives_by_dimension(dimension: ShapeDimension) -> Vec<KnownPrimitive> {
    known_primitives()
        .into_iter()
        .filter(|primitive| primitive.dimension == dimension)
        .collect()
}

pub fn known_primitive(name: &str) -> Option<KnownPrimitive> {
    let registry = Registry::default();
    registry.known_primitive(name)
}

pub fn known_preregistered_objects() -> Vec<PreregisteredObject> {
    let registry = Registry::default();
    registry.preregistered_objects()
}

pub fn known_builtin_objects() -> Vec<KnownBuiltinObject> {
    let registry = Registry::default();
    registry.known_builtin_objects()
}

pub fn preregistered_object(name: &str) -> Option<PreregisteredObject> {
    let registry = Registry::default();
    registry.preregistered_object(name)
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
    pub dimension: ShapeDimension,
    pub parameter_space: String,
    pub fields: Vec<KnownPrimitiveField>,
    pub type_body: Option<String>,
    pub function_body: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeDimension {
    D2,
    D3,
}

impl ShapeDimension {
    pub fn label(self) -> &'static str {
        match self {
            Self::D2 => "2D",
            Self::D3 => "3D",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KnownPrimitiveField {
    pub name: String,
    pub domain: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KnownBuiltinObject {
    pub name: String,
    pub ty: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PreregisteredObjectKind {
    Function,
    Type,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreregisteredObject {
    pub name: String,
    pub kind: PreregisteredObjectKind,
    pub body: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Type {
    Float,
    Int,
    Complex,
    Vec2,
    Vec3,
    Vec4,
    Mat2,
    Mat3,
    Mat4,
    Obj3,
    Product(Vec<Type>),
    Func(Box<Type>, Box<Type>),
}

impl Type {
    fn func(input: Type, output: Type) -> Self {
        Self::Func(Box::new(input), Box::new(output))
    }

    fn glsl_name(&self) -> &'static str {
        match self {
            Self::Float => "float",
            Self::Int => "int",
            Self::Complex => "vec2",
            Self::Vec2 => "vec2",
            Self::Vec3 => "vec3",
            Self::Vec4 => "vec4",
            Self::Mat2 => "mat2",
            Self::Mat3 => "mat3",
            Self::Mat4 => "mat4",
            Self::Obj3 | Self::Product(_) | Self::Func(_, _) => "",
        }
    }

    fn surface_name(&self) -> &'static str {
        match self {
            Self::Float => "R",
            Self::Int => "Z",
            Self::Complex => "C",
            Self::Vec2 => "R2",
            Self::Vec3 => "R3",
            Self::Vec4 => "R4",
            Self::Mat2 => "Mat2",
            Self::Mat3 => "Mat3",
            Self::Mat4 => "Mat4",
            Self::Obj3 => "Obj3",
            Self::Product(_) => "Product",
            Self::Func(_, _) => "Func",
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
    generated: bool,
}

#[derive(Clone, Debug)]
struct ValueBindingDecl {
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
    value_bindings: Vec<ValueBindingDecl>,
    bindings: Vec<BindingDecl>,
    output: OutputDecl,
}

#[derive(Clone, Debug)]
enum Decl {
    Input(InputDecl),
    Func(FuncDecl),
    ValueBinding(ValueBindingDecl),
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
        args: ConstructorArgs,
    },
}

#[derive(Clone, Debug)]
enum ConstructorArgs {
    Named(Vec<(String, Expr)>),
    Positional(Vec<Expr>),
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
    Mat3(Box<ValueExpr>, Box<ValueExpr>, Box<ValueExpr>),
    Derivative {
        epsilon: Box<ValueExpr>,
        func: FunctionExpr,
        at: Box<ValueExpr>,
    },
    Partial {
        axis: usize,
        epsilon: Box<ValueExpr>,
        func: FunctionExpr,
        at: Box<ValueExpr>,
    },
    DirectionalDerivative {
        epsilon: Box<ValueExpr>,
        direction: Box<ValueExpr>,
        func: FunctionExpr,
        at: Box<ValueExpr>,
    },
    Gradient {
        epsilon: Box<ValueExpr>,
        func: FunctionExpr,
        at: Box<ValueExpr>,
    },
    Divergence {
        epsilon: Box<ValueExpr>,
        func: FunctionExpr,
        at: Box<ValueExpr>,
    },
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
            Self::Mat3(_, _, _) => Type::Mat3,
            Self::Derivative { .. } => Type::Float,
            Self::Partial { .. } => Type::Float,
            Self::DirectionalDerivative { .. } => Type::Float,
            Self::Gradient { at, .. } => at.ty(),
            Self::Divergence { .. } => Type::Float,
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
    AmbientTransform {
        object: Box<ObjectExpr>,
        translation: ValueExpr,
        linear: ValueExpr,
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
    generated: bool,
}

#[derive(Clone, Debug)]
struct TypedValueBinding {
    name: String,
    ty: Type,
    expr: ValueExpr,
}

#[derive(Clone, Debug)]
struct TypedProgram {
    inputs: Vec<InputDecl>,
    funcs: Vec<TypedFunc>,
    value_bindings: Vec<TypedValueBinding>,
    bindings: Vec<TypedBinding>,
    output: ObjectExpr,
}

#[derive(Clone, Debug)]
struct EmitLocals {
    point: String,
    func_param: String,
    eps: String,
    dx: String,
    dy: String,
    dz: String,
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

        for binding in &program.value_bindings {
            env.insert(binding.name.clone(), binding.ty.clone())?;
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
            if !matches!(
                output_ty,
                Type::Float
                    | Type::Int
                    | Type::Complex
                    | Type::Vec2
                    | Type::Vec3
                    | Type::Vec4
                    | Type::Mat2
                    | Type::Mat3
                    | Type::Mat4
            ) {
                return Err(Error::new(format!(
                    "function '{}' currently only supports scalar, vector, or matrix outputs",
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

        let mut typed_value_bindings = Vec::new();
        for binding in &program.value_bindings {
            let expr = infer_value_expr(&binding.expr, &env, None)?;
            ensure_type(
                &expr.ty(),
                &binding.ty,
                &format!("binding '{}'", binding.name),
            )?;
            typed_value_bindings.push(TypedValueBinding {
                name: binding.name.clone(),
                ty: binding.ty.clone(),
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
                generated: binding.generated,
            });
        }

        let output = infer_object_expr(&program.output.expr, &env)?;

        Ok(Self {
            inputs: program.inputs.clone(),
            funcs: typed_funcs,
            value_bindings: typed_value_bindings,
            bindings: typed_bindings,
            output,
        })
    }

    fn emit_glsl(&self, registry: &Registry) -> String {
        let mut lines = Vec::new();
        let locals = self.emit_locals();

        for support in self.support_blocks(registry) {
            lines.extend(support.lines().map(str::to_string));
            lines.push(String::new());
        }

        for func in &self.funcs {
            let func_var_names = HashMap::from([("t".to_string(), locals.func_param.clone())]);
            lines.push(format!(
                "{} {}(float {}) {{",
                func.output.glsl_name(),
                helper_name(&func.name),
                locals.func_param
            ));
            lines.push(format!(
                "    return {};",
                emit_value_expr(&func.expr, &self.func_names(), &func_var_names)
            ));
            lines.push("}".to_string());
            lines.push(String::new());
        }

        let signature = self.scene_signature(&locals.point);
        let scene_input_names = self.scene_input_names();
        let object_bindings = self.object_bindings();
        let helper_names = self.func_names();

        for binding in &self.bindings {
            if !binding.generated {
                continue;
            }
            lines.extend(self.emit_generated_binding(
                binding,
                &object_bindings,
                &helper_names,
                &locals,
            ));
        }

        lines.push(format!("float scene_sdf({}) {{", signature.join(", ")));
        lines.extend(self.emit_value_binding_lines(&helper_names));
        let output = emit_object_expr(&self.output, &locals.point, &object_bindings, &helper_names);
        lines.push(format!("    return {};", output));
        lines.push("}".to_string());

        lines.push(String::new());
        lines.push(format!("vec3 scene_grad({}) {{", signature.join(", ")));
        lines.push(format!("    float {} = 0.0005;", locals.eps));
        let mut grad_args = vec![format!("{} + vec3({}, 0.0, 0.0)", locals.point, locals.eps)];
        grad_args.extend(scene_input_names.iter().cloned());
        lines.push(format!(
            "    float {} = scene_sdf({}) - scene_sdf({});",
            locals.dx,
            grad_args.join(", "),
            std::iter::once(format!("{} - vec3({}, 0.0, 0.0)", locals.point, locals.eps))
                .chain(scene_input_names.iter().cloned())
                .collect::<Vec<_>>()
                .join(", ")
        ));
        lines.push(format!(
            "    float {} = scene_sdf({}) - scene_sdf({});",
            locals.dy,
            std::iter::once(format!("{} + vec3(0.0, {}, 0.0)", locals.point, locals.eps))
                .chain(scene_input_names.iter().cloned())
                .collect::<Vec<_>>()
                .join(", "),
            std::iter::once(format!("{} - vec3(0.0, {}, 0.0)", locals.point, locals.eps))
                .chain(scene_input_names.iter().cloned())
                .collect::<Vec<_>>()
                .join(", ")
        ));
        lines.push(format!(
            "    float {} = scene_sdf({}) - scene_sdf({});",
            locals.dz,
            std::iter::once(format!("{} + vec3(0.0, 0.0, {})", locals.point, locals.eps))
                .chain(scene_input_names.iter().cloned())
                .collect::<Vec<_>>()
                .join(", "),
            std::iter::once(format!("{} - vec3(0.0, 0.0, {})", locals.point, locals.eps))
                .chain(scene_input_names.iter().cloned())
                .collect::<Vec<_>>()
                .join(", ")
        ));
        lines.push(format!(
            "    return normalize(vec3({}, {}, {}));",
            locals.dx, locals.dy, locals.dz
        ));
        lines.push("}".to_string());

        lines.join("\n")
    }

    fn scene_signature(&self, point_name: &str) -> Vec<String> {
        let mut signature = vec![format!("vec3 {}", point_name)];
        for input in &self.inputs {
            match input.ty {
                Type::Float
                | Type::Int
                | Type::Complex
                | Type::Vec2
                | Type::Vec3
                | Type::Vec4
                | Type::Mat2
                | Type::Mat3
                | Type::Mat4 => signature.push(format!("{} {}", input.ty.glsl_name(), input.name)),
                Type::Obj3 | Type::Product(_) | Type::Func(_, _) => {}
            }
        }
        signature
    }

    fn scene_input_names(&self) -> Vec<String> {
        self.inputs
            .iter()
            .filter_map(|input| match input.ty {
                Type::Float
                | Type::Int
                | Type::Complex
                | Type::Vec2
                | Type::Vec3
                | Type::Vec4
                | Type::Mat2
                | Type::Mat3
                | Type::Mat4 => Some(input.name.clone()),
                Type::Obj3 | Type::Product(_) | Type::Func(_, _) => None,
            })
            .collect()
    }

    fn emit_value_binding_lines(&self, helper_names: &HashMap<String, String>) -> Vec<String> {
        self.value_bindings
            .iter()
            .map(|binding| {
                format!(
                    "    {} {} = {};",
                    binding.ty.glsl_name(),
                    binding.name,
                    emit_plain_value_expr(&binding.expr, helper_names)
                )
            })
            .collect()
    }

    fn object_bindings(&self) -> BTreeMap<String, ObjectExpr> {
        self.bindings
            .iter()
            .map(|binding| (binding.name.clone(), binding.expr.clone()))
            .collect()
    }

    fn emit_generated_binding(
        &self,
        binding: &TypedBinding,
        object_bindings: &BTreeMap<String, ObjectExpr>,
        helper_names: &HashMap<String, String>,
        locals: &EmitLocals,
    ) -> Vec<String> {
        let signature = self.scene_signature(&locals.point);
        let scene_input_names = self.scene_input_names();
        let mut lines = Vec::new();

        lines.push(format!(
            "float sdf_{}({}) {{",
            binding.name,
            signature.join(", ")
        ));
        lines.extend(self.emit_value_binding_lines(helper_names));
        lines.push(format!(
            "    return {};",
            emit_object_expr(&binding.expr, &locals.point, object_bindings, helper_names)
        ));
        lines.push("}".to_string());
        lines.push(String::new());

        lines.push(format!(
            "vec3 grad_sdf_{}({}) {{",
            binding.name,
            signature.join(", ")
        ));
        lines.push(format!("    float {} = 0.0005;", locals.eps));
        let forward_x =
            std::iter::once(format!("{} + vec3({}, 0.0, 0.0)", locals.point, locals.eps))
                .chain(scene_input_names.iter().cloned())
                .collect::<Vec<_>>()
                .join(", ");
        let backward_x =
            std::iter::once(format!("{} - vec3({}, 0.0, 0.0)", locals.point, locals.eps))
                .chain(scene_input_names.iter().cloned())
                .collect::<Vec<_>>()
                .join(", ");
        let forward_y =
            std::iter::once(format!("{} + vec3(0.0, {}, 0.0)", locals.point, locals.eps))
                .chain(scene_input_names.iter().cloned())
                .collect::<Vec<_>>()
                .join(", ");
        let backward_y =
            std::iter::once(format!("{} - vec3(0.0, {}, 0.0)", locals.point, locals.eps))
                .chain(scene_input_names.iter().cloned())
                .collect::<Vec<_>>()
                .join(", ");
        let forward_z =
            std::iter::once(format!("{} + vec3(0.0, 0.0, {})", locals.point, locals.eps))
                .chain(scene_input_names.iter().cloned())
                .collect::<Vec<_>>()
                .join(", ");
        let backward_z =
            std::iter::once(format!("{} - vec3(0.0, 0.0, {})", locals.point, locals.eps))
                .chain(scene_input_names.iter().cloned())
                .collect::<Vec<_>>()
                .join(", ");
        lines.push(format!(
            "    float {} = sdf_{}({}) - sdf_{}({});",
            locals.dx, binding.name, forward_x, binding.name, backward_x
        ));
        lines.push(format!(
            "    float {} = sdf_{}({}) - sdf_{}({});",
            locals.dy, binding.name, forward_y, binding.name, backward_y
        ));
        lines.push(format!(
            "    float {} = sdf_{}({}) - sdf_{}({});",
            locals.dz, binding.name, forward_z, binding.name, backward_z
        ));
        lines.push(format!(
            "    return normalize(vec3({}, {}, {}));",
            locals.dx, locals.dy, locals.dz
        ));
        lines.push("}".to_string());
        lines.push(String::new());

        lines
    }

    fn emit_locals(&self) -> EmitLocals {
        let mut forbidden = BTreeSet::new();
        for input in &self.inputs {
            match input.ty {
                Type::Float
                | Type::Int
                | Type::Complex
                | Type::Vec2
                | Type::Vec3
                | Type::Vec4
                | Type::Mat2
                | Type::Mat3
                | Type::Mat4 => {
                    forbidden.insert(input.name.clone());
                }
                Type::Obj3 | Type::Product(_) | Type::Func(_, _) => {}
            }
        }
        for binding in &self.value_bindings {
            forbidden.insert(binding.name.clone());
        }

        let point = unique_local_name("p", &forbidden);
        forbidden.insert(point.clone());
        let func_param = unique_local_name("t", &forbidden);
        forbidden.insert(func_param.clone());
        let eps = unique_local_name("eps", &forbidden);
        forbidden.insert(eps.clone());
        let dx = unique_local_name("dx", &forbidden);
        forbidden.insert(dx.clone());
        let dy = unique_local_name("dy", &forbidden);
        forbidden.insert(dy.clone());
        let dz = unique_local_name("dz", &forbidden);

        EmitLocals {
            point,
            func_param,
            eps,
            dx,
            dy,
            dz,
        }
    }

    fn support_blocks(&self, registry: &Registry) -> Vec<&'static str> {
        let mut names = BTreeSet::new();
        for func in &self.funcs {
            collect_value_support(&func.expr, &mut names);
        }
        for binding in &self.value_bindings {
            collect_value_support(&binding.expr, &mut names);
        }
        for binding in &self.bindings {
            collect_object_support(&binding.expr, &mut names);
        }
        collect_object_support(&self.output, &mut names);

        let mut blocks = Vec::new();
        for name in names {
            if let Some(primitive) = registry.primitives.get(name.as_str()) {
                blocks.push(primitive.support_glsl);
                continue;
            }
            if let Some(op) = registry.object_ops.get(name.as_str()) {
                blocks.push(op.support_glsl);
                continue;
            }
            if let Some(func) = registry.value_funcs.get(name.as_str()) {
                if let Some(support_glsl) = func.support_glsl {
                    blocks.push(support_glsl);
                }
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
    associative_binary: bool,
    glsl_name: &'static str,
    support_glsl: &'static str,
}

#[derive(Clone, Debug)]
struct ValueFuncDef {
    ty: Type,
    support_glsl: Option<&'static str>,
    listed: bool,
}

#[derive(Clone, Debug)]
struct Registry {
    primitives: HashMap<&'static str, PrimitiveDef>,
    object_ops: HashMap<&'static str, ObjectOpDef>,
    value_funcs: HashMap<&'static str, ValueFuncDef>,
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
                    support_glsl: "",
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
    fn known_primitives(&self) -> Vec<KnownPrimitive> {
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

    fn known_primitive(&self, name: &str) -> Option<KnownPrimitive> {
        self.known_primitives()
            .into_iter()
            .find(|primitive| primitive.name == name)
    }

    fn known_builtin_objects(&self) -> Vec<KnownBuiltinObject> {
        let mut objects = Vec::new();

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
            });
        }

        let mut op_names: Vec<_> = self.object_ops.keys().copied().collect();
        op_names.sort_unstable();
        for name in op_names {
            let op = &self.object_ops[name];
            objects.push(KnownBuiltinObject {
                name: op.name.to_string(),
                ty: format_object_type(&object_op_type(op)),
            });
        }

        objects
    }

    fn preregistered_objects(&self) -> Vec<PreregisteredObject> {
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

    fn preregistered_object(&self, name: &str) -> Option<PreregisteredObject> {
        self.preregistered_objects()
            .into_iter()
            .find(|object| object.name == name)
    }
}

fn shape_dimension(name: &str) -> ShapeDimension {
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
                PrimitiveFieldKind::Value(ty) => ty.surface_name().to_string(),
                PrimitiveFieldKind::Vec2List => "R2 list".to_string(),
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
        for (name, func) in &registry.value_funcs {
            types.insert((*name).to_string(), func.ty.clone());
        }
        for op in registry.object_ops.values() {
            types.insert(op.name.to_string(), object_op_type(op));
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
        Expr::Constructor { name, args } => {
            let primitive = if let Some(primitive) = env.registry.primitives.get(name.as_str()) {
                primitive
            } else {
                match args {
                    ConstructorArgs::Positional(positional) => {
                        return infer_object_call(
                            &Expr::Call {
                                callee: Box::new(Expr::Ident(name.clone())),
                                args: positional.clone(),
                            },
                            env,
                        )
                    }
                    ConstructorArgs::Named(_) => {
                        return Err(Error::new(format!("unknown primitive '{}'", name)))
                    }
                }
            };
            let fields = match args {
                ConstructorArgs::Named(fields) => fields.clone(),
                ConstructorArgs::Positional(values) => {
                    if let Some(packed) = pack_single_vector_field_args(primitive, values, env)? {
                        packed
                    } else {
                        if values.len() != primitive.fields.len() {
                            return Err(Error::new(format!(
                                "primitive '{}' expects {} field(s)",
                                name,
                                primitive.fields.len()
                            )));
                        }
                        primitive
                            .fields
                            .iter()
                            .zip(values.iter())
                            .map(|(field, value)| (field.name.to_string(), value.clone()))
                            .collect()
                    }
                }
            };
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
            Ok(ObjectExpr::AmbientTransform {
                object: Box::new(object),
                translation: offset,
                linear: identity_mat3(),
            })
        }
        Expr::Binary {
            op: BinOp::Mul,
            left,
            right,
        } => {
            let linear = infer_value_expr(left, env, None)?;
            ensure_type(&linear.ty(), &Type::Mat3, "object action")?;
            let object = infer_object_expr(right, env)?;
            Ok(ObjectExpr::AmbientTransform {
                object: Box::new(object),
                translation: zero_vec3(),
                linear,
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
    let min_total_args = op.value_arg_types.len() + op.object_arg_count;
    if op.associative_binary {
        if args.len() < min_total_args {
            return Err(Error::new(format!(
                "operator '{}' expects at least {} argument(s), got {}",
                name,
                min_total_args,
                args.len()
            )));
        }
    } else if args.len() != min_total_args {
        return Err(Error::new(format!(
            "operator '{}' expects {} argument(s), got {}",
            name,
            min_total_args,
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

    if op.associative_binary {
        Ok(fold_associative_registered_op(
            op.name,
            op.glsl_name,
            &value_args,
            &object_args,
        ))
    } else {
        Ok(ObjectExpr::RegisteredOp {
            name: op.name.to_string(),
            glsl_name: op.glsl_name.to_string(),
            value_args,
            object_args,
        })
    }
}

fn fold_associative_registered_op(
    name: &str,
    glsl_name: &str,
    value_args: &[ValueExpr],
    object_args: &[ObjectExpr],
) -> ObjectExpr {
    debug_assert!(object_args.len() >= 2);
    if object_args.len() == 2 {
        return ObjectExpr::RegisteredOp {
            name: name.to_string(),
            glsl_name: glsl_name.to_string(),
            value_args: value_args.to_vec(),
            object_args: object_args.to_vec(),
        };
    }
    if object_args.len() == 3 {
        let right = fold_associative_registered_op(name, glsl_name, value_args, &object_args[1..]);
        return ObjectExpr::RegisteredOp {
            name: name.to_string(),
            glsl_name: glsl_name.to_string(),
            value_args: value_args.to_vec(),
            object_args: vec![object_args[0].clone(), right],
        };
    }

    let split = object_args.len() / 2;
    let left = fold_associative_registered_op(name, glsl_name, value_args, &object_args[..split]);
    let right = fold_associative_registered_op(name, glsl_name, value_args, &object_args[split..]);
    ObjectExpr::RegisteredOp {
        name: name.to_string(),
        glsl_name: glsl_name.to_string(),
        value_args: value_args.to_vec(),
        object_args: vec![left, right],
    }
}

fn collect_object_support(expr: &ObjectExpr, names: &mut BTreeSet<String>) {
    match expr {
        ObjectExpr::Var(_) => {}
        ObjectExpr::Primitive { name, fields, .. } => {
            names.insert(name.clone());
            for (_, value) in fields {
                match value {
                    PrimitiveArgExpr::Value(value) => collect_value_support(value, names),
                    PrimitiveArgExpr::Vec2List(vertices) => {
                        for vertex in vertices {
                            collect_value_support(vertex, names);
                        }
                    }
                }
            }
        }
        ObjectExpr::AmbientTransform {
            object,
            translation,
            linear,
        } => {
            collect_value_support(translation, names);
            collect_value_support(linear, names);
            collect_object_support(object, names);
        }
        ObjectExpr::RegisteredOp {
            name,
            value_args,
            object_args,
            ..
        } => {
            names.insert(name.clone());
            for arg in value_args {
                collect_value_support(arg, names);
            }
            for arg in object_args {
                collect_object_support(arg, names);
            }
        }
    }
}

fn collect_value_support(expr: &ValueExpr, names: &mut BTreeSet<String>) {
    match expr {
        ValueExpr::Float(_) | ValueExpr::Var(_, _) => {}
        ValueExpr::Call { func, args, .. } => {
            names.insert(func.clone());
            for arg in args {
                collect_value_support(arg, names);
            }
        }
        ValueExpr::Binary { left, right, .. } => {
            collect_value_support(left, names);
            collect_value_support(right, names);
        }
        ValueExpr::Vec2(x, y) => {
            collect_value_support(x, names);
            collect_value_support(y, names);
        }
        ValueExpr::Vec3(x, y, z) => {
            collect_value_support(x, names);
            collect_value_support(y, names);
            collect_value_support(z, names);
        }
        ValueExpr::Mat3(r0, r1, r2) => {
            collect_value_support(r0, names);
            collect_value_support(r1, names);
            collect_value_support(r2, names);
        }
        ValueExpr::Derivative { epsilon, func, at }
        | ValueExpr::Gradient { epsilon, func, at }
        | ValueExpr::Divergence { epsilon, func, at } => {
            collect_value_support(epsilon, names);
            collect_function_support(func, names);
            collect_value_support(at, names);
        }
        ValueExpr::Partial {
            epsilon, func, at, ..
        } => {
            collect_value_support(epsilon, names);
            collect_function_support(func, names);
            collect_value_support(at, names);
        }
        ValueExpr::DirectionalDerivative {
            epsilon,
            direction,
            func,
            at,
        } => {
            collect_value_support(epsilon, names);
            collect_value_support(direction, names);
            collect_function_support(func, names);
            collect_value_support(at, names);
        }
    }
}

fn collect_function_support(func: &FunctionExpr, names: &mut BTreeSet<String>) {
    match &func.kind {
        FunctionExprKind::Named(name) => {
            names.insert(name.clone());
        }
        FunctionExprKind::Compose(outer, inner) => {
            collect_function_support(outer, names);
            collect_function_support(inner, names);
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
            if let Some(result) = infer_differential_builtin(expr, env, lift_param)? {
                return Ok(result);
            }
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
                Type::Float
                | Type::Int
                | Type::Complex
                | Type::Vec2
                | Type::Vec3
                | Type::Vec4
                | Type::Mat2
                | Type::Mat3
                | Type::Mat4 => Ok(ValueExpr::Call {
                    func: name.clone(),
                    args: typed_args,
                    ty: current_ty,
                }),
                Type::Obj3 | Type::Product(_) | Type::Func(_, _) => Err(Error::new(format!(
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
        Expr::Constructor { name, args } => match args {
            ConstructorArgs::Positional(args) => infer_value_expr(
                &Expr::Call {
                    callee: Box::new(Expr::Ident(name.clone())),
                    args: args.clone(),
                },
                env,
                lift_param,
            ),
            ConstructorArgs::Named(_) => {
                Err(Error::new("primitive constructors are object expressions"))
            }
        },
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
            if x.ty() == Type::Float && y.ty() == Type::Float && z.ty() == Type::Float {
                return Ok(ValueExpr::Vec3(Box::new(x), Box::new(y), Box::new(z)));
            }
            ensure_type(&x.ty(), &Type::Vec3, "mat3 row 1")?;
            ensure_type(&y.ty(), &Type::Vec3, "mat3 row 2")?;
            ensure_type(&z.ty(), &Type::Vec3, "mat3 row 3")?;
            Ok(ValueExpr::Mat3(Box::new(x), Box::new(y), Box::new(z)))
        }
        _ => Err(Error::new(
            "only vec2, vec3, and mat3 tuples are supported in value expressions",
        )),
    }
}

fn infer_differential_builtin(
    expr: &Expr,
    env: &Env<'_>,
    lift_param: Option<&str>,
) -> Result<Option<ValueExpr>, Error> {
    let (name, args) = match flatten_call(expr) {
        Ok(parts) => parts,
        Err(_) => return Ok(None),
    };

    let result = match name.as_str() {
        "derivative" => Some(infer_derivative_builtin(&args, env, lift_param)?),
        "partialX" => Some(infer_partial_builtin(&args, env, 0)?),
        "partialY" => Some(infer_partial_builtin(&args, env, 1)?),
        "partialZ" => Some(infer_partial_builtin(&args, env, 2)?),
        "directionalDerivative" => Some(infer_directional_derivative_builtin(&args, env)?),
        "gradient" => Some(infer_gradient_builtin(&args, env)?),
        "divergence" => Some(infer_divergence_builtin(&args, env)?),
        _ => None,
    };

    Ok(result)
}

fn infer_derivative_builtin(
    args: &[&Expr],
    env: &Env<'_>,
    lift_param: Option<&str>,
) -> Result<ValueExpr, Error> {
    if args.len() != 3 && !(args.len() == 2 && lift_param.is_some()) {
        return Err(Error::new(
            "derivative expects epsilon, a unary function, and an evaluation point",
        ));
    }
    let epsilon = infer_value_expr(args[0], env, None)?;
    ensure_type(&epsilon.ty(), &Type::Float, "derivative epsilon")?;
    let func = infer_function_expr(args[1], env)?;
    if func.input != Type::Float || func.output != Type::Float {
        return Err(Error::new("derivative expects a func(float -> float)"));
    }
    let at = if let Some(expr) = args.get(2) {
        let at = infer_value_expr(expr, env, None)?;
        ensure_type(&at.ty(), &Type::Float, "derivative evaluation point")?;
        at
    } else {
        ValueExpr::Var(lift_param.unwrap().to_string(), Type::Float)
    };
    Ok(ValueExpr::Derivative {
        epsilon: Box::new(epsilon),
        func,
        at: Box::new(at),
    })
}

fn infer_partial_builtin(args: &[&Expr], env: &Env<'_>, axis: usize) -> Result<ValueExpr, Error> {
    if args.len() != 3 {
        return Err(Error::new(
            "partial derivative expects epsilon, a scalar field, and an evaluation point",
        ));
    }
    let epsilon = infer_value_expr(args[0], env, None)?;
    ensure_type(&epsilon.ty(), &Type::Float, "partial derivative epsilon")?;
    let func = infer_function_expr(args[1], env)?;
    if func.input != Type::Vec3 || func.output != Type::Float {
        return Err(Error::new(
            "partial derivatives currently expect a func(vec3 -> float)",
        ));
    }
    let at = infer_value_expr(args[2], env, None)?;
    ensure_type(&at.ty(), &Type::Vec3, "partial derivative evaluation point")?;
    Ok(ValueExpr::Partial {
        axis,
        epsilon: Box::new(epsilon),
        func,
        at: Box::new(at),
    })
}

fn infer_directional_derivative_builtin(args: &[&Expr], env: &Env<'_>) -> Result<ValueExpr, Error> {
    if args.len() != 4 {
        return Err(Error::new(
            "directionalDerivative expects epsilon, a direction, a scalar field, and an evaluation point",
        ));
    }
    let epsilon = infer_value_expr(args[0], env, None)?;
    ensure_type(
        &epsilon.ty(),
        &Type::Float,
        "directional derivative epsilon",
    )?;
    let direction = infer_value_expr(args[1], env, None)?;
    ensure_type(
        &direction.ty(),
        &Type::Vec3,
        "directional derivative direction",
    )?;
    let func = infer_function_expr(args[2], env)?;
    if func.input != Type::Vec3 || func.output != Type::Float {
        return Err(Error::new(
            "directionalDerivative currently expects a func(vec3 -> float)",
        ));
    }
    let at = infer_value_expr(args[3], env, None)?;
    ensure_type(
        &at.ty(),
        &Type::Vec3,
        "directional derivative evaluation point",
    )?;
    Ok(ValueExpr::DirectionalDerivative {
        epsilon: Box::new(epsilon),
        direction: Box::new(direction),
        func,
        at: Box::new(at),
    })
}

fn infer_gradient_builtin(args: &[&Expr], env: &Env<'_>) -> Result<ValueExpr, Error> {
    if args.len() != 3 {
        return Err(Error::new(
            "gradient expects epsilon, a scalar field, and an evaluation point",
        ));
    }
    let epsilon = infer_value_expr(args[0], env, None)?;
    ensure_type(&epsilon.ty(), &Type::Float, "gradient epsilon")?;
    let func = infer_function_expr(args[1], env)?;
    if func.input != Type::Vec3 || func.output != Type::Float {
        return Err(Error::new(
            "gradient currently expects a func(vec3 -> float)",
        ));
    }
    let at = infer_value_expr(args[2], env, None)?;
    ensure_type(&at.ty(), &Type::Vec3, "gradient evaluation point")?;
    Ok(ValueExpr::Gradient {
        epsilon: Box::new(epsilon),
        func,
        at: Box::new(at),
    })
}

fn infer_divergence_builtin(args: &[&Expr], env: &Env<'_>) -> Result<ValueExpr, Error> {
    if args.len() != 3 {
        return Err(Error::new(
            "divergence expects epsilon, a vector field, and an evaluation point",
        ));
    }
    let epsilon = infer_value_expr(args[0], env, None)?;
    ensure_type(&epsilon.ty(), &Type::Float, "divergence epsilon")?;
    let func = infer_function_expr(args[1], env)?;
    if func.input != Type::Vec3 || func.output != Type::Vec3 {
        return Err(Error::new(
            "divergence currently expects a func(vec3 -> vec3)",
        ));
    }
    let at = infer_value_expr(args[2], env, None)?;
    ensure_type(&at.ty(), &Type::Vec3, "divergence evaluation point")?;
    Ok(ValueExpr::Divergence {
        epsilon: Box::new(epsilon),
        func,
        at: Box::new(at),
    })
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

fn pack_single_vector_field_args(
    primitive: &PrimitiveDef,
    values: &[Expr],
    env: &Env<'_>,
) -> Result<Option<Vec<(String, Expr)>>, Error> {
    if primitive.fields.len() != 1 {
        return Ok(None);
    }
    let field = &primitive.fields[0];
    let expected = match field.kind {
        PrimitiveFieldKind::Value(Type::Vec2) => 2,
        PrimitiveFieldKind::Value(Type::Vec3) => 3,
        PrimitiveFieldKind::Value(Type::Vec4) => 4,
        _ => return Ok(None),
    };
    if values.len() != expected {
        return Ok(None);
    }

    for (index, value) in values.iter().enumerate() {
        let typed = infer_value_expr(value, env, None)?;
        ensure_type(
            &typed.ty(),
            &Type::Float,
            &format!("{} element {}", field.name, index + 1),
        )?;
    }

    Ok(Some(vec![(
        field.name.to_string(),
        Expr::Tuple(values.to_vec()),
    )]))
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
                Type::Float
                | Type::Int
                | Type::Complex
                | Type::Vec2
                | Type::Vec3
                | Type::Vec4
                | Type::Mat2
                | Type::Mat3
                | Type::Mat4 => Err(Error::new(format!("'{}' is a value, not a function", name))),
                Type::Obj3 | Type::Product(_) => Err(Error::new(format!(
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
        Type::Float
        | Type::Int
        | Type::Complex
        | Type::Vec2
        | Type::Vec3
        | Type::Vec4
        | Type::Mat2
        | Type::Mat3
        | Type::Mat4 => Ok(ValueExpr::Var(name.to_string(), ty)),
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
        Type::Obj3 | Type::Product(_) => Err(Error::new(format!(
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
        (BinOp::Add | BinOp::Sub, Type::Complex, Type::Complex) => Ok(Type::Complex),
        (BinOp::Add | BinOp::Sub, Type::Vec2, Type::Vec2) => Ok(Type::Vec2),
        (BinOp::Mul | BinOp::Div, Type::Complex, Type::Float) => Ok(Type::Complex),
        (BinOp::Mul, Type::Float, Type::Complex) => Ok(Type::Complex),
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

fn emit_value_expr(
    expr: &ValueExpr,
    helper_names: &HashMap<String, String>,
    value_names: &HashMap<String, String>,
) -> String {
    match expr {
        ValueExpr::Float(value) => format_float(*value),
        ValueExpr::Var(name, _) => value_names
            .get(name)
            .cloned()
            .unwrap_or_else(|| name.clone()),
        ValueExpr::Call { func, args, .. } => {
            let rendered_args = args
                .iter()
                .map(|arg| emit_value_expr(arg, helper_names, value_names))
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
        } => {
            if *op == BinOp::Sub && is_float_literal(left, 0.0) {
                return format!("(-{})", emit_value_expr(right, helper_names, value_names));
            }
            format!(
                "({} {} {})",
                emit_value_expr(left, helper_names, value_names),
                op.symbol(),
                emit_value_expr(right, helper_names, value_names)
            )
        }
        ValueExpr::Vec2(x, y) => format!(
            "vec2({}, {})",
            emit_value_expr(x, helper_names, value_names),
            emit_value_expr(y, helper_names, value_names)
        ),
        ValueExpr::Vec3(x, y, z) => format!(
            "vec3({}, {}, {})",
            emit_value_expr(x, helper_names, value_names),
            emit_value_expr(y, helper_names, value_names),
            emit_value_expr(z, helper_names, value_names)
        ),
        ValueExpr::Mat3(r0, r1, r2) => format!(
            "transpose(mat3({}, {}, {}))",
            emit_value_expr(r0, helper_names, value_names),
            emit_value_expr(r1, helper_names, value_names),
            emit_value_expr(r2, helper_names, value_names)
        ),
        ValueExpr::Derivative { epsilon, func, at } => {
            emit_scalar_derivative(func, epsilon, at, helper_names, value_names)
        }
        ValueExpr::Partial {
            axis,
            epsilon,
            func,
            at,
        } => emit_partial_derivative(*axis, func, epsilon, at, helper_names, value_names),
        ValueExpr::DirectionalDerivative {
            epsilon,
            direction,
            func,
            at,
        } => emit_directional_derivative(func, epsilon, direction, at, helper_names, value_names),
        ValueExpr::Gradient { epsilon, func, at } => {
            emit_gradient(func, epsilon, at, helper_names, value_names)
        }
        ValueExpr::Divergence { epsilon, func, at } => {
            emit_divergence(func, epsilon, at, helper_names, value_names)
        }
    }
}

fn emit_plain_value_expr(expr: &ValueExpr, helper_names: &HashMap<String, String>) -> String {
    emit_value_expr(expr, helper_names, &HashMap::new())
}

fn emit_function_application(
    func: &FunctionExpr,
    arg: ValueExpr,
    helper_names: &HashMap<String, String>,
    value_names: &HashMap<String, String>,
) -> String {
    emit_value_expr(&apply_function_expr(func, arg), helper_names, value_names)
}

fn emit_scalar_derivative(
    func: &FunctionExpr,
    epsilon: &ValueExpr,
    at: &ValueExpr,
    helper_names: &HashMap<String, String>,
    value_names: &HashMap<String, String>,
) -> String {
    let plus = ValueExpr::Binary {
        op: BinOp::Add,
        left: Box::new(at.clone()),
        right: Box::new(epsilon.clone()),
        ty: Type::Float,
    };
    let minus = ValueExpr::Binary {
        op: BinOp::Sub,
        left: Box::new(at.clone()),
        right: Box::new(epsilon.clone()),
        ty: Type::Float,
    };
    let twice = ValueExpr::Binary {
        op: BinOp::Mul,
        left: Box::new(ValueExpr::Float(2.0)),
        right: Box::new(epsilon.clone()),
        ty: Type::Float,
    };
    format!(
        "(({} - {}) / {})",
        emit_function_application(func, plus, helper_names, value_names),
        emit_function_application(func, minus, helper_names, value_names),
        emit_value_expr(&twice, helper_names, value_names)
    )
}

fn emit_partial_derivative(
    axis: usize,
    func: &FunctionExpr,
    epsilon: &ValueExpr,
    at: &ValueExpr,
    helper_names: &HashMap<String, String>,
    value_names: &HashMap<String, String>,
) -> String {
    let plus = emit_vec3_axis_offset(at.clone(), epsilon.clone(), axis, BinOp::Add);
    let minus = emit_vec3_axis_offset(at.clone(), epsilon.clone(), axis, BinOp::Sub);
    let twice = ValueExpr::Binary {
        op: BinOp::Mul,
        left: Box::new(ValueExpr::Float(2.0)),
        right: Box::new(epsilon.clone()),
        ty: Type::Float,
    };
    format!(
        "(({} - {}) / {})",
        emit_function_application(func, plus, helper_names, value_names),
        emit_function_application(func, minus, helper_names, value_names),
        emit_value_expr(&twice, helper_names, value_names)
    )
}

fn emit_directional_derivative(
    func: &FunctionExpr,
    epsilon: &ValueExpr,
    direction: &ValueExpr,
    at: &ValueExpr,
    helper_names: &HashMap<String, String>,
    value_names: &HashMap<String, String>,
) -> String {
    let direction_step = ValueExpr::Binary {
        op: BinOp::Mul,
        left: Box::new(ValueExpr::Call {
            func: "normalize".to_string(),
            args: vec![direction.clone()],
            ty: Type::Vec3,
        }),
        right: Box::new(epsilon.clone()),
        ty: Type::Vec3,
    };
    let plus = ValueExpr::Binary {
        op: BinOp::Add,
        left: Box::new(at.clone()),
        right: Box::new(direction_step.clone()),
        ty: Type::Vec3,
    };
    let minus = ValueExpr::Binary {
        op: BinOp::Sub,
        left: Box::new(at.clone()),
        right: Box::new(direction_step),
        ty: Type::Vec3,
    };
    let twice = ValueExpr::Binary {
        op: BinOp::Mul,
        left: Box::new(ValueExpr::Float(2.0)),
        right: Box::new(epsilon.clone()),
        ty: Type::Float,
    };
    format!(
        "(({} - {}) / {})",
        emit_function_application(func, plus, helper_names, value_names),
        emit_function_application(func, minus, helper_names, value_names),
        emit_value_expr(&twice, helper_names, value_names)
    )
}

fn emit_gradient(
    func: &FunctionExpr,
    epsilon: &ValueExpr,
    at: &ValueExpr,
    helper_names: &HashMap<String, String>,
    value_names: &HashMap<String, String>,
) -> String {
    format!(
        "vec3({}, {}, {})",
        emit_partial_derivative(0, func, epsilon, at, helper_names, value_names),
        emit_partial_derivative(1, func, epsilon, at, helper_names, value_names),
        emit_partial_derivative(2, func, epsilon, at, helper_names, value_names)
    )
}

fn emit_divergence(
    func: &FunctionExpr,
    epsilon: &ValueExpr,
    at: &ValueExpr,
    helper_names: &HashMap<String, String>,
    value_names: &HashMap<String, String>,
) -> String {
    let twice = ValueExpr::Binary {
        op: BinOp::Mul,
        left: Box::new(ValueExpr::Float(2.0)),
        right: Box::new(epsilon.clone()),
        ty: Type::Float,
    };
    let denom = emit_value_expr(&twice, helper_names, value_names);
    let dx_plus = emit_function_application(
        func,
        emit_vec3_axis_offset(at.clone(), epsilon.clone(), 0, BinOp::Add),
        helper_names,
        value_names,
    );
    let dx_minus = emit_function_application(
        func,
        emit_vec3_axis_offset(at.clone(), epsilon.clone(), 0, BinOp::Sub),
        helper_names,
        value_names,
    );
    let dy_plus = emit_function_application(
        func,
        emit_vec3_axis_offset(at.clone(), epsilon.clone(), 1, BinOp::Add),
        helper_names,
        value_names,
    );
    let dy_minus = emit_function_application(
        func,
        emit_vec3_axis_offset(at.clone(), epsilon.clone(), 1, BinOp::Sub),
        helper_names,
        value_names,
    );
    let dz_plus = emit_function_application(
        func,
        emit_vec3_axis_offset(at.clone(), epsilon.clone(), 2, BinOp::Add),
        helper_names,
        value_names,
    );
    let dz_minus = emit_function_application(
        func,
        emit_vec3_axis_offset(at.clone(), epsilon.clone(), 2, BinOp::Sub),
        helper_names,
        value_names,
    );
    format!(
        "((({dx_plus}).x - ({dx_minus}).x) / {denom} + (({dy_plus}).y - ({dy_minus}).y) / {denom} + (({dz_plus}).z - ({dz_minus}).z) / {denom})"
    )
}

fn emit_vec3_axis_offset(base: ValueExpr, epsilon: ValueExpr, axis: usize, op: BinOp) -> ValueExpr {
    let offset = match axis {
        0 => ValueExpr::Vec3(
            Box::new(epsilon),
            Box::new(ValueExpr::Float(0.0)),
            Box::new(ValueExpr::Float(0.0)),
        ),
        1 => ValueExpr::Vec3(
            Box::new(ValueExpr::Float(0.0)),
            Box::new(epsilon),
            Box::new(ValueExpr::Float(0.0)),
        ),
        _ => ValueExpr::Vec3(
            Box::new(ValueExpr::Float(0.0)),
            Box::new(ValueExpr::Float(0.0)),
            Box::new(epsilon),
        ),
    };
    ValueExpr::Binary {
        op,
        left: Box::new(base),
        right: Box::new(offset),
        ty: Type::Vec3,
    }
}

fn emit_object_expr(
    expr: &ObjectExpr,
    point_expr: &str,
    object_bindings: &BTreeMap<String, ObjectExpr>,
    helper_names: &HashMap<String, String>,
) -> String {
    match expr {
        ObjectExpr::Var(name) => object_bindings
            .get(name)
            .map(|expr| emit_object_expr(expr, point_expr, object_bindings, helper_names))
            .unwrap_or_else(|| format!("obj_{}", name)),
        ObjectExpr::Primitive { name, kind, fields } => match kind {
            PrimitiveKind::ParamStruct(param_type) => {
                if name == "Plane3D" {
                    let normal = primitive_value_field(fields, "n");
                    let origin = primitive_value_field(fields, "origin");
                    let normal_expr = emit_plain_value_expr(normal, helper_names);
                    let origin_expr = emit_plain_value_expr(origin, helper_names);
                    return format!(
                        "sdf0_Plane3D({}, ParamPlane3D({}, (-dot(normalize({}), {}))))",
                        point_expr, normal_expr, normal_expr, origin_expr
                    );
                }
                let rendered_fields = fields
                    .iter()
                    .map(|(_, expr)| match expr {
                        PrimitiveArgExpr::Value(expr) => emit_plain_value_expr(expr, helper_names),
                        PrimitiveArgExpr::Vec2List(_) => unreachable!(),
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let point_arg = if shape_dimension(name) == ShapeDimension::D2 {
                    format!("({}).xy", point_expr)
                } else {
                    point_expr.to_string()
                };
                format!(
                    "sdf0_{}({}, {}({}))",
                    name, point_arg, param_type, rendered_fields
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
        ObjectExpr::AmbientTransform {
            object,
            translation,
            linear,
        } => {
            let transformed_point =
                emit_transformed_point(point_expr, translation, linear, helper_names);
            emit_object_expr(object, &transformed_point, object_bindings, helper_names)
        }
        ObjectExpr::RegisteredOp {
            name: _,
            glsl_name,
            value_args,
            object_args,
        } => {
            if glsl_name == "op_revolution" {
                let offset = emit_plain_value_expr(&value_args[0], helper_names);
                let revolved_point = format!(
                    "vec3((length(({}).xz) - {}), ({}).y, 0.0)",
                    point_expr, offset, point_expr
                );
                return emit_object_expr(
                    &object_args[0],
                    &revolved_point,
                    object_bindings,
                    helper_names,
                );
            }
            if glsl_name == "op_extrusion" {
                let height = emit_plain_value_expr(&value_args[0], helper_names);
                let base_point = format!("vec3(({}).xy, 0.0)", point_expr);
                let base_distance =
                    emit_object_expr(&object_args[0], &base_point, object_bindings, helper_names);
                return format!(
                    "(min(max({}, abs(({}).z) - {}), 0.0) + length(max(vec2({}, abs(({}).z) - {}), 0.0)))",
                    base_distance, point_expr, height, base_distance, point_expr, height
                );
            }
            let mut args = object_args
                .iter()
                .map(|arg| emit_object_expr(arg, point_expr, object_bindings, helper_names))
                .collect::<Vec<_>>();
            args.extend(
                value_args
                    .iter()
                    .map(|arg| emit_plain_value_expr(arg, helper_names)),
            );
            format!("{}({})", glsl_name, args.join(", "))
        }
    }
}

fn primitive_value_field<'a>(
    fields: &'a [(String, PrimitiveArgExpr)],
    name: &str,
) -> &'a ValueExpr {
    fields
        .iter()
        .find_map(|(field_name, expr)| match (field_name.as_str(), expr) {
            (candidate, PrimitiveArgExpr::Value(expr)) if candidate == name => Some(expr),
            _ => None,
        })
        .unwrap()
}

fn emit_polygon_vertices(vertices: &[ValueExpr], helper_names: &HashMap<String, String>) -> String {
    let mut rendered = vertices
        .iter()
        .map(|vertex| emit_plain_value_expr(vertex, helper_names))
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

fn emit_transformed_point(
    point_expr: &str,
    translation: &ValueExpr,
    linear: &ValueExpr,
    helper_names: &HashMap<String, String>,
) -> String {
    if is_identity_mat3(linear) {
        return format!(
            "({} - {})",
            point_expr,
            emit_plain_value_expr(translation, helper_names)
        );
    }
    if is_zero_vec3(translation) {
        return format!(
            "(transpose({}) * {})",
            emit_plain_value_expr(linear, helper_names),
            point_expr
        );
    }
    format!(
        "(transpose({}) * (({}) - {}))",
        emit_plain_value_expr(linear, helper_names),
        point_expr,
        emit_plain_value_expr(translation, helper_names)
    )
}

fn is_zero_vec3(expr: &ValueExpr) -> bool {
    match expr {
        ValueExpr::Vec3(x, y, z) => {
            is_float_literal(x, 0.0) && is_float_literal(y, 0.0) && is_float_literal(z, 0.0)
        }
        _ => false,
    }
}

fn is_identity_mat3(expr: &ValueExpr) -> bool {
    match expr {
        ValueExpr::Mat3(r0, r1, r2) => {
            is_vec3_literal(r0, [1.0, 0.0, 0.0])
                && is_vec3_literal(r1, [0.0, 1.0, 0.0])
                && is_vec3_literal(r2, [0.0, 0.0, 1.0])
        }
        _ => false,
    }
}

fn is_vec3_literal(expr: &ValueExpr, expected: [f64; 3]) -> bool {
    match expr {
        ValueExpr::Vec3(x, y, z) => {
            is_float_literal(x, expected[0])
                && is_float_literal(y, expected[1])
                && is_float_literal(z, expected[2])
        }
        _ => false,
    }
}

fn is_float_literal(expr: &ValueExpr, expected: f64) -> bool {
    matches!(expr, ValueExpr::Float(value) if (*value - expected).abs() < f64::EPSILON)
}

fn object_op_type(op: &ObjectOpDef) -> Type {
    let object_domain = if op.object_arg_count == 1 {
        Type::Obj3
    } else {
        Type::Product(vec![Type::Obj3; op.object_arg_count])
    };
    let mut ty = Type::func(object_domain, Type::Obj3);
    for value_arg in op.value_arg_types.iter().rev() {
        ty = Type::func(value_arg.clone(), ty);
    }
    ty
}

fn zero_vec3() -> ValueExpr {
    ValueExpr::Vec3(
        Box::new(ValueExpr::Float(0.0)),
        Box::new(ValueExpr::Float(0.0)),
        Box::new(ValueExpr::Float(0.0)),
    )
}

fn identity_mat3() -> ValueExpr {
    ValueExpr::Mat3(
        Box::new(ValueExpr::Vec3(
            Box::new(ValueExpr::Float(1.0)),
            Box::new(ValueExpr::Float(0.0)),
            Box::new(ValueExpr::Float(0.0)),
        )),
        Box::new(ValueExpr::Vec3(
            Box::new(ValueExpr::Float(0.0)),
            Box::new(ValueExpr::Float(1.0)),
            Box::new(ValueExpr::Float(0.0)),
        )),
        Box::new(ValueExpr::Vec3(
            Box::new(ValueExpr::Float(0.0)),
            Box::new(ValueExpr::Float(0.0)),
            Box::new(ValueExpr::Float(1.0)),
        )),
    )
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

fn unique_local_name(base: &str, forbidden: &BTreeSet<String>) -> String {
    if !forbidden.contains(base) {
        return base.to_string();
    }

    let mut counter = 0u64;
    loop {
        let candidate = format!(
            "{}_r{:06x}",
            base,
            stable_name_hash(base, forbidden, counter)
        );
        if !forbidden.contains(&candidate) {
            return candidate;
        }
        counter += 1;
    }
}

fn stable_name_hash(base: &str, forbidden: &BTreeSet<String>, counter: u64) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in base.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash ^= counter;
    hash = hash.wrapping_mul(0x100000001b3);
    for name in forbidden {
        for byte in name.bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash ^= u64::from(b'|');
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash & 0x00ff_ffff
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
    if matches!(
        (actual, expected),
        (Type::Vec2, Type::Complex) | (Type::Complex, Type::Vec2)
    ) {
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
        Type::Float => "R".to_string(),
        Type::Int => "Z".to_string(),
        Type::Complex => "C".to_string(),
        Type::Vec2 => "R2".to_string(),
        Type::Vec3 => "R3".to_string(),
        Type::Vec4 => "R4".to_string(),
        Type::Mat2 => "Mat2".to_string(),
        Type::Mat3 => "Mat3".to_string(),
        Type::Mat4 => "Mat4".to_string(),
        Type::Obj3 => "Obj3".to_string(),
        Type::Product(parts) => parts
            .iter()
            .map(format_type)
            .collect::<Vec<_>>()
            .join(" × "),
        Type::Func(_, _) => {
            let (inputs, output) = flatten_func_type(ty);
            let domain = if inputs.len() == 1 {
                format_type(inputs[0])
            } else {
                format_type(&Type::Product(inputs.into_iter().cloned().collect()))
            };
            format!("Func({}, {})", domain, format_type(output))
        }
    }
}

fn format_object_type(ty: &Type) -> String {
    match ty {
        Type::Float => "R".to_string(),
        Type::Int => "Z".to_string(),
        Type::Complex => "C".to_string(),
        Type::Vec2 => "R2".to_string(),
        Type::Vec3 => "R3".to_string(),
        Type::Vec4 => "R4".to_string(),
        Type::Mat2 => "Mat2".to_string(),
        Type::Mat3 => "Mat3".to_string(),
        Type::Mat4 => "Mat4".to_string(),
        Type::Obj3 => "Obj3".to_string(),
        Type::Product(parts) => parts
            .iter()
            .map(format_object_type)
            .collect::<Vec<_>>()
            .join(" × "),
        Type::Func(input, output) => {
            format!(
                "Hom({}, {})",
                format_object_type(input),
                format_object_type(output)
            )
        }
    }
}

fn flatten_func_type<'a>(ty: &'a Type) -> (Vec<&'a Type>, &'a Type) {
    let mut inputs = Vec::new();
    let mut current = ty;
    while let Type::Func(input, output) = current {
        inputs.push(input.as_ref());
        current = output.as_ref();
    }
    (inputs, current)
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
        let mut value_bindings = Vec::new();
        let mut bindings = Vec::new();
        let mut output = None;

        for raw_line in self.source.lines() {
            let line = strip_line_comment(raw_line).trim();
            if line.is_empty() {
                continue;
            }

            match self.parse_decl(line)? {
                Decl::Input(input) => inputs.push(input),
                Decl::Func(func) => funcs.push(func),
                Decl::ValueBinding(binding) => value_bindings.push(binding),
                Decl::Binding(binding) => bindings.push(binding),
                Decl::Output(out) => {
                    if output.is_some() {
                        return Err(Error::new("multiple out declarations are not supported"));
                    }
                    output = Some(out);
                }
            }
        }

        let output = output.ok_or_else(|| Error::new("missing generate declaration"))?;

        Ok(Program {
            inputs,
            funcs,
            value_bindings,
            bindings,
            output,
        })
    }

    fn parse_decl(&self, line: &str) -> Result<Decl, Error> {
        if let Some(rest) = line.strip_prefix("provided ") {
            let (ty, name) = split_type_name(rest.trim())?;
            return Ok(Decl::Input(InputDecl {
                name: name.to_string(),
                ty: parse_type(ty)?,
            }));
        }

        if let Some(rest) = line
            .strip_prefix("generate ")
            .or_else(|| line.strip_prefix("gen "))
        {
            let expr_source = rest.trim();
            if let Some((left, _)) = expr_source.split_once('=') {
                if parse_type(left.trim()).is_ok() {
                    return Err(Error::new(
                        "use 'generate value' instead of 'generate type = value'",
                    ));
                }
            }
            let expr = ExprParser::new(expr_source).parse()?;
            return Ok(Decl::Output(OutputDecl { expr }));
        }

        let generated = line.starts_with("construct ") || line.starts_with("const ");
        let line = line
            .strip_prefix("construct ")
            .or_else(|| line.strip_prefix("const "))
            .unwrap_or(line);
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
            if generated {
                return Err(Error::new(
                    "'construct' currently only supports Obj3 bindings",
                ));
            }
            return Ok(Decl::Func(FuncDecl {
                name: name.to_string(),
                ty,
                expr,
            }));
        }
        if !matches!(ty, Type::Obj3) {
            if generated {
                return Err(Error::new(
                    "'construct' currently only supports Obj3 bindings",
                ));
            }
            return Ok(Decl::ValueBinding(ValueBindingDecl {
                name: name.to_string(),
                ty,
                expr,
            }));
        }
        Ok(Decl::Binding(BindingDecl {
            name: name.to_string(),
            ty,
            expr,
            generated,
        }))
    }
}

fn strip_line_comment(line: &str) -> &str {
    line.split_once("//").map_or(line, |(before, _)| before)
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
            return Err(Error::new(format!(
                "unexpected trailing token {} in expression",
                self.describe_current_token()
            )));
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
        let mut expr = self.parse_unary()?;
        while matches!(self.peek(), Some(Token::LParen)) {
            let args = self.parse_positional_args()?;
            expr = Expr::Call {
                callee: Box::new(expr),
                args,
            };
        }
        Ok(expr)
    }

    fn parse_unary(&mut self) -> Result<Expr, Error> {
        if matches!(self.peek(), Some(Token::Minus)) {
            self.index += 1;
            let expr = self.parse_unary()?;
            return Ok(Expr::Binary {
                op: BinOp::Sub,
                left: Box::new(Expr::Number(0.0)),
                right: Box::new(expr),
            });
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<Expr, Error> {
        match self.next() {
            Some(Token::Number(value)) => Ok(Expr::Number(value.parse::<f64>().unwrap())),
            Some(Token::Ident(name)) => {
                if matches!(self.peek(), Some(Token::LParen)) {
                    let args = self.parse_mixed_args()?;
                    return Ok(Expr::Constructor { name, args });
                }
                Ok(Expr::Ident(name))
            }
            Some(Token::LParen) => self.parse_paren_or_tuple(),
            _ => Err(Error::new(format!(
                "unexpected token {} in expression",
                self.describe_previous_token()
            ))),
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

    fn parse_mixed_args(&mut self) -> Result<ConstructorArgs, Error> {
        self.expect(Token::LParen)?;
        if matches!(self.peek(), Some(Token::RParen)) {
            self.index += 1;
            return Ok(ConstructorArgs::Positional(Vec::new()));
        }

        if !matches!(
            (self.peek(), self.peek_n(1)),
            (Some(Token::Ident(_)), Some(Token::Equal))
        ) {
            self.index -= 1;
            return Ok(ConstructorArgs::Positional(self.parse_positional_args()?));
        }

        let mut named = Vec::new();
        loop {
            let field_name = match (self.peek(), self.peek_n(1)) {
                (Some(Token::Ident(name)), Some(Token::Equal)) => name.clone(),
                _ => return Err(Error::new("expected named constructor arguments")),
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
        Ok(ConstructorArgs::Named(named))
    }

    fn expect(&mut self, expected: Token) -> Result<(), Error> {
        let token = self
            .next()
            .ok_or_else(|| Error::new("unexpected end of input"))?;
        if token == expected {
            return Ok(());
        }
        Err(Error::new(format!(
            "expected {}, got {}",
            Self::describe_token(&expected),
            Self::describe_token(&token)
        )))
    }

    fn describe_current_token(&self) -> String {
        self.peek()
            .map(Self::describe_token)
            .unwrap_or_else(|| "<end of input>".to_string())
    }

    fn describe_previous_token(&self) -> String {
        self.index
            .checked_sub(1)
            .and_then(|index| self.tokens.get(index))
            .map(Self::describe_token)
            .unwrap_or_else(|| "<start of input>".to_string())
    }

    fn describe_token(token: &Token) -> String {
        match token {
            Token::Ident(name) => format!("identifier '{}'", name),
            Token::Number(value) => format!("number '{}'", value),
            Token::LParen => "'('".to_string(),
            Token::RParen => "')'".to_string(),
            Token::Colon => "':'".to_string(),
            Token::Comma => "','".to_string(),
            Token::Equal => "'='".to_string(),
            Token::Plus => "'+'".to_string(),
            Token::Minus => "'-'".to_string(),
            Token::Star => "'*'".to_string(),
            Token::Slash => "'/'".to_string(),
            Token::At => "'@'".to_string(),
            Token::Arrow => "'->'".to_string(),
        }
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
            if index < chars.len() && matches!(chars[index], 'e' | 'E') {
                let exponent_start = index;
                index += 1;
                if index < chars.len() && matches!(chars[index], '+' | '-') {
                    index += 1;
                }
                let digit_start = index;
                while index < chars.len() && chars[index].is_ascii_digit() {
                    index += 1;
                }
                if digit_start == index {
                    index = exponent_start;
                }
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
        let (input, output) = split_arrow_legacy(inner)?;
        return Ok(Type::func(parse_type(input)?, parse_type(output)?));
    }
    if let Some(inner) = strip_type_head(source, "Func") {
        let (input, output) = split_top_level_comma(inner)?;
        return Ok(Type::func(parse_type(input)?, parse_type(output)?));
    }
    if let Some(inner) = strip_type_head(source, "Hom") {
        let (input, output) = split_top_level_comma(inner)?;
        return Ok(Type::func(parse_type(input)?, parse_type(output)?));
    }
    if let Some(inner) = strip_type_head(source, "End") {
        let ty = parse_type(inner)?;
        return Ok(Type::func(ty.clone(), ty));
    }
    if let Some(parts) = split_top_level_product(source) {
        let mut parsed = Vec::new();
        for part in parts {
            parsed.push(parse_type(part)?);
        }
        return Ok(Type::Product(parsed));
    }
    match source {
        "Float" | "R" => Ok(Type::Float),
        "Int" | "Z" => Ok(Type::Int),
        "Complex" | "C" => Ok(Type::Complex),
        "Vec2" | "R2" => Ok(Type::Vec2),
        "Vec3" | "R3" => Ok(Type::Vec3),
        "Vec4" | "R4" => Ok(Type::Vec4),
        "Mat2" => Ok(Type::Mat2),
        "Mat3" => Ok(Type::Mat3),
        "Mat4" => Ok(Type::Mat4),
        "Obj3" => Ok(Type::Obj3),
        _ => Err(Error::new(format!("unsupported type '{}'", source))),
    }
}

fn split_arrow_legacy(source: &str) -> Result<(&str, &str), Error> {
    source
        .split_once("->")
        .map(|(left, right)| (left.trim(), right.trim()))
        .ok_or_else(|| Error::new("expected '->' in function type"))
}

fn strip_type_head<'a>(source: &'a str, head: &str) -> Option<&'a str> {
    source
        .strip_prefix(head)
        .and_then(|rest| rest.strip_prefix('('))
        .and_then(|rest| rest.strip_suffix(')'))
}

fn split_top_level_comma(source: &str) -> Result<(&str, &str), Error> {
    let mut depth = 0;
    for (index, ch) in source.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                return Ok((source[..index].trim(), source[index + 1..].trim()));
            }
            _ => {}
        }
    }
    Err(Error::new("expected ',' in function type"))
}

fn split_top_level_product(source: &str) -> Option<Vec<&str>> {
    let mut depth = 0;
    let mut parts = Vec::new();
    let mut start = 0;
    let bytes = source.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let ch = source[index..].chars().next().unwrap();
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            '×' if depth == 0 => {
                parts.push(source[start..index].trim());
                start = index + ch.len_utf8();
            }
            'x' if depth == 0 => {
                let prev_space = index > 0 && bytes[index - 1].is_ascii_whitespace();
                let next_index = index + 1;
                let next_space =
                    next_index < bytes.len() && bytes[next_index].is_ascii_whitespace();
                if prev_space && next_space {
                    parts.push(source[start..index - 1].trim());
                    start = next_index + 1;
                }
            }
            _ => {}
        }
        index += ch.len_utf8();
    }
    if parts.is_empty() {
        return None;
    }
    parts.push(source[start..].trim());
    Some(parts)
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
            Expr::Constructor {
                name,
                args: constructor_args,
            } => match constructor_args {
                ConstructorArgs::Positional(constructor_args) => {
                    for arg in constructor_args.iter().rev() {
                        args.push(arg);
                    }
                    args.reverse();
                    return Ok((name.clone(), args));
                }
                ConstructorArgs::Named(_) => {
                    return Err(Error::new("unsupported callable object expression"))
                }
            },
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
