/// Type-checks helper logic for infer_value_expr.
fn infer_value_expr(
    expr: &Expr,
    env: &Env<'_>,
    lift_param: Option<&str>,
) -> Result<ValueExpr, Error> {
    match expr {
        Expr::Bool(value) => Ok(ValueExpr::Bool(*value)),
        Expr::Number(value) => Ok(ValueExpr::Float(*value)),
        Expr::RawString(_) => Err(Error::new(
            "raw GLSL strings are only valid as module function bodies",
        )),
        Expr::Closure { .. } => Err(Error::new("closures are only valid as function bodies")),
        Expr::Operator(op) => Err(Error::new(format!(
            "operator reference '&{}' needs a call or function context",
            op.symbol()
        ))),
        Expr::Ident(name) => infer_identifier_value(name, env, lift_param),
        Expr::FieldAccess { object, field } => {
            infer_value_field_access(object, field, env, lift_param)
        }
        Expr::Conditional {
            condition,
            then_branch,
            else_branch,
        } => infer_conditional_value_expr(
            condition,
            then_branch,
            else_branch.as_deref(),
            env,
            lift_param,
        ),
        Expr::Tuple(items) => infer_tuple_value_expr(items, env, lift_param),
        Expr::Array(_) => Err(Error::new(
            "bracket literals need an expected vector or matrix type; use Array(...) for arrays",
        )),
        Expr::Index { array, index } => infer_index_expr(array, index, env, lift_param),
        Expr::Call { callee, args } => {
            if let Expr::Operator(op) = &**callee {
                if args.len() != 2 {
                    return Err(Error::new(format!(
                        "operator '&{}' expects 2 argument(s), got {}",
                        op.symbol(),
                        args.len()
                    )));
                }
                return infer_value_expr(
                    &Expr::Binary {
                        op: *op,
                        left: Box::new(args[0].clone()),
                        right: Box::new(args[1].clone()),
                    },
                    env,
                    lift_param,
                );
            }
            if let Some(result) = infer_array_builtin(expr, env, lift_param)? {
                return Ok(result);
            }
            if let Some(result) = infer_rot_builtin(callee, args, env, lift_param)? {
                return Ok(result);
            }
            if let Some(result) = infer_differential_builtin(expr, env, lift_param)? {
                return Ok(result);
            }
            let name = match &**callee {
                Expr::Ident(name) => name,
                _ => {
                    let func = infer_function_expr(callee, env)?;
                    if args.len() != 1 {
                        return Err(Error::new("function closures expect one argument"));
                    }
                    let arg = infer_value_expr_for_type(&args[0], &func.input, env, lift_param)?;
                    ensure_type(&arg.ty(), &func.input, "closure call")?;
                    return Ok(apply_function_expr(&func, arg));
                }
            };
            if let Some(result) = infer_type_constructor_call(name, args, env, lift_param)? {
                return Ok(result);
            }
            if let Some(result) = infer_complex_overload_call(name, args, env, lift_param)? {
                return Ok(result);
            }
            if let Some(result) = infer_monoid_pow_call(name, args, env, lift_param)? {
                return Ok(result);
            }
            if let Some(result) =
                infer_projection_or_diagonal_call(name, args, env, lift_param, None)?
            {
                return Ok(result);
            }
            if let Some(result) = infer_product_domain_call(callee, args, env, lift_param)? {
                return Ok(result);
            }
            if let Some(result) = infer_pointwise_value_call(name, args, env, lift_param, None)? {
                return Ok(result);
            }
            infer_value_call(name, args, env, lift_param, None)
        }
        Expr::Binary {
            op: BinOp::Compose,
            left,
            right,
        } => infer_composed_value_expr(left, right, env, lift_param),
        Expr::Binary {
            op: BinOp::Product,
            left,
            right,
        } => infer_function_product_value_expr(left, right, env, lift_param),
        Expr::Unary { op, expr } => {
            let expr = infer_value_expr(expr, env, lift_param)?;
            infer_unary_value_expr(*op, expr)
        }
        Expr::Binary { op, left, right } => {
            let left = infer_value_expr(left, env, lift_param)?;
            let right = match (*op, left.ty()) {
                (BinOp::Mul, Type::Isom2) => {
                    infer_value_expr_for_type(right, &Type::Vec2, env, lift_param)?
                }
                (BinOp::Mul, Type::Isom3) => {
                    infer_value_expr_for_type(right, &Type::Vec3, env, lift_param)?
                }
                (BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge, Type::Int) => {
                    infer_value_expr_for_type(right, &Type::Int, env, lift_param)?
                }
                _ => infer_value_expr(right, env, lift_param)?,
            };
            let (left, right, ty) = match infer_binary_type(*op, &left.ty(), &right.ty()) {
                Ok(ty) => (left, right, ty),
                Err(original_err) => {
                    if let Some(right_cast) =
                        numeric_widen_cast_type_for_binary(&right.ty(), &left.ty())
                            .and_then(|ty| try_numeric_widen_cast_value(&right, &ty))
                    {
                        if let Ok(ty) = infer_binary_type(*op, &left.ty(), &right_cast.ty()) {
                            (left, right_cast, ty)
                        } else {
                            return Err(original_err);
                        }
                    } else if let Some(left_cast) =
                        numeric_widen_cast_type_for_binary(&left.ty(), &right.ty())
                            .and_then(|ty| try_numeric_widen_cast_value(&left, &ty))
                    {
                        if let Ok(ty) = infer_binary_type(*op, &left_cast.ty(), &right.ty()) {
                            (left_cast, right, ty)
                        } else {
                            return Err(original_err);
                        }
                    } else if let Some(right_cast) = try_neutral_cast_value(&right, &left.ty()) {
                        if let Ok(ty) = infer_binary_type(*op, &left.ty(), &right_cast.ty()) {
                            (left, right_cast, ty)
                        } else {
                            return Err(original_err);
                        }
                    } else if let Some(left_cast) = try_neutral_cast_value(&left, &right.ty()) {
                        if let Ok(ty) = infer_binary_type(*op, &left_cast.ty(), &right.ty()) {
                            (left_cast, right, ty)
                        } else {
                            return Err(original_err);
                        }
                    } else {
                        return Err(original_err);
                    }
                }
            };
            Ok(ValueExpr::Binary {
                op: *op,
                left: Box::new(left),
                right: Box::new(right),
                ty,
            })
        }
        Expr::Constructor { name, args } => match args {
            ConstructorArgs::Positional(args) if name == "Array" => {
                infer_array_literal(args, env, lift_param, None)
            }
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

/// Type-checks helper logic for infer_value_field_access.
fn infer_value_field_access(
    object: &Expr,
    field: &str,
    env: &Env<'_>,
    lift_param: Option<&str>,
) -> Result<ValueExpr, Error> {
    if let Ok(value) = infer_value_expr(object, env, lift_param) {
        if let Some((ty, field)) = field_access(&value.ty(), field, env) {
            return Ok(ValueExpr::FieldAccess {
                value: Box::new(value),
                field,
                ty,
            });
        }
    }
    if let Some(param_name) = lift_param {
        let expr = Expr::FieldAccess {
            object: Box::new(object.clone()),
            field: field.to_string(),
        };
        let func = infer_function_expr(&expr, env)?;
        return Ok(apply_function_expr(
            &func,
            ValueExpr::Var {
                name: param_name.to_string(),
                ty: func.input.clone(),
                array_len: None,
            },
        ));
    }
    Err(Error::new(format!("value has no field '{}'", field)))
}

/// Type-checks helper logic for field_access.
fn field_access(ty: &Type, field: &str, env: &Env<'_>) -> Option<(Type, String)> {
    match ty {
        Type::Vec2 | Type::Complex => {
            vector_field_access(2, field).map(|field| (Type::Float, field))
        }
        Type::Vec3 => vector_field_access(3, field).map(|field| (Type::Float, field)),
        Type::Vec4 | Type::Quat => vector_field_access(4, field).map(|field| (Type::Float, field)),
        Type::Custom { name, .. } => {
            let product_type = env.product_type(name)?;
            product_field_access(product_type, field)
        }
        _ => None,
    }
}

/// Type-checks helper logic for vector_field_access.
fn vector_field_access(dimension: usize, field: &str) -> Option<String> {
    let index = positional_product_field_index(field)?;
    if index >= dimension {
        return None;
    }
    Some(["x", "y", "z", "w"][index].to_string())
}

/// Type-checks helper logic for product_field_access.
fn product_field_access(product_type: &ProductTypeDecl, field: &str) -> Option<(Type, String)> {
    if let Some(index) = product_type
        .field_names
        .iter()
        .position(|candidate| candidate == field)
    {
        return Some((product_type.components[index].clone(), field.to_string()));
    }
    if !uses_default_product_field_names(product_type) {
        return None;
    }
    let index = positional_product_field_index(field)?;
    if index >= product_type.components.len() {
        return None;
    }
    Some((
        product_type.components[index].clone(),
        default_product_field_name(product_type.components.len(), index),
    ))
}

/// Type-checks helper logic for uses_default_product_field_names.
fn uses_default_product_field_names(product_type: &ProductTypeDecl) -> bool {
    product_type.field_names.len() == product_type.components.len()
        && product_type
            .field_names
            .iter()
            .enumerate()
            .all(|(index, field)| {
                field == &default_product_field_name(product_type.components.len(), index)
            })
}

/// Type-checks helper logic for positional_product_field_index.
fn positional_product_field_index(field: &str) -> Option<usize> {
    match field {
        "x" => Some(0),
        "y" => Some(1),
        "z" => Some(2),
        "w" => Some(3),
        _ => field.strip_prefix('x')?.parse::<usize>().ok(),
    }
}

/// Type-checks helper logic for default_product_field_name.
fn default_product_field_name(count: usize, index: usize) -> String {
    if index >= count {
        unreachable!("field index outside product");
    }
    format!("x{index}")
}

/// Type-checks helper logic for infer_value_expr_for_type.
fn infer_value_expr_for_type(
    expr: &Expr,
    expected_ty: &Type,
    env: &Env<'_>,
    lift_param: Option<&str>,
) -> Result<ValueExpr, Error> {
    match (expected_ty, expr) {
        (Type::Bool, Expr::Bool(value)) => Ok(ValueExpr::Bool(*value)),
        (_, Expr::Number(value)) if (*value - 0.0).abs() < f64::EPSILON => {
            if expected_ty == &Type::Int {
                return infer_int_expr(expr, env, lift_param);
            }
            if expected_ty == &Type::Float {
                return infer_value_expr(expr, env, lift_param);
            }
            if let Some(kind) = neutral_kind_for_type(expected_ty, NeutralKind::Zero) {
                return Ok(ValueExpr::Neutral {
                    kind,
                    ty: expected_ty.clone(),
                });
            }
            infer_value_expr(expr, env, lift_param)
        }
        (_, Expr::Number(value)) if (*value - 1.0).abs() < f64::EPSILON => {
            if expected_ty == &Type::Int {
                return infer_int_expr(expr, env, lift_param);
            }
            if expected_ty == &Type::Float {
                return infer_value_expr(expr, env, lift_param);
            }
            if let Some(kind) = neutral_kind_for_type(expected_ty, NeutralKind::One) {
                return Ok(ValueExpr::Neutral {
                    kind,
                    ty: expected_ty.clone(),
                });
            }
            infer_value_expr(expr, env, lift_param)
        }
        (_, Expr::Ident(name)) if parse_matrix_basis_name(name).is_some() => {
            let Some((row, column)) = parse_matrix_basis_name(name) else {
                unreachable!();
            };
            let Type::Mat(rows, columns) = expected_ty else {
                return Err(Error::new(format!(
                    "matrix basis literal '{}' needs an expected matrix type",
                    name
                )));
            };
            if row == 0 || column == 0 || row > *rows || column > *columns {
                return Err(Error::new(format!(
                    "matrix basis literal '{}' is outside {}",
                    name,
                    format_type(expected_ty)
                )));
            }
            Ok(ValueExpr::MatrixBasis {
                row,
                column,
                ty: expected_ty.clone(),
            })
        }
        (_, Expr::Ident(name)) if parse_unit_vector_basis_name(name).is_some() => {
            let Some((dimension, index)) = parse_unit_vector_basis_name(name) else {
                unreachable!();
            };
            let Some(expected_dimension) = vector_dimension(expected_ty) else {
                return Err(Error::new(format!(
                    "unit vector literal '{}' needs an expected vector type",
                    name
                )));
            };
            if dimension != expected_dimension {
                return Err(Error::new(format!(
                    "unit vector literal '{}' has dimension {}, expected {}",
                    name,
                    dimension,
                    format_type(expected_ty)
                )));
            }
            if index == 0 || index > dimension {
                return Err(Error::new(format!(
                    "unit vector literal '{}' is outside {}",
                    name,
                    format_type(expected_ty)
                )));
            }
            Ok(ValueExpr::UnitVectorBasis {
                dimension,
                index,
                ty: expected_ty.clone(),
            })
        }
        (_, Expr::Ident(name)) if parse_identity_matrix_name(name).is_some() => {
            let Some(dimension) = parse_identity_matrix_name(name) else {
                unreachable!();
            };
            let expected = Type::Mat(dimension, dimension);
            ensure_type(
                &expected,
                expected_ty,
                &format!("identity literal '{}'", name),
            )?;
            Ok(ValueExpr::Neutral {
                kind: NeutralKind::Identity,
                ty: expected,
            })
        }
        (_, Expr::Ident(name)) if name == "e" && !env.has_binding(name) => {
            if let Some(kind) = neutral_kind_for_type(expected_ty, NeutralKind::Identity) {
                return Ok(ValueExpr::Neutral {
                    kind,
                    ty: expected_ty.clone(),
                });
            }
            infer_value_expr(expr, env, lift_param)
        }
        (
            _,
            Expr::Conditional {
                condition,
                then_branch,
                else_branch,
            },
        ) => infer_conditional_value_expr_for_type(
            condition,
            then_branch,
            else_branch.as_deref(),
            expected_ty,
            env,
            lift_param,
        ),
        (_, Expr::Unary { op, expr }) => {
            let expr = infer_value_expr_for_type(expr, expected_ty, env, lift_param)?;
            let value = infer_unary_value_expr(*op, expr)?;
            ensure_type(&value.ty(), expected_ty, "unary expression")?;
            Ok(value)
        }
        (_, Expr::Ident(name)) if lift_param.is_some() && env.get_value(name).is_none() => {
            let param_name = lift_param_name(lift_param, name)?;
            let func = infer_function_expr_for_type(expr, env, &Type::Float, expected_ty)?;
            Ok(apply_function_expr(
                &func,
                ValueExpr::Var {
                    name: param_name,
                    ty: Type::Float,
                    array_len: None,
                },
            ))
        }
        (
            _,
            Expr::Binary {
                op: BinOp::Compose, ..
            },
        ) if lift_param.is_some() => {
            let Some(param_name) = lift_param else {
                return Err(Error::new(
                    "anonymous composed expression requires a lift parameter".to_string(),
                ));
            };
            let param_name = param_name.to_string();
            let func = infer_function_expr_for_type(expr, env, &Type::Float, expected_ty)?;
            Ok(apply_function_expr(
                &func,
                ValueExpr::Var {
                    name: param_name,
                    ty: Type::Float,
                    array_len: None,
                },
            ))
        }
        (
            Type::Vec2 | Type::Vec3 | Type::Vec4,
            Expr::Binary {
                op: BinOp::Product, ..
            },
        ) if lift_param.is_some() => {
            let Some(param_name) = lift_param else {
                return Err(Error::new(
                    "vector product lift requires a lift parameter".to_string(),
                ));
            };
            let param_name = param_name.to_string();
            let func = infer_function_expr(expr, env)?;
            ensure_type(&func.output, expected_ty, "function product")?;
            Ok(apply_function_expr(
                &func,
                ValueExpr::Var {
                    name: param_name,
                    ty: func.input.clone(),
                    array_len: None,
                },
            ))
        }
        (Type::Vec2 | Type::Vec3 | Type::Vec4, Expr::Array(items)) => {
            infer_vector_literal_for_type(items, expected_ty, env, lift_param)
        }
        (Type::Mat(_, _), Expr::Array(items)) => {
            infer_matrix_literal_for_type(items, expected_ty, env, lift_param)
        }
        (Type::Product(parts), Expr::Tuple(items)) => {
            infer_product_tuple_for_type(items, parts, env, lift_param)
        }
        (Type::Vec4, Expr::Tuple(items)) if items.len() == 2 => {
            let xyz = infer_value_expr_for_type(&items[0], &Type::Vec3, env, lift_param)?;
            let w = infer_value_expr_for_type(&items[1], &Type::Float, env, lift_param)?;
            Ok(ValueExpr::Call {
                func: "vec4".to_string(),
                args: vec![xyz, w],
                ty: Type::Vec4,
            })
        }
        (Type::Float, _) => {
            let value = infer_value_expr(expr, env, lift_param)?;
            Ok(try_numeric_widen_cast_value(&value, expected_ty).unwrap_or(value))
        }
        (Type::Int, _) => {
            if matches!(expr, Expr::Number(_)) {
                return infer_int_expr(expr, env, lift_param);
            }
            let value = infer_value_expr(expr, env, lift_param)?;
            Ok(try_numeric_widen_cast_value(&value, expected_ty).unwrap_or(value))
        }
        (
            Type::Array(element_ty),
            Expr::Constructor {
                name,
                args: ConstructorArgs::Positional(items),
            },
        ) if name == "Array" => infer_array_literal(items, env, lift_param, Some(element_ty)),
        (Type::Array(_), Expr::Array(_)) => Err(Error::new(
            "array values use Array(...); brackets are reserved for vectors and matrices",
        )),
        (_, Expr::Call { callee, args }) => {
            let Expr::Ident(name) = &**callee else {
                return infer_value_expr(expr, env, lift_param);
            };
            if let Some(result) = infer_type_constructor_call(name, args, env, lift_param)? {
                ensure_type(&result.ty(), expected_ty, "constructor expression")?;
                return Ok(result);
            }
            if let Some(result) = infer_rot_builtin(callee, args, env, lift_param)? {
                ensure_type(&result.ty(), expected_ty, "rotation expression")?;
                return Ok(result);
            }
            if let Some(result) = infer_monoid_pow_call(name, args, env, lift_param)? {
                if types_compatible_for_expected(&result.ty(), expected_ty) {
                    return Ok(result);
                }
            }
            if let Some(result) =
                infer_projection_or_diagonal_call(name, args, env, lift_param, Some(expected_ty))?
            {
                return Ok(result);
            }
            if let Some(result) = infer_product_domain_call(callee, args, env, lift_param)? {
                if types_compatible_for_expected(&result.ty(), expected_ty) {
                    return Ok(result);
                }
            }
            if let Some(result) =
                infer_pointwise_value_call(name, args, env, lift_param, Some(expected_ty))?
            {
                return Ok(result);
            }
            infer_value_call(name, args, env, lift_param, Some(expected_ty))
        }
        _ => infer_value_expr(expr, env, lift_param),
    }
}

/// Type-checks helper logic for infer_product_tuple_for_type.
fn infer_product_tuple_for_type(
    items: &[Expr],
    parts: &[Type],
    env: &Env<'_>,
    lift_param: Option<&str>,
) -> Result<ValueExpr, Error> {
    if items.len() != parts.len() {
        return Err(Error::new(format!(
            "product tuple expects {} item(s), got {}",
            parts.len(),
            items.len()
        )));
    }
    let values = items
        .iter()
        .zip(parts.iter())
        .map(|(item, ty)| infer_value_expr_for_type(item, ty, env, lift_param))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ValueExpr::Product(values))
}

/// Type-checks helper logic for infer_conditional_value_expr.
fn infer_conditional_value_expr(
    condition: &Expr,
    then_branch: &Expr,
    else_branch: Option<&Expr>,
    env: &Env<'_>,
    lift_param: Option<&str>,
) -> Result<ValueExpr, Error> {
    let condition = infer_value_expr_for_type(condition, &Type::Bool, env, lift_param)?;
    ensure_type(&condition.ty(), &Type::Bool, "conditional condition")?;
    let then_branch = infer_value_expr(then_branch, env, lift_param)?;
    let output_ty = then_branch.ty();
    let else_branch = match else_branch {
        Some(else_branch) => {
            let else_branch = infer_value_expr(else_branch, env, lift_param)?;
            match cast_value_to_type(else_branch, &output_ty) {
                Ok(else_branch) => else_branch,
                Err(else_branch) => {
                    let else_ty = else_branch.ty();
                    let Ok(then_branch) = cast_value_to_type(then_branch, &else_ty) else {
                        return Err(Error::new(format!(
                            "conditional branches have incompatible types {} and {}",
                            format_type(&output_ty),
                            format_type(&else_ty)
                        )));
                    };
                    return Ok(ValueExpr::Conditional {
                        condition: Box::new(condition),
                        then_branch: Box::new(then_branch),
                        else_branch: Box::new(else_branch),
                        ty: else_ty,
                    });
                }
            }
        }
        None => zero_value_for_type(&output_ty).ok_or_else(|| {
            Error::new(format!(
                "conditional without else cannot use 0 as {}",
                format_type(&output_ty)
            ))
        })?,
    };
    Ok(ValueExpr::Conditional {
        condition: Box::new(condition),
        then_branch: Box::new(then_branch),
        else_branch: Box::new(else_branch),
        ty: output_ty,
    })
}

/// Type-checks helper logic for infer_conditional_value_expr_for_type.
fn infer_conditional_value_expr_for_type(
    condition: &Expr,
    then_branch: &Expr,
    else_branch: Option<&Expr>,
    expected_ty: &Type,
    env: &Env<'_>,
    lift_param: Option<&str>,
) -> Result<ValueExpr, Error> {
    let condition = infer_value_expr_for_type(condition, &Type::Bool, env, lift_param)?;
    ensure_type(&condition.ty(), &Type::Bool, "conditional condition")?;
    let then_branch = infer_value_expr_for_type(then_branch, expected_ty, env, lift_param)?;
    ensure_type(&then_branch.ty(), expected_ty, "conditional then branch")?;
    let else_branch = match else_branch {
        Some(else_branch) => infer_value_expr_for_type(else_branch, expected_ty, env, lift_param)?,
        None => zero_value_for_type(expected_ty).ok_or_else(|| {
            Error::new(format!(
                "conditional without else cannot use 0 as {}",
                format_type(expected_ty)
            ))
        })?,
    };
    ensure_type(&else_branch.ty(), expected_ty, "conditional else branch")?;
    Ok(ValueExpr::Conditional {
        condition: Box::new(condition),
        then_branch: Box::new(then_branch),
        else_branch: Box::new(else_branch),
        ty: expected_ty.clone(),
    })
}

/// Type-checks helper logic for cast_value_to_type.
fn cast_value_to_type(value: ValueExpr, expected_ty: &Type) -> Result<ValueExpr, ValueExpr> {
    if types_compatible_for_expected(&value.ty(), expected_ty) {
        return Ok(value);
    }
    if let Some(cast) = try_numeric_widen_cast_value(&value, expected_ty) {
        return Ok(cast);
    }
    if let Some(cast) = try_neutral_cast_value(&value, expected_ty) {
        return Ok(cast);
    }
    Err(value)
}

/// Type-checks helper logic for zero_value_for_type.
fn zero_value_for_type(ty: &Type) -> Option<ValueExpr> {
    if ty == &Type::Bool {
        return Some(ValueExpr::Bool(false));
    }
    neutral_kind_for_type(ty, NeutralKind::Zero).map(|kind| ValueExpr::Neutral {
        kind,
        ty: ty.clone(),
    })
}

/// Type-checks helper logic for infer_pointwise_value_call.
fn infer_pointwise_value_call(
    name: &str,
    args: &[Expr],
    env: &Env<'_>,
    lift_param: Option<&str>,
    expected_output: Option<&Type>,
) -> Result<Option<ValueExpr>, Error> {
    let Some(param_name) = lift_param else {
        return Ok(None);
    };
    let candidates = infer_pointwise_call_function_candidates(name, args, env)?;
    let matches = candidates
        .into_iter()
        .filter(|func| {
            expected_output
                .map(|expected| types_compatible_for_expected(&func.output, expected))
                .unwrap_or(true)
        })
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Ok(None);
    }
    let func = matches.into_iter().next().unwrap();
    Ok(Some(apply_function_expr(
        &func,
        ValueExpr::Var {
            name: param_name.to_string(),
            ty: func.input.clone(),
            array_len: None,
        },
    )))
}

/// Type-checks helper logic for infer_lifted_value_function.
fn infer_lifted_value_function(
    expr: &Expr,
    env: &Env<'_>,
) -> Result<(Type, Type, ValueExpr), Error> {
    let typed = infer_value_expr(expr, env, Some("t"))?;
    let Some(input) = lifted_param_type(&typed, "t")? else {
        return Err(Error::new("expression is not a function"));
    };
    let output = typed.ty();
    Ok((input, output, typed))
}

/// Type-checks helper logic for ensure_lift_param_type.
fn ensure_lift_param_type(expr: &ValueExpr, name: &str, expected: &Type) -> Result<(), Error> {
    if let Some(actual) = lifted_param_type(expr, name)? {
        ensure_type(&actual, expected, "function parameter")?;
    }
    Ok(())
}

/// Type-checks helper logic for lifted_param_type.
fn lifted_param_type(expr: &ValueExpr, name: &str) -> Result<Option<Type>, Error> {
    let mut ty = None;
    collect_lifted_param_type(expr, name, &mut ty)?;
    Ok(ty)
}

/// Type-checks helper logic for collect_lifted_param_type.
fn collect_lifted_param_type(
    expr: &ValueExpr,
    name: &str,
    ty: &mut Option<Type>,
) -> Result<(), Error> {
    match expr {
        ValueExpr::Bool(_)
        | ValueExpr::Float(_)
        | ValueExpr::Int(_)
        | ValueExpr::Neutral { .. } => {}
        ValueExpr::Var {
            name: var_name,
            ty: var_ty,
            ..
        } if var_name == name => match ty {
            Some(existing) => ensure_type(var_ty, existing, "lifted function parameter")?,
            None => *ty = Some(var_ty.clone()),
        },
        ValueExpr::Var { .. } => {}
        ValueExpr::Call { args, .. } | ValueExpr::Array { elements: args, .. } => {
            for arg in args {
                collect_lifted_param_type(arg, name, ty)?;
            }
        }
        ValueExpr::MonoidPow { exponent, base, .. } => {
            collect_lifted_param_type(exponent, name, ty)?;
            collect_lifted_param_type(base, name, ty)?;
        }
        ValueExpr::NumericWidenCast { value, .. } => {
            collect_lifted_param_type(value, name, ty)?;
        }
        ValueExpr::Conditional {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            collect_lifted_param_type(condition, name, ty)?;
            collect_lifted_param_type(then_branch, name, ty)?;
            collect_lifted_param_type(else_branch, name, ty)?;
        }
        ValueExpr::ObjectGetterCall {
            point, captures, ..
        } => {
            collect_lifted_param_type(point, name, ty)?;
            for capture in captures {
                collect_lifted_param_type(capture, name, ty)?;
            }
        }
        ValueExpr::FieldAccess { value, .. } => {
            collect_lifted_param_type(value, name, ty)?;
        }
        ValueExpr::Index { array, index, .. } => {
            collect_lifted_param_type(array, name, ty)?;
            collect_lifted_param_type(index, name, ty)?;
        }
        ValueExpr::Unary { expr, .. } => {
            collect_lifted_param_type(expr, name, ty)?;
        }
        ValueExpr::Concat { left, right, .. } | ValueExpr::Binary { left, right, .. } => {
            collect_lifted_param_type(left, name, ty)?;
            collect_lifted_param_type(right, name, ty)?;
        }
        ValueExpr::Vec2(x, y) => {
            collect_lifted_param_type(x, name, ty)?;
            collect_lifted_param_type(y, name, ty)?;
        }
        ValueExpr::Vec3(x, y, z) => {
            collect_lifted_param_type(x, name, ty)?;
            collect_lifted_param_type(y, name, ty)?;
            collect_lifted_param_type(z, name, ty)?;
        }
        ValueExpr::Vec4(x, y, z, w) => {
            collect_lifted_param_type(x, name, ty)?;
            collect_lifted_param_type(y, name, ty)?;
            collect_lifted_param_type(z, name, ty)?;
            collect_lifted_param_type(w, name, ty)?;
        }
        ValueExpr::Product(values) => {
            for value in values {
                collect_lifted_param_type(value, name, ty)?;
            }
        }
        ValueExpr::Matrix { rows, .. } => {
            for row in rows {
                collect_lifted_param_type(row, name, ty)?;
            }
        }
        ValueExpr::MatrixBasis { .. } => {}
        ValueExpr::UnitVectorBasis { .. } => {}
        ValueExpr::Derivative { epsilon, at, .. }
        | ValueExpr::Partial { epsilon, at, .. }
        | ValueExpr::Gradient { epsilon, at, .. }
        | ValueExpr::Divergence { epsilon, at, .. } => {
            collect_lifted_param_type(epsilon, name, ty)?;
            collect_lifted_param_type(at, name, ty)?;
        }
    }
    Ok(())
}

/// Type-checks helper logic for neutral_kind_for_type.
fn neutral_kind_for_type(ty: &Type, requested: NeutralKind) -> Option<NeutralKind> {
    match requested {
        NeutralKind::Zero => {
            if has_category(ty, AlgebraicCategory::Ab) {
                Some(NeutralKind::Zero)
            } else {
                None
            }
        }
        NeutralKind::One => {
            if matches!(ty, Type::Mat(rows, columns) if rows == columns) {
                Some(NeutralKind::Identity)
            } else if has_category(ty, AlgebraicCategory::Ring)
                || has_category(ty, AlgebraicCategory::DivRing)
                || has_category(ty, AlgebraicCategory::RAlg)
            {
                Some(NeutralKind::One)
            } else {
                None
            }
        }
        NeutralKind::Identity => {
            if has_category(ty, AlgebraicCategory::Grp)
                || matches!(ty, Type::Mat(rows, columns) if rows == columns)
            {
                Some(NeutralKind::Identity)
            } else {
                None
            }
        }
    }
}

/// Type-checks helper logic for parse_identity_matrix_name.
fn parse_identity_matrix_name(name: &str) -> Option<usize> {
    if let Some(suffixes) = parse_braced_usize_suffixes(name, "I") {
        let [dimension] = suffixes.as_slice() else {
            return None;
        };
        return (*dimension > 0).then_some(*dimension);
    }
    if let Some(suffixes) = parse_braced_usize_suffixes(name, "eye") {
        let [dimension] = suffixes.as_slice() else {
            return None;
        };
        return (*dimension > 0).then_some(*dimension);
    }
    name.strip_prefix('I')?
        .parse::<usize>()
        .ok()
        .filter(|dimension| *dimension > 0)
}

/// Type-checks helper logic for parse_matrix_basis_name.
fn parse_matrix_basis_name(name: &str) -> Option<(usize, usize)> {
    if let Some(suffixes) = parse_braced_usize_suffixes(name, "E") {
        let [row, column] = suffixes.as_slice() else {
            return None;
        };
        return Some((*row, *column));
    }
    let suffix = name.strip_prefix('E')?;
    if let Some((row, column)) = suffix.split_once('_') {
        return Some((row.parse().ok()?, column.parse().ok()?));
    }
    if suffix.len() == 2 && suffix.chars().all(|ch| ch.is_ascii_digit()) {
        let mut digits = suffix.chars();
        return Some((
            digits.next()?.to_digit(10)? as usize,
            digits.next()?.to_digit(10)? as usize,
        ));
    }
    None
}

/// Type-checks helper logic for parse_unit_vector_basis_name.
fn parse_unit_vector_basis_name(name: &str) -> Option<(usize, usize)> {
    let suffixes = parse_braced_usize_suffixes(name, "e")?;
    let [dimension, index] = suffixes.as_slice() else {
        return None;
    };
    (*dimension > 0).then_some((*dimension, *index))
}

/// Type-checks helper logic for parse_braced_usize_suffixes.
fn parse_braced_usize_suffixes(name: &str, prefix: &str) -> Option<Vec<usize>> {
    let mut rest = name.strip_prefix(prefix)?;
    if rest.is_empty() {
        return None;
    }
    let mut values = Vec::new();
    while let Some(inner_start) = rest.strip_prefix('{') {
        let end = inner_start.find('}')?;
        let value = inner_start[..end].parse::<usize>().ok()?;
        values.push(value);
        rest = &inner_start[end + 1..];
    }
    rest.is_empty().then_some(values)
}

/// Type-checks helper logic for parse_projection_name.
fn parse_projection_name(name: &str) -> Option<usize> {
    if let Some(values) = parse_braced_usize_suffixes(name, "p") {
        return (values.len() == 1).then_some(values[0]);
    }
    name.strip_prefix("projection_")?.parse::<usize>().ok()
}

/// Type-checks helper logic for parse_diagonal_name.
fn parse_diagonal_name(name: &str) -> Option<usize> {
    if let Some(values) = parse_braced_usize_suffixes(name, "diag") {
        return (values.len() == 1 && values[0] > 0).then_some(values[0]);
    }
    name.strip_prefix("diagonal")
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|dimension| *dimension > 0)
}

