/// Type-checks raw GLSL runtime output typing.
fn raw_glsl_runtime_output_ty<'a>(body: &FuncBody, output_ty: &'a Type) -> &'a Type {
    if matches!(
        body,
        FuncBody::RawGlsl(_)
            | FuncBody::RawGlslClosure { .. }
            | FuncBody::Expr(Expr::Closure { .. })
    ) {
        innermost_output_ty(output_ty)
    } else {
        output_ty
    }
}

/// Type-checks helper logic for innermost_output_ty.
fn innermost_output_ty(ty: &Type) -> &Type {
    let (_, output) = flatten_func_type(ty);
    output
}

#[derive(Clone)]
struct RawGlslCapture {
    ty: Type,
    glsl_ref: String,
}

#[derive(Clone)]
struct RawGlslTemplateInfo {
    params: Vec<String>,
    body: String,
    template_name: String,
}

#[derive(Clone)]
struct LaneClosureTemplateInfo {
    params: Vec<String>,
    body: Expr,
}

/// Type-checks helper logic for lane_closure_template_info.
fn lane_closure_template_info(body: &FuncBody) -> Option<LaneClosureTemplateInfo> {
    let FuncBody::Expr(Expr::Closure { params, body }) = body else {
        return None;
    };
    let (params, body) = collect_lane_closure_params(params.clone(), body.as_ref().clone());
    Some(LaneClosureTemplateInfo { params, body })
}

/// Type-checks helper logic for collect_lane_closure_params.
fn collect_lane_closure_params(mut params: Vec<String>, body: Expr) -> (Vec<String>, Expr) {
    match body {
        Expr::Closure {
            params: next_params,
            body,
        } => {
            params.extend(next_params);
            collect_lane_closure_params(params, *body)
        }
        body => (params, body),
    }
}

/// Type-checks helper logic for typed_raw_glsl_body.
fn typed_raw_glsl_body(
    input_ty: &Type,
    output_ty: &Type,
    body: &str,
    closure_params: Option<&[String]>,
    env: &Env<'_>,
) -> Result<(Type, Type, String, RawGlslRefs), Error> {
    let mut captures = Vec::new();
    let mut runtime_input = input_ty.clone();
    let mut runtime_output = output_ty.clone();
    while let Type::Func(next_input, next_output) = runtime_output {
        captures.push(runtime_input);
        runtime_input = (*next_input).clone();
        runtime_output = (*next_output).clone();
    }

    let capture_bindings = raw_glsl_capture_bindings(&captures, closure_params)?;
    let (body, refs) = render_checked_raw_glsl_placeholders(body, env, &capture_bindings)?;
    Ok((runtime_input, runtime_output, body, refs))
}

/// Type-checks helper logic for raw_glsl_capture_bindings.
fn raw_glsl_capture_bindings(
    captures: &[Type],
    closure_params: Option<&[String]>,
) -> Result<HashMap<String, RawGlslCapture>, Error> {
    let mut bindings = HashMap::new();
    let Some(params) = closure_params else {
        for (index, capture_ty) in captures.iter().enumerate() {
            let param = if index == 0 {
                "obj".to_string()
            } else {
                format!("arg{index}")
            };
            bindings.insert(
                param.clone(),
                RawGlslCapture {
                    ty: capture_ty.clone(),
                    glsl_ref: param,
                },
            );
        }
        return Ok(bindings);
    };
    let expanded = captures
        .iter()
        .flat_map(|ty| match ty {
            Type::Product(parts) => parts.clone(),
            other => vec![other.clone()],
        })
        .collect::<Vec<_>>();
    if params.len() != expanded.len() {
        return Err(Error::new(format!(
            "raw GLSL closure expects {} capture parameter(s), got {}",
            expanded.len(),
            params.len()
        )));
    }
    for (param, ty) in params.iter().zip(expanded) {
        bindings.insert(
            param.clone(),
            RawGlslCapture {
                ty,
                glsl_ref: param.clone(),
            },
        );
    }
    Ok(bindings)
}

