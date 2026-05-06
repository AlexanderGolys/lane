use super::*;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum EmitItem {
    Func(String),
    Object(String),
}

impl TypedProgram {
    pub(super) fn emit_glsl(&self, registry: &Registry) -> String {
        let mut lines = Vec::new();
        let locals = self.emit_locals();

        for support in self.support_blocks(registry) {
            lines.extend(support.lines().map(str::to_string));
            lines.push(String::new());
        }

        for helper in self.concat_helpers() {
            lines.extend(emit_concat_helper(&helper).lines().map(str::to_string));
            lines.push(String::new());
        }

        let global_value_names = self.global_value_binding_names();
        let emitted_func_names = self.emitted_func_names();
        let helper_names = self.func_names();
        let scene_input_names = self.scene_input_names();
        let object_bindings = self.object_bindings();
        let object_getter_bindings = self.object_getter_bindings();

        for line in self.emit_global_value_binding_lines(&global_value_names, &helper_names) {
            lines.push(line);
        }
        if !global_value_names.is_empty() {
            lines.push(String::new());
        }

        let signature = self.scene_signature(&locals.point);
        if self.output.is_some()
            && self
                .funcs
                .iter()
                .any(|func| matches!(func.body, TypedFuncBody::RawGlsl(_)))
        {
            lines.push(format!("float scene_sdf({});", signature.join(", ")));
            lines.push(format!(
                "{} scene_grad({});",
                ambient_vector_glsl_type(self.ambient_dimension),
                signature.join(", ")
            ));
            lines.push(String::new());
        }
        for prototype in self.object_getter_prototypes(&object_getter_bindings, &locals) {
            lines.push(prototype);
        }
        if !object_getter_bindings.is_empty() {
            lines.push(String::new());
        }
        for prototype in self.raw_glsl_function_ref_prototypes(&emitted_func_names, &locals) {
            lines.push(prototype);
        }
        if self
            .funcs
            .iter()
            .any(|func| !func.raw_glsl_refs.funcs.is_empty())
        {
            lines.push(String::new());
        }
        for item in self.ordered_emitted_items(&emitted_func_names, &object_getter_bindings) {
            match item {
                EmitItem::Func(name) => {
                    let func = self
                        .funcs
                        .iter()
                        .find(|func| func.name == name)
                        .expect("ordered emitted function exists");
                    lines.extend(self.emit_func(func, &helper_names, &locals));
                }
                EmitItem::Object(name) => {
                    let binding = self
                        .bindings
                        .iter()
                        .find(|binding| binding.name == name)
                        .expect("ordered emitted object exists");
                    lines.extend(self.emit_generated_binding(
                        binding,
                        &object_bindings,
                        &helper_names,
                        &scene_input_names,
                        &global_value_names,
                        &locals,
                    ));
                }
            }
        }

        if let Some(scene_output) = &self.output {
            lines.push(format!("float scene_sdf({}) {{", signature.join(", ")));
            let scene_value_names = self.needed_value_binding_names(
                scene_output,
                &object_bindings,
                &global_value_names,
            );
            lines.extend(self.emit_value_binding_lines(
                &helper_names,
                &global_value_names,
                &scene_value_names,
            ));
            let output = emit_object_expr(
                scene_output,
                &locals.point,
                self.ambient_dimension,
                &object_bindings,
                &helper_names,
                &scene_input_names,
            );
            lines.push(format!("    return {};", output));
            lines.push("}".to_string());

            lines.push(String::new());
            lines.push(format!(
                "{} scene_grad({}) {{",
                ambient_vector_glsl_type(self.ambient_dimension),
                signature.join(", ")
            ));
            lines.push(format!(
                "    float {} = {};",
                locals.eps,
                format_float(self.gradient_epsilon)
            ));
            lines.push(format!(
                "    return normalize({});",
                emit_sdf_gradient_expr(
                    "scene_sdf",
                    &locals.point,
                    &locals.eps,
                    &scene_input_names,
                    self.ambient_dimension
                )
            ));
            lines.push("}".to_string());
        }

        suffix_glsl_float_literals(&lines.join("\n"))
    }

    fn scene_signature(&self, point_name: &str) -> Vec<String> {
        self.object_helper_signature(point_name, self.ambient_dimension)
    }

    fn object_helper_signature(&self, point_name: &str, dimension: ShapeDimension) -> Vec<String> {
        vec![format!(
            "{} {}",
            ambient_vector_glsl_type(dimension),
            point_name
        )]
    }

    fn scene_input_names(&self) -> Vec<String> {
        Vec::new()
    }

    fn emit_value_binding_lines(
        &self,
        helper_names: &HashMap<String, String>,
        global_value_names: &BTreeSet<String>,
        needed_value_names: &BTreeSet<String>,
    ) -> Vec<String> {
        self.value_bindings
            .iter()
            .filter(|binding| {
                !global_value_names.contains(&binding.name)
                    && needed_value_names.contains(&binding.name)
            })
            .map(|binding| {
                format!(
                    "    {} = {};",
                    emit_value_binding_type(&binding.ty, &binding.name, binding.expr.array_len()),
                    emit_plain_value_expr(&binding.expr, helper_names)
                )
            })
            .collect()
    }

    fn emit_global_value_binding_lines(
        &self,
        global_value_names: &BTreeSet<String>,
        helper_names: &HashMap<String, String>,
    ) -> Vec<String> {
        self.value_bindings
            .iter()
            .filter(|binding| global_value_names.contains(&binding.name))
            .map(|binding| {
                format!(
                    "const {} = {};",
                    emit_value_binding_type(&binding.ty, &binding.name, binding.expr.array_len()),
                    emit_plain_value_expr(&binding.expr, helper_names)
                )
            })
            .collect()
    }

    fn global_value_binding_names(&self) -> BTreeSet<String> {
        let mut names = BTreeSet::new();
        for binding in &self.value_bindings {
            if (binding.generated || is_global_const_value_expr(&binding.expr, &names))
                && is_global_const_value_expr(&binding.expr, &names)
                && !matches!(binding.ty, Type::Array(_))
            {
                names.insert(binding.name.clone());
            }
        }
        names
    }

    fn emitted_func_names(&self) -> BTreeSet<String> {
        let mut names = self
            .funcs
            .iter()
            .filter_map(|func| {
                (func.generated && !matches!(func.body, TypedFuncBody::RawGlslTemplate))
                    .then_some(func.name.clone())
            })
            .collect::<BTreeSet<_>>();

        for binding in &self.value_bindings {
            if binding.generated {
                collect_value_function_refs(&binding.expr, &mut names);
            }
        }
        let object_bindings = self.object_bindings();
        let global_value_names = self.global_value_binding_names();
        let mut needed_value_names = BTreeSet::new();
        for binding in &self.bindings {
            if binding.generated {
                collect_object_function_refs(&binding.expr, &object_bindings, &mut names);
                needed_value_names.extend(self.needed_value_binding_names(
                    &binding.expr,
                    &object_bindings,
                    &global_value_names,
                ));
            }
        }
        if let Some(output) = &self.output {
            collect_object_function_refs(output, &object_bindings, &mut names);
            needed_value_names.extend(self.needed_value_binding_names(
                output,
                &object_bindings,
                &global_value_names,
            ));
        }
        for binding in &self.value_bindings {
            if needed_value_names.contains(&binding.name) {
                collect_value_function_refs(&binding.expr, &mut names);
            }
        }

        let mut changed = true;
        while changed {
            changed = false;
            for func in &self.funcs {
                if names.contains(&func.name) {
                    let before = names.len();
                    match &func.body {
                        TypedFuncBody::Expr(expr) => collect_value_function_refs(expr, &mut names),
                        TypedFuncBody::RawGlsl(body) => {
                            names.extend(func.raw_glsl_refs.funcs.iter().cloned());
                            collect_raw_glsl_placeholders(body, &mut names);
                        }
                        TypedFuncBody::RawGlslTemplate => {}
                    }
                    changed |= names.len() != before;
                }
            }
        }
        names
    }

    fn needed_value_binding_names(
        &self,
        expr: &ObjectExpr,
        object_bindings: &BTreeMap<String, ObjectBinding>,
        global_value_names: &BTreeSet<String>,
    ) -> BTreeSet<String> {
        let mut names = BTreeSet::new();
        collect_object_value_refs(expr, object_bindings, &mut names);

        let mut changed = true;
        while changed {
            changed = false;
            for binding in &self.value_bindings {
                if names.contains(&binding.name) {
                    let before = names.len();
                    collect_value_refs(&binding.expr, &mut names);
                    changed |= names.len() != before;
                }
            }
        }

        for name in global_value_names {
            names.remove(name);
        }
        names
    }

    fn object_bindings(&self) -> BTreeMap<String, ObjectBinding> {
        self.bindings
            .iter()
            .map(|binding| {
                (
                    binding.name.clone(),
                    ObjectBinding {
                        expr: binding.expr.clone(),
                        generated: binding.generated,
                    },
                )
            })
            .collect()
    }

    fn emit_func(
        &self,
        func: &TypedFunc,
        helper_names: &HashMap<String, String>,
        locals: &EmitLocals,
    ) -> Vec<String> {
        let func_var_names = HashMap::from([("t".to_string(), locals.func_param.clone())]);
        if let TypedFuncBody::RawGlsl(body) = &func.body {
            let rendered = render_raw_glsl_function(body, func);
            return vec![rendered.trim().to_string(), String::new()];
        }
        let body = match &func.body {
            TypedFuncBody::Expr(expr) => {
                let mut lines = func
                    .param_bindings
                    .iter()
                    .map(|binding| {
                        format!(
                            "    {} {} = {};",
                            binding.ty.glsl_name(),
                            binding.name,
                            binding.expr
                        )
                    })
                    .collect::<Vec<_>>();
                lines.push(format!(
                    "    return {};",
                    emit_value_expr(expr, helper_names, &func_var_names)
                ));
                lines.join("\n")
            }
            TypedFuncBody::RawGlsl(_) => unreachable!(),
            TypedFuncBody::RawGlslTemplate => unreachable!(),
        };
        vec![
            format!(
                "{} {}({}) {{",
                func.output.glsl_name(),
                helper_name(&func.name),
                emit_func_signature_params(&func.input, locals)
            ),
            body,
            "}".to_string(),
            String::new(),
        ]
    }

    fn ordered_emitted_items(
        &self,
        emitted_func_names: &BTreeSet<String>,
        object_getter_bindings: &BTreeSet<String>,
    ) -> Vec<EmitItem> {
        let mut items = self
            .funcs
            .iter()
            .filter(|func| emitted_func_names.contains(&func.name))
            .map(|func| (func.line, EmitItem::Func(func.name.clone())))
            .chain(self.bindings.iter().filter_map(|binding| {
                (binding.generated || object_getter_bindings.contains(&binding.name))
                    .then_some((binding.line, EmitItem::Object(binding.name.clone())))
            }))
            .collect::<Vec<_>>();
        items.sort_by_key(|(line, item)| (*line, item.clone()));
        items.into_iter().map(|(_, item)| item).collect()
    }

    fn emit_generated_binding(
        &self,
        binding: &TypedBinding,
        object_bindings: &BTreeMap<String, ObjectBinding>,
        helper_names: &HashMap<String, String>,
        scene_input_names: &[String],
        global_value_names: &BTreeSet<String>,
        locals: &EmitLocals,
    ) -> Vec<String> {
        let dimension = binding.dimension.unwrap_or(self.ambient_dimension);
        let signature = self.object_helper_signature(&locals.point, dimension);
        let mut lines = Vec::new();

        lines.push(format!(
            "float sdf_{}({}) {{",
            binding.name,
            signature.join(", ")
        ));
        let needed_value_names =
            self.needed_value_binding_names(&binding.expr, object_bindings, global_value_names);
        lines.extend(self.emit_value_binding_lines(
            helper_names,
            global_value_names,
            &needed_value_names,
        ));
        lines.push(format!(
            "    return {};",
            emit_object_expr(
                &binding.expr,
                &locals.point,
                dimension,
                object_bindings,
                helper_names,
                scene_input_names,
            )
        ));
        lines.push("}".to_string());
        lines.push(String::new());

        if dimension == ShapeDimension::D3 {
            lines.push(format!(
                "{} grad_sdf_{}({}) {{",
                ambient_vector_glsl_type(dimension),
                binding.name,
                signature.join(", ")
            ));
            lines.push(format!(
                "    float {} = {};",
                locals.eps,
                format_float(self.gradient_epsilon)
            ));
            lines.push(format!(
                "    return normalize({});",
                emit_sdf_gradient_expr(
                    &format!("sdf_{}", binding.name),
                    &locals.point,
                    &locals.eps,
                    scene_input_names,
                    dimension,
                )
            ));
            lines.push("}".to_string());
            lines.push(String::new());
        }

        lines
    }

    fn emit_locals(&self) -> EmitLocals {
        let mut forbidden = BTreeSet::new();
        for input in &self.inputs {
            match input.ty {
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
                    forbidden.insert(input.name.clone());
                }
                Type::Object | Type::Object2D | Type::Product(_) | Type::Func(_, _) => {}
            }
        }
        for binding in &self.value_bindings {
            forbidden.insert(binding.name.clone());
        }

        let point = unique_local_name("p", &forbidden);
        forbidden.insert(point.clone());
        let func_param = unique_local_name("_t", &forbidden);
        forbidden.insert(func_param.clone());
        let eps = unique_local_name("eps", &forbidden);