/// Type-checks helper logic for infer_type_constructor_call.
fn infer_type_constructor_call(
    name: &str,
    args: &[Expr],
    env: &Env<'_>,
    lift_param: Option<&str>,
) -> Result<Option<ValueExpr>, Error> {
    let (ty, arg_types) = match name {
        "Isom2" => (Type::Isom2, vec![Type::Mat(2, 2), Type::Vec2]),
        "Isom3" => (Type::Isom3, vec![Type::Mat(3, 3), Type::Vec3]),
        _ => {
            let Some(product_type) = env.product_type(name) else {
                return Ok(None);
            };
            (
                product_type_decl_type(product_type),
                product_type.components.clone(),
            )
        }
    };
    if args.len() != arg_types.len() {
        return Err(Error::new(format!(
            "constructor '{}' expects {} argument(s), got {}",
            name,
            arg_types.len(),
            args.len()
        )));
    }
    let mut typed_args = Vec::new();
    for (arg, expected_ty) in args.iter().zip(arg_types.iter()) {
        let typed = infer_value_expr_for_type(arg, expected_ty, env, lift_param)?;
        ensure_type(&typed.ty(), expected_ty, &format!("constructor '{}'", name))?;
        typed_args.push(typed);
    }
    Ok(Some(ValueExpr::Call {
        func: name.to_string(),
        args: typed_args,
        ty,
    }))
}