/// Type-checks helper logic for render_checked_raw_glsl_placeholders.
fn render_checked_raw_glsl_placeholders(
    body: &str,
    env: &Env<'_>,
    capture_bindings: &HashMap<String, RawGlslCapture>,
) -> Result<(String, RawGlslRefs), Error> {
    let mut out = String::with_capacity(body.len());
    let mut refs = RawGlslRefs::default();
    let mut index = 0;
    while let Some(relative_start) = body[index..].find("${") {
        let start = index + relative_start;
        out.push_str(&body[index..start]);
        let name_start = start + 2;
        let Some(relative_end) = body[name_start..].find('}') else {
            out.push_str(&body[start..]);
            return Ok((out, refs));
        };
        let end = name_start + relative_end;
        let placeholder = &body[name_start..end];
        if is_placeholder_ident(placeholder) {
            out.push_str(&raw_glsl_placeholder_ref(
                placeholder,
                env,
                capture_bindings,
                &mut refs,
            )?);
        } else {
            out.push_str(&body[start..=end]);
        }
        index = end + 1;
    }
    out.push_str(&body[index..]);
    Ok((out, refs))
}

/// Type-checks helper logic for raw_glsl_placeholder_ref.
fn raw_glsl_placeholder_ref(
    name: &str,
    env: &Env<'_>,
    capture_bindings: &HashMap<String, RawGlslCapture>,
    refs: &mut RawGlslRefs,
) -> Result<String, Error> {
    if let Some((base, field)) = name.split_once('.') {
        let capture = capture_bindings.get(base);
        let value_ty = env.get_value(base).map(|value| &value.ty);
        let ty = capture
            .map(|capture| &capture.ty)
            .or(value_ty)
            .ok_or_else(|| Error::new(format!("unknown raw GLSL placeholder '{}'", name)))?;
        let glsl_ref = capture
            .map(|capture| capture.glsl_ref.as_str())
            .unwrap_or(base);
        if matches!(ty, Type::Object | Type::Object2D) {
            refs.object_getters.insert(glsl_ref.to_string());
        } else if capture.is_none() && !matches!(ty, Type::Func(_, _)) {
            refs.values.insert(base.to_string());
        }
        return raw_glsl_field_ref(glsl_ref, field, ty, env);
    }

    if let Some(capture) = capture_bindings.get(name) {
        if matches!(capture.ty, Type::Object | Type::Object2D) {
            return Err(Error::new(format!(
                "raw GLSL placeholder '{}' is an object; use '{}.sdf' or '{}.grad'",
                name, name, name
            )));
        }
        if matches!(capture.ty, Type::Func(_, _)) {
            refs.funcs.insert(capture.glsl_ref.clone());
        }
        return Ok(capture.glsl_ref.clone());
    }

    if let Some(value) = env.get_value(name).map(|value| &value.ty) {
        if matches!(value, Type::Object | Type::Object2D) {
            return Err(Error::new(format!(
                "raw GLSL placeholder '{}' is an object; use '{}.sdf' or '{}.grad'",
                name, name, name
            )));
        }
        if matches!(value, Type::Func(_, _)) {
            refs.funcs.insert(name.to_string());
        } else {
            refs.values.insert(name.to_string());
        }
        return Ok(name.to_string());
    }

    let overloads = env
        .function_overloads(name)
        .ok_or_else(|| Error::new(format!("unknown raw GLSL placeholder '{}'", name)))?;
    let mut rendered = overloads
        .iter()
        .filter_map(|func| func.glsl_ref.as_deref())
        .collect::<Vec<_>>();
    rendered.sort_unstable();
    rendered.dedup();
    match rendered.as_slice() {
        [rendered] => {
            refs.funcs.insert(name.to_string());
            Ok((*rendered).to_string())
        }
        [] => Err(Error::new(format!(
            "raw GLSL placeholder '{}' cannot be rendered as a GLSL reference",
            name
        ))),
        _ => Err(Error::new(format!(
            "raw GLSL placeholder '{}' is ambiguous between GLSL references",
            name
        ))),
    }
}