        EmitLocals {
            point,
            func_param,
            eps,
        }
    }

    fn support_blocks(&self, registry: &Registry) -> Vec<String> {
        let mut names = BTreeSet::new();
        let product_types = self
            .product_types
            .iter()
            .map(|decl| (decl.name.as_str(), decl))
            .collect::<HashMap<_, _>>();
        for input in &self.inputs {
            collect_type_support(&input.ty, &mut names);
        }
        for func in &self.funcs {
            collect_type_support(&func.input, &mut names);
            collect_type_support(&func.output, &mut names);
            if let TypedFuncBody::Expr(expr) = &func.body {
                collect_value_support(expr, &mut names);
            }
        }
        for binding in &self.value_bindings {
            collect_type_support(&binding.ty, &mut names);
            collect_value_support(&binding.expr, &mut names);
        }
        for binding in &self.bindings {
            collect_object_support(&binding.expr, &mut names);
        }
        if let Some(output) = &self.output {
            collect_object_support(output, &mut names);
        }
        for product_type in &self.product_types {
            if product_type.eager_ops {
                if !product_type.provided {
                    names.insert(product_type_type_support_name(&product_type.name));
                    for &op in product_category_ops(product_type.category) {
                        names.insert(product_type_op_support_name(&product_type.name, op));
                    }
                }
            }
        }

        let mut blocks = Vec::new();
        let mut emitted_builtin_support = BTreeSet::new();
        let mut emitted_product_types = BTreeSet::new();
        let mut emitted_product_ops = BTreeSet::new();
        for name in names {
            if let Some((product_name, support)) = parse_product_support_name(&name) {
                if let Some(product_type) = product_types.get(product_name) {
                    emit_product_support(
                        product_type,
                        support,
                        &product_types,
                        &mut emitted_builtin_support,
                        &mut emitted_product_types,
                        &mut emitted_product_ops,
                        &mut blocks,
                    );
                }
                continue;
            }
            if let Some(func) = name.strip_prefix("complex:") {
                if let Some(support_glsl) = complex_overload_support_glsl(func) {
                    blocks.push(support_glsl.to_string());
                }
                continue;
            }
            if let Some(type_name) = name.strip_prefix("monoid-pow:") {
                if let Some(ty) = self.monoid_pow_type_by_name(type_name) {
                    emit_monoid_pow_support(
                        &ty,
                        &product_types,
                        &mut emitted_builtin_support,
                        &mut emitted_product_types,
                        &mut emitted_product_ops,
                        &mut blocks,
                    );
                }
                continue;
            }
            if let Some(primitive) = registry.primitives.get(name.as_str()) {
                blocks.push(primitive.support_glsl.to_string());
                continue;
            }
            if let Some(op) = registry.object_ops.get(name.as_str()) {
                blocks.push(op.support_glsl.to_string());
            }
            if let Some(support_glsl) = builtin_type_support_glsl(name.as_str()) {
                if emitted_builtin_support.insert(name.clone()) {
                    blocks.push(support_glsl.to_string());
                }
                continue;
            }
            if let Some(func) = registry.value_funcs.get(name.as_str()) {
                if let Some(support_glsl) = func.support_glsl {
                    blocks.push(support_glsl.to_string());
                }
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

    fn object_getter_bindings(&self) -> BTreeSet<String> {
        let mut names = BTreeSet::new();
        for func in &self.funcs {
            if let TypedFuncBody::Expr(expr) = &func.body {
                collect_object_getter_value_refs(expr, &mut names);
            }
            names.extend(func.raw_glsl_refs.object_getters.iter().cloned());
        }
        for binding in &self.value_bindings {
            collect_object_getter_value_refs(&binding.expr, &mut names);
        }
        for binding in &self.bindings {
            collect_object_getter_object_refs(&binding.expr, &mut names);
        }
        if let Some(output) = &self.output {
            collect_object_getter_object_refs(output, &mut names);
        }
        names
    }

    fn object_getter_prototypes(
        &self,
        object_getter_bindings: &BTreeSet<String>,
        locals: &EmitLocals,
    ) -> Vec<String> {
        let mut lines = Vec::new();
        for name in object_getter_bindings {
            let Some(binding) = self.bindings.iter().find(|binding| binding.name == *name) else {
                continue;
            };
            let dimension = binding.dimension.unwrap_or(self.ambient_dimension);
            let signature = self
                .object_helper_signature(&locals.point, dimension)
                .join(", ");
            lines.push(format!("float sdf_{name}({signature});"));
            if dimension == ShapeDimension::D3 {
                lines.push(format!(
                    "{} grad_sdf_{name}({signature});",
                    ambient_vector_glsl_type(dimension)
                ));
            }
        }
        lines
    }

    fn raw_glsl_function_ref_prototypes(
        &self,
        emitted_func_names: &BTreeSet<String>,
        locals: &EmitLocals,
    ) -> Vec<String> {
        let mut names = BTreeSet::new();
        for func in &self.funcs {
            names.extend(func.raw_glsl_refs.funcs.iter().cloned());
        }
        names
            .into_iter()
            .filter(|name| emitted_func_names.contains(name))
            .filter_map(|name| {
                let func = self.funcs.iter().find(|func| func.name == name)?;
                if matches!(
                    func.body,
                    TypedFuncBody::RawGlsl(_) | TypedFuncBody::RawGlslTemplate
                ) {
                    return None;
                }
                Some(format!(
                    "{} {}({} {});",
                    func.output.glsl_name(),
                    helper_name(&func.name),
                    func.input.glsl_name(),
                    locals.func_param
                ))
            })
            .collect()
    }

    fn concat_helpers(&self) -> Vec<ConcatHelper> {
        let mut helpers = BTreeMap::new();
        for func in &self.funcs {
            if let TypedFuncBody::Expr(expr) = &func.body {
                collect_concat_helpers(expr, &mut helpers);
            }
        }
        for binding in &self.value_bindings {
            collect_concat_helpers(&binding.expr, &mut helpers);
        }
        for binding in &self.bindings {
            collect_object_concat_helpers(&binding.expr, &mut helpers);
        }
        if let Some(output) = &self.output {
            collect_object_concat_helpers(output, &mut helpers);
        }
        helpers.into_values().collect()
    }

    fn monoid_pow_type_by_name(&self, name: &str) -> Option<Type> {
        if let Some(ty) = parse_builtin_type_name(name) {
            return Some(ty);
        }
        if let Some(decl) = self.product_types.iter().find(|decl| decl.name == name) {
            return Some(product_type_decl_type(decl));
        }
        for input in &self.inputs {
            if let Some(ty) = find_custom_type_by_name(&input.ty, name) {
                return Some(ty);
            }
        }
        for binding in &self.value_bindings {
            if let Some(ty) = find_custom_type_by_name(&binding.ty, name) {
                return Some(ty);
            }
            if let Some(ty) = find_custom_type_by_name(&binding.expr.ty(), name) {
                return Some(ty);
            }
        }
        for func in &self.funcs {
            if let Some(ty) = find_custom_type_by_name(&func.input, name) {
                return Some(ty);
            }
            if let Some(ty) = find_custom_type_by_name(&func.output, name) {
                return Some(ty);
            }
        }
        None
    }
}

fn find_custom_type_by_name(ty: &Type, name: &str) -> Option<Type> {
    match ty {
        Type::Custom {
            name: ty_name,
            categories,
        } if ty_name == name => Some(Type::Custom {
            name: ty_name.clone(),
            categories: categories.clone(),
        }),
        Type::Array(element) => find_custom_type_by_name(element, name),
        Type::Product(parts) => parts
            .iter()
            .find_map(|part| find_custom_type_by_name(part, name)),
        Type::Func(input, output) => {
            find_custom_type_by_name(input, name).or_else(|| find_custom_type_by_name(output, name))
        }
        _ => None,
    }
}

fn collect_object_getter_object_refs(expr: &ObjectExpr, names: &mut BTreeSet<String>) {
    match expr {
        ObjectExpr::Var(_) => {}
        ObjectExpr::Primitive { fields, .. } => {
            for (_, value) in fields {
                match value {
                    PrimitiveArgExpr::Value(value) => {
                        collect_object_getter_value_refs(value, names);
                    }
                    PrimitiveArgExpr::Vec2List(vertices) => {
                        for vertex in vertices {
                            collect_object_getter_value_refs(vertex, names);
                        }
                    }
                }
            }
        }
        ObjectExpr::AmbientTransform {
            object,
            translation,
            linear,
        } => {
            collect_object_getter_value_refs(translation, names);
            collect_object_getter_value_refs(linear, names);
            collect_object_getter_object_refs(object, names);
        }
        ObjectExpr::IsometryTransform { object, transform } => {
            collect_object_getter_value_refs(transform, names);
            collect_object_getter_object_refs(object, names);
        }
        ObjectExpr::RegisteredOp {
            value_args,
            object_args,
            ..
        } => {
            for arg in value_args {
                collect_object_getter_value_refs(arg, names);
            }
            for arg in object_args {
                collect_object_getter_object_refs(arg, names);
            }
        }
    }
}

fn collect_object_getter_value_refs(expr: &ValueExpr, names: &mut BTreeSet<String>) {
    match expr {
        ValueExpr::Bool(_)
        | ValueExpr::Float(_)
        | ValueExpr::Int(_)
        | ValueExpr::Neutral { .. }
        | ValueExpr::Var { .. } => {}
        ValueExpr::Call { args, .. } => {
            for arg in args {
                collect_object_getter_value_refs(arg, names);
            }
        }
        ValueExpr::MonoidPow { exponent, base, .. } => {
            collect_object_getter_value_refs(exponent, names);
            collect_object_getter_value_refs(base, names);
        }
        ValueExpr::BoolToNumberCast { value, .. } => {
            collect_object_getter_value_refs(value, names);
        }
        ValueExpr::Conditional {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            collect_object_getter_value_refs(condition, names);
            collect_object_getter_value_refs(then_branch, names);
            collect_object_getter_value_refs(else_branch, names);
        }
        ValueExpr::ObjectGetterCall {
            object,
            point,
            captures,
            ..
        } => {
            names.insert(object.clone());
            collect_object_getter_value_refs(point, names);
            for capture in captures {
                collect_object_getter_value_refs(capture, names);
            }
        }
        ValueExpr::FieldAccess { value, .. } => collect_object_getter_value_refs(value, names),
        ValueExpr::Array { elements, .. } => {
            for element in elements {
                collect_object_getter_value_refs(element, names);
            }
        }
        ValueExpr::Index { array, index, .. } => {
            collect_object_getter_value_refs(array, names);
            collect_object_getter_value_refs(index, names);
        }
        ValueExpr::Concat { left, right, .. } => {
            collect_object_getter_value_refs(left, names);
            collect_object_getter_value_refs(right, names);
        }
        ValueExpr::Binary { left, right, .. } => {
            collect_object_getter_value_refs(left, names);
            collect_object_getter_value_refs(right, names);
        }
        ValueExpr::Vec2(x, y) => {
            collect_object_getter_value_refs(x, names);
            collect_object_getter_value_refs(y, names);
        }
        ValueExpr::Vec3(x, y, z) => {
            collect_object_getter_value_refs(x, names);
            collect_object_getter_value_refs(y, names);
            collect_object_getter_value_refs(z, names);
        }
        ValueExpr::Vec4(x, y, z, w) => {
            collect_object_getter_value_refs(x, names);
            collect_object_getter_value_refs(y, names);
            collect_object_getter_value_refs(z, names);
            collect_object_getter_value_refs(w, names);
        }
        ValueExpr::Matrix { rows, .. } => {
            for row in rows {
                collect_object_getter_value_refs(row, names);
            }
        }
        ValueExpr::MatrixBasis { .. } | ValueExpr::UnitVectorBasis { .. } => {}
        ValueExpr::Derivative {
            epsilon, func, at, ..
        }
        | ValueExpr::Gradient {
            epsilon, func, at, ..
        }
        | ValueExpr::Divergence { epsilon, func, at } => {
            collect_object_getter_value_refs(epsilon, names);
            collect_object_getter_function_refs(func, names);
            collect_object_getter_value_refs(at, names);
        }
        ValueExpr::Partial {
            epsilon, func, at, ..
        } => {
            collect_object_getter_value_refs(epsilon, names);
            collect_object_getter_function_refs(func, names);
            collect_object_getter_value_refs(at, names);
        }
    }
}

fn collect_object_getter_function_refs(func: &FunctionExpr, names: &mut BTreeSet<String>) {
    match &func.kind {
        FunctionExprKind::Named(_) => {}
        FunctionExprKind::Operator(_) => {}
        FunctionExprKind::ObjectGetter {
            object, captures, ..
        } => {
            names.insert(object.clone());
            for capture in captures {
                collect_object_getter_value_refs(capture, names);
            }
        }
        FunctionExprKind::Compose(outer, inner) => {
            collect_object_getter_function_refs(outer, names);
            collect_object_getter_function_refs(inner, names);
        }
        FunctionExprKind::PointwiseBinary { left, right, .. } => {
            collect_object_getter_pointwise_call_arg_refs(left, names);
            collect_object_getter_pointwise_call_arg_refs(right, names);
        }
        FunctionExprKind::PointwiseCall { args, .. } => {
            for arg in args {
                collect_object_getter_pointwise_call_arg_refs(arg, names);
            }
        }
        FunctionExprKind::PointwiseConditional {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_object_getter_pointwise_call_arg_refs(condition, names);
            collect_object_getter_pointwise_call_arg_refs(then_branch, names);
            collect_object_getter_pointwise_call_arg_refs(else_branch, names);
        }
        FunctionExprKind::ProductSameDomain(funcs) => {
            for func in funcs {
                collect_object_getter_function_refs(func, names);
            }
        }
        FunctionExprKind::ProductTensor(left, right) => {
            collect_object_getter_function_refs(left, names);
            collect_object_getter_function_refs(right, names);
        }
    }
}

fn collect_object_getter_pointwise_call_arg_refs(
    arg: &PointwiseCallArg,
    names: &mut BTreeSet<String>,
) {
    match arg {
        PointwiseCallArg::Function { func, .. } => collect_object_getter_function_refs(func, names),
        PointwiseCallArg::Value(value) => collect_object_getter_value_refs(value, names),
    }
}

fn collect_type_support(ty: &Type, names: &mut BTreeSet<String>) {
    match ty {
        Type::Isom2 | Type::Isom3 => {
            names.insert(ty.type_name());
        }
        Type::Custom { name, .. } => {
            names.insert(product_type_type_support_name(name));
        }
        Type::Array(element) => collect_type_support(element, names),
        Type::Product(parts) => {
            for part in parts {
                collect_type_support(part, names);
            }
        }
        Type::Func(input, output) => {
            collect_type_support(input, names);
            collect_type_support(output, names);
        }
        Type::Unit
        | Type::Bool
        | Type::Float
        | Type::Int
        | Type::Complex
        | Type::Quat
        | Type::Vec2
        | Type::Vec3
        | Type::Vec4
        | Type::Mat(_, _)
        | Type::Generic(_)
        | Type::VecGeneric(_)
        | Type::MatGeneric(_, _)
        | Type::Object
        | Type::Object2D => {}
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum ProductOp {
    Zero,
    One,
    Identity,
    Add,
    Sub,
    Mult,
    Inv,
    Scale,
}

impl ProductOp {
    fn as_str(self) -> &'static str {
        match self {
            Self::Zero => "zero",
            Self::One => "one",
            Self::Identity => "e",
            Self::Add => "add",
            Self::Sub => "sub",
            Self::Mult => "mult",
            Self::Inv => "inv",
            Self::Scale => "scale",
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum ProductSupport {
    Type,
    Op(ProductOp),
}

fn product_type_type_support_name(name: &str) -> String {
    format!("product:{name}:type")
}

fn product_type_op_support_name(name: &str, op: ProductOp) -> String {
    format!("product:{name}:{}", op.as_str())
}

fn parse_product_support_name(name: &str) -> Option<(&str, ProductSupport)> {
    let rest = name.strip_prefix("product:")?;
    let (product_name, support_name) = rest.rsplit_once(':')?;
    let support = match support_name {
        "type" => ProductSupport::Type,
        "zero" => ProductSupport::Op(ProductOp::Zero),
        "one" => ProductSupport::Op(ProductOp::One),
        "e" => ProductSupport::Op(ProductOp::Identity),
        "add" => ProductSupport::Op(ProductOp::Add),
        "sub" => ProductSupport::Op(ProductOp::Sub),
        "mult" => ProductSupport::Op(ProductOp::Mult),
        "inv" => ProductSupport::Op(ProductOp::Inv),
        "scale" => ProductSupport::Op(ProductOp::Scale),
        _ => return None,
    };
    Some((product_name, support))
}

fn monoid_pow_support_name(ty: &Type) -> String {
    format!("monoid-pow:{}", ty.type_name())
}

fn monoid_pow_function_name(ty: &Type) -> String {
    format!("pow_monoid_{}", sanitize_glsl_name(&ty.type_name()))
}

fn sanitize_glsl_name(name: &str) -> String {
    name.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn emit_monoid_pow_support(
    ty: &Type,
    product_types: &HashMap<&str, &ProductTypeDecl>,
    emitted_builtin_support: &mut BTreeSet<String>,
    emitted_product_types: &mut BTreeSet<String>,
    emitted_product_ops: &mut BTreeSet<String>,
    blocks: &mut Vec<String>,
) {
    emit_component_type_dependency(
        ty,
        product_types,
        emitted_builtin_support,
        emitted_product_types,
        blocks,
    );
    emit_component_op_dependency(
        ty,
        ProductOp::One,
        product_types,
        emitted_builtin_support,
        emitted_product_types,
        emitted_product_ops,
        blocks,
    );
    emit_component_op_dependency(
        ty,
        ProductOp::Mult,
        product_types,
        emitted_builtin_support,
        emitted_product_types,
        emitted_product_ops,
        blocks,
    );
    blocks.push(emit_monoid_pow_function(ty));
}

fn emit_monoid_pow_function(ty: &Type) -> String {
    let name = monoid_pow_function_name(ty);
    let glsl_ty = ty.glsl_name();
    let one = emit_component_neutral(ProductOp::One, ty);
    let multiply_result = emit_component_binary(ProductOp::Mult, ty, "result", "factor");
    let multiply_factor = emit_component_binary(ProductOp::Mult, ty, "factor", "factor");
    format!(
        "{glsl_ty} {name}(int exponent, {glsl_ty} value) {{\n    {glsl_ty} result = {one};\n    {glsl_ty} factor = value;\n    int n = exponent;\n    while (n > 0) {{\n        if ((n % 2) == 1) {{\n            result = {multiply_result};\n        }}\n        factor = {multiply_factor};\n        n = n / 2;\n    }}\n    return result;\n}}"
    )
}

fn product_category_ops(category: AlgebraicCategory) -> &'static [ProductOp] {
    match category {
        AlgebraicCategory::Ab => &[ProductOp::Zero, ProductOp::Add, ProductOp::Sub],
        AlgebraicCategory::Mon => &[ProductOp::One, ProductOp::Mult],
        AlgebraicCategory::Grp => &[ProductOp::Identity, ProductOp::Mult, ProductOp::Inv],
        AlgebraicCategory::Ring => &[
            ProductOp::Zero,
            ProductOp::One,
            ProductOp::Add,
            ProductOp::Sub,
            ProductOp::Mult,
        ],
        AlgebraicCategory::VectR => &[
            ProductOp::Zero,
            ProductOp::Add,
            ProductOp::Sub,
            ProductOp::Scale,
        ],
        AlgebraicCategory::RAlg => &[
            ProductOp::Zero,
            ProductOp::One,
            ProductOp::Add,
            ProductOp::Sub,
            ProductOp::Mult,
            ProductOp::Scale,
        ],
        AlgebraicCategory::DivRing | AlgebraicCategory::Set => &[],
    }
}

fn product_op_for_binary(op: BinOp) -> Option<ProductOp> {
    match op {
        BinOp::Add => Some(ProductOp::Add),
        BinOp::Sub => Some(ProductOp::Sub),
        BinOp::Mul => Some(ProductOp::Mult),
        BinOp::Div => Some(ProductOp::Inv),
        BinOp::Eq
        | BinOp::Ne
        | BinOp::Lt
        | BinOp::Le
        | BinOp::Gt
        | BinOp::Ge
        | BinOp::Product
        | BinOp::Compose => None,
    }
}

fn product_op_for_neutral(kind: NeutralKind) -> Option<ProductOp> {
    match kind {
        NeutralKind::Zero => Some(ProductOp::Zero),
        NeutralKind::One => Some(ProductOp::One),
        NeutralKind::Identity => Some(ProductOp::Identity),
    }
}

fn emit_product_support(
    decl: &ProductTypeDecl,
    support: ProductSupport,
    product_types: &HashMap<&str, &ProductTypeDecl>,
    emitted_builtin_support: &mut BTreeSet<String>,
    emitted_product_types: &mut BTreeSet<String>,
    emitted_product_ops: &mut BTreeSet<String>,
    blocks: &mut Vec<String>,
) {
    match support {
        ProductSupport::Type => emit_product_type_support(
            decl,
            product_types,
            emitted_builtin_support,
            emitted_product_types,
            blocks,
        ),
        ProductSupport::Op(op) => emit_product_op_support(
            decl,
            op,
            product_types,
            emitted_builtin_support,
            emitted_product_types,
            emitted_product_ops,
            blocks,
        ),
    }
}

fn emit_product_type_support(
    decl: &ProductTypeDecl,
    product_types: &HashMap<&str, &ProductTypeDecl>,
    emitted_builtin_support: &mut BTreeSet<String>,
    emitted_product_types: &mut BTreeSet<String>,
    blocks: &mut Vec<String>,
) {
    if decl.provided {
        return;
    }
    if !emitted_product_types.insert(decl.name.clone()) {
        return;
    }
    for component in &decl.components {
        emit_component_type_dependency(
            component,
            product_types,
            emitted_builtin_support,
            emitted_product_types,
            blocks,
        );
    }
    let fields = decl
        .components
        .iter()
        .zip(&decl.field_names)
        .map(|(ty, field)| format!("    {} {};", ty.glsl_name(), field))
        .collect::<Vec<_>>()
        .join("\n");
    blocks.push(format!("struct {} {{\n{}\n}};", decl.name, fields));
}

fn emit_product_op_support(
    decl: &ProductTypeDecl,
    op: ProductOp,
    product_types: &HashMap<&str, &ProductTypeDecl>,
    emitted_builtin_support: &mut BTreeSet<String>,
    emitted_product_types: &mut BTreeSet<String>,
    emitted_product_ops: &mut BTreeSet<String>,
    blocks: &mut Vec<String>,
) {
    let op_key = format!("{}:{}", decl.name, op.as_str());
    if !emitted_product_ops.insert(op_key) {
        return;
    }
    emit_product_type_support(
        decl,
        product_types,
        emitted_builtin_support,
        emitted_product_types,
        blocks,
    );
    for component in &decl.components {
        emit_component_op_dependency(
            component,
            op,
            product_types,
            emitted_builtin_support,
            emitted_product_types,
            emitted_product_ops,
            blocks,
        );
    }
    blocks.push(emit_product_op(decl, op));
}

fn emit_component_type_dependency(
    ty: &Type,
    product_types: &HashMap<&str, &ProductTypeDecl>,
    emitted_builtin_support: &mut BTreeSet<String>,
    emitted_product_types: &mut BTreeSet<String>,
    blocks: &mut Vec<String>,
) {
    match ty {
        Type::Isom2 | Type::Isom3 => {
            emit_builtin_support_once(&ty.type_name(), emitted_builtin_support, blocks)
        }
        Type::Custom { name, .. } => {
            if let Some(decl) = product_types.get(name.as_str()) {
                emit_product_type_support(
                    decl,
                    product_types,
                    emitted_builtin_support,
                    emitted_product_types,
                    blocks,
                );
            }
        }
        _ => {}
    }
}

fn emit_component_op_dependency(
    ty: &Type,
    op: ProductOp,
    product_types: &HashMap<&str, &ProductTypeDecl>,
    emitted_builtin_support: &mut BTreeSet<String>,
    emitted_product_types: &mut BTreeSet<String>,
    emitted_product_ops: &mut BTreeSet<String>,
    blocks: &mut Vec<String>,
) {
    match ty {
        Type::Complex if matches!(op, ProductOp::Mult | ProductOp::Inv) => {
            emit_builtin_support_once("C", emitted_builtin_support, blocks);
        }
        Type::Quat if matches!(op, ProductOp::Mult | ProductOp::Inv) => {
            emit_builtin_support_once("H", emitted_builtin_support, blocks);
        }
        Type::Isom2 | Type::Isom3 => {
            emit_builtin_support_once(&ty.type_name(), emitted_builtin_support, blocks)
        }
        Type::Custom { name, .. } => {
            if let Some(decl) = product_types.get(name.as_str()) {
                let component_op =
                    if matches!(op, ProductOp::One) && has_category(ty, AlgebraicCategory::Grp) {
                        ProductOp::Identity
                    } else {
                        op
                    };
                emit_product_op_support(
                    decl,
                    component_op,
                    product_types,
                    emitted_builtin_support,
                    emitted_product_types,
                    emitted_product_ops,
                    blocks,
                );
            }
        }
        _ => {}
    }
}

fn emit_builtin_support_once(
    name: &str,
    emitted_builtin_support: &mut BTreeSet<String>,
    blocks: &mut Vec<String>,
) {
    if emitted_builtin_support.insert(name.to_string()) {
        if let Some(support_glsl) = builtin_type_support_glsl(name) {
            blocks.push(support_glsl.to_string());
        }
    }
}

fn emit_product_op(decl: &ProductTypeDecl, op: ProductOp) -> String {
    match op {
        ProductOp::Zero | ProductOp::One | ProductOp::Identity => emit_product_neutral(decl, op),
        ProductOp::Add | ProductOp::Sub | ProductOp::Mult => emit_product_binary_op(decl, op),
        ProductOp::Inv => emit_product_unary_op(decl, op),
        ProductOp::Scale => emit_product_scale_op(decl),
    }
}

fn emit_product_neutral(decl: &ProductTypeDecl, op: ProductOp) -> String {
    let fields = decl
        .components
        .iter()
        .map(|ty| emit_component_neutral(op, ty))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{} {}_{} = {}({});",
        decl.name,
        op.as_str(),
        decl.name,
        decl.name,
        fields
    )
}

fn emit_product_binary_op(decl: &ProductTypeDecl, op: ProductOp) -> String {
    let fields = decl
        .components
        .iter()
        .zip(&decl.field_names)
        .map(|(ty, field)| {
            emit_component_binary(op, ty, &format!("a.{field}"), &format!("b.{field}"))
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{} {}_{}({} a, {} b) {{\n    return {}({});\n}}",
        decl.name,
        op.as_str(),
        decl.name,
        decl.name,
        decl.name,
        decl.name,
        fields
    )
}

fn emit_product_unary_op(decl: &ProductTypeDecl, op: ProductOp) -> String {
    let fields = decl
        .components
        .iter()
        .zip(&decl.field_names)
        .map(|(ty, field)| emit_component_unary(op, ty, &format!("value.{field}")))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{} {}_{}({} value) {{\n    return {}({});\n}}",
        decl.name,
        op.as_str(),
        decl.name,
        decl.name,
        decl.name,
        fields
    )
}

fn emit_product_scale_op(decl: &ProductTypeDecl) -> String {
    let fields = decl
        .components
        .iter()
        .zip(&decl.field_names)
        .map(|(ty, field)| emit_component_scale(ty, &format!("value.{field}"), "scalar"))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{} scale_{}({} value, float scalar) {{\n    return {}({});\n}}",
        decl.name, decl.name, decl.name, decl.name, fields
    )
}

fn emit_component_neutral(op: ProductOp, ty: &Type) -> String {
    match op {
        ProductOp::Zero => emit_neutral_value(NeutralKind::Zero, ty),
        ProductOp::One
            if has_category(ty, AlgebraicCategory::Grp)
                && !has_category(ty, AlgebraicCategory::Ring)
                && !has_category(ty, AlgebraicCategory::DivRing)
                && !has_category(ty, AlgebraicCategory::RAlg) =>
        {
            emit_component_neutral(ProductOp::Identity, ty)
        }
        ProductOp::One if matches!(ty, Type::Mat(rows, columns) if rows == columns) => {
            emit_component_neutral(ProductOp::Identity, ty)
        }
        ProductOp::One => emit_neutral_value(NeutralKind::One, ty),
        ProductOp::Identity => match ty {
            Type::Bool => "true".to_string(),
            Type::Float => "1.0".to_string(),
            Type::Int => "1".to_string(),
            Type::Complex => "vec2(1.0, 0.0)".to_string(),
            Type::Quat => "vec4(1.0, 0.0, 0.0, 0.0)".to_string(),
            Type::Isom2 => "Isom2(mat2(1.0), vec2(0.0))".to_string(),
            Type::Isom3 => "Isom3(mat3(1.0), vec3(0.0))".to_string(),
            Type::Custom { name, .. } => format!("e_{name}"),
            _ => emit_neutral_value(NeutralKind::Identity, ty),
        },
        ProductOp::Add | ProductOp::Sub | ProductOp::Mult | ProductOp::Inv | ProductOp::Scale => {
            unreachable!()
        }
    }
}

fn emit_component_binary(op: ProductOp, ty: &Type, left: &str, right: &str) -> String {
    match (op, ty) {
        (ProductOp::Add | ProductOp::Sub, Type::Bool) => format!("({} != {})", left, right),
        (ProductOp::Mult, Type::Bool) => format!("({} && {})", left, right),
        (ProductOp::Add, Type::Custom { name, .. }) => format!("add_{}({}, {})", name, left, right),
        (ProductOp::Sub, Type::Custom { name, .. }) => format!("sub_{}({}, {})", name, left, right),
        (ProductOp::Mult, Type::Complex) => format!("mult_C({}, {})", left, right),
        (ProductOp::Mult, Type::Quat) => format!("mult_H({}, {})", left, right),
        (ProductOp::Mult, Type::Isom2) => format!("mult_Isom2({}, {})", left, right),
        (ProductOp::Mult, Type::Isom3) => format!("mult_Isom3({}, {})", left, right),
        (ProductOp::Mult, Type::Custom { name, .. }) => {
            format!("mult_{}({}, {})", name, left, right)
        }
        (ProductOp::Add | ProductOp::Sub | ProductOp::Mult, _) => {
            format!("({} {} {})", left, product_binary_symbol(op), right)
        }
        _ => unreachable!(),
    }
}

fn emit_component_unary(op: ProductOp, ty: &Type, value: &str) -> String {
    match (op, ty) {
        (ProductOp::Inv, Type::Bool) => value.to_string(),
        (ProductOp::Inv, Type::Float) => format!("(1.0 / {value})"),
        (ProductOp::Inv, Type::Int) => format!("(1 / {value})"),
        (ProductOp::Inv, Type::Complex) => format!("div_C(vec2(1.0, 0.0), {value})"),
        (ProductOp::Inv, Type::Quat) => format!("inv_H({value})"),
        (ProductOp::Inv, Type::Isom2) => format!("inv_Isom2({value})"),
        (ProductOp::Inv, Type::Isom3) => format!("inv_Isom3({value})"),
        (ProductOp::Inv, Type::Custom { name, .. }) => format!("inv_{name}({value})"),
        _ => unreachable!(),
    }
}

fn emit_component_scale(ty: &Type, value: &str, scalar: &str) -> String {
    match ty {
        Type::Custom { name, .. } => format!("scale_{name}({value}, {scalar})"),
        _ => format!("({value} * {scalar})"),
    }
}

fn product_binary_symbol(op: ProductOp) -> &'static str {
    match op {
        ProductOp::Add => "+",
        ProductOp::Sub => "-",
        ProductOp::Mult => "*",
        _ => unreachable!(),
    }
}

#[derive(Clone, Debug)]
struct ConcatHelper {
    element_ty: Type,
    left_len: usize,
    right_len: usize,
}

#[derive(Clone, Debug)]
struct ObjectBinding {
    expr: ObjectExpr,
    generated: bool,
}

impl ConcatHelper {
    fn from_expr(expr: &ValueExpr) -> Option<Self> {
        match expr {
            ValueExpr::Concat {
                element_ty,
                left,
                right,
            } => Some(Self {
                element_ty: element_ty.clone(),
                left_len: left.array_len()?,
                right_len: right.array_len()?,
            }),
            _ => None,
        }
    }
}

fn emit_value_binding_type(ty: &Type, name: &str, array_len: Option<usize>) -> String {
    match ty {
        Type::Array(element_ty) => match array_len {
            Some(len) => format!("{} {}[{len}]", element_ty.glsl_name(), name),
            None => format!("{} {}", ty.glsl_name(), name),
        },
        _ => format!("{} {}", ty.glsl_name(), name),
    }
}

fn emit_array_constructor(
    element_ty: &Type,
    elements: &[ValueExpr],
    helper_names: &HashMap<String, String>,
    value_names: &HashMap<String, String>,
) -> String {
    let rendered = elements
        .iter()
        .map(|element| emit_value_expr(element, helper_names, value_names))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{}[{}]({})",
        element_ty.glsl_name(),
        elements.len(),
        rendered
    )
}

fn emit_concat_helper(helper: &ConcatHelper) -> String {
    let result_len = helper.left_len + helper.right_len;
    format!(
        "{} {}({} left, {} right) {{\n    {} result;\n    for (int i = 0; i < {}; i++) {{\n        result[i] = left[i];\n    }}\n    for (int i = 0; i < {}; i++) {{\n        result[i + {}] = right[i];\n    }}\n    return result;\n}}",
        glsl_sized_array_type(&helper.element_ty, result_len),
        concat_helper_name(helper),
        glsl_sized_array_type(&helper.element_ty, helper.left_len),
        glsl_sized_array_type(&helper.element_ty, helper.right_len),
        glsl_sized_array_type(&helper.element_ty, result_len),
        helper.left_len,
        helper.right_len,
        helper.left_len
    )
}

fn glsl_sized_array_type(element_ty: &Type, len: usize) -> String {
    format!("{}[{len}]", element_ty.glsl_name())
}

fn concat_helper_name(helper: &ConcatHelper) -> String {
    format!(
        "concat_{}_{}_{}",
        sanitize_glsl_identifier(&helper.element_ty.type_name()),
        helper.left_len,
        helper.right_len
    )
}

fn sanitize_glsl_identifier(source: &str) -> String {
    source
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect()
}

fn collect_object_concat_helpers(expr: &ObjectExpr, helpers: &mut BTreeMap<String, ConcatHelper>) {
    match expr {
        ObjectExpr::Var(_) => {}
        ObjectExpr::Primitive { fields, .. } => {
            for (_, value) in fields {
                match value {
                    PrimitiveArgExpr::Value(value) => collect_concat_helpers(value, helpers),
                    PrimitiveArgExpr::Vec2List(vertices) => {
                        for vertex in vertices {
                            collect_concat_helpers(vertex, helpers);
                        }
                    }
                }
            }
        }
        ObjectExpr::AmbientTransform {
            object,
            translation,
            linear,
        } => {
            collect_concat_helpers(translation, helpers);
            collect_concat_helpers(linear, helpers);
            collect_object_concat_helpers(object, helpers);
        }
        ObjectExpr::IsometryTransform { object, transform } => {
            collect_concat_helpers(transform, helpers);
            collect_object_concat_helpers(object, helpers);
        }
        ObjectExpr::RegisteredOp {
            value_args,
            object_args,
            ..
        } => {
            for arg in value_args {
                collect_concat_helpers(arg, helpers);
            }
            for arg in object_args {
                collect_object_concat_helpers(arg, helpers);
            }
        }
    }
}

fn collect_concat_helpers(expr: &ValueExpr, helpers: &mut BTreeMap<String, ConcatHelper>) {
    if let Some(helper) = ConcatHelper::from_expr(expr) {
        helpers.insert(concat_helper_name(&helper), helper);
    }

    match expr {
        ValueExpr::Bool(_)
        | ValueExpr::Float(_)
        | ValueExpr::Int(_)
        | ValueExpr::Neutral { .. }
        | ValueExpr::Var { .. } => {}
        ValueExpr::Call { args, .. } => {
            for arg in args {
                collect_concat_helpers(arg, helpers);
            }
        }
        ValueExpr::MonoidPow { exponent, base, .. } => {
            collect_concat_helpers(exponent, helpers);
            collect_concat_helpers(base, helpers);
        }
        ValueExpr::BoolToNumberCast { value, .. } => {
            collect_concat_helpers(value, helpers);
        }
        ValueExpr::Conditional {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            collect_concat_helpers(condition, helpers);
            collect_concat_helpers(then_branch, helpers);
            collect_concat_helpers(else_branch, helpers);
        }
        ValueExpr::ObjectGetterCall {
            point, captures, ..
        } => {
            collect_concat_helpers(point, helpers);
            for capture in captures {
                collect_concat_helpers(capture, helpers);
            }
        }
        ValueExpr::FieldAccess { value, .. } => collect_concat_helpers(value, helpers),
        ValueExpr::Array { elements, .. } => {
            for element in elements {
                collect_concat_helpers(element, helpers);
            }
        }
        ValueExpr::Index { array, index, .. } => {
            collect_concat_helpers(array, helpers);
            collect_concat_helpers(index, helpers);
        }
        ValueExpr::Concat { left, right, .. } => {
            collect_concat_helpers(left, helpers);
            collect_concat_helpers(right, helpers);
        }
        ValueExpr::Binary { left, right, .. } => {
            collect_concat_helpers(left, helpers);
            collect_concat_helpers(right, helpers);
        }
        ValueExpr::Vec2(x, y) => {
            collect_concat_helpers(x, helpers);
            collect_concat_helpers(y, helpers);
        }
        ValueExpr::Vec3(x, y, z) => {
            collect_concat_helpers(x, helpers);
            collect_concat_helpers(y, helpers);
            collect_concat_helpers(z, helpers);
        }
        ValueExpr::Vec4(x, y, z, w) => {
            collect_concat_helpers(x, helpers);
            collect_concat_helpers(y, helpers);
            collect_concat_helpers(z, helpers);
            collect_concat_helpers(w, helpers);
        }
        ValueExpr::Matrix { rows, .. } => {
            for row in rows {
                collect_concat_helpers(row, helpers);
            }
        }
        ValueExpr::MatrixBasis { .. } | ValueExpr::UnitVectorBasis { .. } => {}
        ValueExpr::Derivative { epsilon, at, .. }
        | ValueExpr::Gradient { epsilon, at, .. }
        | ValueExpr::Divergence { epsilon, at, .. } => {
            collect_concat_helpers(epsilon, helpers);
            collect_concat_helpers(at, helpers);
        }
        ValueExpr::Partial { epsilon, at, .. } => {
            collect_concat_helpers(epsilon, helpers);
            collect_concat_helpers(at, helpers);
        }
    }
}

fn is_global_const_value_expr(expr: &ValueExpr, names: &BTreeSet<String>) -> bool {
    match expr {
        ValueExpr::Bool(_)
        | ValueExpr::Float(_)
        | ValueExpr::Int(_)
        | ValueExpr::Neutral { .. } => true,
        ValueExpr::Var { name, .. } => names.contains(name),
        ValueExpr::Vec2(x, y) => {
            is_global_const_value_expr(x, names) && is_global_const_value_expr(y, names)
        }
        ValueExpr::Vec3(x, y, z) => {
            is_global_const_value_expr(x, names)
                && is_global_const_value_expr(y, names)
                && is_global_const_value_expr(z, names)
        }
        ValueExpr::Vec4(x, y, z, w) => {
            is_global_const_value_expr(x, names)
                && is_global_const_value_expr(y, names)
                && is_global_const_value_expr(z, names)
                && is_global_const_value_expr(w, names)
        }
        ValueExpr::Matrix { rows, .. } => rows
            .iter()
            .all(|row| is_global_const_value_expr(row, names)),
        ValueExpr::MatrixBasis { .. } | ValueExpr::UnitVectorBasis { .. } => true,
        ValueExpr::Binary { left, right, .. } => {
            is_global_const_value_expr(left, names) && is_global_const_value_expr(right, names)
        }
        ValueExpr::BoolToNumberCast { value, .. } => is_global_const_value_expr(value, names),
        ValueExpr::Conditional {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            is_global_const_value_expr(condition, names)
                && is_global_const_value_expr(then_branch, names)
                && is_global_const_value_expr(else_branch, names)
        }
        ValueExpr::Array { .. }
        | ValueExpr::Index { .. }
        | ValueExpr::Concat { .. }
        | ValueExpr::Call { .. }
        | ValueExpr::MonoidPow { .. }
        | ValueExpr::ObjectGetterCall { .. }
        | ValueExpr::FieldAccess { .. }
        | ValueExpr::Derivative { .. }
        | ValueExpr::Partial { .. }
        | ValueExpr::Gradient { .. }
        | ValueExpr::Divergence { .. } => false,
    }
}

fn collect_object_value_refs(
    expr: &ObjectExpr,
    object_bindings: &BTreeMap<String, ObjectBinding>,
    names: &mut BTreeSet<String>,
) {
    match expr {
        ObjectExpr::Var(name) => {
            if let Some(binding) = object_bindings.get(name) {
                if !binding.generated {
                    collect_object_value_refs(&binding.expr, object_bindings, names);
                }
            }
        }
        ObjectExpr::Primitive { fields, .. } => {
            for (_, value) in fields {
                match value {
                    PrimitiveArgExpr::Value(value) => collect_value_refs(value, names),
                    PrimitiveArgExpr::Vec2List(vertices) => {
                        for vertex in vertices {
                            collect_value_refs(vertex, names);
                        }
                    }
                }
            }
        }
        ObjectExpr::AmbientTransform {
            object,
            translation,
            linear,
        } => {
            collect_value_refs(translation, names);
            collect_value_refs(linear, names);
            collect_object_value_refs(object, object_bindings, names);
        }
        ObjectExpr::IsometryTransform { object, transform } => {
            collect_value_refs(transform, names);
            collect_object_value_refs(object, object_bindings, names);
        }
        ObjectExpr::RegisteredOp {
            value_args,
            object_args,
            ..
        } => {
            for arg in value_args {
                collect_value_refs(arg, names);
            }
            for arg in object_args {
                collect_object_value_refs(arg, object_bindings, names);
            }
        }
    }
}

fn collect_value_refs(expr: &ValueExpr, names: &mut BTreeSet<String>) {
    match expr {
        ValueExpr::Bool(_)
        | ValueExpr::Float(_)
        | ValueExpr::Int(_)
        | ValueExpr::Neutral { .. } => {}
        ValueExpr::Var { name, .. } => {
            names.insert(name.clone());
        }
        ValueExpr::Call { args, .. } => {
            for arg in args {
                collect_value_refs(arg, names);
            }
        }
        ValueExpr::MonoidPow { exponent, base, .. } => {
            collect_value_refs(exponent, names);
            collect_value_refs(base, names);
        }
        ValueExpr::BoolToNumberCast { value, .. } => {
            collect_value_refs(value, names);
        }
        ValueExpr::Conditional {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            collect_value_refs(condition, names);
            collect_value_refs(then_branch, names);
            collect_value_refs(else_branch, names);
        }
        ValueExpr::ObjectGetterCall {
            point, captures, ..
        } => {
            collect_value_refs(point, names);
            for capture in captures {
                collect_value_refs(capture, names);
            }
        }
        ValueExpr::FieldAccess { value, .. } => collect_value_refs(value, names),
        ValueExpr::Array { elements, .. } => {
            for element in elements {
                collect_value_refs(element, names);
            }
        }
        ValueExpr::Index { array, index, .. } => {
            collect_value_refs(array, names);
            collect_value_refs(index, names);
        }
        ValueExpr::Concat { left, right, .. } => {
            collect_value_refs(left, names);
            collect_value_refs(right, names);
        }
        ValueExpr::Binary { left, right, .. } => {
            collect_value_refs(left, names);
            collect_value_refs(right, names);
        }
        ValueExpr::Vec2(x, y) => {
            collect_value_refs(x, names);
            collect_value_refs(y, names);
        }
        ValueExpr::Vec3(x, y, z) => {
            collect_value_refs(x, names);
            collect_value_refs(y, names);
            collect_value_refs(z, names);
        }
        ValueExpr::Vec4(x, y, z, w) => {
            collect_value_refs(x, names);
            collect_value_refs(y, names);
            collect_value_refs(z, names);
            collect_value_refs(w, names);
        }
        ValueExpr::Matrix { rows, .. } => {
            for row in rows {
                collect_value_refs(row, names);
            }
        }
        ValueExpr::MatrixBasis { .. } | ValueExpr::UnitVectorBasis { .. } => {}
        ValueExpr::Derivative { epsilon, at, .. }
        | ValueExpr::Gradient { epsilon, at, .. }
        | ValueExpr::Divergence { epsilon, at, .. } => {
            collect_value_refs(epsilon, names);
            collect_value_refs(at, names);
        }
        ValueExpr::Partial { epsilon, at, .. } => {
            collect_value_refs(epsilon, names);
            collect_value_refs(at, names);
        }
    }
}

fn collect_value_function_refs(expr: &ValueExpr, names: &mut BTreeSet<String>) {
    match expr {
        ValueExpr::Bool(_)
        | ValueExpr::Float(_)
        | ValueExpr::Int(_)
        | ValueExpr::Neutral { .. }
        | ValueExpr::Var { .. } => {}
        ValueExpr::Call { func, args, .. } => {
            names.insert(func.clone());
            for arg in args {
                collect_value_function_refs(arg, names);
            }
        }
        ValueExpr::MonoidPow { exponent, base, .. } => {
            collect_value_function_refs(exponent, names);
            collect_value_function_refs(base, names);
        }
        ValueExpr::BoolToNumberCast { value, .. } => {
            collect_value_function_refs(value, names);
        }
        ValueExpr::Conditional {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            collect_value_function_refs(condition, names);
            collect_value_function_refs(then_branch, names);
            collect_value_function_refs(else_branch, names);
        }
        ValueExpr::ObjectGetterCall {
            point, captures, ..
        } => {
            collect_value_function_refs(point, names);
            for capture in captures {
                collect_value_function_refs(capture, names);
            }
        }
        ValueExpr::FieldAccess { value, .. } => collect_value_function_refs(value, names),
        ValueExpr::Array { elements, .. } => {
            for element in elements {
                collect_value_function_refs(element, names);
            }
        }
        ValueExpr::Index { array, index, .. } => {
            collect_value_function_refs(array, names);
            collect_value_function_refs(index, names);
        }
        ValueExpr::Concat { left, right, .. } | ValueExpr::Binary { left, right, .. } => {
            collect_value_function_refs(left, names);
            collect_value_function_refs(right, names);
        }
        ValueExpr::Vec2(x, y) => {
            collect_value_function_refs(x, names);
            collect_value_function_refs(y, names);
        }
        ValueExpr::Vec3(x, y, z) => {
            collect_value_function_refs(x, names);
            collect_value_function_refs(y, names);
            collect_value_function_refs(z, names);
        }
        ValueExpr::Vec4(x, y, z, w) => {
            collect_value_function_refs(x, names);
            collect_value_function_refs(y, names);
            collect_value_function_refs(z, names);
            collect_value_function_refs(w, names);
        }
        ValueExpr::Matrix { rows, .. } => {
            for row in rows {
                collect_value_function_refs(row, names);
            }
        }
        ValueExpr::MatrixBasis { .. } | ValueExpr::UnitVectorBasis { .. } => {}
        ValueExpr::Derivative { epsilon, at, .. }
        | ValueExpr::Partial { epsilon, at, .. }
        | ValueExpr::Gradient { epsilon, at, .. }
        | ValueExpr::Divergence { epsilon, at, .. } => {
            collect_value_function_refs(epsilon, names);
            collect_value_function_refs(at, names);
        }
    }
}

fn collect_object_function_refs(
    expr: &ObjectExpr,
    object_bindings: &BTreeMap<String, ObjectBinding>,
    names: &mut BTreeSet<String>,
) {
    match expr {
        ObjectExpr::Var(name) => {
            if let Some(binding) = object_bindings.get(name) {
                if !binding.generated {
                    collect_object_function_refs(&binding.expr, object_bindings, names);
                }
            }
        }
        ObjectExpr::Primitive { fields, .. } => {
            for (_, value) in fields {
                match value {
                    PrimitiveArgExpr::Value(value) => collect_value_function_refs(value, names),
                    PrimitiveArgExpr::Vec2List(vertices) => {
                        for vertex in vertices {
                            collect_value_function_refs(vertex, names);
                        }
                    }
                }
            }
        }
        ObjectExpr::AmbientTransform {
            object,
            translation,
            linear,
        } => {
            collect_value_function_refs(translation, names);
            collect_value_function_refs(linear, names);
            collect_object_function_refs(object, object_bindings, names);
        }
        ObjectExpr::IsometryTransform { object, transform } => {
            collect_value_function_refs(transform, names);
            collect_object_function_refs(object, object_bindings, names);
        }
        ObjectExpr::RegisteredOp {
            value_args,
            object_args,
            ..
        } => {
            for arg in value_args {
                collect_value_function_refs(arg, names);
            }
            for arg in object_args {
                collect_object_function_refs(arg, object_bindings, names);
            }
        }
    }
}

fn collect_object_support(expr: &ObjectExpr, names: &mut BTreeSet<String>) {
    match expr {
        ObjectExpr::Var(_) => {}
        ObjectExpr::Primitive { name, fields, .. } => {
            names.insert(name.clone());
            for (_, value) in fields {
                match value {
                    PrimitiveArgExpr::Value(value) => collect_value_support(value, names),
                    PrimitiveArgExpr::Vec2List(vertices) => {
                        for vertex in vertices {
                            collect_value_support(vertex, names);
                        }
                    }
                }
            }
        }
        ObjectExpr::AmbientTransform {
            object,
            translation,
            linear,
        } => {
            collect_value_support(translation, names);
            collect_value_support(linear, names);
            collect_object_support(object, names);
        }
        ObjectExpr::IsometryTransform { object, transform } => {
            collect_value_support(transform, names);
            collect_object_support(object, names);
        }
        ObjectExpr::RegisteredOp {
            name,
            value_args,
            object_args,
            ..
        } => {
            names.insert(name.clone());
            for arg in value_args {
                collect_value_support(arg, names);
            }
            for arg in object_args {
                collect_object_support(arg, names);
            }
        }
    }
}

fn collect_value_support(expr: &ValueExpr, names: &mut BTreeSet<String>) {
    match expr {
        ValueExpr::Bool(_)
        | ValueExpr::Float(_)
        | ValueExpr::Int(_)
        | ValueExpr::Neutral { .. }
        | ValueExpr::Var { .. } => {
            if let ValueExpr::Neutral { kind, ty } = expr {
                collect_type_support(ty, names);
                if let Type::Custom { name, .. } = ty {
                    if let Some(op) = product_op_for_neutral(*kind) {
                        names.insert(product_type_op_support_name(name, op));
                    }
                }
            }
        }
        ValueExpr::Call { func, args, ty } => {
            collect_type_support(ty, names);
            if ty == &Type::Complex && complex_overload_support_glsl(func).is_some() {
                names.insert(format!("complex:{func}"));
                if func == "pow" {
                    names.insert("C".to_string());
                    names.insert("complex:exp".to_string());
                    names.insert("complex:log".to_string());
                }
            }
            if func != "rot" {
                names.insert(func.clone());
            }
            for arg in args {
                collect_value_support(arg, names);
            }
        }
        ValueExpr::MonoidPow { exponent, base, ty } => {
            collect_type_support(ty, names);
            names.insert(monoid_pow_support_name(ty));
            collect_value_support(exponent, names);
            collect_value_support(base, names);
        }
        ValueExpr::BoolToNumberCast { value, .. } => {
            collect_value_support(value, names);
        }
        ValueExpr::Conditional {
            condition,
            then_branch,
            else_branch,
            ty,
        } => {
            collect_type_support(ty, names);
            collect_value_support(condition, names);
            collect_value_support(then_branch, names);
            collect_value_support(else_branch, names);
        }
        ValueExpr::ObjectGetterCall {
            point,
            captures,
            ty,
            ..
        } => {
            collect_type_support(ty, names);
            collect_value_support(point, names);
            for capture in captures {
                collect_value_support(capture, names);
            }
        }
        ValueExpr::FieldAccess { value, ty, .. } => {
            collect_type_support(ty, names);
            collect_value_support(value, names);
        }
        ValueExpr::Array { elements, .. } => {
            for element in elements {
                collect_value_support(element, names);
            }
        }
        ValueExpr::Index { array, index, .. } => {
            collect_value_support(array, names);
            collect_value_support(index, names);
        }
        ValueExpr::Concat { left, right, .. } => {
            collect_value_support(left, names);
            collect_value_support(right, names);
        }
        ValueExpr::Binary {
            op, left, right, ..
        } => {
            if let Some(name) = binary_support_name(*op, &left.ty(), &right.ty()) {
                names.insert(name);
            }
            collect_value_support(left, names);
            collect_value_support(right, names);
        }
        ValueExpr::Vec2(x, y) => {
            collect_value_support(x, names);
            collect_value_support(y, names);
        }
        ValueExpr::Vec3(x, y, z) => {
            collect_value_support(x, names);
            collect_value_support(y, names);
            collect_value_support(z, names);
        }
        ValueExpr::Vec4(x, y, z, w) => {
            collect_value_support(x, names);
            collect_value_support(y, names);
            collect_value_support(z, names);
            collect_value_support(w, names);
        }
        ValueExpr::Matrix { rows, .. } => {
            for row in rows {
                collect_value_support(row, names);
            }
        }
        ValueExpr::MatrixBasis { .. } | ValueExpr::UnitVectorBasis { .. } => {}
        ValueExpr::Derivative {
            epsilon, func, at, ..
        }
        | ValueExpr::Gradient {
            epsilon, func, at, ..
        }
        | ValueExpr::Divergence { epsilon, func, at } => {
            collect_value_support(epsilon, names);
            collect_function_support(func, names);
            collect_value_support(at, names);
        }
        ValueExpr::Partial {
            epsilon, func, at, ..
        } => {
            collect_value_support(epsilon, names);
            collect_function_support(func, names);
            collect_value_support(at, names);
        }
    }
}

fn collect_function_support(func: &FunctionExpr, names: &mut BTreeSet<String>) {
    match &func.kind {
        FunctionExprKind::Named(name) => {
            names.insert(name.clone());
        }
        FunctionExprKind::Operator(op) => {
            if let Some((left, right)) = operator_function_support_types(&func.input) {
                if let Some(name) = binary_support_name(*op, &left, &right) {
                    names.insert(name);
                }
            }
        }
        FunctionExprKind::ObjectGetter { captures, .. } => {
            for capture in captures {
                collect_value_support(capture, names);
            }
        }
        FunctionExprKind::Compose(outer, inner) => {
            collect_function_support(outer, names);
            collect_function_support(inner, names);
        }
        FunctionExprKind::PointwiseBinary { left, right, .. } => {
            collect_pointwise_call_arg_support(left, names);
            collect_pointwise_call_arg_support(right, names);
        }
        FunctionExprKind::PointwiseCall { func, args } => {
            names.insert(func.clone());
            for arg in args {
                collect_pointwise_call_arg_support(arg, names);
            }
        }
        FunctionExprKind::PointwiseConditional {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_pointwise_call_arg_support(condition, names);
            collect_pointwise_call_arg_support(then_branch, names);
            collect_pointwise_call_arg_support(else_branch, names);
        }
        FunctionExprKind::ProductSameDomain(funcs) => {
            for func in funcs {
                collect_function_support(func, names);
            }
        }
        FunctionExprKind::ProductTensor(left, right) => {
            collect_function_support(left, names);
            collect_function_support(right, names);
        }
    }
}

fn collect_pointwise_call_arg_support(arg: &PointwiseCallArg, names: &mut BTreeSet<String>) {
    match arg {
        PointwiseCallArg::Function { func, .. } => collect_function_support(func, names),
        PointwiseCallArg::Value(value) => collect_value_support(value, names),
    }
}

fn operator_function_support_types(input: &Type) -> Option<(Type, Type)> {
    match input {
        Type::Vec2 => Some((Type::Float, Type::Float)),
        Type::Product(parts) if parts.len() == 2 => Some((parts[0].clone(), parts[1].clone())),
        _ => None,
    }
}

fn emit_value_expr(
    expr: &ValueExpr,
    helper_names: &HashMap<String, String>,
    value_names: &HashMap<String, String>,
) -> String {
    match expr {
        ValueExpr::Bool(value) => value.to_string(),
        ValueExpr::Float(value) => format_float(*value),
        ValueExpr::Int(value) => value.to_string(),
        ValueExpr::Neutral { kind, ty } => emit_neutral_value(*kind, ty),
        ValueExpr::Var { name, .. } => value_names
            .get(name)
            .cloned()
            .unwrap_or_else(|| name.clone()),
        ValueExpr::Call { func, args, .. } => {
            if let Some(reordered) =
                emit_scalar_first_min_max(func, args, helper_names, value_names)
            {
                return reordered;
            }
            let rendered_args = args
                .iter()
                .map(|arg| emit_value_expr(arg, helper_names, value_names))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "{}({})",
                emitted_function_name(func, helper_names),
                rendered_args
            )
        }
        ValueExpr::MonoidPow { exponent, base, ty } => format!(
            "{}({}, {})",
            monoid_pow_function_name(ty),
            emit_value_expr(exponent, helper_names, value_names),
            emit_value_expr(base, helper_names, value_names)
        ),
        ValueExpr::BoolToNumberCast { value, ty } => {
            let value = emit_value_expr(value, helper_names, value_names);
            match ty {
                Type::Float => format!("({value} ? 1.0 : 0.0)"),
                Type::Int => format!("({value} ? 1 : 0)"),
                _ => unreachable!(),
            }
        }
        ValueExpr::Conditional {
            condition,
            then_branch,
            else_branch,
            ..
        } => format!(
            "({} ? {} : {})",
            emit_value_expr(condition, helper_names, value_names),
            emit_value_expr(then_branch, helper_names, value_names),
            emit_value_expr(else_branch, helper_names, value_names)
        ),
        ValueExpr::ObjectGetterCall {
            object,
            getter,
            point,
            captures,
            ..
        } => {
            let function_name = match getter {
                ObjectGetter::Sdf => format!("sdf_{object}"),
                ObjectGetter::Grad => format!("grad_sdf_{object}"),
            };
            let rendered_args = std::iter::once(emit_value_expr(point, helper_names, value_names))
                .chain(
                    captures
                        .iter()
                        .map(|arg| emit_value_expr(arg, helper_names, value_names)),
                )
                .collect::<Vec<_>>()
                .join(", ");
            format!("{function_name}({rendered_args})")
        }
        ValueExpr::FieldAccess { value, field, .. } => {
            format!(
                "({}).{}",
                emit_value_expr(value, helper_names, value_names),
                field
            )
        }
        ValueExpr::Array {
            element_ty,
            elements,
        } => emit_array_constructor(element_ty, elements, helper_names, value_names),
        ValueExpr::Index { array, index, .. } => format!(
            "{}[{}]",
            emit_value_expr(array, helper_names, value_names),
            emit_value_expr(index, helper_names, value_names)
        ),
        ValueExpr::Concat {
            element_ty: _,
            left,
            right,
        } => format!(
            "{}({}, {})",
            concat_helper_name(&ConcatHelper::from_expr(expr).unwrap()),
            emit_value_expr(left, helper_names, value_names),
            emit_value_expr(right, helper_names, value_names)
        ),
        ValueExpr::Binary {
            op, left, right, ..
        } => {
            if *op == BinOp::Sub && is_float_literal(left, 0.0) {
                return format!("(-{})", emit_value_expr(right, helper_names, value_names));
            }
            emit_binary_expr(*op, left, right, helper_names, value_names)
        }
        ValueExpr::Vec2(x, y) => format!(
            "vec2({}, {})",
            emit_value_expr(x, helper_names, value_names),
            emit_value_expr(y, helper_names, value_names)
        ),
        ValueExpr::Vec3(x, y, z) => format!(
            "vec3({}, {}, {})",
            emit_value_expr(x, helper_names, value_names),
            emit_value_expr(y, helper_names, value_names),
            emit_value_expr(z, helper_names, value_names)
        ),
        ValueExpr::Vec4(x, y, z, w) => format!(
            "vec4({}, {}, {}, {})",
            emit_value_expr(x, helper_names, value_names),
            emit_value_expr(y, helper_names, value_names),
            emit_value_expr(z, helper_names, value_names),
            emit_value_expr(w, helper_names, value_names)
        ),
        ValueExpr::Matrix { columns, rows } => {
            emit_matrix(rows, *columns, helper_names, value_names)
        }
        ValueExpr::MatrixBasis { row, column, ty } => emit_matrix_basis(*row, *column, ty),
        ValueExpr::UnitVectorBasis {
            dimension,
            index,
            ty,
        } => emit_unit_vector_basis(*dimension, *index, ty),
        ValueExpr::Derivative {
            epsilon, func, at, ..
        } => emit_scalar_derivative(func, epsilon, at, helper_names, value_names),
        ValueExpr::Partial {
            axis,
            epsilon,
            func,
            at,
            ..
        } => emit_partial_derivative(*axis, func, epsilon, at, helper_names, value_names),
        ValueExpr::Gradient {
            epsilon, func, at, ..
        } => emit_gradient(func, epsilon, at, helper_names, value_names),
        ValueExpr::Divergence { epsilon, func, at } => {
            emit_divergence(func, epsilon, at, helper_names, value_names)
        }
    }
}

fn emit_scalar_first_min_max(
    func: &str,
    args: &[ValueExpr],
    helper_names: &HashMap<String, String>,
    value_names: &HashMap<String, String>,
) -> Option<String> {
    let [left, right] = args else {
        return None;
    };
    if !matches!(func, "min" | "max") || left.ty() != Type::Float || !is_vector_type(&right.ty()) {
        return None;
    }
    Some(format!(
        "{}({}, {})",
        emitted_function_name(func, helper_names),
        emit_value_expr(right, helper_names, value_names),
        emit_value_expr(left, helper_names, value_names)
    ))
}

fn is_vector_type(ty: &Type) -> bool {
    matches!(ty, Type::Vec2 | Type::Vec3 | Type::Vec4)
}

fn emit_plain_value_expr(expr: &ValueExpr, helper_names: &HashMap<String, String>) -> String {
    emit_value_expr(expr, helper_names, &HashMap::new())
}

fn emit_matrix(
    rows: &[ValueExpr],
    columns: usize,
    helper_names: &HashMap<String, String>,
    value_names: &HashMap<String, String>,
) -> String {
    let rendered_rows = rows
        .iter()
        .map(|row| emit_value_expr(row, helper_names, value_names))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "transpose({}({}))",
        matrix_constructor_type(rows.len(), columns),
        rendered_rows
    )
}