/// Type-checks helper logic for infer_projection_or_diagonal_call.
fn infer_projection_or_diagonal_call(
    name: &str,
    args: &[Expr],
    env: &Env<'_>,
    lift_param: Option<&str>,
    expected_output: Option<&Type>,
) -> Result<Option<ValueExpr>, Error> {
    if args.len() != 1 {
        return Ok(None);
    }
    if let Some(index) = parse_projection_name(name) {
        let arg = infer_value_expr(&args[0], env, lift_param)?;
        let func = if let Some(expected) = expected_output {
            infer_projection_function_for_type(index, &arg.ty(), expected, env)?
        } else {
            infer_projection_function_for_input(index, &arg.ty(), env)?
        };
        return Ok(Some(apply_function_expr(&func, arg)));
    }
    if let Some(dimension) = parse_diagonal_name(name) {
        let arg = infer_value_expr(&args[0], env, lift_param)?;
        let func = if let Some(expected) = expected_output {
            infer_diagonal_function_for_type(dimension, &arg.ty(), expected, env)?
        } else {
            infer_diagonal_function_for_input(dimension, &arg.ty())?
        };
        return Ok(Some(apply_function_expr(&func, arg)));
    }
    Ok(None)
}

/// Type-checks helper logic for infer_product_domain_call.
fn infer_product_domain_call(
    callee: &Expr,
    args: &[Expr],
    env: &Env<'_>,
    lift_param: Option<&str>,
) -> Result<Option<ValueExpr>, Error> {
    let Expr::Ident(name) = callee else {
        return Ok(None);
    };
    let Some(overloads) = env.function_overloads(name) else {
        return Ok(None);
    };
    let mut candidates = Vec::new();
    for overload in overloads {
        let Type::Func(input, output) = &overload.ty else {
            continue;
        };
        let Type::Product(parts) = input.as_ref() else {
            continue;
        };
        if parts.len() != args.len() {
            continue;
        }
        let mut typed_args = Vec::new();
        let mut ok = true;
        for (arg, expected_ty) in args.iter().zip(parts.iter()) {
            match infer_value_expr_for_type(arg, expected_ty, env, lift_param) {
                Ok(typed_arg) => typed_args.push(typed_arg),
                Err(_) => {
                    ok = false;
                    break;
                }
            }
        }
        if ok {
            let cost = typed_args
                .iter()
                .zip(parts.iter())
                .map(|(arg, expected)| call_arg_cost(&arg.ty(), expected, arg))
                .sum::<usize>();
            candidates.push((
                cost,
                ValueExpr::Call {
                    func: name.clone(),
                    args: typed_args,
                    ty: (**output).clone(),
                },
            ));
        }
    }
    candidates.sort_by_key(|(cost, _)| *cost);
    match candidates.len() {
        0 => Ok(None),
        1 => Ok(candidates.pop().map(|(_, candidate)| candidate)),
        _ if candidates[0].0 < candidates[1].0 => Ok(Some(candidates.remove(0).1)),
        _ if tied_candidates_are_neutral_casts(candidates.iter().map(|(_, candidate)| candidate))
        => {
            candidates.sort_by_key(|(_, candidate)| value_call_signature_key(candidate));
            Ok(Some(candidates.remove(0).1))
        }
        _ => Err(Error::new(format!(
            "ambiguous overload for '{}' with provided argument(s)",
            name
        ))),
    }
}

