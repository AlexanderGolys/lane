/// Type-checks helper logic for infer_function_expr.
fn infer_function_expr(expr: &Expr, env: &Env<'_>) -> Result<FunctionExpr, Error> {
    let candidates = infer_function_expr_candidates(expr, env)?;
    if candidates.len() == 1 {
        return Ok(candidates.into_iter().next().unwrap());
    }
    Err(Error::new(format!(
        "ambiguous function expression {}",
        format_function_expr(expr)
    )))
}

/// Type-checks helper logic for infer_function_expr_for_type.
fn infer_function_expr_for_type(
    expr: &Expr,
    env: &Env<'_>,
    expected_input: &Type,
    expected_output: &Type,
) -> Result<FunctionExpr, Error> {
    if let Expr::Ident(name) = expr {
        if let Some(index) = parse_projection_name(name) {
            return infer_projection_function_for_type(index, expected_input, expected_output, env);
        }
        if let Some(dimension) = parse_diagonal_name(name) {
            return infer_diagonal_function_for_type(
                dimension,
                expected_input,
                expected_output,
                env,
            );
        }
    }
    if let Expr::Operator(op) = expr {
        return infer_operator_function_expr_for_type(*op, expected_input, expected_output);
    }
    if let Expr::Array(items) = expr {
        if let Some(function) =
            infer_same_domain_scalar_function_product(items, expected_input, expected_output, env)?
        {
            return Ok(function);
        }
    }
    if let Expr::Tuple(items) = expr {
        if let Some(function) =
            infer_same_domain_scalar_function_product(items, expected_input, expected_output, env)?
        {
            return Ok(function);
        }
    }
    if let Expr::Binary {
        op: BinOp::Product,
        left,
        right,
    } = expr
    {
        if expected_input == &Type::Vec2 && expected_output == &Type::Vec2 {
            let left = infer_function_expr_for_type(left, env, &Type::Float, &Type::Float)?;
            let right = infer_function_expr_for_type(right, env, &Type::Float, &Type::Float)?;
            return Ok(FunctionExpr {
                input: Type::Vec2,
                output: Type::Vec2,
                kind: FunctionExprKind::ProductTensor(Box::new(left), Box::new(right)),
            });
        }
    }
    let candidates = infer_function_expr_candidates(expr, env)?;
    let matches = candidates
        .into_iter()
        .filter(|func| {
            types_match(&func.input, expected_input) && types_match(&func.output, expected_output)
        })
        .collect::<Vec<_>>();
    if matches.len() == 1 {
        return Ok(matches.into_iter().next().unwrap());
    }
    if matches.is_empty() {
        return Err(Error::new(format!(
            "function expression {} does not match Hom({}, {})",
            format_function_expr(expr),
            format_type(expected_input),
            format_type(expected_output)
        )));
    }
    Err(Error::new(format!(
        "ambiguous function expression {}",
        format_function_expr(expr)
    )))
}

fn infer_same_domain_scalar_function_product(
    items: &[Expr],
    expected_input: &Type,
    expected_output: &Type,
    env: &Env<'_>,
) -> Result<Option<FunctionExpr>, Error> {
    let Some(count) = vector_dimension(expected_output) else {
        return Ok(None);
    };
    if count != items.len() {
        return Ok(None);
    }
    let funcs = items
        .iter()
        .map(|item| infer_function_expr_for_type(item, env, expected_input, &Type::Float))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Some(FunctionExpr {
        input: expected_input.clone(),
        output: expected_output.clone(),
        kind: FunctionExprKind::ProductSameDomain(funcs),
    }))
}

/// Type-checks helper logic for infer_projection_function_for_type.
fn infer_projection_function_for_type(
    index: usize,
    input: &Type,
    output: &Type,
    env: &Env<'_>,
) -> Result<FunctionExpr, Error> {
    let Some((component, field)) = projection_component(input, index, env) else {
        return Err(Error::new(format!(
            "projection p{{{index}}} is not valid for {}",
            format_type(input)
        )));
    };
    if !types_compatible_for_expected(&component, output) {
        return Err(Error::new(format!(
            "projection p{{{index}}} expected {}, got {}",
            format_type(output),
            format_type(&component)
        )));
    }
    Ok(FunctionExpr {
        input: input.clone(),
        output: output.clone(),
        kind: FunctionExprKind::Projection { index, field },
    })
}

