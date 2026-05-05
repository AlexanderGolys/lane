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

pub(crate) fn suffix_glsl_float_literals(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let chars = source.chars().collect::<Vec<_>>();
    let mut index = 0;

    while index < chars.len() {
        let ch = chars[index];
        if ch.is_ascii_digit()
            && (index == 0
                || !(chars[index - 1].is_ascii_alphanumeric()
                    || matches!(chars[index - 1], '_' | '.')))
        {
            let start = index;
            while index < chars.len() && chars[index].is_ascii_digit() {
                index += 1;
            }

            let mut is_float = false;
            if index < chars.len()
                && chars[index] == '.'
                && index + 1 < chars.len()
                && chars[index + 1].is_ascii_digit()
            {
                is_float = true;
                index += 1;
                while index < chars.len() && chars[index].is_ascii_digit() {
                    index += 1;
                }
            }

            if index < chars.len() && matches!(chars[index], 'e' | 'E') {
                let exponent_index = index;
                let mut scan = index + 1;
                if scan < chars.len() && matches!(chars[scan], '+' | '-') {
                    scan += 1;
                }
                let digits_start = scan;
                while scan < chars.len() && chars[scan].is_ascii_digit() {
                    scan += 1;
                }
                if scan > digits_start {
                    is_float = true;
                    index = scan;
                } else {
                    index = exponent_index;
                }
            }

            let literal = chars[start..index].iter().collect::<String>();
            out.push_str(&literal);
            if is_float && !matches!(chars.get(index), Some('f' | 'F')) {
                out.push('f');
            }
            continue;
        }

        out.push(ch);
        index += 1;
    }

    out
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
    ("E2", ""),
    ("E3", ""),
    ("H", "#define H vec4"),
];

const COMPLEX_FIELD_SUPPORT_GLSL: &str = "vec2 mult_C(vec2 a, vec2 b) {\n    return vec2((a.x * b.x) - (a.y * b.y), (a.x * b.y) + (a.y * b.x));\n}\n\nvec2 div_C(vec2 a, vec2 b) {\n    return mult_C(a, vec2(b.x, -b.y) / dot(b, b));\n}";

const COMPLEX_OVERLOAD_NAMES: [&str; 10] = [
    "inv", "exp", "log", "sqrt", "sin", "cos", "tan", "sinh", "cosh", "tanh",
];

fn complex_overload_name(name: &str) -> Option<&'static str> {
    match name {
        "cinv" => Some("inv"),
        "cexp" => Some("exp"),
        "clog" => Some("log"),
        "csqrt" => Some("sqrt"),
        "csin" => Some("sin"),
        "ccos" => Some("cos"),
        "ctan" => Some("tan"),
        "csinh" => Some("sinh"),
        "ccosh" => Some("cosh"),
        "ctanh" => Some("tanh"),
        "inv" => Some("inv"),
        "exp" => Some("exp"),
        "log" => Some("log"),
        "sqrt" => Some("sqrt"),
        "sin" => Some("sin"),
        "cos" => Some("cos"),
        "tan" => Some("tan"),
        "sinh" => Some("sinh"),
        "cosh" => Some("cosh"),
        "tanh" => Some("tanh"),
        _ => None,
    }
}

