use super::*;

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

fn lane_closure_template_info(body: &FuncBody) -> Option<LaneClosureTemplateInfo> {
    let FuncBody::Expr(Expr::Closure { params, body }) = body else {
        return None;
    };
    let (params, body) = collect_lane_closure_params(params.clone(), body.as_ref().clone());
    Some(LaneClosureTemplateInfo { params, body })
}

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

fn instantiate_lane_closure_template_func(
    target_input: &Type,
    target_output: &Type,
    expr: &Expr,
    env: &Env<'_>,
) -> Result<Option<(Type, Type, TypedFuncBody, Vec<TypedFuncParamBinding>)>, Error> {
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

fn rebuild_func_type(inputs: &[Type], output: &Type) -> Type {
    inputs.iter().rev().fold(output.clone(), |output, input| {
        Type::func(input.clone(), output)
    })
}

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

fn function_input(ty: &Type) -> Result<&Type, Error> {
    let Type::Func(input, _) = ty else {
        return Err(Error::new(format!(
            "expected function type, got {}",
            format_type(ty)
        )));
    };
    Ok(input)
}

fn function_output(ty: &Type) -> Result<&Type, Error> {
    let Type::Func(_, output) = ty else {
        return Err(Error::new(format!(
            "expected function type, got {}",
            format_type(ty)
        )));
    };
    Ok(output)
}

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

fn rename_raw_glsl_definition(body: &str, template_name: &str, target_name: &str) -> String {
    let needle = format!(" {template_name}(");
    let replacement = format!(" {target_name}(");
    body.replacen(&needle, &replacement, 1)
}

fn closure_param_types(input_ty: &Type) -> Vec<Type> {
    match input_ty {
        Type::Product(parts) => parts.clone(),
        Type::Vec2 => vec![Type::Float, Type::Float],
        Type::Vec3 => vec![Type::Float, Type::Float, Type::Float],
        Type::Vec4 => vec![Type::Float, Type::Float, Type::Float, Type::Float],
        other => vec![other.clone()],
    }
}

fn internal_closure_param_name(name: &str) -> String {
    format!("_{name}")
}

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

impl TypedProgram {
    pub(super) fn from_program(program: &Program, registry: &Registry) -> Result<Self, Error> {
        for product_type in &program.product_types {
            validate_product_type_decl(product_type)
                .map_err(|err| err.with_line(product_type.line))?;
        }
        let mut env = Env::new(
            registry,
            program.ambient_dimension,
            program.derivative_epsilon,
            &program.inputs,
            &program.product_types,
        );

        for input in &program.inputs {
            validate_user_type(&input.ty).map_err(|err| err.with_line(input.line))?;
            if matches!(input.ty, Type::Func(_, _)) {
                env.insert_func(input.name.clone(), input.ty.clone())
                    .map_err(|err| err.with_line(input.line))?;
            } else {
                env.insert_value(input.name.clone(), input.ty.clone())
                    .map_err(|err| err.with_line(input.line))?;
            }
        }
        for func in &program.funcs {
            validate_user_type(&func.ty).map_err(|err| err.with_line(func.line))?;
            env.insert_func_with_templates(
                func.name.clone(),
                func.ty.clone(),
                raw_glsl_template_info(&func.name, &func.body),
                lane_closure_template_info(&func.body),
            )
            .map_err(|err| err.with_line(func.line))?;
        }

        for binding in &program.value_bindings {
            validate_user_type(&binding.ty).map_err(|err| err.with_line(binding.line))?;
            env.insert_value(binding.name.clone(), binding.ty.clone())
                .map_err(|err| err.with_line(binding.line))?;
        }

        for binding in &program.bindings {
            env.insert_value(binding.name.clone(), binding.ty.clone())
                .map_err(|err| err.with_line(binding.line))?;
        }

        let mut typed_funcs = Vec::new();
        let mut typed_value_bindings = Vec::new();
        let mut typed_bindings = Vec::new();

        for binding in &program.inferred_bindings {
            match infer_object_expr(&binding.expr, &env) {
                Ok(expr) => {
                    let dimension = object_dimension(&expr, &env);
                    env.insert_value(binding.name.clone(), object_type_for_dimension(dimension))
                        .map_err(|err| err.with_line(binding.line))?;
                    env.update_object_dimension(&binding.name, dimension);
                    typed_bindings.push(TypedBinding {
                        name: binding.name.clone(),
                        expr,
                        generated: binding.generated,
                        dimension,
                        line: binding.line,
                    });
                }
                Err(object_err) => {
                    if binding.construct {
                        return Err(object_err.with_line(binding.line));
                    }
                    let expr = match infer_value_expr(&binding.expr, &env, None) {
                        Ok(expr) => expr,
                        Err(value_err) => {
                            if let Ok((input, output, expr)) =
                                infer_lifted_value_function(&binding.expr, &env)
                            {
                                env.insert_func(
                                    binding.name.clone(),
                                    Type::func(input.clone(), output.clone()),
                                )
                                .map_err(|err| err.with_line(binding.line))?;
                                typed_funcs.push(TypedFunc {
                                    name: binding.name.clone(),
                                    input,
                                    output,
                                    body: TypedFuncBody::Expr(expr),
                                    param_bindings: Vec::new(),
                                    raw_glsl_refs: RawGlslRefs::default(),
                                    generated: binding.generated,
                                    line: binding.line,
                                });
                                continue;
                            }
                            let func = infer_function_expr(&binding.expr, &env)
                                .map_err(|_| value_err.with_line(binding.line))?;
                            env.insert_func(
                                binding.name.clone(),
                                Type::func(func.input.clone(), func.output.clone()),
                            )
                            .map_err(|err| err.with_line(binding.line))?;
                            typed_funcs.push(TypedFunc {
                                name: binding.name.clone(),
                                input: func.input.clone(),
                                output: func.output.clone(),
                                body: TypedFuncBody::Expr(apply_function_expr(
                                    &func,
                                    ValueExpr::Var {
                                        name: "t".to_string(),
                                        ty: func.input.clone(),
                                        array_len: None,
                                    },
                                )),
                                param_bindings: Vec::new(),
                                raw_glsl_refs: RawGlslRefs::default(),
                                generated: binding.generated,
                                line: binding.line,
                            });
                            continue;
                        }
                    };
                    let ty = expr.ty();
                    validate_user_type(&ty).map_err(|err| err.with_line(binding.line))?;
                    env.insert_value(binding.name.clone(), ty.clone())
                        .map_err(|err| err.with_line(binding.line))?;
                    env.update_array_len(&binding.name, expr.array_len());
                    typed_value_bindings.push(TypedValueBinding {
                        name: binding.name.clone(),
                        ty,
                        expr,
                        generated: binding.generated,
                    });
                }
            }
        }

        for func in &program.funcs {
            let (input_ty, output_ty) = match &func.ty {
                Type::Func(input, output) => ((**input).clone(), (**output).clone()),
                other => {
                    return Err(Error::new(format!(
                        "function '{}' must have a function type, got {}",
                        func.name,
                        format_type(other)
                    ))
                    .with_line(func.line))
                }
            };
            validate_user_type(&input_ty).map_err(|err| err.with_line(func.line))?;
            if !matches!(
                raw_glsl_runtime_output_ty(&func.body, &output_ty),
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
                    | Type::Array(_)
            ) {
                return Err(Error::new(format!(
                    "function '{}' currently only supports scalar, vector, matrix, or array outputs",
                    func.name
                ))
                .with_line(func.line));
            }

            let (input_ty, output_ty, body, param_bindings, raw_glsl_refs) = match &func.body {
                FuncBody::RawGlsl(body) => {
                    let (input_ty, output_ty, body, refs) =
                        typed_raw_glsl_body(&input_ty, &output_ty, body, None, &env)?;
                    (
                        input_ty,
                        output_ty,
                        TypedFuncBody::RawGlsl(body),
                        Vec::new(),
                        refs,
                    )
                }
                FuncBody::RawGlslClosure { .. } => (
                    input_ty,
                    output_ty,
                    TypedFuncBody::RawGlslTemplate,
                    Vec::new(),
                    RawGlslRefs::default(),
                ),
                FuncBody::Expr(expr) => {
                    if lane_closure_template_info(&func.body).is_some()
                        && matches!(output_ty, Type::Func(_, _))
                    {
                        (
                            input_ty,
                            output_ty,
                            TypedFuncBody::RawGlslTemplate,
                            Vec::new(),
                            RawGlslRefs::default(),
                        )
                    } else if let Some((input_ty, output_ty, body, refs)) =
                        instantiate_raw_glsl_template_func(
                            &func.name, &input_ty, &output_ty, expr, &env,
                        )
                        .map_err(|err| err.with_line(func.line))?
                    {
                        (input_ty, output_ty, body, Vec::new(), refs)
                    } else if let Some((input_ty, output_ty, body, param_bindings)) =
                        instantiate_lane_closure_template_func(&input_ty, &output_ty, expr, &env)
                            .map_err(|err| err.with_line(func.line))?
                    {
                        (
                            input_ty,
                            output_ty,
                            body,
                            param_bindings,
                            RawGlslRefs::default(),
                        )
                    } else {
                        let (expr, param_bindings) = if let Expr::Closure { params, body } = expr {
                            infer_explicit_closure_expr(params, body, &env, &input_ty, &output_ty)
                                .map_err(|err| err.with_line(func.line))?
                        } else {
                            let expr = match infer_function_expr_for_type(
                                expr, &env, &input_ty, &output_ty,
                            ) {
                                Ok(func_expr) => apply_function_expr(
                                    &func_expr,
                                    ValueExpr::Var {
                                        name: "t".to_string(),
                                        ty: input_ty.clone(),
                                        array_len: None,
                                    },
                                ),
                                Err(_) => {
                                    infer_value_expr_for_type(expr, &output_ty, &env, Some("t"))
                                        .map_err(|err| err.with_line(func.line))?
                                }
                            };
                            (expr, Vec::new())
                        };
                        ensure_type(&expr.ty(), &output_ty, &format!("function '{}'", func.name))
                            .map_err(|err| err.with_line(func.line))?;
                        if param_bindings.is_empty() {
                            ensure_lift_param_type(&expr, "t", &input_ty)
                                .map_err(|err| err.with_line(func.line))?;
                        }
                        (
                            input_ty,
                            output_ty,
                            TypedFuncBody::Expr(expr),
                            param_bindings,
                            RawGlslRefs::default(),
                        )
                    }
                }
            };
            if input_ty == Type::Unit && output_ty == Type::Unit && func.name != "main" {
                return Err(Error::new(format!(
                    "shader entry function '{}' must be named 'main'",
                    func.name
                ))
                .with_line(func.line));
            }
            typed_funcs.push(TypedFunc {
                name: func.name.clone(),
                input: input_ty,
                output: output_ty,
                body,
                param_bindings,
                raw_glsl_refs,
                generated: func.generated,
                line: func.line,
            });
        }

        for binding in &program.value_bindings {
            let expr = infer_value_expr_for_type(&binding.expr, &binding.ty, &env, None)
                .map_err(|err| err.with_line(binding.line))?;
            ensure_type(
                &expr.ty(),
                &binding.ty,
                &format!("binding '{}'", binding.name),
            )
            .map_err(|err| err.with_line(binding.line))?;
            env.update_array_len(&binding.name, expr.array_len());
            typed_value_bindings.push(TypedValueBinding {
                name: binding.name.clone(),
                ty: binding.ty.clone(),
                expr,
                generated: binding.generated,
            });
        }

        for binding in &program.bindings {
            ensure_type(
                &binding.ty,
                &Type::Object,
                &format!("binding '{}'", binding.name),
            )
            .or_else(|_| {
                ensure_type(
                    &binding.ty,
                    &Type::Object2D,
                    &format!("binding '{}'", binding.name),
                )
            })
            .map_err(|err| err.with_line(binding.line))?;
            let expr = infer_object_expr(&binding.expr, &env)
                .map_err(|err| err.with_line(binding.line))?;
            let dimension = object_dimension(&expr, &env);
            if binding.ty == Type::Object2D && dimension != Some(ShapeDimension::D2) {
                return Err(
                    Error::new(format!("binding '{}' expected Object2D", binding.name))
                        .with_line(binding.line),
                );
            }
            env.update_object_dimension(&binding.name, dimension);
            typed_bindings.push(TypedBinding {
                name: binding.name.clone(),
                expr,
                generated: binding.generated,
                dimension,
                line: binding.line,
            });
        }

        let output = program
            .output
            .as_ref()
            .map(|output| {
                let expr = infer_object_expr(&output.expr, &env)
                    .map_err(|err| err.with_line(output.line))?;
                if program.ambient_dimension == ShapeDimension::D2
                    && object_dimension(&expr, &env) != Some(ShapeDimension::D2)
                {
                    return Err(
                        Error::new("const output expected Object2D in 2D ambient space")
                            .with_line(output.line),
                    );
                }
                Ok(expr)
            })
            .transpose()?;
        if let Some(output) = &output {
            typed_bindings.push(TypedBinding {
                name: "output".to_string(),
                expr: output.clone(),
                generated: true,
                dimension: object_dimension(output, &env),
                line: program
                    .output
                    .as_ref()
                    .map(|output| output.line)
                    .unwrap_or(usize::MAX),
            });
        }

        Ok(Self {
            ambient_dimension: program.ambient_dimension,
            gradient_epsilon: program.gradient_epsilon,
            product_types: program.product_types.clone(),
            inputs: program.inputs.clone(),
            funcs: typed_funcs,
            value_bindings: typed_value_bindings,
            bindings: typed_bindings,
            output,
        })
    }
}

#[derive(Clone)]
struct Env<'a> {
    registry: &'a Registry,
    ambient_dimension: ShapeDimension,
    derivative_epsilon: f64,
    product_types: HashMap<String, ProductTypeDecl>,
    values: HashMap<String, ValueInfo>,
    funcs: HashMap<String, Vec<FunctionInfo>>,
    object_dimensions: HashMap<String, ShapeDimension>,
}

