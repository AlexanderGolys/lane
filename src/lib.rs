use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;

mod emit;
mod parser;
mod registry;
mod typecheck;

pub fn compile_program(source: &str) -> Result<String, Error> {
    let registry = Registry::default();
    let program = parser::Parser::new(source).parse_program()?;
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

pub fn known_builtin_object(name: &str) -> Option<KnownBuiltinObjectDetail> {
    let registry = Registry::default();
    registry.known_builtin_object(name)
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
    pub kind: KnownBuiltinObjectKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KnownBuiltinObjectDetail {
    pub name: String,
    pub ty: String,
    pub kind: KnownBuiltinObjectKind,
    pub body: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KnownBuiltinObjectKind {
    Function,
    Type,
}

pub const TYPE_METATYPE_NAME: &str = "Type";

pub fn known_type_names() -> Vec<&'static str> {
    SURFACE_TYPE_DEFS
        .iter()
        .flat_map(|def| def.aliases.iter().copied())
        .collect()
}

pub fn is_known_type_name(name: &str) -> bool {
    SURFACE_TYPE_DEFS
        .iter()
        .any(|def| def.aliases.contains(&name))
}

const BUILTIN_TYPE_DETAILS: [(&str, &str); 4] = [
    ("C", "#define Complex vec2"),
    ("E2", "#define E2 vec2"),
    ("E3", "#define E3 vec3"),
    ("Quat", "#define Quat vec4"),
];

struct SurfaceTypeDef {
    ty: Type,
    aliases: &'static [&'static str],
    surface_name: &'static str,
}

const SURFACE_TYPE_DEFS: [SurfaceTypeDef; 10] = [
    SurfaceTypeDef {
        ty: Type::Float,
        aliases: &["Float", "R"],
        surface_name: "R",
    },
    SurfaceTypeDef {
        ty: Type::Int,
        aliases: &["Int", "Z"],
        surface_name: "Z",
    },
    SurfaceTypeDef {
        ty: Type::Complex,
        aliases: &["Complex", "C"],
        surface_name: "C",
    },
    SurfaceTypeDef {
        ty: Type::Vec2,
        aliases: &["Vec2", "R2"],
        surface_name: "R2",
    },
    SurfaceTypeDef {
        ty: Type::Vec3,
        aliases: &["Vec3", "R3"],
        surface_name: "R3",
    },
    SurfaceTypeDef {
        ty: Type::Vec4,
        aliases: &["Vec4", "R4"],
        surface_name: "R4",
    },
    SurfaceTypeDef {
        ty: Type::Mat2,
        aliases: &["Mat2"],
        surface_name: "Mat2",
    },
    SurfaceTypeDef {
        ty: Type::Mat3,
        aliases: &["Mat3"],
        surface_name: "Mat3",
    },
    SurfaceTypeDef {
        ty: Type::Mat4,
        aliases: &["Mat4"],
        surface_name: "Mat4",
    },
    SurfaceTypeDef {
        ty: Type::Solid,
        aliases: &["Solid"],
        surface_name: "Solid",
    },
];

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
    Solid,
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
            Self::Solid | Self::Product(_) | Self::Func(_, _) => "",
        }
    }

    fn surface_name(&self) -> &'static str {
        SURFACE_TYPE_DEFS
            .iter()
            .find(|def| &def.ty == self)
            .map(|def| def.surface_name)
            .unwrap_or_else(|| match self {
                Self::Product(_) => "Product",
                Self::Func(_, _) => "Func",
                _ => unreachable!(),
            })
    }
}

fn parse_surface_type_name(name: &str) -> Option<Type> {
    SURFACE_TYPE_DEFS
        .iter()
        .find(|def| def.aliases.contains(&name))
        .map(|def| def.ty.clone())
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

fn object_op_type(op: &ObjectOpDef) -> Type {
    let object_domain = if op.object_arg_count == 1 {
        Type::Solid
    } else {
        Type::Product(vec![Type::Solid; op.object_arg_count])
    };
    let mut ty = Type::func(object_domain, Type::Solid);
    for value_arg in op.value_arg_types.iter().rev() {
        ty = Type::func(value_arg.clone(), ty);
    }
    ty
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
        _ => ty.surface_name().to_string(),
    }
}

fn format_object_type(ty: &Type) -> String {
    match ty {
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
        _ => ty.surface_name().to_string(),
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