fn emit_matrix_basis(row: usize, column: usize, ty: &Type) -> String {
    let Type::Mat(rows, columns) = ty else {
        unreachable!("matrix basis literal has non-matrix type")
    };
    let values = (0..*columns)
        .flat_map(|current_column| {
            (0..*rows).map(move |current_row| {
                if current_row + 1 == row && current_column + 1 == column {
                    "1.0"
                } else {
                    "0.0"
                }
            })
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("{}({})", matrix_glsl_type(*rows, *columns), values)
}

fn emit_unit_vector_basis(dimension: usize, index: usize, ty: &Type) -> String {
    debug_assert_eq!(vector_type_dimension(ty), Some(dimension));
    let values = (1..=dimension)
        .map(|current| if current == index { "1.0" } else { "0.0" })
        .collect::<Vec<_>>()
        .join(", ");
    format!("{}({})", ty.glsl_name(), values)
}

fn emit_binary_expr(
    op: BinOp,
    left: &ValueExpr,
    right: &ValueExpr,
    helper_names: &HashMap<String, String>,
    value_names: &HashMap<String, String>,
) -> String {
    let left_ty = left.ty();
    let right_ty = right.ty();
    if left_ty == Type::Bool {
        if let Some(cast_ty) = bool_numeric_cast_type_for_emit(&right_ty) {
            let left = ValueExpr::BoolToNumberCast {
                value: Box::new(left.clone()),
                ty: cast_ty,
            };
            return emit_binary_expr(op, &left, right, helper_names, value_names);
        }
    }
    if right_ty == Type::Bool {
        if let Some(cast_ty) = bool_numeric_cast_type_for_emit(&left_ty) {
            let right = ValueExpr::BoolToNumberCast {
                value: Box::new(right.clone()),
                ty: cast_ty,
            };
            return emit_binary_expr(op, left, &right, helper_names, value_names);
        }
    }
    let left = emit_value_expr(left, helper_names, value_names);
    let right = emit_value_expr(right, helper_names, value_names);

    match (op, &left_ty, &right_ty) {
        (BinOp::Add | BinOp::Sub, Type::Bool, Type::Bool) => format!("({} != {})", left, right),
        (BinOp::Mul, Type::Bool, Type::Bool) => format!("({} && {})", left, right),
        (BinOp::Div, Type::Bool, Type::Bool) => left,
        (BinOp::Mul, Type::Complex, Type::Complex) => format!("mult_C({}, {})", left, right),
        (BinOp::Div, Type::Complex, Type::Complex) => format!("div_C({}, {})", left, right),
        (BinOp::Mul, Type::Quat, Type::Quat) => format!("mult_H({}, {})", left, right),
        (BinOp::Div, Type::Quat, Type::Quat) => format!("div_H({}, {})", left, right),
        (BinOp::Mul, Type::Isom2, Type::Isom2) => format!("mult_Isom2({}, {})", left, right),
        (BinOp::Mul, Type::Isom2, Type::Vec2) => format!("act_Isom2({}, {})", left, right),
        (BinOp::Mul, Type::Isom3, Type::Isom3) => format!("mult_Isom3({}, {})", left, right),
        (BinOp::Mul, Type::Isom3, Type::Vec3) => format!("act_Isom3({}, {})", left, right),
        (
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div,
            Type::Custom { .. },
            Type::Custom { .. },
        ) if left_ty == right_ty => emit_custom_binary_expr(op, &left_ty, &left, &right),
        (BinOp::Mul | BinOp::Div, Type::Custom { .. }, Type::Float) => {
            emit_custom_scale_expr(op, &left_ty, &left, &right)
        }
        (BinOp::Mul, Type::Float, Type::Custom { .. }) => {
            emit_custom_scale_expr(op, &right_ty, &right, &left)
        }
        (BinOp::Div, Type::Float, Type::Complex) => {
            format!("div_C({}, {})", scalar_to_algebra(&right_ty, &left), right)
        }
        (BinOp::Div, Type::Float, Type::Quat) => {
            format!("div_H({}, {})", scalar_to_algebra(&right_ty, &left), right)
        }
        (BinOp::Add | BinOp::Sub, Type::Complex | Type::Quat, Type::Float) => {
            format!(
                "({} {} {})",
                left,
                op.symbol(),
                scalar_to_algebra(&left_ty, &right)
            )
        }
        (BinOp::Add | BinOp::Sub, Type::Float, Type::Complex | Type::Quat) => {
            format!(
                "({} {} {})",
                scalar_to_algebra(&right_ty, &left),
                op.symbol(),
                right
            )
        }
        _ => format!("({} {} {})", left, op.symbol(), right),
    }
}

fn bool_numeric_cast_type_for_emit(other: &Type) -> Option<Type> {
    if other == &Type::Int {
        Some(Type::Int)
    } else if other == &Type::Float
        || has_category(other, AlgebraicCategory::VectR)
        || has_category(other, AlgebraicCategory::RAlg)
    {
        Some(Type::Float)
    } else {
        None
    }
}

fn emit_custom_binary_expr(op: BinOp, ty: &Type, left: &str, right: &str) -> String {
    let Type::Custom { name, .. } = ty else {
        unreachable!();
    };
    match op {
        BinOp::Add => format!("add_{}({}, {})", name, left, right),
        BinOp::Sub => format!("sub_{}({}, {})", name, left, right),
        BinOp::Mul => format!("mult_{}({}, {})", name, left, right),
        BinOp::Div => format!("mult_{}({}, inv_{}({}))", name, left, name, right),
        BinOp::Eq
        | BinOp::Ne
        | BinOp::Lt
        | BinOp::Le
        | BinOp::Gt
        | BinOp::Ge
        | BinOp::Product
        | BinOp::Compose => unreachable!(),
    }
}

fn emit_custom_scale_expr(op: BinOp, ty: &Type, value: &str, scalar: &str) -> String {
    let Type::Custom { name, .. } = ty else {
        unreachable!();
    };
    match op {
        BinOp::Mul => format!("scale_{}({}, {})", name, value, scalar),
        BinOp::Div => format!("scale_{}({}, (1.0 / {}))", name, value, scalar),
        BinOp::Add
        | BinOp::Sub
        | BinOp::Eq
        | BinOp::Ne
        | BinOp::Lt
        | BinOp::Le
        | BinOp::Gt
        | BinOp::Ge
        | BinOp::Product
        | BinOp::Compose => unreachable!(),
    }
}

fn scalar_to_algebra(ty: &Type, value: &str) -> String {
    match ty {
        Type::Complex => format!("vec2({}, 0.0)", value),
        Type::Quat => format!("vec4({}, 0.0, 0.0, 0.0)", value),
        _ => value.to_string(),
    }
}

fn emit_neutral_value(kind: NeutralKind, ty: &Type) -> String {
    match (kind, ty) {
        (NeutralKind::Zero, Type::Bool) => "false".to_string(),
        (NeutralKind::One, Type::Bool) => "true".to_string(),
        (NeutralKind::Zero, Type::Float) => "0.0".to_string(),
        (NeutralKind::One, Type::Float) => "1.0".to_string(),
        (NeutralKind::Zero, Type::Int) => "0".to_string(),
        (NeutralKind::One, Type::Int) => "1".to_string(),
        (NeutralKind::Zero, Type::Complex) => "vec2(0.0, 0.0)".to_string(),
        (NeutralKind::One, Type::Complex) => "vec2(1.0, 0.0)".to_string(),
        (NeutralKind::Zero, Type::Quat) => "vec4(0.0, 0.0, 0.0, 0.0)".to_string(),
        (NeutralKind::One, Type::Quat) => "vec4(1.0, 0.0, 0.0, 0.0)".to_string(),
        (NeutralKind::Zero, Type::Vec2) => "vec2(0.0)".to_string(),
        (NeutralKind::Zero, Type::Vec3) => "vec3(0.0)".to_string(),
        (NeutralKind::Zero, Type::Vec4) => "vec4(0.0)".to_string(),
        (NeutralKind::Zero, Type::Mat(rows, columns)) => matrix_zero_expr(*rows, *columns),
        (NeutralKind::Identity, Type::Mat(rows, columns)) if rows == columns => {
            format!("mat{}(1.0)", rows)
        }
        (NeutralKind::Identity, Type::Isom2) => "Isom2(mat2(1.0), vec2(0.0))".to_string(),
        (NeutralKind::Identity, Type::Isom3) => "Isom3(mat3(1.0), vec3(0.0))".to_string(),
        (_, Type::Custom { name, .. }) => match kind {
            NeutralKind::Zero => format!("zero_{name}"),
            NeutralKind::One => format!("one_{name}"),
            NeutralKind::Identity => format!("e_{name}"),
        },
        _ => unreachable!("unsupported neutral literal"),
    }
}

fn matrix_zero_expr(rows: usize, columns: usize) -> String {
    if rows == columns {
        format!("mat{rows}(0.0)")
    } else {
        format!("{}(0.0)", matrix_glsl_type(rows, columns))
    }
}

fn binary_support_name(op: BinOp, left: &Type, right: &Type) -> Option<String> {
    match (op, left, right) {
        (BinOp::Mul | BinOp::Div, Type::Complex, Type::Complex)
        | (BinOp::Div, Type::Float, Type::Complex) => Some("C".to_string()),
        (BinOp::Mul | BinOp::Div, Type::Quat, Type::Quat)
        | (BinOp::Div, Type::Float, Type::Quat) => Some("H".to_string()),
        (BinOp::Mul, Type::Isom2, Type::Isom2) | (BinOp::Mul, Type::Isom2, Type::Vec2) => {
            Some("Isom2".to_string())
        }
        (BinOp::Mul, Type::Isom3, Type::Isom3) | (BinOp::Mul, Type::Isom3, Type::Vec3) => {
            Some("Isom3".to_string())
        }
        (
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div,
            Type::Custom { name, .. },
            Type::Custom { .. },
        ) if left == right => product_op_for_binary(op)
            .map(|product_op| product_type_op_support_name(name, product_op)),
        (BinOp::Mul | BinOp::Div, Type::Custom { name, .. }, Type::Float) => {
            Some(product_type_op_support_name(name, ProductOp::Scale))
        }
        (BinOp::Mul, Type::Float, Type::Custom { name, .. }) => {
            Some(product_type_op_support_name(name, ProductOp::Scale))
        }
        _ => None,
    }
}

fn emit_function_application(
    func: &FunctionExpr,
    arg: ValueExpr,
    helper_names: &HashMap<String, String>,
    value_names: &HashMap<String, String>,
) -> String {
    emit_value_expr(&apply_function_expr(func, arg), helper_names, value_names)
}

fn emit_sdf_gradient_expr(
    function_name: &str,
    point_name: &str,
    epsilon_name: &str,
    scene_input_names: &[String],
    dimension: ShapeDimension,
) -> String {
    match dimension {
        ShapeDimension::D2 => format!(
            "vec2({}, {})",
            emit_sdf_partial_derivative(
                function_name,
                point_name,
                epsilon_name,
                scene_input_names,
                dimension,
                0
            ),
            emit_sdf_partial_derivative(
                function_name,
                point_name,
                epsilon_name,
                scene_input_names,
                dimension,
                1
            ),
        ),
        ShapeDimension::D3 => format!(
            "vec3({}, {}, {})",
            emit_sdf_partial_derivative(
                function_name,
                point_name,
                epsilon_name,
                scene_input_names,
                dimension,
                0
            ),
            emit_sdf_partial_derivative(
                function_name,
                point_name,
                epsilon_name,
                scene_input_names,
                dimension,
                1
            ),
            emit_sdf_partial_derivative(
                function_name,
                point_name,
                epsilon_name,
                scene_input_names,
                dimension,
                2
            ),
        ),
    }
}

fn emit_sdf_partial_derivative(
    function_name: &str,
    point_name: &str,
    epsilon_name: &str,
    scene_input_names: &[String],
    dimension: ShapeDimension,
    axis: usize,
) -> String {
    let offset = match (dimension, axis) {
        (ShapeDimension::D2, 0) => format!("vec2({}, 0.0)", epsilon_name),
        (ShapeDimension::D2, 1) => format!("vec2(0.0, {})", epsilon_name),
        (ShapeDimension::D3, 0) => format!("vec3({}, 0.0, 0.0)", epsilon_name),
        (ShapeDimension::D3, 1) => format!("vec3(0.0, {}, 0.0)", epsilon_name),
        (ShapeDimension::D3, 2) => format!("vec3(0.0, 0.0, {})", epsilon_name),
        _ => unreachable!(),
    };
    let forward = emit_sdf_call(
        function_name,
        &format!("{} + {}", point_name, offset),
        scene_input_names,
    );
    let backward = emit_sdf_call(
        function_name,
        &format!("{} - {}", point_name, offset),
        scene_input_names,
    );
    format!("(({} - {}) / (2.0 * {}))", forward, backward, epsilon_name)
}

fn emit_sdf_call(function_name: &str, point_expr: &str, scene_input_names: &[String]) -> String {
    let args = std::iter::once(point_expr.to_string())
        .chain(scene_input_names.iter().cloned())
        .collect::<Vec<_>>()
        .join(", ");
    format!("{}({})", function_name, args)
}

fn emit_scalar_derivative(
    func: &FunctionExpr,
    epsilon: &ValueExpr,
    at: &ValueExpr,
    helper_names: &HashMap<String, String>,
    value_names: &HashMap<String, String>,
) -> String {
    emit_derivative(func, epsilon, at, helper_names, value_names)
}

fn emit_partial_derivative(
    axis: usize,
    func: &FunctionExpr,
    epsilon: &ValueExpr,
    at: &ValueExpr,
    helper_names: &HashMap<String, String>,
    value_names: &HashMap<String, String>,
) -> String {
    emit_axis_derivative(axis, func, epsilon, at, helper_names, value_names)
}

fn emit_gradient(
    func: &FunctionExpr,
    epsilon: &ValueExpr,
    at: &ValueExpr,
    helper_names: &HashMap<String, String>,
    value_names: &HashMap<String, String>,
) -> String {
    emit_derivative(func, epsilon, at, helper_names, value_names)
}

fn emit_derivative(
    func: &FunctionExpr,
    epsilon: &ValueExpr,
    at: &ValueExpr,
    helper_names: &HashMap<String, String>,
    value_names: &HashMap<String, String>,
) -> String {
    let input_dim = derivative_emit_dimension(&func.input).unwrap();
    let output_dim = derivative_emit_dimension(&func.output).unwrap();
    if input_dim == 1 {
        return emit_axis_derivative(0, func, epsilon, at, helper_names, value_names);
    }
    if output_dim == 1 {
        let components = (0..input_dim)
            .map(|axis| emit_axis_derivative(axis, func, epsilon, at, helper_names, value_names))
            .collect::<Vec<_>>();
        return format!(
            "{}({})",
            vector_constructor(input_dim),
            components.join(", ")
        );
    }
    let rows = (0..input_dim)
        .map(|axis| emit_axis_derivative(axis, func, epsilon, at, helper_names, value_names))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "transpose({}({}))",
        matrix_constructor_type(input_dim, output_dim),
        rows
    )
}

fn emit_axis_derivative(
    axis: usize,
    func: &FunctionExpr,
    epsilon: &ValueExpr,
    at: &ValueExpr,
    helper_names: &HashMap<String, String>,
    value_names: &HashMap<String, String>,
) -> String {
    let plus = emit_axis_offset(at.clone(), epsilon.clone(), axis, BinOp::Add);
    let minus = emit_axis_offset(at.clone(), epsilon.clone(), axis, BinOp::Sub);
    let twice = ValueExpr::Binary {
        op: BinOp::Mul,
        left: Box::new(ValueExpr::Float(2.0)),
        right: Box::new(epsilon.clone()),
        ty: Type::Float,
    };
    format!(
        "(({} - {}) / {})",
        emit_function_application(func, plus, helper_names, value_names),
        emit_function_application(func, minus, helper_names, value_names),
        emit_value_expr(&twice, helper_names, value_names)
    )
}

fn derivative_emit_dimension(ty: &Type) -> Option<usize> {
    match ty {
        Type::Float => Some(1),
        Type::Vec2 => Some(2),
        Type::Vec3 => Some(3),
        Type::Vec4 => Some(4),
        _ => None,
    }
}

fn vector_constructor(dimension: usize) -> &'static str {
    match dimension {
        2 => "vec2",
        3 => "vec3",
        4 => "vec4",
        _ => unreachable!(),
    }
}