/// Type-checks helper logic for infer_projection_function_for_input.
fn infer_projection_function_for_input(
    index: usize,
    input: &Type,
    env: &Env<'_>,
) -> Result<FunctionExpr, Error> {
    let Some((output, field)) = projection_component(input, index, env) else {
        return Err(Error::new(format!(
            "projection p{{{index}}} is not valid for {}",
            format_type(input)
        )));
    };
    Ok(FunctionExpr {
        input: input.clone(),
        output,
        kind: FunctionExprKind::Projection { index, field },
    })
}

/// Type-checks helper logic for projection_component.
fn projection_component(ty: &Type, index: usize, env: &Env<'_>) -> Option<(Type, Option<String>)> {
    match ty {
        Type::Product(parts) => parts.get(index).cloned().map(|ty| (ty, None)),
        Type::Vec2 | Type::Complex => vector_projection_component(2, index),
        Type::Vec3 => vector_projection_component(3, index),
        Type::Vec4 | Type::Quat => vector_projection_component(4, index),
        Type::Custom { name, .. } => {
            let product_type = env.product_type(name)?;
            product_field_access(product_type, &format!("x{index}"))
                .map(|(ty, field)| (ty, Some(field)))
        }
        _ => None,
    }
}

/// Type-checks helper logic for vector_projection_component.
fn vector_projection_component(dimension: usize, index: usize) -> Option<(Type, Option<String>)> {
    if index >= dimension {
        return None;
    }
    vector_field_access(dimension, &format!("x{index}")).map(|field| (Type::Float, Some(field)))
}

/// Type-checks helper logic for infer_diagonal_function_for_type.
fn infer_diagonal_function_for_type(
    dimension: usize,
    input: &Type,
    output: &Type,
    env: &Env<'_>,
) -> Result<FunctionExpr, Error> {
    if !diagonal_output_matches(dimension, input, output, env) {
        return Err(Error::new(format!(
            "diag{{{dimension}}} does not match Hom({}, {})",
            format_type(input),
            format_type(output)
        )));
    }
    Ok(FunctionExpr {
        input: input.clone(),
        output: output.clone(),
        kind: FunctionExprKind::Diagonal { dimension },
    })
}

/// Type-checks helper logic for infer_diagonal_function_for_input.
fn infer_diagonal_function_for_input(
    dimension: usize,
    input: &Type,
) -> Result<FunctionExpr, Error> {
    let output = diagonal_output_type(dimension, input);
    Ok(FunctionExpr {
        input: input.clone(),
        output,
        kind: FunctionExprKind::Diagonal { dimension },
    })
}

/// Type-checks helper logic for diagonal_output_type.
fn diagonal_output_type(dimension: usize, input: &Type) -> Type {
    if input == &Type::Float {
        match dimension {
            2 => return Type::Vec2,
            3 => return Type::Vec3,
            4 => return Type::Vec4,
            _ => {}
        }
    }
    Type::Product(std::iter::repeat_n(input.clone(), dimension).collect())
}

/// Type-checks helper logic for diagonal_output_matches.
fn diagonal_output_matches(dimension: usize, input: &Type, output: &Type, env: &Env<'_>) -> bool {
    if types_compatible_for_expected(&diagonal_output_type(dimension, input), output) {
        return true;
    }
    match output {
        Type::Product(parts) if parts.len() == dimension => parts
            .iter()
            .all(|part| types_compatible_for_expected(input, part)),
        Type::Vec2 | Type::Vec3 | Type::Vec4 => {
            input == &Type::Float && vector_dimension(output) == Some(dimension)
        }
        Type::Custom { name, .. } => env.product_type(name).is_some_and(|product_type| {
            product_type.components.len() == dimension
                && product_type
                    .components
                    .iter()
                    .all(|part| types_compatible_for_expected(input, part))
        }),
        _ => false,
    }
}