#[derive(Clone)]
struct ValueInfo {
    ty: Type,
    array_len: Option<usize>,
}

#[derive(Clone)]
struct FunctionInfo {
    ty: Type,
    builtin: bool,
    glsl_ref: Option<String>,
    raw_glsl_template: Option<RawGlslTemplateInfo>,
    lane_closure_template: Option<LaneClosureTemplateInfo>,
}

impl<'a> Env<'a> {
    fn new(
        registry: &'a Registry,
        ambient_dimension: ShapeDimension,
        derivative_epsilon: f64,
        _inputs: &[InputDecl],
        product_types: &[ProductTypeDecl],
    ) -> Self {
        let values = HashMap::new();
        let mut funcs: HashMap<String, Vec<FunctionInfo>> = HashMap::new();
        for (name, func) in &registry.value_funcs {
            funcs
                .entry((*name).to_string())
                .or_default()
                .push(FunctionInfo {
                    ty: func.ty.clone(),
                    builtin: true,
                    glsl_ref: func.support_glsl.map(|_| (*name).to_string()),
                    raw_glsl_template: None,
                    lane_closure_template: None,
                });
        }
        for (name, overloads) in glsl_builtin_value_func_overloads() {
            let funcs = funcs.entry(name.to_string()).or_default();
            for ty in overloads {
                if !funcs.iter().any(|func| func.ty == ty) {
                    funcs.push(FunctionInfo {
                        ty,
                        builtin: true,
                        glsl_ref: Some(name.to_string()),
                        raw_glsl_template: None,
                        lane_closure_template: None,
                    });
                }
            }
        }
        for name in COMPLEX_OVERLOAD_NAMES {
            funcs
                .entry(name.to_string())
                .or_default()
                .push(FunctionInfo {
                    ty: Type::func(Type::Complex, Type::Complex),
                    builtin: true,
                    glsl_ref: Some(name.to_string()),
                    raw_glsl_template: None,
                    lane_closure_template: None,
                });
        }
        for op in registry.object_ops.values() {
            funcs
                .entry(op.name.to_string())
                .or_default()
                .push(FunctionInfo {
                    ty: object_op_type(op),
                    builtin: true,
                    glsl_ref: Some(op.glsl_name.to_string()),
                    raw_glsl_template: None,
                    lane_closure_template: None,
                });
        }
        Self {
            registry,
            ambient_dimension,
            derivative_epsilon,
            product_types: product_types
                .iter()
                .map(|decl| (decl.name.clone(), decl.clone()))
                .collect(),
            values,
            funcs,
            object_dimensions: HashMap::new(),
        }
    }