fn emit_axis_offset(base: ValueExpr, epsilon: ValueExpr, axis: usize, op: BinOp) -> ValueExpr {
    let ty = base.ty();
    let offset = match ty {
        Type::Float => epsilon,
        Type::Vec2 => ValueExpr::Vec2(
            Box::new(if axis == 0 {
                epsilon.clone()
            } else {
                ValueExpr::Float(0.0)
            }),
            Box::new(if axis == 1 {
                epsilon
            } else {
                ValueExpr::Float(0.0)
            }),
        ),
        Type::Vec3 => ValueExpr::Vec3(
            Box::new(if axis == 0 {
                epsilon.clone()
            } else {
                ValueExpr::Float(0.0)
            }),
            Box::new(if axis == 1 {
                epsilon.clone()
            } else {
                ValueExpr::Float(0.0)
            }),
            Box::new(if axis == 2 {
                epsilon
            } else {
                ValueExpr::Float(0.0)
            }),
        ),
        Type::Vec4 => ValueExpr::Vec4(
            Box::new(if axis == 0 {
                epsilon.clone()
            } else {
                ValueExpr::Float(0.0)
            }),
            Box::new(if axis == 1 {
                epsilon.clone()
            } else {
                ValueExpr::Float(0.0)
            }),
            Box::new(if axis == 2 {
                epsilon.clone()
            } else {
                ValueExpr::Float(0.0)
            }),
            Box::new(if axis == 3 {
                epsilon
            } else {
                ValueExpr::Float(0.0)
            }),
        ),
        _ => unreachable!(),
    };
    ValueExpr::Binary {
        op,
        left: Box::new(base),
        right: Box::new(offset),
        ty,
    }
}