/// Type-checks helper logic for raw_glsl_field_ref.
fn raw_glsl_field_ref(base: &str, field: &str, ty: &Type, env: &Env<'_>) -> Result<String, Error> {
    match (ty, field) {
        (Type::Object, "sdf") | (Type::Object2D, "sdf") => Ok(format!("sdf_{base}")),
        (Type::Object, "grad") => Ok(format!("grad_sdf_{base}")),
        (Type::Object2D, "grad") => Err(Error::new(format!(
            "raw GLSL placeholder '{}.grad' requires a 3D object",
            base
        ))),
        (Type::Object | Type::Object2D, _) => Err(Error::new(format!(
            "unknown object placeholder '{}.{}'",
            base, field
        ))),
        (Type::Custom { name, .. }, _) => {
            if let Some((_, resolved_field)) = env
                .product_type(name)
                .and_then(|product_type| product_field_access(product_type, field))
            {
                Ok(format!("{base}.{resolved_field}"))
            } else {
                Err(Error::new(format!(
                    "unknown product placeholder '{}.{}'",
                    base, field
                )))
            }
        }
        _ => Err(Error::new(format!(
            "raw GLSL placeholder '{}.{}' requires an object or product value",
            base, field
        ))),
    }
}

/// Type-checks helper logic for raw_glsl_template_info.
fn raw_glsl_template_info(name: &str, body: &FuncBody) -> Option<RawGlslTemplateInfo> {
    let FuncBody::RawGlslClosure { params, body } = body else {
        return None;
    };
    Some(RawGlslTemplateInfo {
        params: params.clone(),
        body: body.clone(),
        template_name: name.to_string(),
    })
}

/// Type-checks helper logic for instantiate_raw_glsl_template_func.
fn instantiate_raw_glsl_template_func(
    target_name: &str,
    target_input: &Type,
    target_output: &Type,
    expr: &Expr,
    env: &Env<'_>,
) -> Result<Option<(Type, Type, TypedFuncBody, RawGlslRefs)>, Error> {
    let Some((template_name, args)) = flatten_raw_template_call(expr) else {
        return Ok(None);
    };
    let Some(overloads) = env.function_overloads(template_name) else {
        return Ok(None);
    };
    let target_ty = Type::func(target_input.clone(), target_output.clone());
    let mut candidates = Vec::new();
    for overload in overloads {
        let Some(template) = &overload.raw_glsl_template else {
            continue;
        };
        let (all_inputs, final_output) = flatten_raw_template_inputs(&overload.ty);
        if args.len() >= all_inputs.len() {
            continue;
        };
        let inputs = all_inputs
            .iter()
            .take(args.len())
            .cloned()
            .collect::<Vec<_>>();
        let output = rebuild_func_type(&all_inputs[args.len()..], &final_output);
        if !raw_glsl_template_type_match(&output, &target_ty) {
            continue;
        }
        let captures = raw_glsl_template_captures(&template.params, &inputs, &args, env)?;
        let (body, refs) = render_checked_raw_glsl_placeholders(&template.body, env, &captures)?;
        let body = rename_raw_glsl_definition(&body, &template.template_name, target_name);
        candidates.push((
            target_input.clone(),
            target_output.clone(),
            TypedFuncBody::RawGlsl(body),
            refs,
        ));
    }
    match candidates.len() {
        0 => Ok(None),
        1 => Ok(candidates.pop()),
        _ => Err(Error::new(format!(
            "ambiguous raw GLSL template instantiation for '{}'",
            template_name
        ))),
    }
}