    fn insert_value(&mut self, name: String, ty: Type) -> Result<(), Error> {
        if self.values.contains_key(&name) || self.funcs.contains_key(&name) {
            return Err(Error::new(format!("duplicate declaration for '{}'", name)));
        }
        let dimension = object_type_dimension(&ty);
        self.values.insert(
            name.clone(),
            ValueInfo {
                ty,
                array_len: None,
            },
        );
        if let Some(dimension) = dimension {
            self.object_dimensions.insert(name, dimension);
        }
        Ok(())
    }

    fn insert_func(&mut self, name: String, ty: Type) -> Result<(), Error> {
        self.insert_func_with_templates(name, ty, None, None)
    }

    fn insert_func_with_templates(
        &mut self,
        name: String,
        ty: Type,
        raw_glsl_template: Option<RawGlslTemplateInfo>,
        lane_closure_template: Option<LaneClosureTemplateInfo>,
    ) -> Result<(), Error> {
        if self.values.contains_key(&name) {
            return Err(Error::new(format!("duplicate declaration for '{}'", name)));
        }
        let (domain, _) = function_domain_and_output(&ty)?;
        let overloads = self.funcs.entry(name.clone()).or_default();
        let (_, output) = function_domain_and_output(&ty)?;
        for func in overloads.iter() {
            let Ok((existing_domain, existing_output)) = function_domain_and_output(&func.ty)
            else {
                continue;
            };
            if existing_domain == domain {
                if func.builtin && existing_output == output {
                    return Ok(());
                }
                return Err(Error::new(format!(
                    "duplicate overload for '{}' with domain {}",
                    name,
                    format_type(&domain)
                )));
            }
        }
        overloads.push(FunctionInfo {
            ty,
            builtin: false,
            glsl_ref: Some(name),
            raw_glsl_template,
            lane_closure_template,
        });
        Ok(())
    }

    fn get(&self, name: &str) -> Option<&Type> {
        self.values.get(name).map(|info| &info.ty).or_else(|| {
            self.funcs
                .get(name)
                .and_then(|funcs| (funcs.len() == 1).then_some(&funcs[0].ty))
        })
    }

    fn get_value(&self, name: &str) -> Option<&ValueInfo> {
        self.values.get(name)
    }

    fn function_overloads(&self, name: &str) -> Option<&[FunctionInfo]> {
        self.funcs.get(name).map(|funcs| funcs.as_slice())
    }

    fn has_binding(&self, name: &str) -> bool {
        self.values.contains_key(name) || self.funcs.contains_key(name)
    }

    fn update_array_len(&mut self, name: &str, array_len: Option<usize>) {
        if let Some(info) = self.values.get_mut(name) {
            info.array_len = array_len;
        }
    }

    fn update_object_dimension(&mut self, name: &str, dimension: Option<ShapeDimension>) {
        if let Some(dimension) = dimension {
            self.object_dimensions.insert(name.to_string(), dimension);
        }
    }

    fn object_dimension(&self, name: &str) -> Option<ShapeDimension> {
        self.object_dimensions.get(name).copied()
    }

    fn scene_input_values(&self) -> Vec<ValueExpr> {
        Vec::new()
    }

    fn product_type(&self, name: &str) -> Option<&ProductTypeDecl> {
        self.product_types.get(name)
    }
}

fn validate_product_type_decl(decl: &ProductTypeDecl) -> Result<(), Error> {
    if decl.category == AlgebraicCategory::DivRing {
        return Err(Error::new(format!(
            "product type '{}' cannot be declared as DivRing",
            decl.name
        )));
    }
    if !product_category_supported(decl.category) {
        return Err(Error::new(format!(
            "product type '{}' does not support category {}",
            decl.name,
            category_name(decl.category)
        )));
    }
    for component in &decl.components {
        if !has_category(component, decl.category) {
            return Err(Error::new(format!(
                "product type '{}' component {} does not satisfy {}",
                decl.name,
                format_type(component),
                category_name(decl.category)
            )));
        }
    }
    Ok(())
}

fn product_category_supported(category: AlgebraicCategory) -> bool {
    matches!(
        category,
        AlgebraicCategory::Ab
            | AlgebraicCategory::Mon
            | AlgebraicCategory::Grp
            | AlgebraicCategory::Ring
            | AlgebraicCategory::RVect
            | AlgebraicCategory::RAlg
            | AlgebraicCategory::Set
    )
}

fn object_type_dimension(ty: &Type) -> Option<ShapeDimension> {
    match ty {
        Type::Object2D => Some(ShapeDimension::D2),
        _ => None,
    }
}

fn object_type_for_dimension(dimension: Option<ShapeDimension>) -> Type {
    match dimension {
        Some(ShapeDimension::D2) => Type::Object2D,
        _ => Type::Object,
    }
}