fn emit_divergence(
    func: &FunctionExpr,
    epsilon: &ValueExpr,
    at: &ValueExpr,
    helper_names: &HashMap<String, String>,
    value_names: &HashMap<String, String>,
) -> String {
    let dimension = derivative_emit_dimension(&func.input).unwrap();
    let twice = ValueExpr::Binary {
        op: BinOp::Mul,
        left: Box::new(ValueExpr::Float(2.0)),
        right: Box::new(epsilon.clone()),
        ty: Type::Float,
    };
    let denom = emit_value_expr(&twice, helper_names, value_names);
    let components = ["x", "y", "z", "w"];
    let terms = (0..dimension)
        .map(|axis| {
            let plus = emit_function_application(
                func,
                emit_axis_offset(at.clone(), epsilon.clone(), axis, BinOp::Add),
                helper_names,
                value_names,
            );
            let minus = emit_function_application(
                func,
                emit_axis_offset(at.clone(), epsilon.clone(), axis, BinOp::Sub),
                helper_names,
                value_names,
            );
            let component = components[axis];
            format!("(({plus}).{component} - ({minus}).{component}) / {denom}")
        })
        .collect::<Vec<_>>();
    format!("({})", terms.join(" + "))
}

fn emit_object_expr(
    expr: &ObjectExpr,
    point_expr: &str,
    ambient_dimension: ShapeDimension,
    object_bindings: &BTreeMap<String, ObjectBinding>,
    helper_names: &HashMap<String, String>,
    scene_input_names: &[String],
) -> String {
    match expr {
        ObjectExpr::Var(name) => object_bindings
            .get(name)
            .map(|binding| {
                if binding.generated {
                    emit_generated_object_call(name, point_expr, scene_input_names)
                } else {
                    emit_object_expr(
                        &binding.expr,
                        point_expr,
                        ambient_dimension,
                        object_bindings,
                        helper_names,
                        scene_input_names,
                    )
                }
            })
            .unwrap_or_else(|| format!("obj_{}", name)),
        ObjectExpr::Primitive { name, kind, fields } => match kind {
            PrimitiveKind::ParamStruct(param_type) => {
                if name == "Plane3D" {
                    let normal = primitive_value_field(fields, "n");
                    let origin = primitive_value_field(fields, "origin");
                    let normal_expr = emit_plain_value_expr(normal, helper_names);
                    let origin_expr = emit_plain_value_expr(origin, helper_names);
                    return format!(
                        "sdf0_Plane3D({}, ParamPlane3D({}, (-dot(normalize({}), {}))))",
                        point_expr, normal_expr, normal_expr, origin_expr
                    );
                }
                let rendered_fields = fields
                    .iter()
                    .map(|(_, expr)| match expr {
                        PrimitiveArgExpr::Value(expr) => emit_plain_value_expr(expr, helper_names),
                        PrimitiveArgExpr::Vec2List(_) => unreachable!(),
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let point_arg = if registry::shape_dimension(name) == ShapeDimension::D2
                    && ambient_dimension == ShapeDimension::D3
                {
                    format!("({}).xy", point_expr)
                } else {
                    point_expr.to_string()
                };
                format!(
                    "sdf0_{}({}, {}({}))",
                    name, point_arg, param_type, rendered_fields
                )
            }
            PrimitiveKind::Polygon2D => {
                let vertices = fields
                    .iter()
                    .find_map(|(field_name, expr)| match (field_name.as_str(), expr) {
                        ("points", PrimitiveArgExpr::Vec2List(vertices)) => Some(vertices),
                        _ => None,
                    })
                    .unwrap();
                format!(
                    "sdf0_Polygon2D({}, {}, {})",
                    primitive_2d_point_arg(point_expr, ambient_dimension),
                    emit_polygon_vertices(vertices, helper_names),
                    vertices.len()
                )
            }
        },
        ObjectExpr::AmbientTransform {
            object,
            translation,
            linear,
        } => {
            let transformed_point =
                emit_transformed_point(point_expr, translation, linear, helper_names);
            emit_object_expr(
                object,
                &transformed_point,
                ambient_dimension,
                object_bindings,
                helper_names,
                scene_input_names,
            )
        }
        ObjectExpr::IsometryTransform { object, transform } => {
            let type_name = if ambient_dimension == ShapeDimension::D2 {
                "Isom2"
            } else {
                "Isom3"
            };
            let inverse = format!(
                "inv_{}({})",
                type_name,
                emit_plain_value_expr(transform, helper_names)
            );
            let transformed_point = format!("act_{}({}, {})", type_name, inverse, point_expr);
            emit_object_expr(
                object,
                &transformed_point,
                ambient_dimension,
                object_bindings,
                helper_names,
                scene_input_names,
            )
        }
        ObjectExpr::RegisteredOp {
            name: _,
            glsl_name,
            value_args,
            object_args,
        } => {
            if glsl_name == "_op_revolution" {
                let offset = emit_plain_value_expr(&value_args[0], helper_names);
                let revolved_point = format!("_op_revolution_point({}, {})", point_expr, offset);
                return emit_object_expr(
                    &object_args[0],
                    &revolved_point,
                    ambient_dimension,
                    object_bindings,
                    helper_names,
                    scene_input_names,
                );
            }
            if glsl_name == "_op_extrusion" {
                let height = emit_plain_value_expr(&value_args[0], helper_names);
                let base_point = format!("vec3(({}).xy, 0.0)", point_expr);
                let base_distance = emit_object_expr(
                    &object_args[0],
                    &base_point,
                    ambient_dimension,
                    object_bindings,
                    helper_names,
                    scene_input_names,
                );
                return format!(
                    "_op_extrusion({}, ({}).z, {})",
                    base_distance, point_expr, height
                );
            }
            if glsl_name == "_op_rot" {
                let binormal = emit_plain_value_expr(&value_args[0], helper_names);
                let anchor = emit_plain_value_expr(&value_args[1], helper_names);
                let angle = emit_plain_value_expr(&value_args[2], helper_names);
                let rotated_point = format!(
                    "_op_rot_inverse_point({}, {}, {}, {})",
                    point_expr, binormal, anchor, angle
                );
                return emit_object_expr(
                    &object_args[0],
                    &rotated_point,
                    ambient_dimension,
                    object_bindings,
                    helper_names,
                    scene_input_names,
                );
            }
            if glsl_name == "_op_rot2D" {
                let anchor = emit_plain_value_expr(&value_args[0], helper_names);
                let angle = emit_plain_value_expr(&value_args[1], helper_names);
                let rotated_point = if ambient_dimension == ShapeDimension::D2 {
                    format!(
                        "({} + (transpose(_op_rot2D_matrix({})) * ({} - {})))",
                        anchor, angle, point_expr, anchor
                    )
                } else {
                    format!(
                        "_op_rot2D_inverse_point({}, {}, {})",
                        point_expr, anchor, angle
                    )
                };
                return emit_object_expr(
                    &object_args[0],
                    &rotated_point,
                    ambient_dimension,
                    object_bindings,
                    helper_names,
                    scene_input_names,
                );
            }
            let mut args = object_args
                .iter()
                .map(|arg| {
                    emit_object_expr(
                        arg,
                        point_expr,
                        ambient_dimension,
                        object_bindings,
                        helper_names,
                        scene_input_names,
                    )
                })
                .collect::<Vec<_>>();
            args.extend(
                value_args
                    .iter()
                    .map(|arg| emit_plain_value_expr(arg, helper_names)),
            );
            format!("{}({})", glsl_name, args.join(", "))
        }
    }
}

fn emit_generated_object_call(
    name: &str,
    point_expr: &str,
    scene_input_names: &[String],
) -> String {
    let mut args = vec![point_expr.to_string()];
    args.extend(scene_input_names.iter().cloned());
    format!("sdf_{}({})", name, args.join(", "))
}

fn ambient_vector_glsl_type(dimension: ShapeDimension) -> &'static str {
    match dimension {
        ShapeDimension::D2 => "vec2",
        ShapeDimension::D3 => "vec3",
    }
}

fn primitive_2d_point_arg(point_expr: &str, ambient_dimension: ShapeDimension) -> String {
    match ambient_dimension {
        ShapeDimension::D2 => point_expr.to_string(),
        ShapeDimension::D3 => format!("{}.xy", point_expr),
    }
}

fn primitive_value_field<'a>(
    fields: &'a [(String, PrimitiveArgExpr)],
    name: &str,
) -> &'a ValueExpr {
    fields
        .iter()
        .find_map(|(field_name, expr)| match (field_name.as_str(), expr) {
            (candidate, PrimitiveArgExpr::Value(expr)) if candidate == name => Some(expr),
            _ => None,
        })
        .unwrap()
}