/// Type-checks helper logic for infer_function_expr_candidates.
fn infer_function_expr_candidates(expr: &Expr, env: &Env<'_>) -> Result<Vec<FunctionExpr>, Error> {
    match expr {
        Expr::Operator(op) => infer_operator_function_expr_candidates(*op),
        Expr::Ident(name) => {
            if let Some(overloads) = env.function_overloads(name) {
                let mut candidates = Vec::new();
                for overload in overloads {
                    if let Type::Func(input, output) = &overload.ty {
                        candidates.push(FunctionExpr {
                            input: (**input).clone(),
                            output: (**output).clone(),
                            kind: FunctionExprKind::Named(name.clone()),
                        });
                    }
                }
                if !candidates.is_empty() {
                    return Ok(candidates);
                }
            }
            let ty = env
                .get(name)
                .cloned()
                .ok_or_else(|| Error::new(format!("unknown identifier '{}'", name)))?;
            match ty {
                Type::Func(input, output) => Ok(vec![FunctionExpr {
                    input: (*input).clone(),
                    output: (*output).clone(),
                    kind: FunctionExprKind::Named(name.clone()),
                }]),
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
                | Type::Mat(_, _)
                | Type::Generic(_)
                | Type::VecGeneric(_)
                | Type::MatGeneric(_, _)
                | Type::Power(_, _)
                | Type::Array(_) => {
                    Err(Error::new(format!("'{}' is a value, not a function", name)))
                }
                Type::Object | Type::Object2D | Type::Product(_) => Err(Error::new(format!(
                    "object '{}' is not a function expression",
                    name
                ))),
            }
        }
        Expr::FieldAccess { object, field } => {
            let Expr::Ident(object_name) = &**object else {
                return Err(Error::new("object getter must target a named object"));
            };
            let getter = match field.as_str() {
                "sdf" => ObjectGetter::Sdf,
                "grad" => ObjectGetter::Grad,
                _ => {
                    return Err(Error::new(format!(
                        "object '{}' has no getter '{}'",
                        object_name, field
                    )))
                }
            };
            let ty = env
                .get(object_name)
                .ok_or_else(|| Error::new(format!("unknown identifier '{}'", object_name)))?;
            if !matches!(ty, Type::Object | Type::Object2D) {
                return Err(Error::new(format!("'{}' is not an object", object_name)));
            }
            let dimension = env
                .object_dimension(object_name)
                .or_else(|| object_type_dimension(ty))
                .unwrap_or(env.ambient_dimension);
            let input = ambient_vector_type(dimension);
            let output = match getter {
                ObjectGetter::Sdf => Type::Float,
                ObjectGetter::Grad => ambient_vector_type(dimension),
            };
            Ok(vec![FunctionExpr {
                input,
                output,
                kind: FunctionExprKind::ObjectGetter {
                    object: object_name.clone(),
                    getter,
                    captures: Vec::new(),
                },
            }])
        }
        Expr::Binary {
            op: BinOp::Compose,
            left,
            right,
        } => {
            let inners = infer_function_expr_candidates(right, env)?;
            if let Expr::Ident(name) = &**left {
                if let Some(index) = parse_projection_name(name) {
                    let mut candidates = Vec::new();
                    for inner in &inners {
                        if let Ok(outer) =
                            infer_projection_function_for_input(index, &inner.output, env)
                        {
                            candidates.push(FunctionExpr {
                                input: inner.input.clone(),
                                output: outer.output.clone(),
                                kind: FunctionExprKind::Compose(
                                    Box::new(outer),
                                    Box::new(inner.clone()),
                                ),
                            });
                        }
                    }
                    if !candidates.is_empty() {
                        return Ok(candidates);
                    }
                }
                if let Some(dimension) = parse_diagonal_name(name) {
                    let candidates = inners
                        .iter()
                        .map(|inner| {
                            let outer =
                                infer_diagonal_function_for_input(dimension, &inner.output)?;
                            Ok(FunctionExpr {
                                input: inner.input.clone(),
                                output: outer.output.clone(),
                                kind: FunctionExprKind::Compose(
                                    Box::new(outer),
                                    Box::new(inner.clone()),
                                ),
                            })
                        })
                        .collect::<Result<Vec<_>, Error>>()?;
                    if !candidates.is_empty() {
                        return Ok(candidates);
                    }
                }
            }
            let outers = infer_function_expr_candidates(left, env)?;
            let mut candidates = Vec::new();
            for outer in &outers {
                for inner in &inners {
                    if types_match(&inner.output, &outer.input) {
                        candidates.push(FunctionExpr {
                            input: inner.input.clone(),
                            output: outer.output.clone(),
                            kind: FunctionExprKind::Compose(
                                Box::new(outer.clone()),
                                Box::new(inner.clone()),
                            ),
                        });
                    }
                }
            }
            if candidates.is_empty() {
                return Err(Error::new(format!(
                    "cannot compose {} @ {}",
                    format_function_expr(left),
                    format_function_expr(right)
                )));
            }
            Ok(candidates)
        }
        Expr::Tuple(items) => infer_same_domain_function_product_candidates(items, env),
        Expr::Binary {
            op: BinOp::Product,
            left,
            right,
        } => infer_tensor_function_product_candidates(left, right, env),
        Expr::Call { callee, args } => {
            let Expr::Ident(name) = &**callee else {
                return Err(Error::new("unsupported function call expression"));
            };
            infer_pointwise_call_function_candidates(name, args, env)
        }
        Expr::Unary { op, expr } => infer_pointwise_unary_function_candidates(*op, expr, env),
        Expr::Conditional {
            condition,
            then_branch,
            else_branch,
        } => infer_conditional_function_candidates(
            condition,
            then_branch,
            else_branch.as_deref(),
            env,
        ),
        Expr::Array(items) => infer_same_domain_vector_function_candidates(items, env),
        Expr::Binary { op, left, right } => {
            infer_pointwise_binary_function_candidates(*op, left, right, env)
        }
        _ => Err(Error::new(
            "function composition currently only supports named unary functions",
        )),
    }
}

