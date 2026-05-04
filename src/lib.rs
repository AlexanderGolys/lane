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
    line: Option<usize>,
}

impl Error {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            line: None,
        }
    }

    fn with_line(mut self, line: usize) -> Self {
        if self.line.is_none() {
            self.line = Some(line);
        }
        self
    }

    pub fn line(&self) -> Option<usize> {
        self.line
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(line) = self.line {
            write!(f, "line {line}: {}", self.message)
        } else {
            f.write_str(&self.message)
        }
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
    Category,
}

pub const TYPE_METATYPE_NAME: &str = "Type";
pub const CATEGORY_METATYPE_NAME: &str = "Cat";

pub fn known_type_names() -> Vec<&'static str> {
    let mut names = BUILTIN_TYPE_DEFS
        .iter()
        .flat_map(|def| def.aliases.iter().copied())
        .collect::<Vec<_>>();
    names.extend(MATRIX_TYPE_NAMES.iter().copied());
    names
}

pub fn is_known_type_name(name: &str) -> bool {
    parse_builtin_type_name(name).is_some()
}

pub fn known_category_names() -> Vec<&'static str> {
    ALGEBRAIC_CATEGORY_DEFS.iter().map(|def| def.name).collect()
}

pub fn is_known_category_name(name: &str) -> bool {
    category_by_name(name).is_some()
}

const BUILTIN_TYPE_DETAILS: [(&str, &str); 4] = [
    ("C", "#define Complex vec2"),
    ("E2", "#define E2 vec2"),
    ("E3", "#define E3 vec3"),
    ("H", "#define H vec4"),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AlgebraicCategory {
    Ab,
    Mon,
    Grp,
    Ring,
    Field,
    VectR,
    AlgR,
}

struct AlgebraicCategoryDef {
    category: AlgebraicCategory,
    name: &'static str,
}

const ALGEBRAIC_CATEGORY_DEFS: [AlgebraicCategoryDef; 7] = [
    AlgebraicCategoryDef {
        category: AlgebraicCategory::Ab,
        name: "Ab",
    },
    AlgebraicCategoryDef {
        category: AlgebraicCategory::Mon,
        name: "Mon",
    },
    AlgebraicCategoryDef {
        category: AlgebraicCategory::Grp,
        name: "Grp",
    },
    AlgebraicCategoryDef {
        category: AlgebraicCategory::Ring,
        name: "Ring",
    },
    AlgebraicCategoryDef {
        category: AlgebraicCategory::Field,
        name: "Field",
    },
    AlgebraicCategoryDef {
        category: AlgebraicCategory::VectR,
        name: "VectR",
    },
    AlgebraicCategoryDef {
        category: AlgebraicCategory::AlgR,
        name: "AlgR",
    },
];

fn category_by_name(name: &str) -> Option<AlgebraicCategory> {
    ALGEBRAIC_CATEGORY_DEFS
        .iter()
        .find(|def| def.name == name)
        .map(|def| def.category)
}

fn category_name(category: AlgebraicCategory) -> &'static str {
    ALGEBRAIC_CATEGORY_DEFS
        .iter()
        .find(|def| def.category == category)
        .map(|def| def.name)
        .unwrap()
}

fn type_category_signature(name: &str) -> Option<String> {
    let ty = parse_builtin_type_name(name)?;
    let categories = minimal_categories(type_direct_categories(&ty));
    if categories.is_empty() {
        return Some(TYPE_METATYPE_NAME.to_string());
    }
    Some(format_categories(&categories))
}

fn minimal_categories(categories: Vec<AlgebraicCategory>) -> Vec<AlgebraicCategory> {
    categories
        .iter()
        .copied()
        .filter(|category| {
            !categories
                .iter()
                .any(|other| other != category && category_implies(*other, *category))
        })
        .collect()
}

fn type_direct_categories(ty: &Type) -> Vec<AlgebraicCategory> {
    if let Type::Mat(rows, columns) = ty {
        let mut categories = Vec::new();
        if rows == columns {
            categories.push(AlgebraicCategory::Ring);
        }
        categories.push(AlgebraicCategory::VectR);
        return categories;
    }

    BUILTIN_TYPE_DEFS
        .iter()
        .find(|def| &def.ty == ty)
        .map(|def| def.categories.to_vec())
        .unwrap_or_default()
}

fn format_categories(categories: &[AlgebraicCategory]) -> String {
    categories
        .iter()
        .map(|category| category_name(*category))
        .collect::<Vec<_>>()
        .join(" × ")
}