fn function_domain_and_output(ty: &Type) -> Result<(Type, Type), Error> {
    let (inputs, output) = flatten_func_type(ty);
    if inputs.is_empty() {
        return Err(Error::new(format!(
            "expected function type, got {}",
            format_type(ty)
        )));
    }
    let domain = if inputs.len() == 1 {
        (*inputs[0]).clone()
    } else {
        Type::Product(inputs.into_iter().cloned().collect())
    };
    Ok((domain, (*output).clone()))
}

fn infer_object_expr(expr: &Expr, env: &Env<'_>) -> Result<ObjectExpr, Error> {
    match expr {
        Expr::Ident(name) => {
            let ty = env
                .get(name)
                .ok_or_else(|| Error::new(format!("unknown identifier '{}'", name)))?;
            if !matches!(ty, Type::Object | Type::Object2D) {
                ensure_type(ty, &Type::Object, &format!("identifier '{}'", name))?;
            }
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
            if env.ambient_dimension == ShapeDimension::D2
                && registry::shape_dimension(name) == ShapeDimension::D3
            {
                return Err(Error::new(format!(
                    "primitive '{}' is 3D but ambient space is 2D",
                    name
                )));
            }
            if let Some(fields) = infer_segment_length_constructor(name, args, env)? {
                return Ok(ObjectExpr::Primitive {
                    name: name.clone(),
                    kind: primitive.kind.clone(),
                    fields,
                });
            }
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
                        let typed = infer_value_expr_for_type(&value.1, expected_ty, env, None)?;
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
            let offset_ty = ambient_vector_type(env.ambient_dimension);
            let offset = infer_value_expr_for_type(right, &offset_ty, env, None)?;
            ensure_type(&offset.ty(), &offset_ty, "object shift")?;
            Ok(ObjectExpr::AmbientTransform {
                object: Box::new(object),
                translation: offset,
                linear: ambient_identity_matrix(env.ambient_dimension),
            })
        }
        Expr::Binary {
            op: BinOp::Mul,
            left,
            right,
        } => {
            let action = infer_value_expr(left, env, None)?;
            let object = infer_object_expr(right, env)?;
            match action.ty() {
                Type::Mat(2, 2) if env.ambient_dimension == ShapeDimension::D2 => {
                    Ok(ObjectExpr::AmbientTransform {
                        object: Box::new(object),
                        translation: zero_vec2(),
                        linear: action,
                    })
                }
                Type::Isom2 if env.ambient_dimension == ShapeDimension::D2 => {
                    Ok(ObjectExpr::IsometryTransform {
                        object: Box::new(object),
                        transform: action,
                    })
                }
                Type::Mat(3, 3) if env.ambient_dimension == ShapeDimension::D3 => {
                    Ok(ObjectExpr::AmbientTransform {
                        object: Box::new(object),
                        translation: zero_vec3(),
                        linear: action,
                    })
                }
                Type::Isom3 if env.ambient_dimension == ShapeDimension::D3 => {
                    Ok(ObjectExpr::IsometryTransform {
                        object: Box::new(object),
                        transform: action,
                    })
                }
                ty => Err(Error::new(format!(
                    "unsupported object action: {} * Object",
                    format_type(&ty)
                ))),
            }
        }
        Expr::Call { .. } => infer_object_call(expr, env),
        Expr::Bool(_)
        | Expr::Number(_)
        | Expr::RawString(_)
        | Expr::Closure { .. }
        | Expr::Operator(_)
        | Expr::Tuple(_)
        | Expr::Array(_)
        | Expr::Index { .. }
        | Expr::FieldAccess { .. }
        | Expr::Conditional { .. } => Err(Error::new("expected an Object expression")),
        Expr::Binary { .. } => Err(Error::new("unsupported object expression")),
    }
}

fn object_dimension(expr: &ObjectExpr, env: &Env<'_>) -> Option<ShapeDimension> {
    match expr {
        ObjectExpr::Var(name) => env.object_dimension(name),
        ObjectExpr::Primitive { name, .. } => Some(registry::shape_dimension(name)),
        ObjectExpr::AmbientTransform { object, .. }
        | ObjectExpr::IsometryTransform { object, .. } => object_dimension(object, env),
        ObjectExpr::RegisteredOp {
            glsl_name,
            object_args,
            ..
        } => match glsl_name.as_str() {
            "op_revolution" | "op_extrusion" => Some(ShapeDimension::D3),
            _ => object_args
                .first()
                .and_then(|object| object_dimension(object, env)),
        },
    }
}

fn infer_segment_length_constructor(
    name: &str,
    args: &ConstructorArgs,
    env: &Env<'_>,
) -> Result<Option<Vec<(String, PrimitiveArgExpr)>>, Error> {
    if !matches!(name, "Segment2D" | "Segment3D") {
        return Ok(None);
    }

    let length_expr = match args {
        ConstructorArgs::Named(fields) if fields.len() == 1 && fields[0].0 == "length" => {
            infer_value_expr(&fields[0].1, env, None)?
        }
        ConstructorArgs::Positional(values) if values.len() == 1 => {
            infer_value_expr(&values[0], env, None)?
        }
        _ => return Ok(None),
    };
    ensure_type(
        &length_expr.ty(),
        &Type::Float,
        &format!("primitive '{name}' length constructor"),
    )?;

    let half_length = ValueExpr::Binary {
        op: BinOp::Mul,
        left: Box::new(ValueExpr::Float(0.5)),
        right: Box::new(length_expr),
        ty: Type::Float,
    };
    let neg_half_length = ValueExpr::Binary {
        op: BinOp::Mul,
        left: Box::new(ValueExpr::Float(-1.0)),
        right: Box::new(half_length.clone()),
        ty: Type::Float,
    };

    let (a, b) = if name == "Segment2D" {
        (
            ValueExpr::Vec2(Box::new(neg_half_length), Box::new(ValueExpr::Float(0.0))),
            ValueExpr::Vec2(Box::new(half_length), Box::new(ValueExpr::Float(0.0))),
        )
    } else {
        (
            ValueExpr::Vec3(
                Box::new(neg_half_length),
                Box::new(ValueExpr::Float(0.0)),
                Box::new(ValueExpr::Float(0.0)),
            ),
            ValueExpr::Vec3(
                Box::new(half_length),
                Box::new(ValueExpr::Float(0.0)),
                Box::new(ValueExpr::Float(0.0)),
            ),
        )
    };

    Ok(Some(vec![
        ("a".to_string(), PrimitiveArgExpr::Value(a)),
        ("b".to_string(), PrimitiveArgExpr::Value(b)),
    ]))
}