/// Type-checks helper logic for infer_operator_function_expr_for_type.
fn infer_operator_function_expr_for_type(
    op: BinOp,
    expected_input: &Type,
    expected_output: &Type,
) -> Result<FunctionExpr, Error> {
    let Some([left, right]) = operator_input_types(expected_input) else {
        return Err(Error::new(format!(
            "operator '&{}' needs a binary domain",
            op.symbol()
        )));
    };
    let output = infer_binary_type(op, &left, &right)?;
    ensure_type(
        &output,
        expected_output,
        &format!("operator '&{}'", op.symbol()),
    )?;
    Ok(FunctionExpr {
        input: expected_input.clone(),
        output: expected_output.clone(),
        kind: FunctionExprKind::Operator(op),
    })
}

/// Type-checks helper logic for infer_operator_function_expr_candidates.
fn infer_operator_function_expr_candidates(op: BinOp) -> Result<Vec<FunctionExpr>, Error> {
    let mut candidates: Vec<FunctionExpr> = Vec::new();
    let types = operator_candidate_types();
    for left in &types {
        for right in &types {
            let Ok(output) = infer_binary_type(op, left, right) else {
                continue;
            };
            let input = operator_candidate_input_type(left, right);
            let candidate = FunctionExpr {
                input,
                output,
                kind: FunctionExprKind::Operator(op),
            };
            if !candidates.iter().any(|existing| {
                existing.input == candidate.input && existing.output == candidate.output
            }) {
                candidates.push(candidate);
            }
        }
    }
    Ok(candidates)
}

/// Type-checks helper logic for operator_candidate_input_type.
fn operator_candidate_input_type(left: &Type, right: &Type) -> Type {
    Type::Product(vec![left.clone(), right.clone()])
}