/// Type-checks helper logic for infer_monoid_pow_call.
fn infer_monoid_pow_call(
    name: &str,
    args: &[Expr],
    env: &Env<'_>,
    lift_param: Option<&str>,
) -> Result<Option<ValueExpr>, Error> {
    if name != "pow" || args.len() != 2 {
        return Ok(None);
    }

    let Ok(exponent) = infer_int_expr(&args[0], env, lift_param) else {
        return Ok(None);
    };
    let base = infer_value_expr(&args[1], env, lift_param)?;
    let ty = base.ty();
    if !is_monoid_pow_type(&ty) {
        return Ok(None);
    }
    Ok(Some(ValueExpr::MonoidPow {
        exponent: Box::new(exponent),
        base: Box::new(base),
        ty,
    }))
}

/// Type-checks helper logic for is_monoid_pow_type.
fn is_monoid_pow_type(ty: &Type) -> bool {
    is_value_type(ty) && has_category(ty, AlgebraicCategory::Mon)
}

/// Type-checks helper logic for infer_value_call.
fn infer_value_call(
    name: &str,
    args: &[Expr],
    env: &Env<'_>,
    lift_param: Option<&str>,
    expected_output: Option<&Type>,
) -> Result<ValueExpr, Error> {
    let overloads = env
        .function_overloads(name)
        .ok_or_else(|| Error::new(format!("unknown function '{}'", name)))?;
    let mut candidates = Vec::new();
    let mut first_arg_error = None;
    for overload in overloads {
        let Ok((inputs, output)) = call_inputs_and_output(&overload.ty) else {
            continue;
        };
        if args.len() != inputs.len() {
            continue;
        }
        let mut substitutions = GenericSubstitution::default();
        if let Some(expected) = expected_output {
            if !unify_types(&output, expected, &mut substitutions)
                && !types_compatible_for_expected(&output, expected)
            {
                continue;
            }
        }
        let mut typed_args = Vec::new();
        let mut cost = 0usize;
        let mut ok = true;
        for (arg, input_ty) in args.iter().zip(inputs.iter()) {
            match infer_value_expr_for_type(arg, input_ty, env, lift_param) {
                Ok(typed_arg) => {
                    if !unify_types(input_ty, &typed_arg.ty(), &mut substitutions)
                        && !types_compatible_for_expected(&typed_arg.ty(), input_ty)
                    {
                        ok = false;
                        break;
                    }
                    cost += call_arg_cost(&typed_arg.ty(), input_ty, &typed_arg);
                    typed_args.push(typed_arg);
                }
                Err(err) => {
                    if first_arg_error.is_none() {
                        first_arg_error = Some(err);
                    }
                    ok = false;
                    break;
                }
            }
        }
        let output = substitute_type(&output, &substitutions);
        if ok && is_value_type(&output) {
            candidates.push((cost, output, typed_args));
        }
    }

    candidates.sort_by_key(|(cost, _, args)| (*cost, value_signature_key(args)));
    let Some((best_cost, best_output, best_args)) = candidates.first().cloned() else {
        if overloads.len() == 1 {
            if let Some(err) = first_arg_error {
                return Err(err);
            }
        }
        return Err(Error::new(format!(
            "no overload of '{}' matches provided argument(s)",
            name
        )));
    };
    let tied = candidates
        .iter()
        .skip(1)
        .filter(|(cost, _, _)| *cost == best_cost)
        .collect::<Vec<_>>();
    if !tied.is_empty()
        && !candidates
            .iter()
            .take(tied.len() + 1)
            .all(|(_, _, args)| args.iter().any(|arg| matches!(arg, ValueExpr::Neutral { .. })))
    {
        return Err(Error::new(format!(
            "ambiguous overload for '{}' with provided argument(s)",
            name
        )));
    }
    Ok(ValueExpr::Call {
        func: name.to_string(),
        args: best_args,
        ty: best_output,
    })
}