/// Type-checks helper logic for instantiate_lane_closure_template_func.
type LaneClosureTemplateCandidate = (Type, Type, TypedFuncBody, Vec<TypedFuncParamBinding>);

/// Type-checks and instantiates a Lane closure template into a typed function candidate.
fn instantiate_lane_closure_template_func(
    target_input: &Type,
    target_output: &Type,
    expr: &Expr,
    env: &Env<'_>,
) -> Result<Option<LaneClosureTemplateCandidate>, Error> {
    let Some((template_name, args)) = flatten_raw_template_call(expr) else {
        return Ok(None);
    };
    let Some(overloads) = env.function_overloads(template_name) else {
        return Ok(None);
    };
    let target_ty = Type::func(target_input.clone(), target_output.clone());
    let mut candidates = Vec::new();
    for overload in overloads {
        let Some(template) = &overload.lane_closure_template else {
            continue;
        };
        let (all_inputs, final_output) = flatten_raw_template_inputs(&overload.ty);
        if args.len() >= all_inputs.len() || args.len() >= template.params.len() {
            continue;
        }
        let output = rebuild_func_type(&all_inputs[args.len()..], &final_output);
        if !raw_glsl_template_type_match(&output, &target_ty) {
            continue;
        }
        let mut substitutions = HashMap::new();
        for (param, arg) in template.params.iter().zip(args.iter()) {
            substitutions.insert(param.clone(), (*arg).clone());
        }
        let mut body = template.body.clone();
        substitute_exprs(&mut body, &substitutions);
        let remaining_params = template.params[args.len()..].to_vec();
        if !remaining_params.is_empty() {
            body = Expr::Closure {
                params: remaining_params,
                body: Box::new(body),
            };
        }
        let (expr, param_bindings) = if let Expr::Closure { params, body } = &body {
            infer_explicit_closure_expr(params, body, env, target_input, target_output)?
        } else {
            let expr = infer_value_expr_for_type(&body, target_output, env, Some("t"))?;
            ensure_lift_param_type(&expr, "t", target_input)?;
            (expr, Vec::new())
        };
        ensure_type(&expr.ty(), target_output, "Lane closure template body")?;
        candidates.push((
            target_input.clone(),
            target_output.clone(),
            TypedFuncBody::Expr(expr),
            param_bindings,
        ));
    }
    match candidates.len() {
        0 => Ok(None),
        1 => Ok(candidates.pop()),
        _ => Err(Error::new(format!(
            "ambiguous Lane closure template instantiation for '{}'",
            template_name
        ))),
    }
}

/// Type-checks helper logic for substitute_exprs.
fn substitute_exprs(expr: &mut Expr, substitutions: &HashMap<String, Expr>) {
    match expr {
        Expr::Ident(name) => {
            if let Some(replacement) = substitutions.get(name) {
                *expr = replacement.clone();
            }
        }
        Expr::Closure { params, body } => {
            let scoped = substitutions
                .iter()
                .filter(|(name, _)| !params.contains(name))
                .map(|(name, expr)| (name.clone(), expr.clone()))
                .collect::<HashMap<_, _>>();
            substitute_exprs(body, &scoped);
        }
        Expr::Tuple(items) | Expr::Array(items) => {
            for item in items {
                substitute_exprs(item, substitutions);
            }
        }
        Expr::Call { callee, args } => {
            substitute_exprs(callee, substitutions);
            for arg in args {
                substitute_exprs(arg, substitutions);
            }
        }
        Expr::FieldAccess { object, .. } => substitute_exprs(object, substitutions),
        Expr::Conditional {
            condition,
            then_branch,
            else_branch,
        } => {
            substitute_exprs(condition, substitutions);
            substitute_exprs(then_branch, substitutions);
            if let Some(else_branch) = else_branch {
                substitute_exprs(else_branch, substitutions);
            }
        }
        Expr::Index { array, index } => {
            substitute_exprs(array, substitutions);
            substitute_exprs(index, substitutions);
        }
        Expr::Unary { expr, .. } => substitute_exprs(expr, substitutions),
        Expr::Binary { left, right, .. } => {
            substitute_exprs(left, substitutions);
            substitute_exprs(right, substitutions);
        }
        Expr::Constructor { name, args } => {
            if let Some(Expr::Ident(replacement)) = substitutions.get(name) {
                *name = replacement.clone();
            }
            match args {
                ConstructorArgs::Named(args) => {
                    for (_, arg) in args {
                        substitute_exprs(arg, substitutions);
                    }
                }
                ConstructorArgs::Positional(args) => {
                    for arg in args {
                        substitute_exprs(arg, substitutions);
                    }
                }
            }
        }
        Expr::Bool(_) | Expr::Number(_) | Expr::RawString(_) | Expr::Operator(_) => {}
    }
}