/// Type-checks helper logic for operator_input_types.
fn operator_input_types(input: &Type) -> Option<[Type; 2]> {
    match input {
        Type::Vec2 => Some([Type::Float, Type::Float]),
        Type::Product(parts) if parts.len() == 2 => Some([parts[0].clone(), parts[1].clone()]),
        _ => None,
    }
}

/// Type-checks helper logic for operator_candidate_types.
fn operator_candidate_types() -> Vec<Type> {
    let mut types = float_gen_types();
    types.extend([
        Type::Int,
        Type::Bool,
        Type::Complex,
        Type::Quat,
        Type::Isom2,
        Type::Isom3,
    ]);
    types.extend(matrix_types());
    types
}

/// Type-checks helper logic for infer_pointwise_binary_function_candidates.
fn infer_pointwise_binary_function_candidates(
    op: BinOp,
    left: &Expr,
    right: &Expr,
    env: &Env<'_>,
) -> Result<Vec<FunctionExpr>, Error> {
    let left_candidates = infer_pointwise_binary_arg_candidates(left, env);
    let right_candidates = infer_pointwise_binary_arg_candidates(right, env);
    let mut candidates = Vec::new();
    for left in &left_candidates {
        for right in &right_candidates {
            let input = match (&left.domain, &right.domain) {
                (Some(left_input), Some(right_input))
                    if types_equivalent(left_input, right_input) =>
                {
                    normalize_scalar_product_type(left_input)
                }
                (Some(input), None) | (None, Some(input)) => normalize_scalar_product_type(input),
                (None, None) => continue,
                _ => continue,
            };
            if !left.lifted && !right.lifted {
                continue;
            }
            if let Ok(output) = infer_binary_type(op, &left.output, &right.output) {
                candidates.push(FunctionExpr {
                    input,
                    output,
                    kind: FunctionExprKind::PointwiseBinary {
                        op,
                        left: left.arg.clone(),
                        right: right.arg.clone(),
                    },
                });
            }
        }
    }
    if candidates.is_empty() {
        return Err(Error::new(
            "no pointwise function arithmetic overload matches provided operands",
        ));
    }
    Ok(candidates)
}

/// Type-checks helper logic for infer_pointwise_unary_function_candidates.
fn infer_pointwise_unary_function_candidates(
    op: UnaryOp,
    expr: &Expr,
    env: &Env<'_>,
) -> Result<Vec<FunctionExpr>, Error> {
    let args = infer_pointwise_binary_arg_candidates(expr, env);
    let mut candidates = Vec::new();
    for arg in &args {
        if !arg.lifted {
            continue;
        }
        let Some(input) = arg.domain.as_ref() else {
            continue;
        };
        if let Ok(output) = infer_unary_type(op, &arg.output) {
            candidates.push(FunctionExpr {
                input: normalize_scalar_product_type(input),
                output,
                kind: FunctionExprKind::PointwiseUnary {
                    op,
                    arg: arg.arg.clone(),
                },
            });
        }
    }
    if candidates.is_empty() {
        return Err(Error::new(
            "no pointwise function unary overload matches provided operand",
        ));
    }
    Ok(candidates)
}