fn emit_polygon_vertices(vertices: &[ValueExpr], helper_names: &HashMap<String, String>) -> String {
    let mut rendered = vertices
        .iter()
        .map(|vertex| emit_plain_value_expr(vertex, helper_names))
        .collect::<Vec<_>>();
    let fill = rendered
        .last()
        .cloned()
        .unwrap_or_else(|| "vec2(0.0, 0.0)".to_string());
    while rendered.len() < 16 {
        rendered.push(fill.clone());
    }
    format!("vec2[16]({})", rendered.join(", "))
}

fn emit_transformed_point(
    point_expr: &str,
    translation: &ValueExpr,
    linear: &ValueExpr,
    helper_names: &HashMap<String, String>,
) -> String {
    if is_identity_mat2(linear) {
        return format!(
            "({} - {})",
            point_expr,
            emit_plain_value_expr(translation, helper_names)
        );
    }
    if is_identity_mat3(linear) {
        return format!(
            "({} - {})",
            point_expr,
            emit_plain_value_expr(translation, helper_names)
        );
    }
    if is_zero_vec2(translation) {
        return format!(
            "(transpose({}) * {})",
            emit_plain_value_expr(linear, helper_names),
            point_expr
        );
    }
    if is_zero_vec3(translation) {
        return format!(
            "(transpose({}) * {})",
            emit_plain_value_expr(linear, helper_names),
            point_expr
        );
    }
    format!(
        "(transpose({}) * (({}) - {}))",
        emit_plain_value_expr(linear, helper_names),
        point_expr,
        emit_plain_value_expr(translation, helper_names)
    )
}