struct BuiltinTypeDef {
    ty: Type,
    aliases: &'static [&'static str],
    display_name: &'static str,
    categories: &'static [AlgebraicCategory],
}

const MATRIX_TYPE_NAMES: [&str; 9] = [
    "Mat2", "Mat2x3", "Mat2x4", "Mat3x2", "Mat3", "Mat3x4", "Mat4x2", "Mat4x3", "Mat4",
];

const BUILTIN_TYPE_DEFS: [BuiltinTypeDef; 8] = [
    BuiltinTypeDef {
        ty: Type::Float,
        aliases: &["Float", "R"],
        display_name: "R",
        categories: &[
            AlgebraicCategory::Field,
            AlgebraicCategory::Grp,
            AlgebraicCategory::AlgR,
            AlgebraicCategory::VectR,
        ],
    },
    BuiltinTypeDef {
        ty: Type::Int,
        aliases: &["Int", "Z"],
        display_name: "Z",
        categories: &[AlgebraicCategory::Ring],
    },
    BuiltinTypeDef {
        ty: Type::Complex,
        aliases: &["Complex", "C"],
        display_name: "C",
        categories: &[
            AlgebraicCategory::Field,
            AlgebraicCategory::Grp,
            AlgebraicCategory::AlgR,
            AlgebraicCategory::VectR,
        ],
    },
    BuiltinTypeDef {
        ty: Type::Vec2,
        aliases: &["Vec2", "R2", "E2"],
        display_name: "R2",
        categories: &[AlgebraicCategory::VectR],
    },
    BuiltinTypeDef {
        ty: Type::Vec3,
        aliases: &["Vec3", "R3", "E3"],
        display_name: "R3",
        categories: &[AlgebraicCategory::VectR],
    },
    BuiltinTypeDef {
        ty: Type::Vec4,
        aliases: &["Vec4", "R4"],
        display_name: "R4",
        categories: &[AlgebraicCategory::VectR],
    },
    BuiltinTypeDef {
        ty: Type::Quat,
        aliases: &["H"],
        display_name: "H",
        categories: &[
            AlgebraicCategory::Field,
            AlgebraicCategory::Grp,
            AlgebraicCategory::AlgR,
            AlgebraicCategory::VectR,
        ],
    },
    BuiltinTypeDef {
        ty: Type::Object,
        aliases: &["Object", "Object3D"],
        display_name: "Object",
        categories: &[],
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
    Quat,
    Vec2,
    Vec3,
    Vec4,
    Mat(usize, usize),
    Object,
    Product(Vec<Type>),
    Func(Box<Type>, Box<Type>),
}

impl Type {
    fn func(input: Type, output: Type) -> Self {
        Self::Func(Box::new(input), Box::new(output))
    }

    fn glsl_name(&self) -> String {
        match self {
            Self::Float => "float".to_string(),
            Self::Int => "int".to_string(),
            Self::Complex => "vec2".to_string(),
            Self::Quat => "vec4".to_string(),
            Self::Vec2 => "vec2".to_string(),
            Self::Vec3 => "vec3".to_string(),
            Self::Vec4 => "vec4".to_string(),
            Self::Mat(rows, columns) => matrix_glsl_type(*rows, *columns),
            Self::Object | Self::Product(_) | Self::Func(_, _) => "".to_string(),
        }
    }

    fn type_name(&self) -> String {
        if let Self::Mat(rows, columns) = self {
            return matrix_type_name(*rows, *columns);
        }
        BUILTIN_TYPE_DEFS
            .iter()
            .find(|def| &def.ty == self)
            .map(|def| def.display_name.to_string())
            .unwrap_or_else(|| match self {
                Self::Product(_) => "Product".to_string(),
                Self::Func(_, _) => "Func".to_string(),
                _ => unreachable!(),
            })
    }
}

fn parse_builtin_type_name(name: &str) -> Option<Type> {
    BUILTIN_TYPE_DEFS
        .iter()
        .find(|def| def.aliases.contains(&name))
        .map(|def| def.ty.clone())
        .or_else(|| parse_matrix_type_name(name))
}

fn parse_matrix_type_name(name: &str) -> Option<Type> {
    let suffix = name.strip_prefix("Mat")?;
    if suffix.len() == 1 {
        let dimension = parse_matrix_dimension(suffix)?;
        return Some(Type::Mat(dimension, dimension));
    }

    let (rows, columns) = suffix.split_once('x')?;
    Some(Type::Mat(
        parse_matrix_dimension(rows)?,
        parse_matrix_dimension(columns)?,
    ))
}

fn parse_matrix_dimension(source: &str) -> Option<usize> {
    match source {
        "2" => Some(2),
        "3" => Some(3),
        "4" => Some(4),
        _ => None,
    }
}

fn matrix_type_name(rows: usize, columns: usize) -> String {
    if rows == columns {
        format!("Mat{rows}")
    } else {
        format!("Mat{rows}x{columns}")
    }
}

fn matrix_glsl_type(rows: usize, columns: usize) -> String {
    if rows == columns {
        format!("mat{rows}")
    } else {
        format!("mat{columns}x{rows}")
    }
}

fn matrix_constructor_type(rows: usize, columns: usize) -> String {
    if rows == columns {
        format!("mat{rows}")
    } else {
        format!("mat{rows}x{columns}")
    }
}

fn has_category(ty: &Type, category: AlgebraicCategory) -> bool {
    if let Type::Mat(rows, columns) = ty {
        return category == AlgebraicCategory::VectR
            || (rows == columns
                && (category == AlgebraicCategory::Ring
                    || category_implies(AlgebraicCategory::Ring, category)));
    }
    let Some(def) = BUILTIN_TYPE_DEFS.iter().find(|def| &def.ty == ty) else {
        return false;
    };
    def.categories
        .iter()
        .any(|candidate| *candidate == category || category_implies(*candidate, category))
}

fn category_implies(source: AlgebraicCategory, target: AlgebraicCategory) -> bool {
    matches!(
        (source, target),
        (AlgebraicCategory::AlgR, AlgebraicCategory::Ring)
            | (AlgebraicCategory::AlgR, AlgebraicCategory::VectR)
            | (AlgebraicCategory::AlgR, AlgebraicCategory::Ab)
            | (AlgebraicCategory::AlgR, AlgebraicCategory::Mon)
            | (AlgebraicCategory::Ring, AlgebraicCategory::Ab)
            | (AlgebraicCategory::Ring, AlgebraicCategory::Mon)
            | (AlgebraicCategory::Field, AlgebraicCategory::Grp)
            | (AlgebraicCategory::Field, AlgebraicCategory::Ring)
            | (AlgebraicCategory::Field, AlgebraicCategory::Ab)
            | (AlgebraicCategory::Field, AlgebraicCategory::Mon)
            | (AlgebraicCategory::Grp, AlgebraicCategory::Mon)
            | (AlgebraicCategory::VectR, AlgebraicCategory::Ab)
    )
}

#[derive(Clone, Debug)]
struct InputDecl {
    name: String,
    ty: Type,
    line: usize,
}

#[derive(Clone, Debug)]
struct FuncDecl {
    name: String,
    ty: Type,
    expr: Expr,
    line: usize,
}

#[derive(Clone, Debug)]
struct BindingDecl {
    name: String,
    ty: Type,
    expr: Expr,
    generated: bool,
    line: usize,
}

#[derive(Clone, Debug)]
struct ValueBindingDecl {
    name: String,
    ty: Type,
    expr: Expr,
    line: usize,
}

#[derive(Clone, Debug)]
struct OutputDecl {
    expr: Expr,
    line: usize,
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
    Vec4(
        Box<ValueExpr>,
        Box<ValueExpr>,
        Box<ValueExpr>,
        Box<ValueExpr>,
    ),
    Matrix {
        columns: usize,
        rows: Vec<ValueExpr>,
    },
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
            Self::Vec4(_, _, _, _) => Type::Vec4,
            Self::Matrix { columns, rows } => Type::Mat(rows.len(), *columns),
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
struct ValueFuncOverloadDef {
    glsl_name: &'static str,
    ty: Type,
}

#[derive(Clone, Debug)]
struct Registry {
    primitives: HashMap<&'static str, PrimitiveDef>,
    object_ops: HashMap<&'static str, ObjectOpDef>,
    value_funcs: HashMap<&'static str, ValueFuncDef>,
    value_func_overloads: HashMap<&'static str, Vec<ValueFuncOverloadDef>>,
}

fn object_op_type(op: &ObjectOpDef) -> Type {
    let object_domain = if op.object_arg_count == 1 {
        Type::Object
    } else {
        Type::Product(vec![Type::Object; op.object_arg_count])
    };
    let mut ty = Type::func(object_domain, Type::Object);
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
        (Type::Vec2, Type::Complex)
            | (Type::Complex, Type::Vec2)
            | (Type::Vec4, Type::Quat)
            | (Type::Quat, Type::Vec4)
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
        _ => ty.type_name().to_string(),
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
        _ => ty.type_name().to_string(),
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
