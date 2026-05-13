use super::*;

/// Canonicalizes pure surface syntax before semantic analysis.
pub(super) fn preprocess_program(mut program: Program) -> Result<Program, Error> {
    for input in &mut program.inputs {
        preprocess_input_decl(input)?;
    }
    for func in &mut program.funcs {
        preprocess_func_decl(func)?;
    }
    for binding in &mut program.value_bindings {
        preprocess_value_binding_decl(binding)?;
    }
    for binding in &mut program.bindings {
        preprocess_expr(&mut binding.expr)?;
    }
    for binding in &mut program.inferred_bindings {
        preprocess_inferred_binding_decl(binding);
        preprocess_expr(&mut binding.expr)?;
    }
    Ok(program)
}

fn preprocess_input_decl(input: &mut InputDecl) -> Result<(), Error> {
    if let Some(name) = operator_decl_helper_name(&input.name) {
        if !matches!(input.ty, Type::Func(_, _)) {
            return Err(Error::new(format!(
                "operator declaration '{}' must have a function type",
                input.name
            ))
            .with_line(input.line));
        }
        input.name = name;
        return Ok(());
    }
    if let Some(name) = neutral_decl_helper_name(&input.name, &input.ty) {
        input.name = name;
    }
    Ok(())
}

fn preprocess_func_decl(func: &mut FuncDecl) -> Result<(), Error> {
    if let Some(name) = operator_decl_helper_name(&func.name) {
        if !matches!(func.ty, Type::Func(_, _)) {
            return Err(Error::new(format!(
                "operator declaration '{}' must have a function type",
                func.name
            ))
            .with_line(func.line));
        }
        func.name = name;
    }
    preprocess_func_body(&mut func.body)
}

fn preprocess_value_binding_decl(binding: &mut ValueBindingDecl) -> Result<(), Error> {
    if let Some(name) = neutral_decl_helper_name(&binding.name, &binding.ty) {
        binding.name = name;
    }
    preprocess_expr(&mut binding.expr)
}

fn preprocess_inferred_binding_decl(binding: &mut InferredBindingDecl) {
    if let Some(name) = operator_decl_helper_name(&binding.name) {
        binding.name = name;
    }
}

fn preprocess_func_body(body: &mut FuncBody) -> Result<(), Error> {
    match body {
        FuncBody::Expr(expr) => preprocess_expr(expr),
        FuncBody::RawGlsl(_) | FuncBody::RawGlslClosure { .. } => Ok(()),
    }
}

fn preprocess_expr(expr: &mut Expr) -> Result<(), Error> {
    match expr {
        Expr::Bool(_)
        | Expr::Number(_)
        | Expr::RawString(_)
        | Expr::Ident(_)
        | Expr::Operator(_) => Ok(()),
        Expr::Closure { body, .. } => preprocess_expr(body),
        Expr::Tuple(items) | Expr::Array(items) => preprocess_exprs(items),
        Expr::Call { callee, args } => {
            preprocess_expr(callee)?;
            preprocess_exprs(args)?;
            let replacement = if let Expr::Operator(op) = &**callee {
                if args.len() != 2 {
                    return Err(Error::new(format!(
                        "operator '&{}' expects 2 argument(s), got {}",
                        op.symbol(),
                        args.len()
                    )));
                }
                Some(Expr::Binary {
                    op: *op,
                    left: Box::new(args[0].clone()),
                    right: Box::new(args[1].clone()),
                })
            } else {
                None
            };
            if let Some(replacement) = replacement {
                *expr = replacement;
                preprocess_expr(expr)?;
            }
            Ok(())
        }
        Expr::FieldAccess { object, .. } => preprocess_expr(object),
        Expr::Conditional {
            condition,
            then_branch,
            else_branch,
        } => {
            preprocess_expr(condition)?;
            preprocess_expr(then_branch)?;
            if let Some(else_branch) = else_branch {
                preprocess_expr(else_branch)?;
            }
            Ok(())
        }
        Expr::Index { array, index } => {
            preprocess_expr(array)?;
            preprocess_expr(index)
        }
        Expr::Unary { expr, .. } => preprocess_expr(expr),
        Expr::Binary { left, right, .. } => {
            preprocess_expr(left)?;
            preprocess_expr(right)
        }
        Expr::Constructor { args, .. } => match args {
            ConstructorArgs::Named(args) => {
                for (_, expr) in args {
                    preprocess_expr(expr)?;
                }
                Ok(())
            }
            ConstructorArgs::Positional(args) => preprocess_exprs(args),
        },
    }
}

fn preprocess_exprs(exprs: &mut [Expr]) -> Result<(), Error> {
    for expr in exprs {
        preprocess_expr(expr)?;
    }
    Ok(())
}

/// Returns the internal helper name for a neutral-slot declaration such as `A 0 = z`.
pub(super) fn neutral_decl_helper_name(name: &str, ty: &Type) -> Option<String> {
    let type_name = ty.type_name();
    match name {
        "0" => Some(neutral_helper_name("zero", &type_name)),
        "1" => Some(neutral_helper_name("one", &type_name)),
        "e" if matches!(ty, Type::Custom { .. } | Type::Isom2 | Type::Isom3) => {
            Some(neutral_helper_name("e", &type_name))
        }
        _ => None,
    }
}

fn neutral_helper_name(kind: &str, type_name: &str) -> String {
    format!("__{kind}_{type_name}")
}

pub(super) fn is_compiler_helper_name(name: &str) -> bool {
    matches!(
        name,
        "__add" | "__sub" | "__mult" | "__div" | "__inv" | "__scale"
    ) || name.starts_with("__zero_")
        || name.starts_with("__one_")
        || name.starts_with("__e_")
}

/// Returns the internal helper name for an operator-reference declaration such as
/// `Hom(A × A, A) &+ = add`.
fn operator_decl_helper_name(name: &str) -> Option<String> {
    let op_name = normalized_operator_decl_name(name)?;
    match op_name {
        "+" => Some("__add".to_string()),
        "*" => Some("__mult".to_string()),
        "/" => Some("__div".to_string()),
        "~" => Some("__inv".to_string()),
        "-" => Some("__sub".to_string()),
        _ => None,
    }
}

/// Normalizes operator-reference declaration names, accepting both `&+` and `&(+)`.
fn normalized_operator_decl_name(name: &str) -> Option<&str> {
    let rest = name.strip_prefix('&')?;
    rest.strip_prefix('(')
        .and_then(|inner| inner.strip_suffix(')'))
        .or(Some(rest))
}