fn is_zero_vec2(expr: &ValueExpr) -> bool {
    match expr {
        ValueExpr::Vec2(x, y) => is_float_literal(x, 0.0) && is_float_literal(y, 0.0),
        _ => false,
    }
}

fn is_zero_vec3(expr: &ValueExpr) -> bool {
    match expr {
        ValueExpr::Vec3(x, y, z) => {
            is_float_literal(x, 0.0) && is_float_literal(y, 0.0) && is_float_literal(z, 0.0)
        }
        _ => false,
    }
}

fn is_identity_mat2(expr: &ValueExpr) -> bool {
    match expr {
        ValueExpr::Matrix { columns: 2, rows } if rows.len() == 2 => {
            is_vec2_literal(&rows[0], [1.0, 0.0]) && is_vec2_literal(&rows[1], [0.0, 1.0])
        }
        _ => false,
    }
}

fn is_identity_mat3(expr: &ValueExpr) -> bool {
    match expr {
        ValueExpr::Matrix { columns: 3, rows } if rows.len() == 3 => {
            is_vec3_literal(&rows[0], [1.0, 0.0, 0.0])
                && is_vec3_literal(&rows[1], [0.0, 1.0, 0.0])
                && is_vec3_literal(&rows[2], [0.0, 0.0, 1.0])
        }
        _ => false,
    }
}

