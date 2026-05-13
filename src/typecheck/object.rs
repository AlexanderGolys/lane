// Type-checks Lane object expressions denoting SDF objects.
// Object inference is separated because primitive constructors, object operators, transforms, and metadata live on the SDF/object side rather than the scalar value side.
// It is part of semantic analysis and produces typed object IR used by GLSL emission.

/// Type-checks helper logic for infer_object_expr.
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
        | Expr::Conditional { .. }
        | Expr::Unary { .. } => Err(Error::new("expected an Object expression")),
        Expr::Binary { .. } => Err(Error::new("unsupported object expression")),
    }
}

/// Type-checks helper logic for object_dimension.
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
            "_op_revolution" | "_op_extrusion" => Some(ShapeDimension::D3),
            _ => object_args
                .first()
                .and_then(|object| object_dimension(object, env)),
        },
    }
}

/// Type-checks helper logic for infer_segment_length_constructor.
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

/// Type-checks helper logic for infer_object_call.
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
        Ok(registered_object_expr(
            op.name,
            op.glsl_name,
            value_args,
            object_args,
        ))
    }
}

/// Type-checks helper logic for object_op_output_dimension.
fn object_op_output_dimension(op: &ObjectOpDef) -> ShapeDimension {
    match op.glsl_name {
        "_op_revolution" | "_op_extrusion" | "_op_rot" => ShapeDimension::D3,
        _ => ShapeDimension::D2,
    }
}

/// Type-checks helper logic for infer_rotation_object_call.
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

    Ok(registered_object_expr(op.name, op.glsl_name, value_args, object_args))
}

fn registered_object_expr(
    name: &str,
    glsl_name: &str,
    value_args: Vec<ValueExpr>,
    object_args: Vec<ObjectExpr>,
) -> ObjectExpr {
    ObjectExpr::RegisteredOp {
        name: name.to_string(),
        glsl_name: glsl_name.to_string(),
        value_args,
        object_args,
    }
}

/// Type-checks helper logic for ensure_object_op_arg_dimensions.
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

/// Type-checks helper logic for rotation_value_arg_count.
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

/// Type-checks helper logic for infer_rot_value_args.
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

/// Type-checks helper logic for infer_rot2d_value_args.
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

/// Type-checks helper logic for fold_associative_registered_op.
fn fold_associative_registered_op(
    name: &str,
    glsl_name: &str,
    value_args: &[ValueExpr],
    object_args: &[ObjectExpr],
) -> ObjectExpr {
    debug_assert!(object_args.len() >= 2);
    if object_args.len() == 2 {
        return registered_object_expr(
            name,
            glsl_name,
            value_args.to_vec(),
            object_args.to_vec(),
        );
    }
    if object_args.len() == 3 {
        let right = fold_associative_registered_op(name, glsl_name, value_args, &object_args[1..]);
        return registered_object_expr(
            name,
            glsl_name,
            value_args.to_vec(),
            vec![object_args[0].clone(), right],
        );
    }

    let split = object_args.len() / 2;
    let left = fold_associative_registered_op(name, glsl_name, value_args, &object_args[..split]);
    let right = fold_associative_registered_op(name, glsl_name, value_args, &object_args[split..]);
    registered_object_expr(
        name,
        glsl_name,
        value_args.to_vec(),
        vec![left, right],
    )
}