/// Type-checks helper logic for call_arg_cost.
fn call_arg_cost(actual: &Type, expected: &Type, arg: &ValueExpr) -> usize {
    usize::from(!types_match(actual, expected))
        + usize::from(matches!(arg, ValueExpr::Neutral { .. }))
        + usize::from(matches!(arg, ValueExpr::NumericWidenCast { .. }))
}

/// Checks whether all tied candidates are caused by neutral literal casts.
fn tied_candidates_are_neutral_casts<'a>(candidates: impl Iterator<Item = &'a ValueExpr>) -> bool {
    let candidates = candidates.collect::<Vec<_>>();
    !candidates.is_empty()
        && candidates
            .iter()
            .all(|candidate| value_call_args(candidate).is_some_and(|args| args.iter().any(|arg| matches!(arg, ValueExpr::Neutral { .. }))))
}

/// Returns a stable key for choosing between equivalent neutral-literal overloads.
fn value_call_signature_key(candidate: &ValueExpr) -> String {
    value_call_args(candidate)
        .map(value_signature_key)
        .unwrap_or_default()
}

/// Returns the argument list for a value call candidate.
fn value_call_args(candidate: &ValueExpr) -> Option<&[ValueExpr]> {
    match candidate {
        ValueExpr::Call { args, .. } => Some(args),
        _ => None,
    }
}