/// Type-checks helper logic for infer_conditional_function_candidates.
fn infer_conditional_function_candidates(
    condition: &Expr,
    then_branch: &Expr,
    else_branch: Option<&Expr>,
    env: &Env<'_>,
) -> Result<Vec<FunctionExpr>, Error> {
    let condition_candidates =
        infer_pointwise_arg_candidates_for_expected(condition, &Type::Bool, env);
    let then_candidates = infer_pointwise_binary_arg_candidates(then_branch, env);
    let mut candidates = Vec::new();

    for condition in &condition_candidates {
        for then_branch in &then_candidates {
            let else_candidates = match else_branch {
                Some(else_branch) => infer_pointwise_binary_arg_candidates(else_branch, env),
                None => {
                    let Some(zero) = zero_value_for_type(&then_branch.output) else {
                        continue;
                    };
                    vec![PointwiseBinaryArgCandidate {
                        arg: PointwiseCallArg::Value(Box::new(zero)),
                        output: then_branch.output.clone(),
                        domain: None,
                        lifted: false,
                    }]
                }
            };
            for else_branch in &else_candidates {
                if !types_equivalent(&then_branch.output, &else_branch.output) {
                    continue;
                }
                let mut input = None;
                if !merge_pointwise_domain(&mut input, condition.domain.as_ref())
                    || !merge_pointwise_domain(&mut input, then_branch.domain.as_ref())
                    || !merge_pointwise_domain(&mut input, else_branch.domain.as_ref())
                {
                    continue;
                }
                if !condition.lifted && !then_branch.lifted && !else_branch.lifted {
                    continue;
                }
                let Some(input) = input else {
                    continue;
                };
                candidates.push(FunctionExpr {
                    input: normalize_scalar_product_type(&input),
                    output: then_branch.output.clone(),
                    kind: FunctionExprKind::PointwiseConditional {
                        condition: condition.arg.clone(),
                        then_branch: then_branch.arg.clone(),
                        else_branch: else_branch.arg.clone(),
                    },
                });
            }
        }
    }

    Ok(candidates)
}

/// Type-checks helper logic for infer_pointwise_arg_candidates_for_expected.
fn infer_pointwise_arg_candidates_for_expected(
    expr: &Expr,
    expected_ty: &Type,
    env: &Env<'_>,
) -> Vec<PointwiseBinaryArgCandidate> {
    let mut candidates = Vec::new();
    if let Ok(funcs) = infer_function_expr_candidates(expr, env) {
        candidates.extend(funcs.into_iter().filter_map(|func| {
            (types_equivalent(&func.output, expected_ty)
                || can_cast_function_output_to_expected(&func.output, expected_ty))
            .then(|| PointwiseBinaryArgCandidate {
                output: expected_ty.clone(),
                domain: Some(func.input.clone()),
                lifted: true,
                arg: PointwiseCallArg::Function {
                    expected: expected_ty.clone(),
                    func: Box::new(func),
                },
            })
        }));
    }
    if let Ok(value) = infer_value_expr_for_type(expr, expected_ty, env, None) {
        if types_compatible_for_expected(&value.ty(), expected_ty) {
            candidates.push(PointwiseBinaryArgCandidate {
                arg: PointwiseCallArg::Value(Box::new(value)),
                output: expected_ty.clone(),
                domain: None,
                lifted: false,
            });
        }
    }
    candidates
}

/// Type-checks helper logic for merge_pointwise_domain.
fn merge_pointwise_domain(target: &mut Option<Type>, domain: Option<&Type>) -> bool {
    let Some(domain) = domain else {
        return true;
    };
    match target {
        Some(existing) => types_equivalent(existing, domain),
        None => {
            *target = Some(domain.clone());
            true
        }
    }
}

#[derive(Clone)]
struct PointwiseBinaryArgCandidate {
    arg: PointwiseCallArg,
    output: Type,
    domain: Option<Type>,
    lifted: bool,
}

/// Type-checks helper logic for infer_pointwise_binary_arg_candidates.
fn infer_pointwise_binary_arg_candidates(
    expr: &Expr,
    env: &Env<'_>,
) -> Vec<PointwiseBinaryArgCandidate> {
    let mut candidates = Vec::new();
    if let Ok(funcs) = infer_function_expr_candidates(expr, env) {
        candidates.extend(funcs.into_iter().map(|func| PointwiseBinaryArgCandidate {
            output: func.output.clone(),
            domain: Some(func.input.clone()),
            lifted: true,
            arg: PointwiseCallArg::Function {
                expected: func.output.clone(),
                func: Box::new(func),
            },
        }));
    }
    if let Ok(value) = infer_value_expr(expr, env, None) {
        candidates.push(PointwiseBinaryArgCandidate {
            output: value.ty(),
            domain: None,
            lifted: false,
            arg: PointwiseCallArg::Value(Box::new(value)),
        });
    }
    candidates
}