fn complex_overload_support_glsl(name: &str) -> Option<&'static str> {
    match name {
        "inv" => Some("vec2 inv(vec2 z) {\n    return vec2(z.x, -z.y) / dot(z, z);\n}"),
        "exp" => Some("vec2 exp(vec2 z) {\n    float scale = exp(z.x);\n    return scale * vec2(cos(z.y), sin(z.y));\n}"),
        "log" => Some("vec2 log(vec2 z) {\n    return vec2(log(length(z)), atan(z.y, z.x));\n}"),
        "sqrt" => Some("vec2 sqrt(vec2 z) {\n    float r = length(z);\n    float a = sqrt(max((r + z.x) * 0.5, 0.0));\n    float b = sqrt(max((r - z.x) * 0.5, 0.0));\n    return vec2(a, sign(z.y) * b);\n}"),
        "sin" => Some("vec2 sin(vec2 z) {\n    return vec2(sin(z.x) * cosh(z.y), cos(z.x) * sinh(z.y));\n}"),
        "cos" => Some("vec2 cos(vec2 z) {\n    return vec2(cos(z.x) * cosh(z.y), -sin(z.x) * sinh(z.y));\n}"),
        "tan" => Some("vec2 tan(vec2 z) {\n    float d = cos(2.0 * z.x) + cosh(2.0 * z.y);\n    return vec2(sin(2.0 * z.x), sinh(2.0 * z.y)) / d;\n}"),
        "sinh" => Some("vec2 sinh(vec2 z) {\n    return vec2(sinh(z.x) * cos(z.y), cosh(z.x) * sin(z.y));\n}"),
        "cosh" => Some("vec2 cosh(vec2 z) {\n    return vec2(cosh(z.x) * cos(z.y), sinh(z.x) * sin(z.y));\n}"),
        "tanh" => Some("vec2 tanh(vec2 z) {\n    float d = cosh(2.0 * z.x) + cos(2.0 * z.y);\n    return vec2(sinh(2.0 * z.x), sin(2.0 * z.y)) / d;\n}"),
        _ => None,
    }
}

const E2_GROUP_SUPPORT_GLSL: &str = "struct E2 {\n    mat2 A;\n    vec2 t;\n};\n\nvec2 act_E2(E2 g, vec2 p) {\n    return (g.A * p) + g.t;\n}\n\nE2 mult_E2(E2 a, E2 b) {\n    return E2(a.A * b.A, (a.A * b.t) + a.t);\n}\n\nE2 inv_E2(E2 g) {\n    mat2 inverse_linear = transpose(g.A);\n    return E2(inverse_linear, -(inverse_linear * g.t));\n}";

const QUAT_FIELD_SUPPORT_GLSL: &str = "vec4 mult_H(vec4 a, vec4 b) {\n    return vec4(\n        a.x * b.x - a.y * b.y - a.z * b.z - a.w * b.w,\n        a.x * b.y + a.y * b.x + a.z * b.w - a.w * b.z,\n        a.x * b.z - a.y * b.w + a.z * b.x + a.w * b.y,\n        a.x * b.w + a.y * b.z - a.z * b.y + a.w * b.x\n    );\n}\n\nvec4 inv_H(vec4 q) {\n    return vec4(q.x, -q.y, -q.z, -q.w) / dot(q, q);\n}\n\nvec4 div_H(vec4 a, vec4 b) {\n    return mult_H(a, inv_H(b));\n}";

const E3_GROUP_SUPPORT_GLSL: &str = "struct E3 {\n    mat3 A;\n    vec3 t;\n};\n\nvec3 act_E3(E3 g, vec3 p) {\n    return (g.A * p) + g.t;\n}\n\nE3 mult_E3(E3 a, E3 b) {\n    return E3(a.A * b.A, (a.A * b.t) + a.t);\n}\n\nE3 inv_E3(E3 g) {\n    mat3 inverse_linear = transpose(g.A);\n    return E3(inverse_linear, -(inverse_linear * g.t));\n}\n\nmat3 rot_E3_matrix(vec3 binormal, float angle) {\n    vec3 axis = normalize(binormal);\n    float c = cos(angle);\n    float s = sin(angle);\n    float oc = 1.0 - c;\n    return mat3(\n        vec3((axis.x * axis.x * oc) + c, (axis.y * axis.x * oc) + (axis.z * s), (axis.z * axis.x * oc) - (axis.y * s)),\n        vec3((axis.x * axis.y * oc) - (axis.z * s), (axis.y * axis.y * oc) + c, (axis.z * axis.y * oc) + (axis.x * s)),\n        vec3((axis.x * axis.z * oc) + (axis.y * s), (axis.y * axis.z * oc) - (axis.x * s), (axis.z * axis.z * oc) + c)\n    );\n}\n\nE3 rot(vec3 binormal, vec3 anchor, float angle) {\n    mat3 A = rot_E3_matrix(binormal, angle);\n    return E3(A, anchor - (A * anchor));\n}";

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
        .join(", ")
}