/// Formats an argument-type signature for deterministic neutral overload selection.
fn value_signature_key(args: &[ValueExpr]) -> String {
    args.iter()
        .map(|arg| format_type(&arg.ty()))
        .collect::<Vec<_>>()
        .join(" x ")
}

/// Type-checks helper logic for call_inputs_and_output.
fn call_inputs_and_output(ty: &Type) -> Result<(Vec<Type>, Type), Error> {
    let (inputs, output) = flatten_func_type(ty);
    if inputs.is_empty() {
        return Err(Error::new(format!(
            "expected function type, got {}",
            format_type(ty)
        )));
    }
    let mut flattened = Vec::new();
    for input in inputs {
        match input {
            Type::Product(parts) => flattened.extend(parts.iter().cloned()),
            other => flattened.push(other.clone()),
        }
    }
    Ok((flattened, (*output).clone()))
}

/// Type-checks helper logic for types_compatible_for_expected.
fn types_compatible_for_expected(actual: &Type, expected: &Type) -> bool {
    types_match(actual, expected)
        || matches!(
            (actual, expected),
            (Type::Vec2, Type::Complex)
                | (Type::Complex, Type::Vec2)
                | (Type::Vec4, Type::Quat)
                | (Type::Quat, Type::Vec4)
        )
        || matches!(
            (actual, expected),
            (Type::Custom { name: actual, .. }, Type::Custom { name: expected, .. })
                if actual == expected
        )
        || matches!(
            (actual, expected),
            (Type::Product(actual), Type::Product(expected))
                if actual.len() == expected.len()
                    && actual
                        .iter()
                        .zip(expected.iter())
                        .all(|(actual, expected)| types_compatible_for_expected(actual, expected))
        )
}

/// Type-checks helper logic for is_value_type.
fn is_value_type(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Unit
            | Type::Bool
            | Type::Float
            | Type::Int
            | Type::Complex
            | Type::Quat
            | Type::Isom2
            | Type::Isom3
            | Type::Custom { .. }
            | Type::Vec2
            | Type::Vec3
            | Type::Vec4
            | Type::Generic(_)
            | Type::VecGeneric(_)
            | Type::MatGeneric(_, _)
            | Type::Mat(_, _)
            | Type::Array(_)
    )
}

/// Type-checks helper logic for infer_int_expr.
fn infer_int_expr(
    expr: &Expr,
    env: &Env<'_>,
    lift_param: Option<&str>,
) -> Result<ValueExpr, Error> {
    if let Expr::Number(value) = expr {
        let rounded = value.round();
        if (value - rounded).abs() < f64::EPSILON {
            return Ok(ValueExpr::Int(rounded as i64));
        }
    }

    let value = infer_value_expr(expr, env, lift_param)?;
    ensure_type(&value.ty(), &Type::Int, "integer expression")?;
    Ok(value)
}

/// Type-checks helper logic for infer_array_literal.
fn infer_array_literal(
    items: &[Expr],
    env: &Env<'_>,
    lift_param: Option<&str>,
    expected_element_ty: Option<&Type>,
) -> Result<ValueExpr, Error> {
    if items.is_empty() {
        return Err(Error::new("Array(...) requires at least one element"));
    }

    let element_ty = if let Some(expected) = expected_element_ty {
        expected.clone()
    } else {
        infer_value_expr(&items[0], env, lift_param)?.ty()
    };
    if !is_array_element_type(&element_ty) {
        return Err(Error::new(format!(
            "arrays of {} are not supported",
            format_type(&element_ty)
        )));
    }

    let mut elements = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let value = infer_value_expr_for_type(item, &element_ty, env, lift_param)?;
        ensure_type(
            &value.ty(),
            &element_ty,
            &format!("array element {}", index + 1),
        )?;
        elements.push(value);
    }

    Ok(ValueExpr::Array {
        element_ty,
        elements,
    })
}

/// Type-checks helper logic for infer_index_expr.
fn infer_index_expr(
    array: &Expr,
    index: &Expr,
    env: &Env<'_>,
    lift_param: Option<&str>,
) -> Result<ValueExpr, Error> {
    let array = infer_value_expr(array, env, lift_param)?;
    let element_ty = match array.ty() {
        Type::Array(element_ty) => *element_ty,
        other => {
            return Err(Error::new(format!(
                "indexing expected Array(T), got {}",
                format_type(&other)
            )))
        }
    };
    let index = infer_int_expr(index, env, lift_param)?;
    Ok(ValueExpr::Index {
        array: Box::new(array),
        index: Box::new(index),
        ty: element_ty,
    })
}