/// Type-checks helper logic for infer_pointwise_call_function_candidates.
fn infer_pointwise_call_function_candidates(
    name: &str,
    args: &[Expr],
    env: &Env<'_>,
) -> Result<Vec<FunctionExpr>, Error> {
    let Some(overloads) = env.function_overloads(name) else {
        return Err(Error::new(format!("unknown function '{}'", name)));
    };
    let mut candidates = Vec::new();

    for overload in overloads {
        let Ok((inputs, output)) = call_inputs_and_output(&overload.ty) else {
            continue;
        };
        if args.len() != inputs.len() || inputs.iter().any(|ty| matches!(ty, Type::Func(_, _))) {
            continue;
        }

        let mut call_args = Vec::new();
        let mut domain: Option<Type> = None;
        let mut lifted = false;
        let mut ok = true;

        for (arg, input_ty) in args.iter().zip(inputs.iter()) {
            match infer_pointwise_call_function_arg(arg, input_ty, env) {
                Ok(Some(func)) => {
                    if let Some(existing_domain) = &domain {
                        if !types_equivalent(existing_domain, &func.input) {
                            ok = false;
                            break;
                        }
                    } else {
                        domain = Some(func.input.clone());
                    }
                    lifted = true;
                    call_args.push(PointwiseCallArg::Function {
                        func: Box::new(func),
                        expected: input_ty.clone(),
                    });
                }
                Ok(None) => {
                    let Ok(value) = infer_value_expr_for_type(arg, input_ty, env, None) else {
                        ok = false;
                        break;
                    };
                    if ensure_type(&value.ty(), input_ty, &format!("call '{}(...)'", name)).is_err()
                    {
                        ok = false;
                        break;
                    }
                    call_args.push(PointwiseCallArg::Value(Box::new(value)));
                }
                Err(_) => {
                    ok = false;
                    break;
                }
            }
        }

        if ok && lifted {
            candidates.push(FunctionExpr {
                input: normalize_scalar_product_type(&domain.unwrap()),
                output,
                kind: FunctionExprKind::PointwiseCall {
                    func: name.to_string(),
                    args: call_args,
                },
            });
        }
    }

    Ok(candidates)
}

/// Type-checks helper logic for infer_pointwise_call_function_arg.
fn infer_pointwise_call_function_arg(
    arg: &Expr,
    expected_output: &Type,
    env: &Env<'_>,
) -> Result<Option<FunctionExpr>, Error> {
    let Ok(candidates) = infer_function_expr_candidates(arg, env) else {
        return Ok(None);
    };
    let candidates = candidates
        .into_iter()
        .filter(|func| {
            types_equivalent(&func.output, expected_output)
                || can_cast_function_output_to_expected(&func.output, expected_output)
        })
        .collect::<Vec<_>>();
    if candidates.len() == 1 {
        Ok(Some(candidates.into_iter().next().unwrap()))
    } else {
        Ok(None)
    }
}

/// Type-checks helper logic for can_cast_function_output_to_expected.
fn can_cast_function_output_to_expected(output: &Type, expected: &Type) -> bool {
    output == &Type::Bool && matches!(expected, Type::Float | Type::Int)
}

/// Type-checks helper logic for infer_same_domain_function_product_candidates.
fn infer_same_domain_function_product_candidates(
    items: &[Expr],
    env: &Env<'_>,
) -> Result<Vec<FunctionExpr>, Error> {
    if !(2..=4).contains(&items.len()) {
        return Err(Error::new("function tuples support two to four functions"));
    }
    let mut funcs = Vec::new();
    for item in items {
        funcs.push(infer_function_expr(item, env)?);
    }
    let input = funcs[0].input.clone();
    if funcs
        .iter()
        .any(|func| !types_equivalent(&func.input, &input))
    {
        return Err(Error::new(
            "function tuple entries must have equivalent domains",
        ));
    }
    let output = scalar_product_output(funcs.iter().map(|func| &func.output))?;
    Ok(vec![FunctionExpr {
        input: normalize_scalar_product_type(&input),
        output,
        kind: FunctionExprKind::ProductSameDomain(funcs),
    }])
}