fn is_vec2_literal(expr: &ValueExpr, expected: [f64; 2]) -> bool {
    match expr {
        ValueExpr::Vec2(x, y) => {
            is_float_literal(x, expected[0]) && is_float_literal(y, expected[1])
        }
        _ => false,
    }
}

fn is_vec3_literal(expr: &ValueExpr, expected: [f64; 3]) -> bool {
    match expr {
        ValueExpr::Vec3(x, y, z) => {
            is_float_literal(x, expected[0])
                && is_float_literal(y, expected[1])
                && is_float_literal(z, expected[2])
        }
        _ => false,
    }
}

fn is_float_literal(expr: &ValueExpr, expected: f64) -> bool {
    matches!(expr, ValueExpr::Float(value) if (*value - expected).abs() < f64::EPSILON)
}

fn emitted_function_name(name: &str, helper_names: &HashMap<String, String>) -> String {
    helper_names
        .get(name)
        .cloned()
        .unwrap_or_else(|| name.to_string())
}

fn collect_raw_glsl_placeholders(body: &str, names: &mut BTreeSet<String>) {
    let mut index = 0;
    while let Some(relative_start) = body[index..].find("${") {
        let name_start = index + relative_start + 2;
        let Some(relative_end) = body[name_start..].find('}') else {
            return;
        };
        let end = name_start + relative_end;
        let name = &body[name_start..end];
        if is_placeholder_ident(name) {
            names.insert(name.to_string());
        }
        index = end + 1;
    }
}

fn render_raw_glsl_function(body: &str, func: &TypedFunc) -> String {
    let rendered_body = render_raw_glsl_placeholders(body, &func.name);
    let signature = raw_glsl_function_signature(func);
    format!(
        "{signature} {{\n{}\n}}",
        indent_raw_glsl_body(&rendered_body)
    )
}

fn raw_glsl_function_signature(func: &TypedFunc) -> String {
    let output = if func.output == Type::Unit {
        "void".to_string()
    } else {
        func.output.glsl_name()
    };
    let name = if func.input == Type::Unit && func.output == Type::Unit {
        "main".to_string()
    } else {
        helper_name(&func.name)
    };
    format!(
        "{output} {}({})",
        name,
        emit_raw_glsl_signature_params(&func.input)
    )
}

fn emit_func_signature_params(input: &Type, locals: &EmitLocals) -> String {
    match input {
        Type::Unit => String::new(),
        Type::Product(parts) => parts
            .iter()
            .enumerate()
            .map(|(index, ty)| format!("{} _t{index}", ty.glsl_name()))
            .collect::<Vec<_>>()
            .join(", "),
        ty => format!("{} {}", ty.glsl_name(), locals.func_param),
    }
}

fn emit_raw_glsl_signature_params(input: &Type) -> String {
    match input {
        Type::Unit => String::new(),
        Type::Product(parts) => parts
            .iter()
            .enumerate()
            .map(|(index, ty)| format!("{} _t{index}", ty.glsl_name()))
            .collect::<Vec<_>>()
            .join(", "),
        ty => format!("{} _t", ty.glsl_name()),
    }
}

fn indent_raw_glsl_body(body: &str) -> String {
    body.trim()
        .lines()
        .map(|line| {
            if line.trim().is_empty() {
                String::new()
            } else {
                format!("    {}", line)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_raw_glsl_placeholders(body: &str, _function_name: &str) -> String {
    let mut out = String::with_capacity(body.len());
    let mut index = 0;
    while let Some(relative_start) = body[index..].find("${") {
        let start = index + relative_start;
        out.push_str(&body[index..start]);
        let name_start = start + 2;
        let Some(relative_end) = body[name_start..].find('}') else {
            out.push_str(&body[start..]);
            return out;
        };
        let end = name_start + relative_end;
        let name = &body[name_start..end];
        if let Some(getter) = raw_glsl_object_getter_placeholder(name) {
            out.push_str(getter);
        } else if is_placeholder_ident(name) {
            out.push_str(name);
        } else {
            out.push_str(&body[start..=end]);
        }
        index = end + 1;
    }
    out.push_str(&body[index..]);
    out
}

fn raw_glsl_object_getter_placeholder(name: &str) -> Option<&'static str> {
    let (_, getter) = name.split_once('.')?;
    match getter {
        "sdf" => Some("scene_sdf"),
        "grad" => Some("scene_grad"),
        _ => None,
    }
}

fn helper_name(name: &str) -> String {
    name.to_string()
}

fn unique_local_name(base: &str, forbidden: &BTreeSet<String>) -> String {
    if !forbidden.contains(base) {
        return base.to_string();
    }

    let mut counter = 0u64;
    loop {
        let candidate = format!(
            "{}_r{:06x}",
            base,
            stable_name_hash(base, forbidden, counter)
        );
        if !forbidden.contains(&candidate) {
            return candidate;
        }
        counter += 1;
    }
}

fn stable_name_hash(base: &str, forbidden: &BTreeSet<String>, counter: u64) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in base.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash ^= counter;
    hash = hash.wrapping_mul(0x100000001b3);
    for name in forbidden {
        for byte in name.bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash ^= u64::from(b'|');
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash & 0x00ff_ffff
}

fn format_float(value: f64) -> String {
    if value.fract().abs() < f64::EPSILON {
        format!("{value:.1}")
    } else {
        value.to_string()
    }
}
