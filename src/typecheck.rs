use super::*;

impl TypedProgram {
    pub(super) fn from_program(program: &Program, registry: &Registry) -> Result<Self, Error> {
        let mut env = Env::new(registry);

        for input in &program.inputs {
            env.insert(input.name.clone(), input.ty.clone())?;
        }

        for func in &program.funcs {
            env.insert(func.name.clone(), func.ty.clone())?;
        }

        for binding in &program.value_bindings {
            env.insert(binding.name.clone(), binding.ty.clone())?;
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
            if !matches!(
                output_ty,
                Type::Float
                    | Type::Int
                    | Type::Complex
                    | Type::Quat
                    | Type::Vec2
                    | Type::Vec3
                    | Type::Vec4
                    | Type::Mat(_, _)
            ) {
                return Err(Error::new(format!(
                    "function '{}' currently only supports scalar, vector, or matrix outputs",
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

        let mut typed_value_bindings = Vec::new();
        for binding in &program.value_bindings {
            let expr = infer_value_expr(&binding.expr, &env, None)?;
            ensure_type(
                &expr.ty(),
                &binding.ty,
                &format!("binding '{}'", binding.name),
            )?;
            typed_value_bindings.push(TypedValueBinding {
                name: binding.name.clone(),
                ty: binding.ty.clone(),
                expr,
            });
        }

        let mut typed_bindings = Vec::new();
        for binding in &program.bindings {
            ensure_type(
                &binding.ty,
                &Type::Object,
                &format!("binding '{}'", binding.name),
            )?;
            let expr = infer_object_expr(&binding.expr, &env)?;
            typed_bindings.push(TypedBinding {
                name: binding.name.clone(),
                expr,
                generated: binding.generated,
            });
        }

        let output = infer_object_expr(&program.output.expr, &env)?;

        Ok(Self {
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
    types: HashMap<String, Type>,
}

impl<'a> Env<'a> {
    fn new(registry: &'a Registry) -> Self {
        let mut types = HashMap::new();
        for (name, func) in &registry.value_funcs {
            types.insert((*name).to_string(), func.ty.clone());
        }
        for op in registry.object_ops.values() {
            types.insert(op.name.to_string(), object_op_type(op));
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
            ensure_type(ty, &Type::Object, &format!("identifier '{}'", name))?;
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
                        let typed = infer_value_expr(&value.1, env, None)?;
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
            let offset = infer_value_expr(right, env, None)?;
            ensure_type(&offset.ty(), &Type::Vec3, "object shift")?;
            Ok(ObjectExpr::AmbientTransform {
                object: Box::new(object),
                translation: offset,
                linear: identity_mat3(),
            })
        }
        Expr::Binary {
            op: BinOp::Mul,
            left,
            right,
        } => {
            let linear = infer_value_expr(left, env, None)?;
            ensure_type(&linear.ty(), &Type::Mat(3, 3), "object action")?;
            let object = infer_object_expr(right, env)?;
            Ok(ObjectExpr::AmbientTransform {
                object: Box::new(object),
                translation: zero_vec3(),
                linear,
            })
        }
        Expr::Call { .. } => infer_object_call(expr, env),
        Expr::Number(_) | Expr::Tuple(_) => Err(Error::new("expected an Object expression")),
        Expr::Binary { .. } => Err(Error::new("unsupported object expression")),
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
        Expr::Number(value) => Ok(ValueExpr::Float(*value)),
        Expr::Ident(name) => infer_identifier_value(name, env, lift_param),
        Expr::Tuple(items) => infer_tuple_value_expr(items, env, lift_param),
        Expr::Call { callee, args } => {
            if let Some(result) = infer_rot_point_builtin(callee, args, env, lift_param)? {
                return Ok(result);
            }
            if let Some(result) = infer_differential_builtin(expr, env, lift_param)? {
                return Ok(result);
            }
            let name = match &**callee {
                Expr::Ident(name) => name,
                _ => return Err(Error::new("only named value functions are supported")),
            };
            let mut current_ty = env
                .get(name)
                .cloned()
                .ok_or_else(|| Error::new(format!("unknown function '{}'", name)))?;
            let mut typed_args = Vec::new();
            let mut index = 0;
            while index < args.len() {
                let (input_ty, output_ty) = match current_ty {
                    Type::Func(input, output) => (*input, *output),
                    _ => {
                        return Err(Error::new(format!(
                            "'{}' is not callable with more arguments",
                            name
                        )))
                    }
                };
                match input_ty {
                    Type::Product(items) => {
                        if args.len() - index < items.len() {
                            return Err(Error::new(format!(
                                "call '{}(...)' expected {} argument(s), got {}",
                                name,
                                items.len(),
                                args.len() - index
                            )));
                        }
                        for expected_ty in items {
                            let typed_arg = infer_value_expr(&args[index], env, lift_param)?;
                            ensure_type(
                                &typed_arg.ty(),
                                &expected_ty,
                                &format!("call '{}(...)'", name),
                            )?;
                            typed_args.push(typed_arg);
                            index += 1;
                        }
                    }
                    input_ty => {
                        let typed_arg = infer_value_expr(&args[index], env, lift_param)?;
                        ensure_type(&typed_arg.ty(), &input_ty, &format!("call '{}(...)'", name))?;
                        typed_args.push(typed_arg);
                        index += 1;
                    }
                }
                current_ty = output_ty;
            }

            match current_ty {
                Type::Float
                | Type::Int
                | Type::Complex
                | Type::Quat
                | Type::Vec2
                | Type::Vec3
                | Type::Vec4
                | Type::Mat(_, _) => Ok(ValueExpr::Call {
                    func: name.clone(),
                    args: typed_args,
                    ty: current_ty,
                }),
                Type::Object | Type::Product(_) | Type::Func(_, _) => Err(Error::new(format!(
                    "value expression '{}' does not return a value type",
                    name
                ))),
            }
        }
        Expr::Binary {
            op: BinOp::Compose,
            left,
            right,
        } => infer_composed_value_expr(left, right, env, lift_param),
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
        Expr::Constructor { name, args } => match args {
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
        "partialX" => Some(infer_partial_builtin(&args, env, 0)?),
        "partialY" => Some(infer_partial_builtin(&args, env, 1)?),
        "partialZ" => Some(infer_partial_builtin(&args, env, 2)?),
        "directionalDerivative" => Some(infer_directional_derivative_builtin(&args, env)?),
        "gradient" => Some(infer_gradient_builtin(&args, env)?),
        "divergence" => Some(infer_divergence_builtin(&args, env)?),
        _ => None,
    };

    Ok(result)
}

fn infer_derivative_builtin(
    args: &[&Expr],
    env: &Env<'_>,
    lift_param: Option<&str>,
) -> Result<ValueExpr, Error> {
    if args.len() != 3 && !(args.len() == 2 && lift_param.is_some()) {
        return Err(Error::new(
            "derivative expects epsilon, a unary function, and an evaluation point",
        ));
    }
    let epsilon = infer_value_expr(args[0], env, None)?;
    ensure_type(&epsilon.ty(), &Type::Float, "derivative epsilon")?;
    let func = infer_function_expr(args[1], env)?;
    if func.input != Type::Float || func.output != Type::Float {
        return Err(Error::new("derivative expects a func(float -> float)"));
    }
    let at = if let Some(expr) = args.get(2) {
        let at = infer_value_expr(expr, env, None)?;
        ensure_type(&at.ty(), &Type::Float, "derivative evaluation point")?;
        at
    } else {
        ValueExpr::Var(lift_param.unwrap().to_string(), Type::Float)
    };
    Ok(ValueExpr::Derivative {
        epsilon: Box::new(epsilon),
        func,
        at: Box::new(at),
    })
}

fn infer_partial_builtin(args: &[&Expr], env: &Env<'_>, axis: usize) -> Result<ValueExpr, Error> {
    if args.len() != 3 {
        return Err(Error::new(
            "partial derivative expects epsilon, a scalar field, and an evaluation point",
        ));
    }
    let epsilon = infer_value_expr(args[0], env, None)?;
    ensure_type(&epsilon.ty(), &Type::Float, "partial derivative epsilon")?;
    let func = infer_function_expr(args[1], env)?;
    if func.input != Type::Vec3 || func.output != Type::Float {
        return Err(Error::new(
            "partial derivatives currently expect a func(vec3 -> float)",
        ));
    }
    let at = infer_value_expr(args[2], env, None)?;
    ensure_type(&at.ty(), &Type::Vec3, "partial derivative evaluation point")?;
    Ok(ValueExpr::Partial {
        axis,
        epsilon: Box::new(epsilon),
        func,
        at: Box::new(at),
    })
}

fn infer_directional_derivative_builtin(args: &[&Expr], env: &Env<'_>) -> Result<ValueExpr, Error> {
    if args.len() != 4 {
        return Err(Error::new(
            "directionalDerivative expects epsilon, a direction, a scalar field, and an evaluation point",
        ));
    }
    let epsilon = infer_value_expr(args[0], env, None)?;
    ensure_type(
        &epsilon.ty(),
        &Type::Float,
        "directional derivative epsilon",
    )?;
    let direction = infer_value_expr(args[1], env, None)?;
    ensure_type(
        &direction.ty(),
        &Type::Vec3,
        "directional derivative direction",
    )?;
    let func = infer_function_expr(args[2], env)?;
    if func.input != Type::Vec3 || func.output != Type::Float {
        return Err(Error::new(
            "directionalDerivative currently expects a func(vec3 -> float)",
        ));
    }
    let at = infer_value_expr(args[3], env, None)?;
    ensure_type(
        &at.ty(),
        &Type::Vec3,
        "directional derivative evaluation point",
    )?;
    Ok(ValueExpr::DirectionalDerivative {
        epsilon: Box::new(epsilon),
        direction: Box::new(direction),
        func,
        at: Box::new(at),
    })
}

fn infer_gradient_builtin(args: &[&Expr], env: &Env<'_>) -> Result<ValueExpr, Error> {
    if args.len() != 3 {
        return Err(Error::new(
            "gradient expects epsilon, a scalar field, and an evaluation point",
        ));
    }
    let epsilon = infer_value_expr(args[0], env, None)?;
    ensure_type(&epsilon.ty(), &Type::Float, "gradient epsilon")?;
    let func = infer_function_expr(args[1], env)?;
    if func.input != Type::Vec3 || func.output != Type::Float {
        return Err(Error::new(
            "gradient currently expects a func(vec3 -> float)",
        ));
    }
    let at = infer_value_expr(args[2], env, None)?;
    ensure_type(&at.ty(), &Type::Vec3, "gradient evaluation point")?;
    Ok(ValueExpr::Gradient {
        epsilon: Box::new(epsilon),
        func,
        at: Box::new(at),
    })
}

fn infer_divergence_builtin(args: &[&Expr], env: &Env<'_>) -> Result<ValueExpr, Error> {
    if args.len() != 3 {
        return Err(Error::new(
            "divergence expects epsilon, a vector field, and an evaluation point",
        ));
    }
    let epsilon = infer_value_expr(args[0], env, None)?;
    ensure_type(&epsilon.ty(), &Type::Float, "divergence epsilon")?;
    let func = infer_function_expr(args[1], env)?;
    if func.input != Type::Vec3 || func.output != Type::Vec3 {
        return Err(Error::new(
            "divergence currently expects a func(vec3 -> vec3)",
        ));
    }
    let at = infer_value_expr(args[2], env, None)?;
    ensure_type(&at.ty(), &Type::Vec3, "divergence evaluation point")?;
    Ok(ValueExpr::Divergence {
        epsilon: Box::new(epsilon),
        func,
        at: Box::new(at),
    })
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
        ValueExpr::Var(param_name.to_string(), Type::Float),
    ))
}

fn infer_function_expr(expr: &Expr, env: &Env<'_>) -> Result<FunctionExpr, Error> {
    match expr {
        Expr::Ident(name) => {
            let ty = env
                .get(name)
                .cloned()
                .ok_or_else(|| Error::new(format!("unknown identifier '{}'", name)))?;
            match ty {
                Type::Func(input, output) => Ok(FunctionExpr {
                    input: (*input).clone(),
                    output: (*output).clone(),
                    kind: FunctionExprKind::Named(name.clone()),
                }),
                Type::Float
                | Type::Int
                | Type::Complex
                | Type::Quat
                | Type::Vec2
                | Type::Vec3
                | Type::Vec4
                | Type::Mat(_, _) => {
                    Err(Error::new(format!("'{}' is a value, not a function", name)))
                }
                Type::Object | Type::Product(_) => Err(Error::new(format!(
                    "object '{}' is not a function expression",
                    name
                ))),
            }
        }
        Expr::Binary {
            op: BinOp::Compose,
            left,
            right,
        } => {
            let outer = infer_function_expr(left, env)?;
            let inner = infer_function_expr(right, env)?;
            if inner.output != outer.input {
                return Err(Error::new(format!(
                    "cannot compose {} @ {} because {} does not match {}",
                    format_function_expr(left),
                    format_function_expr(right),
                    format_type(&inner.output),
                    format_type(&outer.input)
                )));
            }
            Ok(FunctionExpr {
                input: inner.input.clone(),
                output: outer.output.clone(),
                kind: FunctionExprKind::Compose(Box::new(outer), Box::new(inner)),
            })
        }
        _ => Err(Error::new(
            "function composition currently only supports named unary functions",
        )),
    }
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
    let ty = env
        .get(name)
        .cloned()
        .ok_or_else(|| Error::new(format!("unknown identifier '{}'", name)))?;
    match ty {
        Type::Float
        | Type::Int
        | Type::Complex
        | Type::Quat
        | Type::Vec2
        | Type::Vec3
        | Type::Vec4
        | Type::Mat(_, _) => Ok(ValueExpr::Var(name.to_string(), ty)),
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
        Type::Object | Type::Product(_) => Err(Error::new(format!(
            "object '{}' is not a value expression",
            name
        ))),
    }
}

fn infer_rot_point_builtin(
    callee: &Expr,
    args: &[Expr],
    env: &Env<'_>,
    lift_param: Option<&str>,
) -> Result<Option<ValueExpr>, Error> {
    if !matches!(callee, Expr::Ident(name) if name == "rot") {
        return Ok(None);
    }
    if args.len() != 4 {
        return Ok(None);
    }

    let point = infer_value_expr(&args[0], env, lift_param)?;
    let binormal = infer_value_expr(&args[1], env, lift_param)?;
    let anchor = infer_value_expr(&args[2], env, lift_param)?;
    let angle = infer_value_expr(&args[3], env, lift_param)?;
    ensure_type(&point.ty(), &Type::Vec3, "rot point")?;
    ensure_type(&binormal.ty(), &Type::Vec3, "rot binormal")?;
    ensure_type(&anchor.ty(), &Type::Vec3, "rot anchor")?;
    ensure_type(&angle.ty(), &Type::Float, "rot angle")?;

    Ok(Some(ValueExpr::Call {
        func: "rot_point".to_string(),
        args: vec![point, binormal, anchor, angle],
        ty: Type::Vec3,
    }))
}

fn infer_binary_type(op: BinOp, left: &Type, right: &Type) -> Result<Type, Error> {
    if left == right {
        let category = match op {
            BinOp::Add | BinOp::Sub => AlgebraicCategory::Ab,
            BinOp::Mul => AlgebraicCategory::Mon,
            BinOp::Div => AlgebraicCategory::Field,
            BinOp::Compose => unreachable!(),
        };
        if has_category(left, category) {
            return Ok(left.clone());
        }
    }

    if has_category(left, AlgebraicCategory::AlgR) && right == &Type::Float {
        return Ok(left.clone());
    }

    if left == &Type::Float && has_category(right, AlgebraicCategory::AlgR) {
        return Ok(right.clone());
    }

    if matches!(op, BinOp::Mul | BinOp::Div)
        && has_category(left, AlgebraicCategory::VectR)
        && right == &Type::Float
    {
        return Ok(left.clone());
    }

    if op == BinOp::Mul && left == &Type::Float && has_category(right, AlgebraicCategory::VectR) {
        return Ok(right.clone());
    }

    Err(Error::new(format!(
        "unsupported operands for binary operator: {} {} {}",
        format_type(left),
        op.symbol(),
        format_type(right)
    )))
}
fn zero_vec3() -> ValueExpr {
    ValueExpr::Vec3(
        Box::new(ValueExpr::Float(0.0)),
        Box::new(ValueExpr::Float(0.0)),
        Box::new(ValueExpr::Float(0.0)),
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