/// Type-checks helper logic for infer_array_builtin.
fn infer_array_builtin(
    expr: &Expr,
    env: &Env<'_>,
    lift_param: Option<&str>,
) -> Result<Option<ValueExpr>, Error> {
    let (name, args) = match flatten_call(expr) {
        Ok(parts) => parts,
        Err(_) => return Ok(None),
    };

    match name.as_str() {
        "size" => {
            if args.len() != 1 {
                return Err(Error::new("size expects one array argument"));
            }
            let array = infer_value_expr(args[0], env, lift_param)?;
            if !matches!(array.ty(), Type::Array(_)) {
                return Err(Error::new(format!(
                    "size expected Array(T), got {}",
                    format_type(&array.ty())
                )));
            }
            let len = array
                .array_len()
                .ok_or_else(|| Error::new("size requires a statically known array length"))?;
            Ok(Some(ValueExpr::Int(len as i64)))
        }
        "concat" => {
            if args.len() != 2 {
                return Err(Error::new("concat expects two array arguments"));
            }
            let left = infer_value_expr(args[0], env, lift_param)?;
            let right = infer_value_expr(args[1], env, lift_param)?;
            let left_element_ty = match left.ty() {
                Type::Array(element_ty) => *element_ty,
                other => {
                    return Err(Error::new(format!(
                        "concat left argument expected Array(T), got {}",
                        format_type(&other)
                    )))
                }
            };
            let right_element_ty = match right.ty() {
                Type::Array(element_ty) => *element_ty,
                other => {
                    return Err(Error::new(format!(
                        "concat right argument expected Array(T), got {}",
                        format_type(&other)
                    )))
                }
            };
            ensure_type(&right_element_ty, &left_element_ty, "concat element type")?;
            if left.array_len().is_none() || right.array_len().is_none() {
                return Err(Error::new(
                    "concat requires statically known lengths for both arrays",
                ));
            }
            Ok(Some(ValueExpr::Concat {
                element_ty: left_element_ty,
                left: Box::new(left),
                right: Box::new(right),
            }))
        }
        _ => Ok(None),
    }
}

/// Type-checks helper logic for infer_tuple_value_expr.
fn infer_tuple_value_expr(
    items: &[Expr],
    env: &Env<'_>,
    lift_param: Option<&str>,
) -> Result<ValueExpr, Error> {
    let values = items
        .iter()
        .map(|item| infer_value_expr(item, env, lift_param))
        .collect::<Result<Vec<_>, _>>()?;

    if values.iter().all(|value| value.ty() == Type::Float) {
        return infer_vector_tuple(values);
    }

    infer_matrix_tuple(values)
}

/// Type-checks helper logic for infer_vector_literal_for_type.
fn infer_vector_literal_for_type(
    items: &[Expr],
    expected_ty: &Type,
    env: &Env<'_>,
    lift_param: Option<&str>,
) -> Result<ValueExpr, Error> {
    if expected_ty == &Type::Vec4 && items.len() == 2 {
        let xyz = infer_value_expr_for_type(&items[0], &Type::Vec3, env, lift_param)?;
        let w = infer_value_expr_for_type(&items[1], &Type::Float, env, lift_param)?;
        return Ok(ValueExpr::Call {
            func: "vec4".to_string(),
            args: vec![xyz, w],
            ty: Type::Vec4,
        });
    }

    let Some(expected_len) = vector_dimension(expected_ty) else {
        unreachable!();
    };
    if items.len() != expected_len {
        return Err(Error::new(format!(
            "{} literal expects {} element(s), got {}",
            format_type(expected_ty),
            expected_len,
            items.len()
        )));
    }
    let values = items
        .iter()
        .map(|item| infer_value_expr_for_type(item, &Type::Float, env, lift_param))
        .collect::<Result<Vec<_>, _>>()?;
    infer_vector_tuple(values)
}

/// Type-checks helper logic for infer_matrix_literal_for_type.
fn infer_matrix_literal_for_type(
    items: &[Expr],
    expected_ty: &Type,
    env: &Env<'_>,
    lift_param: Option<&str>,
) -> Result<ValueExpr, Error> {
    let Type::Mat(rows, columns) = expected_ty else {
        unreachable!();
    };
    if items.len() != *rows {
        return Err(Error::new(format!(
            "{} literal expects {} row(s), got {}",
            format_type(expected_ty),
            rows,
            items.len()
        )));
    }
    let row_ty = vector_type(*columns);
    let mut row_values = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let row = infer_value_expr_for_type(item, &row_ty, env, lift_param)?;
        ensure_type(&row.ty(), &row_ty, &format!("matrix row {}", index + 1))?;
        row_values.push(row);
    }
    Ok(ValueExpr::Matrix {
        columns: *columns,
        rows: row_values,
    })
}

/// Type-checks helper logic for infer_vector_tuple.
fn infer_vector_tuple(values: Vec<ValueExpr>) -> Result<ValueExpr, Error> {
    match values.as_slice() {
        [x, y] => Ok(ValueExpr::Vec2(Box::new(x.clone()), Box::new(y.clone()))),
        [x, y, z] => Ok(ValueExpr::Vec3(
            Box::new(x.clone()),
            Box::new(y.clone()),
            Box::new(z.clone()),
        )),
        [x, y, z, w] => Ok(ValueExpr::Vec4(
            Box::new(x.clone()),
            Box::new(y.clone()),
            Box::new(z.clone()),
            Box::new(w.clone()),
        )),
        _ => Err(Error::new(
            "only vec2, vec3, vec4, and 2x2 through 4x4 matrix tuples are supported in value expressions",
        )),
    }
}

/// Type-checks helper logic for infer_matrix_tuple.
fn infer_matrix_tuple(rows: Vec<ValueExpr>) -> Result<ValueExpr, Error> {
    let Some(columns) = rows.first().and_then(|row| vector_dimension(&row.ty())) else {
        return Err(Error::new(
            "matrix tuple rows must be vec2, vec3, or vec4 values",
        ));
    };
    if !(2..=4).contains(&rows.len()) || !(2..=4).contains(&columns) {
        return Err(Error::new(
            "only 2x2 through 4x4 matrix tuples are supported in value expressions",
        ));
    }
    for (index, row) in rows.iter().enumerate() {
        ensure_type(
            &row.ty(),
            &vector_type(columns),
            &format!("matrix row {}", index + 1),
        )?;
    }
    Ok(ValueExpr::Matrix { columns, rows })
}

/// Type-checks helper logic for vector_dimension.
fn vector_dimension(ty: &Type) -> Option<usize> {
    match ty {
        Type::Vec2 => Some(2),
        Type::Vec3 => Some(3),
        Type::Vec4 => Some(4),
        _ => None,
    }
}

/// Type-checks helper logic for vector_type.
fn vector_type(dimension: usize) -> Type {
    match dimension {
        2 => Type::Vec2,
        3 => Type::Vec3,
        4 => Type::Vec4,
        _ => unreachable!(),
    }
}