fn infer_object_call(expr: &Expr, env: &Env<'_>) -> Result<ObjectExpr, Error> {
    let (name, args) = flatten_call(expr)?;
    let op = env
        .registry
        .object_ops
        .get(name.as_str())
        .ok_or_else(|| Error::new(format!("unknown object operator '{}'", name)))?;
    if env.ambient_dimension == ShapeDimension::D2
        && object_op_output_dimension(op) == ShapeDimension::D3
    {
        return Err(Error::new(format!(
            "operator '{}' produces Object3D but ambient space is 2D",
            name
        )));
    }
    if matches!(name.as_str(), "rot" | "rot2D") {
        return infer_rotation_object_call(&name, &args, op, env);
    }
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
    ensure_object_op_arg_dimensions(op, &object_args, env)?;

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

fn object_op_output_dimension(op: &ObjectOpDef) -> ShapeDimension {
    match op.glsl_name {
        "op_revolution" | "op_extrusion" | "op_rot" => ShapeDimension::D3,
        _ => ShapeDimension::D2,
    }
}

fn infer_rotation_object_call(
    name: &str,
    args: &[&Expr],
    op: &ObjectOpDef,
    env: &Env<'_>,
) -> Result<ObjectExpr, Error> {
    let value_arg_count = rotation_value_arg_count(name, args)?;
    if args.len() != value_arg_count + op.object_arg_count {
        return Err(Error::new(format!(
            "operator '{}' expects {} object argument(s)",
            name, op.object_arg_count
        )));
    }

    let value_args = match name {
        "rot" => infer_rot_value_args(&args[..value_arg_count], env)?,
        "rot2D" => infer_rot2d_value_args(&args[..value_arg_count], env)?,
        _ => unreachable!(),
    };
    let object_args = args[value_arg_count..]
        .iter()
        .map(|arg| infer_object_expr(arg, env))
        .collect::<Result<Vec<_>, _>>()?;
    ensure_object_op_arg_dimensions(op, &object_args, env)?;

    Ok(ObjectExpr::RegisteredOp {
        name: op.name.to_string(),
        glsl_name: op.glsl_name.to_string(),
        value_args,
        object_args,
    })
}

fn ensure_object_op_arg_dimensions(
    op: &ObjectOpDef,
    object_args: &[ObjectExpr],
    env: &Env<'_>,
) -> Result<(), Error> {
    if op.name != "revolution" {
        return Ok(());
    }
    let Some(object) = object_args.first() else {
        return Ok(());
    };
    if object_dimension(object, env) == Some(ShapeDimension::D2) {
        return Ok(());
    }
    Err(Error::new(
        "operator 'revolution' expects an Object2D argument",
    ))
}

fn rotation_value_arg_count(name: &str, args: &[&Expr]) -> Result<usize, Error> {
    let max_value_args = match name {
        "rot" => 3,
        "rot2D" => 2,
        _ => unreachable!(),
    };
    if args.is_empty() {
        return Err(Error::new(format!(
            "operator '{}' expects an object argument",
            name
        )));
    }
    if args.len() > max_value_args + 1 {
        return Err(Error::new(format!(
            "operator '{}' expects at most {} value argument(s)",
            name, max_value_args
        )));
    }
    Ok(args.len() - 1)
}

fn infer_rot_value_args(args: &[&Expr], env: &Env<'_>) -> Result<Vec<ValueExpr>, Error> {
    let (binormal, anchor, angle) = match args {
        [] => (unit_z_vec3(), zero_vec3(), ValueExpr::Float(0.0)),
        [angle] => (
            unit_z_vec3(),
            zero_vec3(),
            infer_value_expr(angle, env, None)?,
        ),
        [binormal, angle] => (
            infer_value_expr_for_type(binormal, &Type::Vec3, env, None)?,
            zero_vec3(),
            infer_value_expr(angle, env, None)?,
        ),
        [binormal, anchor, angle] => (
            infer_value_expr_for_type(binormal, &Type::Vec3, env, None)?,
            infer_value_expr_for_type(anchor, &Type::Vec3, env, None)?,
            infer_value_expr(angle, env, None)?,
        ),
        _ => unreachable!(),
    };
    ensure_type(&binormal.ty(), &Type::Vec3, "rot binormal")?;
    ensure_type(&anchor.ty(), &Type::Vec3, "rot anchor")?;
    ensure_type(&angle.ty(), &Type::Float, "rot angle")?;
    Ok(vec![binormal, anchor, angle])
}

fn infer_rot2d_value_args(args: &[&Expr], env: &Env<'_>) -> Result<Vec<ValueExpr>, Error> {
    let (anchor, angle) = match args {
        [] => (zero_vec2(), ValueExpr::Float(0.0)),
        [angle] => (zero_vec2(), infer_value_expr(angle, env, None)?),
        [anchor, angle] => (
            infer_value_expr_for_type(anchor, &Type::Vec2, env, None)?,
            infer_value_expr(angle, env, None)?,
        ),
        _ => unreachable!(),
    };
    ensure_type(&anchor.ty(), &Type::Vec2, "rot2D anchor")?;
    ensure_type(&angle.ty(), &Type::Float, "rot2D angle")?;
    Ok(vec![anchor, angle])
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
        Expr::Binary { op, left, right } => {
            let left = infer_value_expr(left, env, lift_param)?;
            let right = match (*op, left.ty()) {
                (BinOp::Mul, Type::Isom2) => {
                    infer_value_expr_for_type(right, &Type::Vec2, env, lift_param)?
                }
                (BinOp::Mul, Type::Isom3) => {
                    infer_value_expr_for_type(right, &Type::Vec3, env, lift_param)?
                }
                _ => infer_value_expr(right, env, lift_param)?,
            };
            let (left, right, ty) = match infer_binary_type(*op, &left.ty(), &right.ty()) {
                Ok(ty) => (left, right, ty),
                Err(original_err) => {
                    if let Some(right_cast) = try_int_literal_cast_value(&right, &left.ty()) {
                        if let Ok(ty) = infer_binary_type(*op, &left.ty(), &right_cast.ty()) {
                            (left, right_cast, ty)
                        } else {
                            return Err(original_err);
                        }
                    } else if let Some(left_cast) = try_int_literal_cast_value(&left, &right.ty()) {
                        if let Ok(ty) = infer_binary_type(*op, &left_cast.ty(), &right.ty()) {
                            (left_cast, right, ty)
                        } else {
                            return Err(original_err);
                        }
                    } else if let Some(right_cast) =
                        try_bool_to_number_cast_value(&right, &left.ty())
                    {
                        if let Ok(ty) = infer_binary_type(*op, &left.ty(), &right_cast.ty()) {
                            (left, right_cast, ty)
                        } else {
                            return Err(original_err);
                        }
                    } else if let Some(left_cast) =
                        try_bool_to_number_cast_value(&left, &right.ty())
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
    if lift_param.is_some() {
        let expr = Expr::FieldAccess {
            object: Box::new(object.clone()),
            field: field.to_string(),
        };
        let func = infer_function_expr(&expr, env)?;
        return Ok(apply_function_expr(
            &func,
            ValueExpr::Var {
                name: lift_param.unwrap().to_string(),
                ty: func.input.clone(),
                array_len: None,
            },
        ));
    }
    Err(Error::new(format!("value has no field '{}'", field)))
}

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

fn vector_field_access(dimension: usize, field: &str) -> Option<String> {
    let index = positional_product_field_index(field)?;
    if index >= dimension {
        return None;
    }
    Some(["x", "y", "z", "w"][index].to_string())
}

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

fn positional_product_field_index(field: &str) -> Option<usize> {
    match field {
        "x" => Some(0),
        "y" => Some(1),
        "z" => Some(2),
        "w" => Some(3),
        _ => field.strip_prefix('x')?.parse::<usize>().ok(),
    }
}

fn default_product_field_name(count: usize, index: usize) -> String {
    match (count, index) {
        (_, index) if index >= count => unreachable!("field index outside product"),
        (1..=4, 0) => "x".to_string(),
        (2..=4, 1) => "y".to_string(),
        (3..=4, 2) => "z".to_string(),
        (4, 3) => "w".to_string(),
        _ => format!("x{index}"),
    }
}

fn infer_value_expr_for_type(
    expr: &Expr,
    expected_ty: &Type,
    env: &Env<'_>,
    lift_param: Option<&str>,
) -> Result<ValueExpr, Error> {
    match (expected_ty, expr) {
        (Type::Bool, Expr::Bool(value)) => Ok(ValueExpr::Bool(*value)),
        (_, Expr::Number(value)) if (*value - 0.0).abs() < f64::EPSILON => {
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
        (_, Expr::Ident(name)) if lift_param.is_some() && env.get_value(name).is_none() => {
            let func = infer_function_expr_for_type(expr, env, &Type::Float, expected_ty)?;
            Ok(apply_function_expr(
                &func,
                ValueExpr::Var {
                    name: lift_param.unwrap().to_string(),
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
            let func = infer_function_expr_for_type(expr, env, &Type::Float, expected_ty)?;
            Ok(apply_function_expr(
                &func,
                ValueExpr::Var {
                    name: lift_param.unwrap().to_string(),
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
            let func = infer_function_expr(expr, env)?;
            ensure_type(&func.output, expected_ty, "function product")?;
            Ok(apply_function_expr(
                &func,
                ValueExpr::Var {
                    name: lift_param.unwrap().to_string(),
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
        (Type::Vec4, Expr::Tuple(items)) if items.len() == 2 => {
            let xyz = infer_value_expr_for_type(&items[0], &Type::Vec3, env, lift_param)?;
            let w = infer_value_expr_for_type(&items[1], &Type::Float, env, lift_param)?;
            return Ok(ValueExpr::Call {
                func: "vec4".to_string(),
                args: vec![xyz, w],
                ty: Type::Vec4,
            });
        }
        (Type::Float, _) => {
            let value = infer_value_expr(expr, env, lift_param)?;
            if let Some(cast) = try_bool_to_number_cast_value(&value, expected_ty) {
                return Ok(cast);
            }
            Ok(value)
        }
        (Type::Int, _) => {
            if matches!(expr, Expr::Number(_)) {
                return infer_int_expr(expr, env, lift_param);
            }
            let value = infer_value_expr(expr, env, lift_param)?;
            if let Some(cast) = try_bool_to_number_cast_value(&value, expected_ty) {
                return Ok(cast);
            }
            Ok(value)
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

fn cast_value_to_type(value: ValueExpr, expected_ty: &Type) -> Result<ValueExpr, ValueExpr> {
    if types_compatible_for_expected(&value.ty(), expected_ty) {
        return Ok(value);
    }
    if let Some(cast) = try_int_literal_cast_value(&value, expected_ty) {
        return Ok(cast);
    }
    if let Some(cast) = try_bool_to_number_cast_value(&value, expected_ty) {
        return Ok(cast);
    }
    if let Some(cast) = try_neutral_cast_value(&value, expected_ty) {
        return Ok(cast);
    }
    Err(value)
}

fn zero_value_for_type(ty: &Type) -> Option<ValueExpr> {
    if ty == &Type::Bool {
        return Some(ValueExpr::Bool(false));
    }
    neutral_kind_for_type(ty, NeutralKind::Zero).map(|kind| ValueExpr::Neutral {
        kind,
        ty: ty.clone(),
    })
}

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

fn ensure_lift_param_type(expr: &ValueExpr, name: &str, expected: &Type) -> Result<(), Error> {
    if let Some(actual) = lifted_param_type(expr, name)? {
        ensure_type(&actual, expected, "function parameter")?;
    }
    Ok(())
}

fn lifted_param_type(expr: &ValueExpr, name: &str) -> Result<Option<Type>, Error> {
    let mut ty = None;
    collect_lifted_param_type(expr, name, &mut ty)?;
    Ok(ty)
}

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
        ValueExpr::BoolToNumberCast { value, .. } => {
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
            if has_category(ty, AlgebraicCategory::Grp) {
                Some(NeutralKind::Identity)
            } else if matches!(ty, Type::Mat(rows, columns) if rows == columns) {
                Some(NeutralKind::Identity)
            } else {
                None
            }
        }
    }
}

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

fn parse_unit_vector_basis_name(name: &str) -> Option<(usize, usize)> {
    let suffixes = parse_braced_usize_suffixes(name, "e")?;
    let [dimension, index] = suffixes.as_slice() else {
        return None;
    };
    (*dimension > 0).then_some((*dimension, *index))
}

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
        _ => Err(Error::new(format!(
            "ambiguous overload for '{}' with provided argument(s)",
            name
        ))),
    }
}

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

fn is_monoid_pow_type(ty: &Type) -> bool {
    is_value_type(ty) && has_category(ty, AlgebraicCategory::Mon)
}

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

    candidates.sort_by_key(|(cost, _, _)| *cost);
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
    if candidates
        .iter()
        .skip(1)
        .any(|(cost, _, _)| *cost == best_cost)
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

fn call_arg_cost(actual: &Type, expected: &Type, arg: &ValueExpr) -> usize {
    usize::from(!types_match(actual, expected))
        + usize::from(matches!(arg, ValueExpr::Neutral { .. }))
}

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
}

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

fn vector_dimension(ty: &Type) -> Option<usize> {
    match ty {
        Type::Vec2 => Some(2),
        Type::Vec3 => Some(3),
        Type::Vec4 => Some(4),
        _ => None,
    }
}

fn vector_type(dimension: usize) -> Type {
    match dimension {
        2 => Type::Vec2,
        3 => Type::Vec3,
        4 => Type::Vec4,
        _ => unreachable!(),
    }
}

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

fn derivative_dimension(ty: &Type) -> Option<usize> {
    match ty {
        Type::Float => Some(1),
        Type::Vec2 => Some(2),
        Type::Vec3 => Some(3),
        Type::Vec4 => Some(4),
        _ => None,
    }
}

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

fn divergence_dimension(ty: &Type) -> Option<usize> {
    match ty {
        Type::Vec2 => Some(2),
        Type::Vec3 => Some(3),
        Type::Vec4 => Some(4),
        _ => None,
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
        ValueExpr::Var {
            name: param_name.to_string(),
            ty: Type::Float,
            array_len: None,
        },
    ))
}

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

fn infer_function_expr_for_type(
    expr: &Expr,
    env: &Env<'_>,
    expected_input: &Type,
    expected_output: &Type,
) -> Result<FunctionExpr, Error> {
    if let Expr::Operator(op) = expr {
        return infer_operator_function_expr_for_type(*op, expected_input, expected_output);
    }
    if let Expr::Tuple(items) = expr {
        if let Some(count) = vector_dimension(expected_output) {
            if count == items.len() {
                let funcs = items
                    .iter()
                    .map(|item| {
                        infer_function_expr_for_type(item, env, expected_input, &Type::Float)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                return Ok(FunctionExpr {
                    input: expected_input.clone(),
                    output: expected_output.clone(),
                    kind: FunctionExprKind::ProductSameDomain(funcs),
                });
            }
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
                    captures: env.scene_input_values(),
                },
            }])
        }
        Expr::Binary {
            op: BinOp::Compose,
            left,
            right,
        } => {
            let outers = infer_function_expr_candidates(left, env)?;
            let inners = infer_function_expr_candidates(right, env)?;
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
        Expr::Binary { op, left, right } => {
            infer_pointwise_binary_function_candidates(*op, left, right, env)
        }
        _ => Err(Error::new(
            "function composition currently only supports named unary functions",
        )),
    }
}

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

fn operator_candidate_input_type(left: &Type, right: &Type) -> Type {
    Type::Product(vec![left.clone(), right.clone()])
}

fn operator_input_types(input: &Type) -> Option<[Type; 2]> {
    match input {
        Type::Vec2 => Some([Type::Float, Type::Float]),
        Type::Product(parts) if parts.len() == 2 => Some([parts[0].clone(), parts[1].clone()]),
        _ => None,
    }
}

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

fn can_cast_function_output_to_expected(output: &Type, expected: &Type) -> bool {
    output == &Type::Bool && matches!(expected, Type::Float | Type::Int)
}

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

fn infer_tensor_function_product_candidates(
    left: &Expr,
    right: &Expr,
    env: &Env<'_>,
) -> Result<Vec<FunctionExpr>, Error> {
    let left = infer_function_expr(left, env)?;
    let right = infer_function_expr(right, env)?;
    if left.input != Type::Float || right.input != Type::Float {
        return Err(Error::new(
            "function product syntax currently supports scalar domains",
        ));
    }
    if left.output != Type::Float || right.output != Type::Float {
        return Err(Error::new(
            "function product syntax currently supports scalar codomains",
        ));
    }
    Ok(vec![FunctionExpr {
        input: Type::Vec2,
        output: Type::Vec2,
        kind: FunctionExprKind::ProductTensor(Box::new(left), Box::new(right)),
    }])
}

fn scalar_product_output<'a>(parts: impl Iterator<Item = &'a Type>) -> Result<Type, Error> {
    let count = parts
        .map(scalar_product_part_len)
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| Error::new("function products currently require scalar codomains"))?
        .into_iter()
        .sum::<usize>();
    match count {
        2 => Ok(Type::Vec2),
        3 => Ok(Type::Vec3),
        4 => Ok(Type::Vec4),
        _ => Err(Error::new(
            "function products currently support R2, R3, and R4 codomains",
        )),
    }
}

fn scalar_product_part_len(ty: &Type) -> Option<usize> {
    match ty {
        Type::Float => Some(1),
        Type::Product(parts) if parts.iter().all(|part| part == &Type::Float) => Some(parts.len()),
        _ => None,
    }
}

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

fn types_equivalent(left: &Type, right: &Type) -> bool {
    types_match(
        &normalize_scalar_product_type(left),
        &normalize_scalar_product_type(right),
    )
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
    let Some(info) = env.get_value(name).cloned() else {
        if let Some(dimension) = parse_identity_matrix_name(name) {
            return Ok(ValueExpr::Neutral {
                kind: NeutralKind::Identity,
                ty: Type::Mat(dimension, dimension),
            });
        }
        if parse_matrix_basis_name(name).is_some() {
            return Err(Error::new(format!(
                "matrix basis literal '{}' needs an expected matrix type",
                name
            )));
        }
        if parse_unit_vector_basis_name(name).is_some() {
            return Err(Error::new(format!(
                "unit vector literal '{}' needs an expected vector type",
                name
            )));
        }
        if lift_param.is_none() {
            return Err(Error::new(format!(
                "function '{}' needs an explicit call outside function bodies",
                name
            )));
        }
        let param_name = lift_param.unwrap().to_string();
        let overloads = env
            .function_overloads(name)
            .ok_or_else(|| Error::new(format!("unknown identifier '{}'", name)))?;
        let mut candidates = Vec::new();
        for overload in overloads {
            let Ok((inputs, output)) = call_inputs_and_output(&overload.ty) else {
                continue;
            };
            if inputs.len() == 1 && inputs[0] == Type::Float {
                candidates.push((output, inputs[0].clone()));
            }
        }
        if candidates.len() != 1 {
            return Err(Error::new(format!(
                "ambiguous function '{}' cannot be lifted implicitly",
                name
            )));
        }
        let (output, input) = candidates.remove(0);
        return Ok(ValueExpr::Call {
            func: name.to_string(),
            args: vec![ValueExpr::Var {
                name: param_name,
                ty: input,
                array_len: None,
            }],
            ty: output,
        });
    };
    match info.ty {
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
        | Type::Array(_) => Ok(ValueExpr::Var {
            name: name.to_string(),
            ty: info.ty,
            array_len: info.array_len,
        }),
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
                args: vec![ValueExpr::Var {
                    name: param_name,
                    ty: Type::Float,
                    array_len: None,
                }],
                ty: (*output).clone(),
            })
        }
        Type::Object | Type::Object2D | Type::Product(_) => Err(Error::new(format!(
            "object '{}' is not a value expression",
            name
        ))),
    }
}

fn infer_rot_builtin(
    callee: &Expr,
    args: &[Expr],
    env: &Env<'_>,
    lift_param: Option<&str>,
) -> Result<Option<ValueExpr>, Error> {
    let Expr::Ident(name) = callee else {
        return Ok(None);
    };

    if name == "rot2D" && args.len() == 2 {
        let anchor = infer_value_expr_for_type(&args[0], &Type::Vec2, env, lift_param)?;
        let angle = infer_value_expr(&args[1], env, lift_param)?;
        ensure_type(&anchor.ty(), &Type::Vec2, "rot2D anchor")?;
        ensure_type(&angle.ty(), &Type::Float, "rot2D angle")?;
        return Ok(Some(ValueExpr::Call {
            func: "rot2D".to_string(),
            args: vec![anchor, angle],
            ty: Type::Isom2,
        }));
    }

    if name != "rot" || args.len() != 3 {
        return Ok(None);
    }

    let binormal = infer_value_expr_for_type(&args[0], &Type::Vec3, env, lift_param)?;
    let anchor = infer_value_expr_for_type(&args[1], &Type::Vec3, env, lift_param)?;
    let angle = infer_value_expr(&args[2], env, lift_param)?;
    ensure_type(&binormal.ty(), &Type::Vec3, "rot binormal")?;
    ensure_type(&anchor.ty(), &Type::Vec3, "rot anchor")?;
    ensure_type(&angle.ty(), &Type::Float, "rot angle")?;

    Ok(Some(ValueExpr::Call {
        func: "rot".to_string(),
        args: vec![binormal, anchor, angle],
        ty: Type::Isom3,
    }))
}

fn infer_complex_overload_call(
    name: &str,
    args: &[Expr],
    env: &Env<'_>,
    lift_param: Option<&str>,
) -> Result<Option<ValueExpr>, Error> {
    let Some(func) = complex_overload_name(name) else {
        return Ok(None);
    };
    if args.len() != 1 {
        return Ok(None);
    }
    let arg = infer_value_expr(&args[0], env, lift_param)?;
    if arg.ty() != Type::Complex {
        return Ok(None);
    }
    Ok(Some(ValueExpr::Call {
        func: func.to_string(),
        args: vec![arg],
        ty: Type::Complex,
    }))
}

fn infer_binary_type(op: BinOp, left: &Type, right: &Type) -> Result<Type, Error> {
    if left == right && matches!(op, BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div) {
        let category = match op {
            BinOp::Add | BinOp::Sub => AlgebraicCategory::Ab,
            BinOp::Mul => AlgebraicCategory::Mon,
            BinOp::Div => AlgebraicCategory::DivRing,
            BinOp::Eq
            | BinOp::Ne
            | BinOp::Lt
            | BinOp::Le
            | BinOp::Gt
            | BinOp::Ge
            | BinOp::Product
            | BinOp::Compose => unreachable!(),
        };
        if has_category(left, category) {
            return Ok(left.clone());
        }
    }

    if matches!(op, BinOp::Eq | BinOp::Ne) && left == right && is_equality_comparable_type(left) {
        return Ok(Type::Bool);
    }

    if matches!(op, BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge)
        && left == right
        && is_ordered_comparable_type(left)
    {
        return Ok(Type::Bool);
    }

    if left == &Type::Bool {
        if let Some(cast_ty) = bool_numeric_cast_type_for_binary(right) {
            return infer_binary_type(op, &cast_ty, right);
        }
    }

    if right == &Type::Bool {
        if let Some(cast_ty) = bool_numeric_cast_type_for_binary(left) {
            return infer_binary_type(op, left, &cast_ty);
        }
    }

    if has_category(left, AlgebraicCategory::RAlg) && right == &Type::Float {
        return Ok(left.clone());
    }

    if left == &Type::Float && has_category(right, AlgebraicCategory::RAlg) {
        return Ok(right.clone());
    }

    if op == BinOp::Mul && left == &Type::Isom3 && right == &Type::Vec3 {
        return Ok(Type::Vec3);
    }

    if op == BinOp::Mul && left == &Type::Isom2 && right == &Type::Vec2 {
        return Ok(Type::Vec2);
    }

    if matches!(op, BinOp::Mul | BinOp::Div)
        && has_category(left, AlgebraicCategory::RVect)
        && right == &Type::Float
    {
        return Ok(left.clone());
    }

    if op == BinOp::Mul && left == &Type::Float && has_category(right, AlgebraicCategory::RVect) {
        return Ok(right.clone());
    }

    Err(Error::new(format!(
        "unsupported operands for binary operator: {} {} {}",
        format_type(left),
        op.symbol(),
        format_type(right)
    )))
}

fn is_equality_comparable_type(ty: &Type) -> bool {
    matches!(ty, Type::Bool | Type::Float | Type::Int)
}

fn is_ordered_comparable_type(ty: &Type) -> bool {
    matches!(ty, Type::Float | Type::Int)
}

fn bool_numeric_cast_type_for_binary(other: &Type) -> Option<Type> {
    if other == &Type::Int {
        Some(Type::Int)
    } else if other == &Type::Float
        || has_category(other, AlgebraicCategory::RVect)
        || has_category(other, AlgebraicCategory::RAlg)
    {
        Some(Type::Float)
    } else {
        None
    }
}

fn try_int_literal_cast_value(value: &ValueExpr, expected_ty: &Type) -> Option<ValueExpr> {
    if expected_ty != &Type::Int {
        return None;
    }
    let ValueExpr::Float(value) = value else {
        return None;
    };
    let rounded = value.round();
    if (value - rounded).abs() < f64::EPSILON {
        Some(ValueExpr::Int(rounded as i64))
    } else {
        None
    }
}

fn try_bool_to_number_cast_value(value: &ValueExpr, expected_ty: &Type) -> Option<ValueExpr> {
    if !matches!(expected_ty, Type::Float | Type::Int) || value.ty() != Type::Bool {
        return None;
    }
    Some(ValueExpr::BoolToNumberCast {
        value: Box::new(value.clone()),
        ty: expected_ty.clone(),
    })
}

fn try_neutral_cast_value(value: &ValueExpr, expected_ty: &Type) -> Option<ValueExpr> {
    let kind = match value {
        ValueExpr::Float(value) if (*value - 0.0).abs() < f64::EPSILON => NeutralKind::Zero,
        ValueExpr::Float(value) if (*value - 1.0).abs() < f64::EPSILON => NeutralKind::One,
        ValueExpr::Neutral { .. } => return None,
        _ => return None,
    };
    neutral_kind_for_type(expected_ty, kind).map(|kind| ValueExpr::Neutral {
        kind,
        ty: expected_ty.clone(),
    })
}

fn zero_vec2() -> ValueExpr {
    ValueExpr::Vec2(
        Box::new(ValueExpr::Float(0.0)),
        Box::new(ValueExpr::Float(0.0)),
    )
}

fn zero_vec3() -> ValueExpr {
    ValueExpr::Vec3(
        Box::new(ValueExpr::Float(0.0)),
        Box::new(ValueExpr::Float(0.0)),
        Box::new(ValueExpr::Float(0.0)),
    )
}

fn ambient_vector_type(dimension: ShapeDimension) -> Type {
    match dimension {
        ShapeDimension::D2 => Type::Vec2,
        ShapeDimension::D3 => Type::Vec3,
    }
}

fn ambient_identity_matrix(dimension: ShapeDimension) -> ValueExpr {
    match dimension {
        ShapeDimension::D2 => identity_mat2(),
        ShapeDimension::D3 => identity_mat3(),
    }
}

fn identity_mat2() -> ValueExpr {
    ValueExpr::Matrix {
        columns: 2,
        rows: vec![
            ValueExpr::Vec2(
                Box::new(ValueExpr::Float(1.0)),
                Box::new(ValueExpr::Float(0.0)),
            ),
            ValueExpr::Vec2(
                Box::new(ValueExpr::Float(0.0)),
                Box::new(ValueExpr::Float(1.0)),
            ),
        ],
    }
}

fn unit_z_vec3() -> ValueExpr {
    ValueExpr::Vec3(
        Box::new(ValueExpr::Float(0.0)),
        Box::new(ValueExpr::Float(0.0)),
        Box::new(ValueExpr::Float(1.0)),
    )
}

fn identity_mat3() -> ValueExpr {
    ValueExpr::Matrix {
        columns: 3,
        rows: vec![
            ValueExpr::Vec3(
                Box::new(ValueExpr::Float(1.0)),
                Box::new(ValueExpr::Float(0.0)),
                Box::new(ValueExpr::Float(0.0)),
            ),
            ValueExpr::Vec3(
                Box::new(ValueExpr::Float(0.0)),
                Box::new(ValueExpr::Float(1.0)),
                Box::new(ValueExpr::Float(0.0)),
            ),
            ValueExpr::Vec3(
                Box::new(ValueExpr::Float(0.0)),
                Box::new(ValueExpr::Float(0.0)),
                Box::new(ValueExpr::Float(1.0)),
            ),
        ],
    }
}
