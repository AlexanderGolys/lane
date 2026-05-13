use super::*;
include!("typecheck/raw_glsl.rs");

impl TypedProgram {
    /// Type-checks helper logic for from_program.
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

        for category_type in &program.category_types {
            validate_category_type_decl(category_type, &env)
                .map_err(|err| err.with_line(category_type.line))?;
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
                    | Type::Product(_)
            ) {
                return Err(Error::new(format!(
                    "function '{}' currently only supports scalar, vector, matrix, array, or product outputs",
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

        Ok(Self {
            ambient_dimension: program.ambient_dimension,
            gradient_epsilon: program.gradient_epsilon,
            product_types: program.product_types.clone(),
            category_types: program.category_types.clone(),
            inputs: program.inputs.clone(),
            funcs: typed_funcs,
            value_bindings: typed_value_bindings,
            bindings: typed_bindings,
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
    /// Type-checks helper logic for new.
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

    /// Type-checks helper logic for insert_value.
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

    /// Type-checks helper logic for insert_func.
    fn insert_func(&mut self, name: String, ty: Type) -> Result<(), Error> {
        self.insert_func_with_templates(name, ty, None, None)
    }

    /// Type-checks helper logic for insert_func_with_templates.
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

    /// Type-checks helper logic for get.
    fn get(&self, name: &str) -> Option<&Type> {
        self.values.get(name).map(|info| &info.ty).or_else(|| {
            self.funcs
                .get(name)
                .and_then(|funcs| (funcs.len() == 1).then_some(&funcs[0].ty))
        })
    }

    /// Type-checks helper logic for get_value.
    fn get_value(&self, name: &str) -> Option<&ValueInfo> {
        self.values.get(name)
    }

    /// Type-checks helper logic for function_overloads.
    fn function_overloads(&self, name: &str) -> Option<&[FunctionInfo]> {
        self.funcs.get(name).map(|funcs| funcs.as_slice())
    }

    /// Type-checks helper logic for has_binding.
    fn has_binding(&self, name: &str) -> bool {
        self.values.contains_key(name) || self.funcs.contains_key(name)
    }

    /// Type-checks helper logic for update_array_len.
    fn update_array_len(&mut self, name: &str, array_len: Option<usize>) {
        if let Some(info) = self.values.get_mut(name) {
            info.array_len = array_len;
        }
    }

    /// Type-checks helper logic for update_object_dimension.
    fn update_object_dimension(&mut self, name: &str, dimension: Option<ShapeDimension>) {
        if let Some(dimension) = dimension {
            self.object_dimensions.insert(name.to_string(), dimension);
        }
    }

    /// Type-checks helper logic for object_dimension.
    fn object_dimension(&self, name: &str) -> Option<ShapeDimension> {
        self.object_dimensions.get(name).copied()
    }

    /// Type-checks helper logic for product_type.
    fn product_type(&self, name: &str) -> Option<&ProductTypeDecl> {
        self.product_types.get(name)
    }
}

/// Type-checks helper logic for validate_product_type_decl.
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
        if !product_component_satisfies_category(component, decl.category) {
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

/// Validates required category declarations (`provided` operations, base type, and laws) before registration.
fn validate_category_type_decl(decl: &CategoryTypeDecl, env: &Env<'_>) -> Result<(), Error> {
    validate_user_type(&decl.base)?;
    if !has_category(&decl.base, AlgebraicCategory::Set) {
        return Err(Error::new(format!(
            "category type '{}' base {} does not satisfy Set",
            decl.name,
            format_type(&decl.base)
        )));
    }
    match decl.category {
        AlgebraicCategory::Ab => {
            let zero = require_category_op(&decl.ops.zero, &decl.name, "0")?;
            let add = require_category_op(&decl.ops.add, &decl.name, "+")?;
            let neg = require_category_op(&decl.ops.neg, &decl.name, "-")?;
            ensure_value_name_type(
                env,
                zero,
                &decl.base,
                &format!("category type '{}'", decl.name),
            )?;
            ensure_func_name_type(
                env,
                add,
                &Type::func(
                    Type::Product(vec![decl.base.clone(), decl.base.clone()]),
                    decl.base.clone(),
                ),
                &format!("category type '{}'", decl.name),
            )?;
            ensure_func_name_type(
                env,
                neg,
                &Type::func(decl.base.clone(), decl.base.clone()),
                &format!("category type '{}'", decl.name),
            )?;
        }
        _ => {
            return Err(Error::new(format!(
                "category type '{}' does not support category {} yet",
                decl.name,
                category_name(decl.category)
            )));
        }
    }
    Ok(())
}

// Validates a category declaration defines a required operation before use.
fn require_category_op<'a>(
    op: &'a Option<String>,
    type_name: &str,
    key: &str,
) -> Result<&'a str, Error> {
    op.as_deref().ok_or_else(|| {
        Error::new(format!(
            "category type '{}' requires operation '{}'",
            type_name, key
        ))
    })
}

/// Ensures a referenced value name exists in scope and matches the expected type.
fn ensure_value_name_type(
    env: &Env<'_>,
    name: &str,
    expected: &Type,
    context: &str,
) -> Result<(), Error> {
    let Some(value) = env.values.get(name) else {
        return Err(Error::new(format!(
            "{} references unknown value '{}'",
            context, name
        )));
    };
    ensure_type(
        &value.ty,
        expected,
        &format!("{} operation '{}'", context, name),
    )
}

/// Ensures a referenced function name exists and has a compatible signature for the call site.
fn ensure_func_name_type(
    env: &Env<'_>,
    name: &str,
    expected: &Type,
    context: &str,
) -> Result<(), Error> {
    let Some(funcs) = env.funcs.get(name) else {
        return Err(Error::new(format!(
            "{} references unknown function '{}'",
            context, name
        )));
    };
    if funcs
        .iter()
        .any(|func| types_compatible_for_expected(&func.ty, expected))
    {
        return Ok(());
    }
    Err(Error::new(format!(
        "{} operation '{}' expects {}",
        context,
        name,
        format_type(expected)
    )))
}

/// Type-checks helper logic for product_component_satisfies_category.
fn product_component_satisfies_category(component: &Type, category: AlgebraicCategory) -> bool {
    has_category(component, category)
        || (category == AlgebraicCategory::Grp && has_category(component, AlgebraicCategory::Ab))
}

/// Type-checks helper logic for product_category_supported.
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

/// Type-checks helper logic for object_type_dimension.
fn object_type_dimension(ty: &Type) -> Option<ShapeDimension> {
    match ty {
        Type::Object2D => Some(ShapeDimension::D2),
        _ => None,
    }
}

/// Type-checks helper logic for object_type_for_dimension.
fn object_type_for_dimension(dimension: Option<ShapeDimension>) -> Type {
    match dimension {
        Some(ShapeDimension::D2) => Type::Object2D,
        _ => Type::Object,
    }
}

/// Type-checks helper logic for function_domain_and_output.
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

// Object inference lives in a separate file to keep this pass navigable.
include!("typecheck/object.rs");

// Value inference covers scalar, vector, tuple, constructor, and builtin calls.
include!("typecheck/value.rs");

// Function inference covers typed function expressions and pointwise candidates.
include!("typecheck/function.rs");

/// Type-checks helper logic for infer_identifier_value.
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
        let param_name = lift_param_name(lift_param, name)?;
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
        | Type::Power(_, _)
        | Type::Array(_)
        | Type::Product(_) => Ok(ValueExpr::Var {
            name: name.to_string(),
            ty: info.ty,
            array_len: info.array_len,
        }),
        Type::Func(input, output) => {
            if *input != Type::Float {
                return Err(Error::new(format!(
                    "function '{}' cannot be lifted implicitly",
                    name
                )));
            }
            let param_name = lift_param_name(lift_param, name)?;
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
        Type::Object | Type::Object2D => Err(Error::new(format!(
            "object '{}' is not a value expression",
            name
        ))),
    }
}

/// Type-checks helper logic for infer_rot_builtin.
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

/// Type-checks helper logic for infer_complex_overload_call.
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

/// Type-checks helper logic for infer_unary_value_expr.
fn infer_unary_value_expr(op: UnaryOp, expr: ValueExpr) -> Result<ValueExpr, Error> {
    let ty = expr.ty();
    infer_unary_type(op, &ty)?;
    Ok(ValueExpr::Unary {
        op,
        expr: Box::new(expr),
        ty,
    })
}

/// Type-checks helper logic for infer_unary_type.
fn infer_unary_type(op: UnaryOp, ty: &Type) -> Result<Type, Error> {
    match op {
        UnaryOp::Inv => {
            if has_category(ty, AlgebraicCategory::Grp) {
                Ok(ty.clone())
            } else {
                Err(Error::new(format!(
                    "unsupported operand for unary operator: {}{}",
                    op.symbol(),
                    format_type(ty)
                )))
            }
        }
    }
}

fn lift_param_name(lift_param: Option<&str>, name: &str) -> Result<String, Error> {
    lift_param
        .map(ToString::to_string)
        .ok_or_else(|| {
            Error::new(format!("function '{name}' needs an explicit call outside function bodies"))
        })
}

/// Type-checks helper logic for infer_binary_type.
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

/// Type-checks helper logic for is_equality_comparable_type.
fn is_equality_comparable_type(ty: &Type) -> bool {
    matches!(ty, Type::Bool | Type::Float | Type::Int)
}

/// Type-checks helper logic for is_ordered_comparable_type.
fn is_ordered_comparable_type(ty: &Type) -> bool {
    matches!(ty, Type::Float | Type::Int)
}

/// Type-checks helper logic for bool_numeric_cast_type_for_binary.
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

/// Type-checks helper logic for try_int_literal_cast_value.
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

/// Type-checks helper logic for try_bool_to_number_cast_value.
fn try_bool_to_number_cast_value(value: &ValueExpr, expected_ty: &Type) -> Option<ValueExpr> {
    if !matches!(expected_ty, Type::Float | Type::Int) || value.ty() != Type::Bool {
        return None;
    }
    Some(ValueExpr::BoolToNumberCast {
        value: Box::new(value.clone()),
        ty: expected_ty.clone(),
    })
}

/// Type-checks helper logic for try_neutral_cast_value.
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

/// Type-checks helper logic for zero_vec2.
fn zero_vec2() -> ValueExpr {
    ValueExpr::Vec2(
        Box::new(ValueExpr::Float(0.0)),
        Box::new(ValueExpr::Float(0.0)),
    )
}

/// Type-checks helper logic for zero_vec3.
fn zero_vec3() -> ValueExpr {
    ValueExpr::Vec3(
        Box::new(ValueExpr::Float(0.0)),
        Box::new(ValueExpr::Float(0.0)),
        Box::new(ValueExpr::Float(0.0)),
    )
}

/// Type-checks helper logic for ambient_vector_type.
fn ambient_vector_type(dimension: ShapeDimension) -> Type {
    match dimension {
        ShapeDimension::D2 => Type::Vec2,
        ShapeDimension::D3 => Type::Vec3,
    }
}

/// Type-checks helper logic for ambient_identity_matrix.
fn ambient_identity_matrix(dimension: ShapeDimension) -> ValueExpr {
    match dimension {
        ShapeDimension::D2 => identity_mat2(),
        ShapeDimension::D3 => identity_mat3(),
    }
}

/// Type-checks helper logic for identity_mat2.
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

/// Type-checks helper logic for unit_z_vec3.
fn unit_z_vec3() -> ValueExpr {
    ValueExpr::Vec3(
        Box::new(ValueExpr::Float(0.0)),
        Box::new(ValueExpr::Float(0.0)),
        Box::new(ValueExpr::Float(1.0)),
    )
}

/// Type-checks helper logic for identity_mat3.
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