/// Type-checks helper logic for flatten_raw_template_call.
fn flatten_raw_template_call(expr: &Expr) -> Option<(&str, Vec<&Expr>)> {
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
            Expr::Constructor {
                name,
                args: ConstructorArgs::Positional(call_args),
            } => {
                for arg in call_args.iter().rev() {
                    args.push(arg);
                }
                args.reverse();
                return Some((name.as_str(), args));
            }
            Expr::Ident(name) => {
                args.reverse();
                return Some((name.as_str(), args));
            }
            _ => return None,
        }
    }
}

/// Type-checks helper logic for flatten_raw_template_inputs.
fn flatten_raw_template_inputs(ty: &Type) -> (Vec<Type>, Type) {
    let (inputs, output) = flatten_func_type(ty);
    let mut flattened = Vec::new();
    for input in inputs {
        match input {
            Type::Product(parts) => flattened.extend(parts.iter().cloned()),
            other => flattened.push(other.clone()),
        }
    }
    (flattened, output.clone())
}

/// Type-checks helper logic for rebuild_func_type.
fn rebuild_func_type(inputs: &[Type], output: &Type) -> Type {
    inputs.iter().rev().fold(output.clone(), |output, input| {
        Type::func(input.clone(), output)
    })
}

/// Type-checks helper logic for raw_glsl_template_type_match.
fn raw_glsl_template_type_match(actual: &Type, expected: &Type) -> bool {
    match (actual, expected) {
        (Type::Func(actual_input, actual_output), Type::Func(expected_input, expected_output)) => {
            raw_glsl_template_type_match(actual_input, expected_input)
                && raw_glsl_template_type_match(actual_output, expected_output)
        }
        (
            Type::Custom {
                name: actual_name, ..
            },
            Type::Custom {
                name: expected_name,
                ..
            },
        ) => actual_name == expected_name,
        _ => actual == expected,
    }
}

/// Type-checks helper logic for raw_glsl_template_captures.
fn raw_glsl_template_captures(
    params: &[String],
    inputs: &[Type],
    args: &[&Expr],
    env: &Env<'_>,
) -> Result<HashMap<String, RawGlslCapture>, Error> {
    if params.len() != inputs.len() {
        return Err(Error::new(format!(
            "raw GLSL closure expects {} capture parameter(s), got {}",
            inputs.len(),
            params.len()
        )));
    }
    let mut captures = HashMap::new();
    for ((param, input_ty), arg) in params.iter().zip(inputs.iter()).zip(args.iter()) {
        let capture = raw_glsl_template_capture(input_ty, arg, env)?;
        captures.insert(param.clone(), capture);
    }
    Ok(captures)
}

