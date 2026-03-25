use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;

pub fn compile_program(source: &str) -> Result<String, Error> {
    let registry = Registry::default();
    let program = Parser::new(source).parse_program()?;
    let typed = TypedProgram::from_program(&program, &registry)?;
    Ok(typed.emit_glsl(&registry))
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
enum Type {
    Float,
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
            Self::Vec3 => "vec3",
            Self::Obj3 | Self::Func(_, _) => "",
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
    ty: Type,
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
    Vec3(Box<ValueExpr>, Box<ValueExpr>, Box<ValueExpr>),
}

impl ValueExpr {
    fn ty(&self) -> Type {
        match self {
            Self::Float(_) => Type::Float,
            Self::Var(_, ty) => ty.clone(),
            Self::Call { ty, .. } => ty.clone(),
            Self::Binary { ty, .. } => ty.clone(),
            Self::Vec3(_, _, _) => Type::Vec3,
        }
    }
}

#[derive(Clone, Debug)]
enum ObjectExpr {
    Var(String),
    Primitive {
        name: String,
        param_type: String,
        fields: Vec<(String, ValueExpr)>,
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
            if output_ty != Type::Float && output_ty != Type::Vec3 {
                return Err(Error::new(format!(
                    "function '{}' currently only supports float or vec3 outputs",
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

        ensure_type(&program.output.ty, &Type::Obj3, "output")?;
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
                Type::Float | Type::Vec3 => {
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
    param_type: &'static str,
    fields: Vec<(&'static str, Type)>,
    support_glsl: &'static str,
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
        let primitives = HashMap::from([(
            "Ball3D",
            PrimitiveDef {
                param_type: "ParamBall3D",
                fields: vec![("r", Type::Float)],
                support_glsl: "struct ParamBall3D {\n    float r;\n};\n\nfloat sdf0_Ball3D(vec3 p, ParamBall3D params) {\n    return length(p) - params.r;\n}",
            },
        )]);

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
            for (expected_name, expected_ty) in &primitive.fields {
                let value = fields
                    .iter()
                    .find(|(field_name, _)| field_name == expected_name)
                    .ok_or_else(|| {
                        Error::new(format!(
                            "primitive '{}' is missing field '{}'",
                            name, expected_name
                        ))
                    })?;
                let typed = infer_value_expr(&value.1, env, None)?;
                ensure_type(
                    &typed.ty(),
                    expected_ty,
                    &format!("field '{}.{}'", name, expected_name),
                )?;
                typed_fields.push(((*expected_name).to_string(), typed));
            }
            Ok(ObjectExpr::Primitive {
                name: name.clone(),
                param_type: primitive.param_type.to_string(),
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
        Expr::Tuple(items) => {
            if items.len() != 3 {
                return Err(Error::new(
                    "only vec3 tuples are supported in value expressions",
                ));
            }
            let x = infer_value_expr(&items[0], env, lift_param)?;
            let y = infer_value_expr(&items[1], env, lift_param)?;
            let z = infer_value_expr(&items[2], env, lift_param)?;
            ensure_type(&x.ty(), &Type::Float, "vec3 element 1")?;
            ensure_type(&y.ty(), &Type::Float, "vec3 element 2")?;
            ensure_type(&z.ty(), &Type::Float, "vec3 element 3")?;
            Ok(ValueExpr::Vec3(Box::new(x), Box::new(y), Box::new(z)))
        }
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
                Type::Float | Type::Vec3 => Ok(ValueExpr::Call {
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
        Type::Float | Type::Vec3 => Ok(ValueExpr::Var(name.to_string(), ty)),
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
        (BinOp::Add | BinOp::Sub, Type::Vec3, Type::Vec3) => Ok(Type::Vec3),
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
        ObjectExpr::Primitive {
            name,
            param_type,
            fields,
        } => {
            let rendered_fields = fields
                .iter()
                .map(|(_, expr)| emit_value_expr(expr, helper_names))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "sdf0_{}({}, {}({}))",
                name, point_expr, param_type, rendered_fields
            )
        }
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
            let (left, expr_source) = split_once_required(rest.trim(), '=')?;
            let ty = parse_type(left.trim())?;
            let expr = ExprParser::new(expr_source.trim()).parse()?;
            return Ok(Decl::Output(OutputDecl { ty, expr }));
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
        let mut expr = self.parse_postfix()?;
        loop {
            let op = match self.peek() {
                Some(Token::Star) => BinOp::Mul,
                Some(Token::Slash) => BinOp::Div,
                _ => break,
            };
            self.index += 1;
            let rhs = self.parse_postfix()?;
            expr = Expr::Binary {
                op,
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::compile_program;

    #[test]
    fn emits_only_used_support_code() {
        let source = "Obj3 A = Ball3D(r=3)\nout: Obj3 = A\n";
        let glsl = compile_program(source).unwrap();

        assert!(glsl.contains("struct ParamBall3D"));
        assert!(glsl.contains("float sdf0_Ball3D"));
        assert!(!glsl.contains("op_smooth_union"));
    }

    #[test]
    fn rejects_unknown_primitive_field() {
        let source = "Obj3 A = Ball3D(radius=3)\nout: Obj3 = A\n";
        let error = compile_program(source).unwrap_err().to_string();
        assert!(error.contains("missing field 'r'"));
    }

    #[test]
    fn rejects_old_binding_syntax() {
        let source = "A : Obj3 = Ball3D(r=3)\nout: Obj3 = A\n";
        let error = compile_program(source).unwrap_err().to_string();
        assert!(error.contains("use 'type name = value'"));
    }
}