/// Type-checks helper logic for validate_user_type.
fn validate_user_type(ty: &Type) -> Result<(), Error> {
    match ty {
        Type::Array(element_ty) => {
            if !is_array_element_type(element_ty) {
                return Err(Error::new(format!(
                    "arrays of {} are not supported",
                    format_type(element_ty)
                )));
            }
            Ok(())
        }
        Type::Func(input, output) => {
            validate_user_type(input)?;
            validate_user_type(output)
        }
        Type::Product(parts) => {
            for part in parts {
                validate_user_type(part)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Type-checks helper logic for is_array_element_type.
fn is_array_element_type(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Bool
            | Type::Float
            | Type::Int
            | Type::Complex
            | Type::Quat
            | Type::Isom2
            | Type::Isom3
            | Type::Custom { .. }
            | Type::Vec2
            | Type::Vec3
            | Type::Vec4
            | Type::Mat(_, _)
    )
}

/// Type-checks helper logic for infer_differential_builtin.
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
        "dfdx" => Some(infer_partial_builtin(&args, env, lift_param, 0)?),
        "dfdy" => Some(infer_partial_builtin(&args, env, lift_param, 1)?),
        "dfdz" => Some(infer_partial_builtin(&args, env, lift_param, 2)?),
        "dfdw" => Some(infer_partial_builtin(&args, env, lift_param, 3)?),
        "gradient" | "grad" => Some(infer_gradient_builtin(&args, env, lift_param)?),
        "divergence" => Some(infer_divergence_builtin(&args, env, lift_param)?),
        _ => None,
    };

    Ok(result)
}

/// Type-checks helper logic for infer_derivative_builtin.
fn infer_derivative_builtin(
    args: &[&Expr],
    env: &Env<'_>,
    lift_param: Option<&str>,
) -> Result<ValueExpr, Error> {
    if args.len() != 2 && !(args.len() == 1 && lift_param.is_some()) {
        return Err(Error::new(
            "derivative expects a unary function and an evaluation point",
        ));
    }
    let (func, at) =
        infer_differential_func_and_point(args[0], args.get(1).copied(), env, lift_param)?;
    let ty = derivative_output_type(&func.input, &func.output)
        .ok_or_else(|| Error::new("derivative expects Hom(Rn, Rm) for n,m in 1..4"))?;
    Ok(ValueExpr::Derivative {
        epsilon: Box::new(ValueExpr::Float(env.derivative_epsilon)),
        func,
        at: Box::new(at),
        ty,
    })
}

/// Type-checks helper logic for infer_partial_builtin.
fn infer_partial_builtin(
    args: &[&Expr],
    env: &Env<'_>,
    lift_param: Option<&str>,
    axis: usize,
) -> Result<ValueExpr, Error> {
    if args.len() != 2 && !(args.len() == 1 && lift_param.is_some()) {
        return Err(Error::new(
            "partial derivative expects a field and an evaluation point",
        ));
    }
    let (func, at) =
        infer_differential_func_and_point(args[0], args.get(1).copied(), env, lift_param)?;
    let input_dim = derivative_dimension(&func.input)
        .ok_or_else(|| Error::new("partial derivative expects Hom(Rn, Rm) for n,m in 1..4"))?;
    if axis >= input_dim {
        return Err(Error::new(format!(
            "partial derivative axis {} is not valid for {}",
            axis + 1,
            format_type(&func.input)
        )));
    }
    Ok(ValueExpr::Partial {
        axis,
        epsilon: Box::new(ValueExpr::Float(env.derivative_epsilon)),
        ty: func.output.clone(),
        func,
        at: Box::new(at),
    })
}

/// Type-checks helper logic for infer_differential_func_and_point.
fn infer_differential_func_and_point(
    func_arg: &Expr,
    at_arg: Option<&Expr>,
    env: &Env<'_>,
    lift_param: Option<&str>,
) -> Result<(FunctionExpr, ValueExpr), Error> {
    let candidates = infer_function_expr_candidates(func_arg, env)?
        .into_iter()
        .filter(|func| {
            derivative_dimension(&func.input).is_some()
                && derivative_dimension(&func.output).is_some()
        })
        .collect::<Vec<_>>();
    let Some(at_arg) = at_arg else {
        let Some(lift_param) = lift_param else {
            return Err(Error::new(
                "differential operator needs an evaluation point",
            ));
        };
        let matches = candidates
            .into_iter()
            .filter(|func| func.input == Type::Float)
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(Error::new(
                "differential operator could not infer a scalar lifted function",
            ));
        }
        let func = matches.into_iter().next().unwrap();
        return Ok((
            func,
            ValueExpr::Var {
                name: lift_param.to_string(),
                ty: Type::Float,
                array_len: None,
            },
        ));
    };
    let mut matches = Vec::new();
    for func in candidates {
        if let Ok(at) = infer_value_expr_for_type(at_arg, &func.input, env, None) {
            if ensure_type(&at.ty(), &func.input, "differential evaluation point").is_ok() {
                matches.push((func, at));
            }
        }
    }
    match matches.len() {
        1 => Ok(matches.pop().unwrap()),
        0 => Err(Error::new(
            "differential operator has no matching function overload",
        )),
        _ => Err(Error::new("ambiguous differential operator overload")),
    }
}

/// Type-checks helper logic for derivative_dimension.
fn derivative_dimension(ty: &Type) -> Option<usize> {
    match ty {
        Type::Float => Some(1),
        Type::Vec2 => Some(2),
        Type::Vec3 => Some(3),
        Type::Vec4 => Some(4),
        _ => None,
    }
}

/// Type-checks helper logic for derivative_output_type.
fn derivative_output_type(input: &Type, output: &Type) -> Option<Type> {
    let input_dim = derivative_dimension(input)?;
    let output_dim = derivative_dimension(output)?;
    if input_dim == 1 && output_dim == 1 {
        Some(Type::Float)
    } else if input_dim == 1 {
        Some(vector_type(output_dim))
    } else if output_dim == 1 {
        Some(vector_type(input_dim))
    } else {
        Some(Type::Mat(input_dim, output_dim))
    }
}

/// Type-checks helper logic for infer_gradient_builtin.
fn infer_gradient_builtin(
    args: &[&Expr],
    env: &Env<'_>,
    lift_param: Option<&str>,
) -> Result<ValueExpr, Error> {
    if args.len() != 2 && !(args.len() == 1 && lift_param.is_some()) {
        return Err(Error::new(
            "gradient expects a scalar field and an evaluation point",
        ));
    }
    let (func, at) =
        infer_differential_func_and_point(args[0], args.get(1).copied(), env, lift_param)?;
    if func.output != Type::Float {
        return Err(Error::new("gradient expects a scalar-valued field"));
    }
    let ty = derivative_output_type(&func.input, &func.output)
        .ok_or_else(|| Error::new("gradient expects Hom(Rn, R) for n in 1..4"))?;
    if func.input == Type::Float {
        Ok(ValueExpr::Derivative {
            epsilon: Box::new(ValueExpr::Float(env.derivative_epsilon)),
            func,
            at: Box::new(at),
            ty,
        })
    } else {
        Ok(ValueExpr::Gradient {
            epsilon: Box::new(ValueExpr::Float(env.derivative_epsilon)),
            func,
            at: Box::new(at),
            ty,
        })
    }
}

/// Type-checks helper logic for infer_divergence_builtin.
fn infer_divergence_builtin(
    args: &[&Expr],
    env: &Env<'_>,
    lift_param: Option<&str>,
) -> Result<ValueExpr, Error> {
    if args.len() != 2 && !(args.len() == 1 && lift_param.is_some()) {
        return Err(Error::new(
            "divergence expects a vector field and an evaluation point",
        ));
    }
    let (func, at) =
        infer_differential_func_and_point(args[0], args.get(1).copied(), env, lift_param)?;
    let input_dim = divergence_dimension(&func.input)
        .ok_or_else(|| Error::new("divergence expects Hom(Rn, Rn) for n in 2..4"))?;
    let output_dim = divergence_dimension(&func.output)
        .ok_or_else(|| Error::new("divergence expects Hom(Rn, Rn) for n in 2..4"))?;
    if input_dim != output_dim {
        return Err(Error::new(
            "divergence expects a same-dimensional vector field",
        ));
    }
    Ok(ValueExpr::Divergence {
        epsilon: Box::new(ValueExpr::Float(env.derivative_epsilon)),
        func,
        at: Box::new(at),
    })
}

/// Type-checks helper logic for divergence_dimension.
fn divergence_dimension(ty: &Type) -> Option<usize> {
    match ty {
        Type::Vec2 => Some(2),
        Type::Vec3 => Some(3),
        Type::Vec4 => Some(4),
        _ => None,
    }
}

/// Type-checks helper logic for infer_vec2_list_expr.
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

/// Type-checks helper logic for pack_single_vector_field_args.
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

/// Type-checks helper logic for infer_composed_value_expr.
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
        ValueExpr::Var {
            name: param_name.to_string(),
            ty: Type::Float,
            array_len: None,
        },
    ))
}

/// Type-checks helper logic for infer_function_product_value_expr.
fn infer_function_product_value_expr(
    left: &Expr,
    right: &Expr,
    env: &Env<'_>,
    lift_param: Option<&str>,
) -> Result<ValueExpr, Error> {
    let param_name = lift_param
        .ok_or_else(|| Error::new("function products are only supported inside function bodies"))?;
    let product = infer_function_expr(
        &Expr::Binary {
            op: BinOp::Product,
            left: Box::new(left.clone()),
            right: Box::new(right.clone()),
        },
        env,
    )?;
    let arg = ValueExpr::Var {
        name: param_name.to_string(),
        ty: product.input.clone(),
        array_len: None,
    };
    Ok(apply_function_expr(&product, arg))
}