struct BuiltinTypeDef {
    ty: Type,
    aliases: &'static [&'static str],
    display_name: &'static str,
    support_glsl: Option<&'static str>,
    categories: &'static [AlgebraicCategory],
}

const MATRIX_TYPE_NAMES: [&str; 9] = [
    "Mat2", "Mat2x3", "Mat2x4", "Mat3x2", "Mat3", "Mat3x4", "Mat4x2", "Mat4x3", "Mat4",
];

const BUILTIN_TYPE_DEFS: [BuiltinTypeDef; 10] = [
    BuiltinTypeDef {
        ty: Type::Float,
        aliases: &["Float", "R"],
        display_name: "R",
        support_glsl: None,
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
        support_glsl: None,
        categories: &[AlgebraicCategory::Ring],
    },
    BuiltinTypeDef {
        ty: Type::Complex,
        aliases: &["Complex", "C"],
        display_name: "C",
        support_glsl: Some(COMPLEX_FIELD_SUPPORT_GLSL),
        categories: &[
            AlgebraicCategory::Field,
            AlgebraicCategory::Grp,
            AlgebraicCategory::AlgR,
            AlgebraicCategory::VectR,
        ],
    },
    BuiltinTypeDef {
        ty: Type::Vec2,
        aliases: &["Vec2", "R2"],
        display_name: "R2",
        support_glsl: None,
        categories: &[AlgebraicCategory::VectR],
    },
    BuiltinTypeDef {
        ty: Type::Vec3,
        aliases: &["Vec3", "R3"],
        display_name: "R3",
        support_glsl: None,
        categories: &[AlgebraicCategory::VectR],
    },
    BuiltinTypeDef {
        ty: Type::Vec4,
        aliases: &["Vec4", "R4"],
        display_name: "R4",
        support_glsl: None,
        categories: &[AlgebraicCategory::VectR],
    },
    BuiltinTypeDef {
        ty: Type::Quat,
        aliases: &["H"],
        display_name: "H",
        support_glsl: Some(QUAT_FIELD_SUPPORT_GLSL),
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
        support_glsl: None,
        categories: &[],
    },
    BuiltinTypeDef {
        ty: Type::E2,
        aliases: &["E2"],
        display_name: "E2",
        support_glsl: Some(E2_GROUP_SUPPORT_GLSL),
        categories: &[AlgebraicCategory::Grp],
    },
    BuiltinTypeDef {
        ty: Type::E3,
        aliases: &["E3"],
        display_name: "E3",
        support_glsl: Some(E3_GROUP_SUPPORT_GLSL),
        categories: &[AlgebraicCategory::Grp],
    },
];

fn builtin_type_support_glsl(name: &str) -> Option<&'static str> {
    BUILTIN_TYPE_DEFS
        .iter()
        .find(|def| def.aliases.contains(&name))
        .and_then(|def| def.support_glsl)
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
    Quat,
    E2,
    E3,
    Custom {
        name: String,
        categories: Vec<AlgebraicCategory>,
    },
    Vec2,
    Vec3,
    Vec4,
    Mat(usize, usize),
    Array(Box<Type>),
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
            Self::E2 => "E2".to_string(),
            Self::E3 => "E3".to_string(),
            Self::Custom { name, .. } => name.clone(),
            Self::Vec2 => "vec2".to_string(),
            Self::Vec3 => "vec3".to_string(),
            Self::Vec4 => "vec4".to_string(),
            Self::Mat(rows, columns) => matrix_glsl_type(*rows, *columns),
            Self::Array(element) => format!("{}[]", element.glsl_name()),
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
                Self::Custom { name, .. } => name.clone(),
                Self::Product(_) => "Product".to_string(),
                Self::Func(_, _) => "Func".to_string(),
                Self::Array(element) => format!("Array({})", format_type(element)),
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