/// Type-checks helper logic for raw_glsl_template_capture.
fn raw_glsl_template_capture(
    input_ty: &Type,
    arg: &Expr,
    env: &Env<'_>,
) -> Result<RawGlslCapture, Error> {
    match input_ty {
        Type::Object | Type::Object2D => {
            let Expr::Ident(name) = arg else {
                return Err(Error::new(
                    "raw GLSL object template arguments must be named objects",
                ));
            };
            let ty = env
                .get_value(name)
                .map(|value| value.ty.clone())
                .ok_or_else(|| Error::new(format!("unknown object '{}'", name)))?;
            ensure_type(&ty, input_ty, "raw GLSL object template argument")?;
            Ok(RawGlslCapture {
                ty,
                glsl_ref: name.clone(),
            })
        }
        Type::Func(_, _) => {
            if let Expr::Ident(name) = arg {
                if let Some(overloads) = env.function_overloads(name) {
                    if overloads
                        .iter()
                        .any(|func| raw_glsl_template_type_match(&func.ty, input_ty))
                    {
                        return Ok(RawGlslCapture {
                            ty: input_ty.clone(),
                            glsl_ref: name.clone(),
                        });
                    }
                }
            }
            let func = infer_function_expr_for_type(
                arg,
                env,
                function_input(input_ty)?,
                function_output(input_ty)?,
            )?;
            let glsl_ref = raw_glsl_function_expr_ref(&func)?;
            Ok(RawGlslCapture {
                ty: input_ty.clone(),
                glsl_ref,
            })
        }
        _ if is_value_type(input_ty) => {
            let Expr::Ident(name) = arg else {
                return Err(Error::new(
                    "raw GLSL value template arguments must be named values",
                ));
            };
            let ty = env
                .get_value(name)
                .map(|value| value.ty.clone())
                .ok_or_else(|| Error::new(format!("unknown value '{}'", name)))?;
            ensure_type(&ty, input_ty, "raw GLSL value template argument")?;
            Ok(RawGlslCapture {
                ty,
                glsl_ref: name.clone(),
            })
        }
        _ => Err(Error::new(format!(
            "raw GLSL template argument type {} is not supported yet",
            format_type(input_ty)
        ))),
    }
}

/// Type-checks helper logic for function_input.
fn function_input(ty: &Type) -> Result<&Type, Error> {
    let Type::Func(input, _) = ty else {
        return Err(Error::new(format!(
            "expected function type, got {}",
            format_type(ty)
        )));
    };
    Ok(input)
}

/// Type-checks helper logic for function_output.
fn function_output(ty: &Type) -> Result<&Type, Error> {
    let Type::Func(_, output) = ty else {
        return Err(Error::new(format!(
            "expected function type, got {}",
            format_type(ty)
        )));
    };
    Ok(output)
}

/// Type-checks helper logic for raw_glsl_function_expr_ref.
fn raw_glsl_function_expr_ref(func: &FunctionExpr) -> Result<String, Error> {
    match &func.kind {
        FunctionExprKind::Named(name) => Ok(name.clone()),
        FunctionExprKind::Operator(op) => Err(Error::new(format!(
            "raw GLSL template operator argument '&{}' must be bound through Lane",
            op.symbol()
        ))),
        FunctionExprKind::ObjectGetter { object, getter, .. } => match getter {
            ObjectGetter::Sdf => Ok(format!("sdf_{object}")),
            ObjectGetter::Grad => Ok(format!("grad_sdf_{object}")),
        },
        _ => Err(Error::new(
            "raw GLSL template function arguments must have a GLSL reference",
        )),
    }
}

/// Type-checks helper logic for rename_raw_glsl_definition.
fn rename_raw_glsl_definition(body: &str, template_name: &str, target_name: &str) -> String {
    let needle = format!(" {template_name}(");
    let replacement = format!(" {target_name}(");
    body.replacen(&needle, &replacement, 1)
}

/// Type-checks helper logic for closure_param_types.
fn closure_param_types(input_ty: &Type) -> Vec<Type> {
    match input_ty {
        Type::Product(parts) => parts.clone(),
        Type::Vec2 => vec![Type::Float, Type::Float],
        Type::Vec3 => vec![Type::Float, Type::Float, Type::Float],
        Type::Vec4 => vec![Type::Float, Type::Float, Type::Float, Type::Float],
        other => vec![other.clone()],
    }
}

