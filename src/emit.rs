use super::*;

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
        for line in self.emit_global_value_binding_lines(&global_value_names, &self.func_names()) {
            lines.push(line);
        }
        if !global_value_names.is_empty() {
            lines.push(String::new());
        }

        for func in &self.funcs {
            let func_var_names = HashMap::from([("t".to_string(), locals.func_param.clone())]);
            lines.push(format!(
                "{} {}(float {}) {{",
                func.output.glsl_name(),
                helper_name(&func.name),
                locals.func_param
            ));
            lines.push(format!(
                "    return {};",
                emit_value_expr(&func.expr, &self.func_names(), &func_var_names)
            ));
            lines.push("}".to_string());
            lines.push(String::new());
        }

        let signature = self.scene_signature(&locals.point);
        let scene_input_names = self.scene_input_names();
        let object_bindings = self.object_bindings();
        let helper_names = self.func_names();

        for binding in &self.bindings {
            if !binding.generated {
                continue;
            }
            lines.extend(self.emit_generated_binding(
                binding,
                &object_bindings,
                &helper_names,
                &scene_input_names,
                &global_value_names,
                &locals,
            ));
        }

        lines.push(format!("float scene_sdf({}) {{", signature.join(", ")));
        let scene_value_names =
            self.needed_value_binding_names(&self.output, &object_bindings, &global_value_names);
        lines.extend(self.emit_value_binding_lines(
            &helper_names,
            &global_value_names,
            &scene_value_names,
        ));
        let output = emit_object_expr(
            &self.output,
            &locals.point,
            &object_bindings,
            &helper_names,
            &scene_input_names,
        );
        lines.push(format!("    return {};", output));
        lines.push("}".to_string());

        lines.push(String::new());
        lines.push(format!("vec3 scene_grad({}) {{", signature.join(", ")));
        lines.push(format!("    float {} = 0.0005;", locals.eps));
        let mut grad_args = vec![format!("{} + vec3({}, 0.0, 0.0)", locals.point, locals.eps)];
        grad_args.extend(scene_input_names.iter().cloned());
        lines.push(format!(
            "    float {} = scene_sdf({}) - scene_sdf({});",
            locals.dx,
            grad_args.join(", "),
            std::iter::once(format!("{} - vec3({}, 0.0, 0.0)", locals.point, locals.eps))
                .chain(scene_input_names.iter().cloned())
                .collect::<Vec<_>>()
                .join(", ")
        ));
        lines.push(format!(
            "    float {} = scene_sdf({}) - scene_sdf({});",
            locals.dy,
            std::iter::once(format!("{} + vec3(0.0, {}, 0.0)", locals.point, locals.eps))
                .chain(scene_input_names.iter().cloned())
                .collect::<Vec<_>>()
                .join(", "),
            std::iter::once(format!("{} - vec3(0.0, {}, 0.0)", locals.point, locals.eps))
                .chain(scene_input_names.iter().cloned())
                .collect::<Vec<_>>()
                .join(", ")
        ));
        lines.push(format!(
            "    float {} = scene_sdf({}) - scene_sdf({});",
            locals.dz,
            std::iter::once(format!("{} + vec3(0.0, 0.0, {})", locals.point, locals.eps))
                .chain(scene_input_names.iter().cloned())
                .collect::<Vec<_>>()
                .join(", "),
            std::iter::once(format!("{} - vec3(0.0, 0.0, {})", locals.point, locals.eps))
                .chain(scene_input_names.iter().cloned())
                .collect::<Vec<_>>()
                .join(", ")
        ));
        lines.push(format!(
            "    return normalize(vec3({}, {}, {}));",
            locals.dx, locals.dy, locals.dz
        ));
        lines.push("}".to_string());

        suffix_glsl_float_literals(&lines.join("\n"))
    }

    fn scene_signature(&self, point_name: &str) -> Vec<String> {
        let mut signature = vec![format!("vec3 {}", point_name)];
        for input in &self.inputs {
            match input.ty {
                Type::Float
                | Type::Int
                | Type::Complex
                | Type::Quat
                | Type::SE3
                | Type::E3
                | Type::Custom { .. }
                | Type::Vec2
                | Type::Vec3
                | Type::Vec4
                | Type::Mat(_, _)
                | Type::Array(_) => {
                    signature.push(format!("{} {}", input.ty.glsl_name(), input.name))
                }
                Type::Object | Type::Product(_) | Type::Func(_, _) => {}
            }
        }
        signature
    }

    fn scene_input_names(&self) -> Vec<String> {
        self.inputs
            .iter()
            .filter_map(|input| match input.ty {
                Type::Float
                | Type::Int
                | Type::Complex
                | Type::Quat
                | Type::SE3
                | Type::E3
                | Type::Custom { .. }
                | Type::Vec2
                | Type::Vec3
                | Type::Vec4
                | Type::Mat(_, _)
                | Type::Array(_) => Some(input.name.clone()),
                Type::Object | Type::Product(_) | Type::Func(_, _) => None,
            })
            .collect()
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
            if is_global_const_value_expr(&binding.expr, &names)
                && !matches!(binding.ty, Type::Array(_))
            {
                names.insert(binding.name.clone());
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

    fn emit_generated_binding(
        &self,
        binding: &TypedBinding,
        object_bindings: &BTreeMap<String, ObjectBinding>,
        helper_names: &HashMap<String, String>,
        scene_input_names: &[String],
        global_value_names: &BTreeSet<String>,
        locals: &EmitLocals,
    ) -> Vec<String> {
        let signature = self.scene_signature(&locals.point);
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
                object_bindings,
                helper_names,
                scene_input_names,
            )
        ));
        lines.push("}".to_string());
        lines.push(String::new());

        lines.push(format!(
            "vec3 grad_sdf_{}({}) {{",
            binding.name,
            signature.join(", ")
        ));
        lines.push(format!("    float {} = 0.0005;", locals.eps));
        let forward_x =
            std::iter::once(format!("{} + vec3({}, 0.0, 0.0)", locals.point, locals.eps))
                .chain(scene_input_names.iter().cloned())
                .collect::<Vec<_>>()
                .join(", ");
        let backward_x =
            std::iter::once(format!("{} - vec3({}, 0.0, 0.0)", locals.point, locals.eps))
                .chain(scene_input_names.iter().cloned())
                .collect::<Vec<_>>()
                .join(", ");
        let forward_y =
            std::iter::once(format!("{} + vec3(0.0, {}, 0.0)", locals.point, locals.eps))
                .chain(scene_input_names.iter().cloned())
                .collect::<Vec<_>>()
                .join(", ");
        let backward_y =
            std::iter::once(format!("{} - vec3(0.0, {}, 0.0)", locals.point, locals.eps))
                .chain(scene_input_names.iter().cloned())
                .collect::<Vec<_>>()
                .join(", ");
        let forward_z =
            std::iter::once(format!("{} + vec3(0.0, 0.0, {})", locals.point, locals.eps))
                .chain(scene_input_names.iter().cloned())
                .collect::<Vec<_>>()
                .join(", ");
        let backward_z =
            std::iter::once(format!("{} - vec3(0.0, 0.0, {})", locals.point, locals.eps))
                .chain(scene_input_names.iter().cloned())
                .collect::<Vec<_>>()
                .join(", ");
        lines.push(format!(
            "    float {} = sdf_{}({}) - sdf_{}({});",
            locals.dx, binding.name, forward_x, binding.name, backward_x
        ));
        lines.push(format!(
            "    float {} = sdf_{}({}) - sdf_{}({});",
            locals.dy, binding.name, forward_y, binding.name, backward_y
        ));
        lines.push(format!(
            "    float {} = sdf_{}({}) - sdf_{}({});",
            locals.dz, binding.name, forward_z, binding.name, backward_z
        ));
        lines.push(format!(
            "    return normalize(vec3({}, {}, {}));",
            locals.dx, locals.dy, locals.dz
        ));
        lines.push("}".to_string());
        lines.push(String::new());

        lines
    }

    fn emit_locals(&self) -> EmitLocals {
        let mut forbidden = BTreeSet::new();
        for input in &self.inputs {
            match input.ty {
                Type::Float
                | Type::Int
                | Type::Complex
                | Type::Quat
                | Type::SE3
                | Type::E3
                | Type::Custom { .. }
                | Type::Vec2
                | Type::Vec3
                | Type::Vec4
                | Type::Mat(_, _)
                | Type::Array(_) => {
                    forbidden.insert(input.name.clone());
                }
                Type::Object | Type::Product(_) | Type::Func(_, _) => {}
            }
        }
        for binding in &self.value_bindings {
            forbidden.insert(binding.name.clone());
        }

        let point = unique_local_name("p", &forbidden);
        forbidden.insert(point.clone());
        let func_param = unique_local_name("t", &forbidden);
        forbidden.insert(func_param.clone());
        let eps = unique_local_name("eps", &forbidden);
        forbidden.insert(eps.clone());
        let dx = unique_local_name("dx", &forbidden);
        forbidden.insert(dx.clone());
        let dy = unique_local_name("dy", &forbidden);
        forbidden.insert(dy.clone());
        let dz = unique_local_name("dz", &forbidden);

        EmitLocals {
            point,
            func_param,
            eps,
            dx,
            dy,
            dz,
        }
    }

    fn support_blocks(&self, registry: &Registry) -> Vec<&'static str> {
        let mut names = BTreeSet::new();
        for input in &self.inputs {
            collect_type_support(&input.ty, &mut names);
        }
        for func in &self.funcs {
            collect_type_support(&func.output, &mut names);
            collect_value_support(&func.expr, &mut names);
        }
        for binding in &self.value_bindings {
            collect_type_support(&binding.ty, &mut names);
            collect_value_support(&binding.expr, &mut names);
        }
        for binding in &self.bindings {
            collect_object_support(&binding.expr, &mut names);
        }
        collect_object_support(&self.output, &mut names);

        let mut blocks = Vec::new();
        for name in names {
            if let Some(primitive) = registry.primitives.get(name.as_str()) {
                blocks.push(primitive.support_glsl);
                continue;
            }
            if let Some(op) = registry.object_ops.get(name.as_str()) {
                blocks.push(op.support_glsl);
            }
            if let Some(support_glsl) = builtin_type_support_glsl(name.as_str()) {
                blocks.push(support_glsl);
                continue;
            }
            if let Some(func) = registry.value_funcs.get(name.as_str()) {
                if let Some(support_glsl) = func.support_glsl {
                    blocks.push(support_glsl);
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

    fn concat_helpers(&self) -> Vec<ConcatHelper> {
        let mut helpers = BTreeMap::new();
        for func in &self.funcs {
            collect_concat_helpers(&func.expr, &mut helpers);
        }
        for binding in &self.value_bindings {
            collect_concat_helpers(&binding.expr, &mut helpers);
        }
        for binding in &self.bindings {
            collect_object_concat_helpers(&binding.expr, &mut helpers);
        }
        collect_object_concat_helpers(&self.output, &mut helpers);
        helpers.into_values().collect()
    }
}

fn collect_type_support(ty: &Type, names: &mut BTreeSet<String>) {
    match ty {
        Type::SE3 | Type::E3 => {
            names.insert(ty.type_name());
        }
        Type::Custom { .. } => {}
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
        Type::Float
        | Type::Int
        | Type::Complex
        | Type::Quat
        | Type::Vec2
        | Type::Vec3
        | Type::Vec4
        | Type::Mat(_, _)
        | Type::Object => {}
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
        ValueExpr::Float(_) | ValueExpr::Int(_) | ValueExpr::Var { .. } => {}
        ValueExpr::Call { args, .. } => {
            for arg in args {
                collect_concat_helpers(arg, helpers);
            }
        }
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
        ValueExpr::DirectionalDerivative {
            epsilon,
            direction,
            at,
            ..
        } => {
            collect_concat_helpers(epsilon, helpers);
            collect_concat_helpers(direction, helpers);
            collect_concat_helpers(at, helpers);
        }
    }
}

fn is_global_const_value_expr(expr: &ValueExpr, names: &BTreeSet<String>) -> bool {
    match expr {
        ValueExpr::Float(_) | ValueExpr::Int(_) => true,
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
        ValueExpr::Binary { left, right, .. } => {
            is_global_const_value_expr(left, names) && is_global_const_value_expr(right, names)
        }
        ValueExpr::Array { .. }
        | ValueExpr::Index { .. }
        | ValueExpr::Concat { .. }
        | ValueExpr::Call { .. }
        | ValueExpr::Derivative { .. }
        | ValueExpr::Partial { .. }
        | ValueExpr::DirectionalDerivative { .. }
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
        ValueExpr::Float(_) | ValueExpr::Int(_) => {}
        ValueExpr::Var { name, .. } => {
            names.insert(name.clone());
        }
        ValueExpr::Call { args, .. } => {
            for arg in args {
                collect_value_refs(arg, names);
            }
        }
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
        ValueExpr::DirectionalDerivative {
            epsilon,
            direction,
            at,
            ..
        } => {
            collect_value_refs(epsilon, names);
            collect_value_refs(direction, names);
            collect_value_refs(at, names);
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
        ValueExpr::Float(_) | ValueExpr::Int(_) | ValueExpr::Var { .. } => {}
        ValueExpr::Call { func, args, ty } => {
            collect_type_support(ty, names);
            if func != "rot" {
                names.insert(func.clone());
            }
            for arg in args {
                collect_value_support(arg, names);
            }
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
                names.insert(name.to_string());
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
        ValueExpr::Derivative { epsilon, func, at }
        | ValueExpr::Gradient { epsilon, func, at }
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
        ValueExpr::DirectionalDerivative {
            epsilon,
            direction,
            func,
            at,
        } => {
            collect_value_support(epsilon, names);
            collect_value_support(direction, names);
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
        FunctionExprKind::Compose(outer, inner) => {
            collect_function_support(outer, names);
            collect_function_support(inner, names);
        }
    }
}
fn emit_value_expr(
    expr: &ValueExpr,
    helper_names: &HashMap<String, String>,
    value_names: &HashMap<String, String>,
) -> String {
    match expr {
        ValueExpr::Float(value) => format_float(*value),
        ValueExpr::Int(value) => value.to_string(),
        ValueExpr::Var { name, .. } => value_names
            .get(name)
            .cloned()
            .unwrap_or_else(|| name.clone()),
        ValueExpr::Call { func, args, .. } => {
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
        ValueExpr::Derivative { epsilon, func, at } => {
            emit_scalar_derivative(func, epsilon, at, helper_names, value_names)
        }
        ValueExpr::Partial {
            axis,
            epsilon,
            func,
            at,
        } => emit_partial_derivative(*axis, func, epsilon, at, helper_names, value_names),
        ValueExpr::DirectionalDerivative {
            epsilon,
            direction,
            func,
            at,
        } => emit_directional_derivative(func, epsilon, direction, at, helper_names, value_names),
        ValueExpr::Gradient { epsilon, func, at } => {
            emit_gradient(func, epsilon, at, helper_names, value_names)
        }
        ValueExpr::Divergence { epsilon, func, at } => {
            emit_divergence(func, epsilon, at, helper_names, value_names)
        }
    }
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

fn emit_binary_expr(
    op: BinOp,
    left: &ValueExpr,
    right: &ValueExpr,
    helper_names: &HashMap<String, String>,
    value_names: &HashMap<String, String>,
) -> String {
    let left_ty = left.ty();
    let right_ty = right.ty();
    let left = emit_value_expr(left, helper_names, value_names);
    let right = emit_value_expr(right, helper_names, value_names);

    match (op, &left_ty, &right_ty) {
        (BinOp::Mul, Type::Complex, Type::Complex) => format!("mult_C({}, {})", left, right),
        (BinOp::Div, Type::Complex, Type::Complex) => format!("div_C({}, {})", left, right),
        (BinOp::Mul, Type::Quat, Type::Quat) => format!("mult_H({}, {})", left, right),
        (BinOp::Div, Type::Quat, Type::Quat) => format!("div_H({}, {})", left, right),
        (BinOp::Mul, Type::SE3, Type::SE3) => format!("mult_SE3({}, {})", left, right),
        (BinOp::Div, Type::SE3, Type::SE3) => format!("div_SE3({}, {})", left, right),
        (BinOp::Mul, Type::E3, Type::E3) => format!("mult_E3({}, {})", left, right),
        (BinOp::Div, Type::E3, Type::E3) => format!("div_E3({}, {})", left, right),
        (BinOp::Mul, Type::E3, Type::Vec3) => format!("act_E3({}, {})", left, right),
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

fn emit_custom_binary_expr(op: BinOp, ty: &Type, left: &str, right: &str) -> String {
    let Type::Custom { name, .. } = ty else {
        unreachable!();
    };
    match op {
        BinOp::Add => format!("add_{}({}, {})", name, left, right),
        BinOp::Sub => format!("sub_{}({}, {})", name, left, right),
        BinOp::Mul => format!("mult_{}({}, {})", name, left, right),
        BinOp::Div => format!("mult_{}({}, inv_{}({}))", name, left, name, right),
        BinOp::Compose => unreachable!(),
    }
}

fn emit_custom_scale_expr(op: BinOp, ty: &Type, value: &str, scalar: &str) -> String {
    let Type::Custom { name, .. } = ty else {
        unreachable!();
    };
    match op {
        BinOp::Mul => format!("scale_{}({}, {})", name, value, scalar),
        BinOp::Div => format!("scale_{}({}, (1.0 / {}))", name, value, scalar),
        BinOp::Add | BinOp::Sub | BinOp::Compose => unreachable!(),
    }
}

fn scalar_to_algebra(ty: &Type, value: &str) -> String {
    match ty {
        Type::Complex => format!("vec2({}, 0.0)", value),
        Type::Quat => format!("vec4({}, 0.0, 0.0, 0.0)", value),
        _ => value.to_string(),
    }
}

fn binary_support_name(op: BinOp, left: &Type, right: &Type) -> Option<&'static str> {
    match (op, left, right) {
        (BinOp::Mul | BinOp::Div, Type::Complex, Type::Complex)
        | (BinOp::Div, Type::Float, Type::Complex) => Some("C"),
        (BinOp::Mul | BinOp::Div, Type::Quat, Type::Quat)
        | (BinOp::Div, Type::Float, Type::Quat) => Some("H"),
        (BinOp::Mul | BinOp::Div, Type::SE3, Type::SE3) => Some("SE3"),
        (BinOp::Mul | BinOp::Div, Type::E3, Type::E3) | (BinOp::Mul, Type::E3, Type::Vec3) => {
            Some("E3")
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

fn emit_scalar_derivative(
    func: &FunctionExpr,
    epsilon: &ValueExpr,
    at: &ValueExpr,
    helper_names: &HashMap<String, String>,
    value_names: &HashMap<String, String>,
) -> String {
    let plus = ValueExpr::Binary {
        op: BinOp::Add,
        left: Box::new(at.clone()),
        right: Box::new(epsilon.clone()),
        ty: Type::Float,
    };
    let minus = ValueExpr::Binary {
        op: BinOp::Sub,
        left: Box::new(at.clone()),
        right: Box::new(epsilon.clone()),
        ty: Type::Float,
    };
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

fn emit_partial_derivative(
    axis: usize,
    func: &FunctionExpr,
    epsilon: &ValueExpr,
    at: &ValueExpr,
    helper_names: &HashMap<String, String>,
    value_names: &HashMap<String, String>,
) -> String {
    let plus = emit_vec3_axis_offset(at.clone(), epsilon.clone(), axis, BinOp::Add);
    let minus = emit_vec3_axis_offset(at.clone(), epsilon.clone(), axis, BinOp::Sub);
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

fn emit_directional_derivative(
    func: &FunctionExpr,
    epsilon: &ValueExpr,
    direction: &ValueExpr,
    at: &ValueExpr,
    helper_names: &HashMap<String, String>,
    value_names: &HashMap<String, String>,
) -> String {
    let direction_step = ValueExpr::Binary {
        op: BinOp::Mul,
        left: Box::new(ValueExpr::Call {
            func: "normalize".to_string(),
            args: vec![direction.clone()],
            ty: Type::Vec3,
        }),
        right: Box::new(epsilon.clone()),
        ty: Type::Vec3,
    };
    let plus = ValueExpr::Binary {
        op: BinOp::Add,
        left: Box::new(at.clone()),
        right: Box::new(direction_step.clone()),
        ty: Type::Vec3,
    };
    let minus = ValueExpr::Binary {
        op: BinOp::Sub,
        left: Box::new(at.clone()),
        right: Box::new(direction_step),
        ty: Type::Vec3,
    };
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

fn emit_gradient(
    func: &FunctionExpr,
    epsilon: &ValueExpr,
    at: &ValueExpr,
    helper_names: &HashMap<String, String>,
    value_names: &HashMap<String, String>,
) -> String {
    format!(
        "vec3({}, {}, {})",
        emit_partial_derivative(0, func, epsilon, at, helper_names, value_names),
        emit_partial_derivative(1, func, epsilon, at, helper_names, value_names),
        emit_partial_derivative(2, func, epsilon, at, helper_names, value_names)
    )
}

fn emit_divergence(
    func: &FunctionExpr,
    epsilon: &ValueExpr,
    at: &ValueExpr,
    helper_names: &HashMap<String, String>,
    value_names: &HashMap<String, String>,
) -> String {
    let twice = ValueExpr::Binary {
        op: BinOp::Mul,
        left: Box::new(ValueExpr::Float(2.0)),
        right: Box::new(epsilon.clone()),
        ty: Type::Float,
    };
    let denom = emit_value_expr(&twice, helper_names, value_names);
    let dx_plus = emit_function_application(
        func,
        emit_vec3_axis_offset(at.clone(), epsilon.clone(), 0, BinOp::Add),
        helper_names,
        value_names,
    );
    let dx_minus = emit_function_application(
        func,
        emit_vec3_axis_offset(at.clone(), epsilon.clone(), 0, BinOp::Sub),
        helper_names,
        value_names,
    );
    let dy_plus = emit_function_application(
        func,
        emit_vec3_axis_offset(at.clone(), epsilon.clone(), 1, BinOp::Add),
        helper_names,
        value_names,
    );
    let dy_minus = emit_function_application(
        func,
        emit_vec3_axis_offset(at.clone(), epsilon.clone(), 1, BinOp::Sub),
        helper_names,
        value_names,
    );
    let dz_plus = emit_function_application(
        func,
        emit_vec3_axis_offset(at.clone(), epsilon.clone(), 2, BinOp::Add),
        helper_names,
        value_names,
    );
    let dz_minus = emit_function_application(
        func,
        emit_vec3_axis_offset(at.clone(), epsilon.clone(), 2, BinOp::Sub),
        helper_names,
        value_names,
    );
    format!(
        "((({dx_plus}).x - ({dx_minus}).x) / {denom} + (({dy_plus}).y - ({dy_minus}).y) / {denom} + (({dz_plus}).z - ({dz_minus}).z) / {denom})"
    )
}

fn emit_vec3_axis_offset(base: ValueExpr, epsilon: ValueExpr, axis: usize, op: BinOp) -> ValueExpr {
    let offset = match axis {
        0 => ValueExpr::Vec3(
            Box::new(epsilon),
            Box::new(ValueExpr::Float(0.0)),
            Box::new(ValueExpr::Float(0.0)),
        ),
        1 => ValueExpr::Vec3(
            Box::new(ValueExpr::Float(0.0)),
            Box::new(epsilon),
            Box::new(ValueExpr::Float(0.0)),
        ),
        _ => ValueExpr::Vec3(
            Box::new(ValueExpr::Float(0.0)),
            Box::new(ValueExpr::Float(0.0)),
            Box::new(epsilon),
        ),
    };
    ValueExpr::Binary {
        op,
        left: Box::new(base),
        right: Box::new(offset),
        ty: Type::Vec3,
    }
}

fn emit_object_expr(
    expr: &ObjectExpr,
    point_expr: &str,
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
                let point_arg = if registry::shape_dimension(name) == ShapeDimension::D2 {
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
                    "sdf0_Polygon2D({}.xy, {}, {})",
                    point_expr,
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
                object_bindings,
                helper_names,
                scene_input_names,
            )
        }
        ObjectExpr::IsometryTransform { object, transform } => {
            let inverse = format!("inv_E3({})", emit_plain_value_expr(transform, helper_names));
            let transformed_point = format!("act_E3({}, {})", inverse, point_expr);
            emit_object_expr(
                object,
                &transformed_point,
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
            if glsl_name == "op_revolution" {
                let offset = emit_plain_value_expr(&value_args[0], helper_names);
                let revolved_point = format!("op_revolution_point({}, {})", point_expr, offset);
                return emit_object_expr(
                    &object_args[0],
                    &revolved_point,
                    object_bindings,
                    helper_names,
                    scene_input_names,
                );
            }
            if glsl_name == "op_extrusion" {
                let height = emit_plain_value_expr(&value_args[0], helper_names);
                let base_point = format!("vec3(({}).xy, 0.0)", point_expr);
                let base_distance = emit_object_expr(
                    &object_args[0],
                    &base_point,
                    object_bindings,
                    helper_names,
                    scene_input_names,
                );
                return format!(
                    "op_extrusion({}, ({}).z, {})",
                    base_distance, point_expr, height
                );
            }
            if glsl_name == "op_rot" {
                let binormal = emit_plain_value_expr(&value_args[0], helper_names);
                let anchor = emit_plain_value_expr(&value_args[1], helper_names);
                let angle = emit_plain_value_expr(&value_args[2], helper_names);
                let rotated_point = format!(
                    "op_rot_inverse_point({}, {}, {}, {})",
                    point_expr, binormal, anchor, angle
                );
                return emit_object_expr(
                    &object_args[0],
                    &rotated_point,
                    object_bindings,
                    helper_names,
                    scene_input_names,
                );
            }
            if glsl_name == "op_rot2D" {
                let anchor = emit_plain_value_expr(&value_args[0], helper_names);
                let angle = emit_plain_value_expr(&value_args[1], helper_names);
                let rotated_point = format!(
                    "op_rot2D_inverse_point({}, {}, {})",
                    point_expr, anchor, angle
                );
                return emit_object_expr(
                    &object_args[0],
                    &rotated_point,
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
    if is_identity_mat3(linear) {
        return format!(
            "({} - {})",
            point_expr,
            emit_plain_value_expr(translation, helper_names)
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

fn is_zero_vec3(expr: &ValueExpr) -> bool {
    match expr {
        ValueExpr::Vec3(x, y, z) => {
            is_float_literal(x, 0.0) && is_float_literal(y, 0.0) && is_float_literal(z, 0.0)
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

fn helper_name(name: &str) -> String {
    format!("dsl_{}", name)
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