fn custom_type(name: &str, category: AlgebraicCategory) -> Type {
    Type::Custom {
        name: name.to_string(),
        categories: vec![category],
    }
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
    if let Type::Custom { categories, .. } = ty {
        return categories
            .iter()
            .any(|candidate| *candidate == category || category_implies(*candidate, category));
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
struct ProvidedTypeDecl {
    name: String,
    category: AlgebraicCategory,
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
struct InferredBindingDecl {
    name: String,
    expr: Expr,
    generated: bool,
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
    inferred_bindings: Vec<InferredBindingDecl>,
    output: OutputDecl,
}

#[derive(Clone, Debug)]
enum Decl {
    ProvidedType(ProvidedTypeDecl),
    Input(InputDecl),
    Func(FuncDecl),
    ValueBinding(ValueBindingDecl),
    Binding(BindingDecl),
    InferredBinding(InferredBindingDecl),
    Output(OutputDecl),
}

#[derive(Clone, Debug)]
enum Expr {
    Number(f64),
    Ident(String),
    Tuple(Vec<Expr>),
    Array(Vec<Expr>),
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },
    Index {
        array: Box<Expr>,
        index: Box<Expr>,
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
    Int(i64),
    Neutral {
        kind: NeutralKind,
        ty: Type,
    },
    Var {
        name: String,
        ty: Type,
        array_len: Option<usize>,
    },
    Call {
        func: String,
        args: Vec<ValueExpr>,
        ty: Type,
    },
    Array {
        element_ty: Type,
        elements: Vec<ValueExpr>,
    },
    Index {
        array: Box<ValueExpr>,
        index: Box<ValueExpr>,
        ty: Type,
    },
    Concat {
        element_ty: Type,
        left: Box<ValueExpr>,
        right: Box<ValueExpr>,
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
            Self::Int(_) => Type::Int,
            Self::Neutral { ty, .. } => ty.clone(),
            Self::Var { ty, .. } => ty.clone(),
            Self::Call { ty, .. } => ty.clone(),
            Self::Array { element_ty, .. } => Type::Array(Box::new(element_ty.clone())),
            Self::Index { ty, .. } => ty.clone(),
            Self::Concat { element_ty, .. } => Type::Array(Box::new(element_ty.clone())),
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

    fn array_len(&self) -> Option<usize> {
        match self {
            Self::Var { array_len, .. } => *array_len,
            Self::Array { elements, .. } => Some(elements.len()),
            Self::Concat { left, right, .. } => Some(left.array_len()? + right.array_len()?),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NeutralKind {
    Zero,
    One,
    Identity,
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
    IsometryTransform {
        object: Box<ObjectExpr>,
        transform: ValueExpr,
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
        Type::Object
    } else {
        Type::Product(vec![Type::Object; op.object_arg_count])
    };
    let output = Type::func(object_domain, Type::Object);
    match op.value_arg_types.as_slice() {
        [] => output,
        [value_arg] => Type::func(value_arg.clone(), output),
        value_args => Type::func(Type::Product(value_args.to_vec()), output),
    }
}

fn ensure_type(actual: &Type, expected: &Type, context: &str) -> Result<(), Error> {
    if actual == expected {
        return Ok(());
    }
    if let (Type::Array(actual), Type::Array(expected)) = (actual, expected) {
        return ensure_type(actual, expected, context);
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
        Type::Array(element) => format!("Array({})", format_type(element)),
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
            Expr::Index { .. } | Expr::Array(_) => {
                return Err(Error::new("unsupported callable object expression"))
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