/// Type-checks helper logic for internal_closure_param_name.
fn internal_closure_param_name(name: &str) -> String {
    format!("_{name}")
}

/// Type-checks helper logic for infer_explicit_closure_expr.
fn infer_explicit_closure_expr(
    params: &[String],
    body: &Expr,
    env: &Env<'_>,
    input_ty: &Type,
    output_ty: &Type,
) -> Result<(ValueExpr, Vec<TypedFuncParamBinding>), Error> {
    let binds_whole_vector =
        params.len() == 1 && matches!(input_ty, Type::Vec2 | Type::Vec3 | Type::Vec4);
    let binds_whole_product = params.len() == 1 && matches!(input_ty, Type::Product(_));
    let param_types = if binds_whole_vector {
        vec![input_ty.clone()]
    } else if binds_whole_product {
        Vec::new()
    } else {
        closure_param_types(input_ty)
    };
    if !binds_whole_product && params.len() != param_types.len() {
        return Err(Error::new(format!(
            "closure expects {} parameter(s), got {}",
            param_types.len(),
            params.len()
        )));
    }
    let mut renamed = HashMap::new();
    let mut local_env = env.clone();
    let mut param_bindings = Vec::new();
    for param in params {
        if param.starts_with('_') {
            return Err(Error::new("closure parameter names cannot start with '_'"));
        }
    }
    if binds_whole_product {
        let Type::Product(parts) = input_ty else {
            unreachable!()
        };
        let param = params[0].clone();
        let mut body = body.clone();
        rewrite_whole_product_param_fields(
            &mut body,
            &param,
            parts,
            &mut local_env,
            &mut param_bindings,
        )?;
        let expr = infer_value_expr_for_type(&body, output_ty, &local_env, None)?;
        ensure_type(&expr.ty(), output_ty, "closure body")?;
        return Ok((expr, param_bindings));
    }
    for (param, ty) in params.iter().zip(param_types.iter()) {
        let internal = internal_closure_param_name(param);
        renamed.insert(param.clone(), internal.clone());
        local_env.insert_value(internal.clone(), ty.clone())?;
        param_bindings.push(TypedFuncParamBinding {
            ty: ty.clone(),
            name: internal,
            expr: if binds_whole_vector {
                "_t".to_string()
            } else {
                closure_param_source_expr(input_ty, param_bindings.len())?
            },
        });
    }
    let mut body = body.clone();
    rename_expr(&mut body, &renamed);
    let expr = infer_value_expr_for_type(&body, output_ty, &local_env, None)?;
    ensure_type(&expr.ty(), output_ty, "closure body")?;
    Ok((expr, param_bindings))
}