/// Type-checks helper logic for infer_same_domain_vector_function_candidates.
fn infer_same_domain_vector_function_candidates(
    items: &[Expr],
    env: &Env<'_>,
) -> Result<Vec<FunctionExpr>, Error> {
    if !(2..=4).contains(&items.len()) {
        return Err(Error::new("function vectors support two to four functions"));
    }
    let funcs = items
        .iter()
        .map(|item| infer_function_expr(item, env))
        .collect::<Result<Vec<_>, _>>()?;
    let input = funcs[0].input.clone();
    if funcs
        .iter()
        .any(|func| !types_equivalent(&func.input, &input) || func.output != Type::Float)
    {
        return Err(Error::new(
            "function vector entries must have equivalent domains and scalar codomains",
        ));
    }
    Ok(vec![FunctionExpr {
        input: normalize_scalar_product_type(&input),
        output: vector_type(items.len()),
        kind: FunctionExprKind::ProductSameDomain(funcs),
    }])
}

/// Type-checks helper logic for infer_tensor_function_product_candidates.
fn infer_tensor_function_product_candidates(
    left: &Expr,
    right: &Expr,
    env: &Env<'_>,
) -> Result<Vec<FunctionExpr>, Error> {
    let left = infer_tensor_product_part(left, env)?;
    let right = infer_tensor_product_part(right, env)?;
    let input = scalar_product_output([&left.input, &right.input].into_iter())?;
    let output = scalar_product_output([&left.output, &right.output].into_iter())?;
    Ok(vec![FunctionExpr {
        input,
        output,
        kind: FunctionExprKind::ProductTensor(Box::new(left), Box::new(right)),
    }])
}

/// Type-checks helper logic for infer_tensor_product_part.
fn infer_tensor_product_part(expr: &Expr, env: &Env<'_>) -> Result<FunctionExpr, Error> {
    infer_function_expr(expr, env)
        .or_else(|_| infer_function_expr_for_type(expr, env, &Type::Float, &Type::Float))
}

/// Type-checks helper logic for scalar_product_output.
fn scalar_product_output<'a>(parts: impl Iterator<Item = &'a Type>) -> Result<Type, Error> {
    let parts = parts.cloned().collect::<Vec<_>>();
    let scalar_count = parts
        .iter()
        .map(scalar_product_part_len)
        .collect::<Option<Vec<_>>>();
    if let Some(count) = scalar_count.map(|parts| parts.into_iter().sum::<usize>()) {
        return match count {
            2 => Ok(Type::Vec2),
            3 => Ok(Type::Vec3),
            4 => Ok(Type::Vec4),
            _ => Err(Error::new(
                "function products currently support R2, R3, and R4 scalar codomains",
            )),
        };
    }
    Ok(Type::Product(parts))
}

/// Type-checks helper logic for scalar_product_part_len.
fn scalar_product_part_len(ty: &Type) -> Option<usize> {
    match ty {
        Type::Float => Some(1),
        Type::Product(parts) if parts.iter().all(|part| part == &Type::Float) => Some(parts.len()),
        _ => None,
    }
}

/// Type-checks helper logic for normalize_scalar_product_type.
fn normalize_scalar_product_type(ty: &Type) -> Type {
    match ty {
        Type::Product(parts) if parts.iter().all(|part| part == &Type::Float) => {
            match parts.len() {
                2 => Type::Vec2,
                3 => Type::Vec3,
                4 => Type::Vec4,
                _ => ty.clone(),
            }
        }
        other => other.clone(),
    }
}

/// Type-checks helper logic for types_equivalent.
fn types_equivalent(left: &Type, right: &Type) -> bool {
    types_match(
        &normalize_scalar_product_type(left),
        &normalize_scalar_product_type(right),
    )
}

/// Type-checks helper logic for format_function_expr.
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