/// Type-checks helper logic for rewrite_whole_product_param_fields.
fn rewrite_whole_product_param_fields(
    expr: &mut Expr,
    param: &str,
    parts: &[Type],
    env: &mut Env<'_>,
    param_bindings: &mut Vec<TypedFuncParamBinding>,
) -> Result<(), Error> {
    match expr {
        Expr::FieldAccess { object, field } => {
            if matches!(&**object, Expr::Ident(name) if name == param) {
                let Some(index) = positional_product_field_index(field) else {
                    return Err(Error::new(format!(
                        "product parameter '{}' has no positional field '{}'",
                        param, field
                    )));
                };
                let Some(ty) = parts.get(index) else {
                    return Err(Error::new(format!(
                        "product parameter '{}' has no field '{}'",
                        param, field
                    )));
                };
                let internal = format!("__lane_product_param_{index}");
                if !env.has_binding(&internal) {
                    env.insert_value(internal.clone(), ty.clone())?;
                    param_bindings.push(TypedFuncParamBinding {
                        ty: ty.clone(),
                        name: internal.clone(),
                        expr: format!("_t{index}"),
                    });
                }
                *expr = Expr::Ident(internal);
                return Ok(());
            }
            rewrite_whole_product_param_fields(object, param, parts, env, param_bindings)
        }
        Expr::Closure { .. } => Ok(()),
        Expr::Tuple(items) | Expr::Array(items) => {
            for item in items {
                rewrite_whole_product_param_fields(item, param, parts, env, param_bindings)?;
            }
            Ok(())
        }
        Expr::Call { callee, args } => {
            if let Expr::Ident(name) = &**callee {
                if let Some(index) = parse_projection_name(name) {
                    if args.len() == 1 && matches!(&args[0], Expr::Ident(name) if name == param) {
                        let Some(ty) = parts.get(index) else {
                            return Err(Error::new(format!(
                                "product parameter '{}' has no projection p{{{}}}",
                                param, index
                            )));
                        };
                        let internal = format!("__lane_product_param_{index}");
                        if !env.has_binding(&internal) {
                            env.insert_value(internal.clone(), ty.clone())?;
                            param_bindings.push(TypedFuncParamBinding {
                                ty: ty.clone(),
                                name: internal.clone(),
                                expr: format!("_t{index}"),
                            });
                        }
                        *expr = Expr::Ident(internal);
                        return Ok(());
                    }
                }
            }
            rewrite_whole_product_param_fields(callee, param, parts, env, param_bindings)?;
            for arg in args {
                rewrite_whole_product_param_fields(arg, param, parts, env, param_bindings)?;
            }
            Ok(())
        }
        Expr::Conditional {
            condition,
            then_branch,
            else_branch,
        } => {
            rewrite_whole_product_param_fields(condition, param, parts, env, param_bindings)?;
            rewrite_whole_product_param_fields(then_branch, param, parts, env, param_bindings)?;
            if let Some(else_branch) = else_branch {
                rewrite_whole_product_param_fields(else_branch, param, parts, env, param_bindings)?;
            }
            Ok(())
        }
        Expr::Index { array, index } => {
            rewrite_whole_product_param_fields(array, param, parts, env, param_bindings)?;
            rewrite_whole_product_param_fields(index, param, parts, env, param_bindings)
        }
        Expr::Unary { expr, .. } => {
            rewrite_whole_product_param_fields(expr, param, parts, env, param_bindings)
        }
        Expr::Binary { left, right, .. } => {
            rewrite_whole_product_param_fields(left, param, parts, env, param_bindings)?;
            rewrite_whole_product_param_fields(right, param, parts, env, param_bindings)
        }
        Expr::Constructor { args, .. } => {
            match args {
                ConstructorArgs::Named(args) => {
                    for (_, arg) in args {
                        rewrite_whole_product_param_fields(arg, param, parts, env, param_bindings)?;
                    }
                }
                ConstructorArgs::Positional(args) => {
                    for arg in args {
                        rewrite_whole_product_param_fields(arg, param, parts, env, param_bindings)?;
                    }
                }
            }
            Ok(())
        }
        Expr::Bool(_)
        | Expr::Number(_)
        | Expr::RawString(_)
        | Expr::Ident(_)
        | Expr::Operator(_) => Ok(()),
    }
}

/// Type-checks helper logic for closure_param_source_expr.
fn closure_param_source_expr(input_ty: &Type, index: usize) -> Result<String, Error> {
    match input_ty {
        Type::Vec2 => Ok(format!("_t.{}", ["x", "y"][index])),
        Type::Vec3 => Ok(format!("_t.{}", ["x", "y", "z"][index])),
        Type::Vec4 => Ok(format!("_t.{}", ["x", "y", "z", "w"][index])),
        Type::Product(_) => Ok(format!("_t{index}")),
        _ if index == 0 => Ok("_t".to_string()),
        _ => Err(Error::new("closure parameter source is unavailable")),
    }
}
