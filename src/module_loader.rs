use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use super::*;

pub(crate) struct ModuleLoader {
    base_dir: PathBuf,
}

impl ModuleLoader {
    /// Performs `new` behavior.
    pub(crate) fn new(base_dir: &Path) -> Self {
        Self {
            base_dir: base_dir.to_path_buf(),
        }
    }

    /// Performs `load_main` behavior.
    pub(crate) fn load_main(&self, source: &str) -> Result<Program, Error> {
        let mut stack = Vec::new();
        let parsed = parser::Parser::new(source).parse_program()?;
        if parsed.is_module {
            return Err(Error::new("#module is only valid in imported module files"));
        }
        let mut merged = self.expand_program(parsed, false, "main", &mut stack)?;
        expand_referenced_name_templates(&mut merged);
        reject_duplicate_product_types(&merged)?;
        merged.imports.clear();
        Ok(merged)
    }

    /// Performs `load_document` behavior.
    pub(crate) fn load_document(&self, source: &str) -> Result<Program, Error> {
        let mut stack = Vec::new();
        let parsed = parser::Parser::new(source).parse_program()?;
        let mut merged = if parsed.is_module {
            self.expand_program(parsed, true, "document", &mut stack)?
        } else {
            self.expand_program(parsed, false, "main", &mut stack)?
        };
        expand_referenced_name_templates(&mut merged);
        reject_duplicate_product_types(&merged)?;
        merged.imports.clear();
        Ok(merged)
    }

    /// Performs `expand_program` behavior.
    fn expand_program(
        &self,
        mut program: Program,
        require_module: bool,
        module_key: &str,
        stack: &mut Vec<PathBuf>,
    ) -> Result<Program, Error> {
        if require_module && !program.is_module {
            return Err(Error::new(format!(
                "imported module '{module_key}' must start with #module"
            )));
        }

        let imports = std::mem::take(&mut program.imports);
        let mut merged = empty_program_like(&program);
        let mut line_offset = 10_000usize;
        for import in imports {
            let imported = self
                .load_import(&import.path, stack)
                .map_err(|err| err.with_line(import.line))?;
            append_program(&mut merged, imported, line_offset);
            line_offset += 10_000;
        }

        if program.is_module {
            mangle_private_module_names(&mut program, module_key);
        }
        append_program(&mut merged, program, 0);
        Ok(merged)
    }

    /// Performs `load_import` behavior.
    fn load_import(&self, import_path: &str, stack: &mut Vec<PathBuf>) -> Result<Program, Error> {
        let path = self.resolve_import(import_path)?;
        let canonical = path.canonicalize().unwrap_or(path.clone());
        if let Some(cycle) = import_cycle_error(stack, &canonical) {
            return Err(cycle);
        }

        stack.push(canonical);
        let source = fs::read_to_string(&path).map_err(|err| Error::new(err.to_string()))?;
        let parsed = parser::Parser::new(&source).parse_program()?;
        let module_key = import_path.replace(['/', '\\', '.', '-'], "_");
        let expanded = self.expand_program(parsed, true, &module_key, stack);
        stack.pop();
        expanded
    }

    /// Performs `resolve_import` behavior.
    fn resolve_import(&self, import_path: &str) -> Result<PathBuf, Error> {
        resolve_import_path(import_path, &self.base_dir)
    }
}

/// Resolves import paths from `LANE_MODULE_PATH`, local modules, and manifest modules.
pub fn resolve_import_path(
    import_path: &str,
    base_dir: impl AsRef<Path>,
) -> Result<PathBuf, Error> {
    let relative = import_candidate(import_path);
    let base_dir = base_dir.as_ref();
    let mut roots = Vec::new();
    if let Ok(paths) = std::env::var("LANE_MODULE_PATH") {
        roots.extend(std::env::split_paths(&paths));
    }
    roots.push(base_dir.to_path_buf());
    roots.push(base_dir.join("modules"));
    roots.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("modules"));

    for root in roots {
        let candidate = root.join(&relative);
        if candidate.exists() {
            return Ok(candidate);
        }
    }
    Err(Error::new(format!("module '{import_path}' not found")))
}

/// Performs `import_candidate` behavior.
fn import_candidate(import_path: &str) -> PathBuf {
    let path = PathBuf::from(import_path);
    if path.extension().is_some() {
        path
    } else {
        path.with_extension("lane")
    }
}

#[derive(Clone)]
enum NameTemplatePart {
    Literal(String),
    Placeholder(String),
}

/// Performs `expand_referenced_name_templates` behavior.
fn expand_referenced_name_templates(program: &mut Program) {
    let references = referenced_names(program);
    if references.is_empty() {
        return;
    }

    let mut concrete_funcs = Vec::new();
    let mut seen = program
        .funcs
        .iter()
        .map(|func| (func.name.clone(), format_type(&func.ty)))
        .collect::<HashSet<_>>();

    program.funcs.retain(|func| {
        let Some(parts) = parse_name_template(&func.name) else {
            return true;
        };
        for reference in &references {
            let Some(captures) = match_name_template(&parts, reference) else {
                continue;
            };
            for substitutions in template_substitution_variants(&func.ty, &func.body, &captures) {
                let concrete = instantiate_name_template_func(func, &substitutions);
                if seen.insert((concrete.name.clone(), format_type(&concrete.ty))) {
                    concrete_funcs.push(concrete);
                }
            }
        }
        false
    });

    program.funcs.extend(concrete_funcs);
}

/// Performs `referenced_names` behavior.
fn referenced_names(program: &Program) -> HashSet<String> {
    let mut names = HashSet::new();
    for func in &program.funcs {
        collect_func_body_names(&func.body, &mut names);
    }
    for binding in &program.value_bindings {
        collect_expr_names(&binding.expr, &mut names);
    }
    for binding in &program.bindings {
        collect_expr_names(&binding.expr, &mut names);
    }
    for binding in &program.inferred_bindings {
        collect_expr_names(&binding.expr, &mut names);
    }
    names
}

/// Performs `collect_func_body_names` behavior.
fn collect_func_body_names(body: &FuncBody, names: &mut HashSet<String>) {
    match body {
        FuncBody::Expr(expr) => collect_expr_names(expr, names),
        FuncBody::RawGlsl(_) | FuncBody::RawGlslClosure { .. } => {}
    }
}

/// Performs `collect_expr_names` behavior.
fn collect_expr_names(expr: &Expr, names: &mut HashSet<String>) {
    match expr {
        Expr::Ident(name) => {
            names.insert(name.clone());
        }
        Expr::Closure { body, .. } => collect_expr_names(body, names),
        Expr::Tuple(items) | Expr::Array(items) => {
            for item in items {
                collect_expr_names(item, names);
            }
        }
        Expr::Call { callee, args } => {
            collect_expr_names(callee, names);
            for arg in args {
                collect_expr_names(arg, names);
            }
        }
        Expr::FieldAccess { object, .. } => collect_expr_names(object, names),
        Expr::Conditional {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_expr_names(condition, names);
            collect_expr_names(then_branch, names);
            if let Some(else_branch) = else_branch {
                collect_expr_names(else_branch, names);
            }
        }
        Expr::Index { array, index } => {
            collect_expr_names(array, names);
            collect_expr_names(index, names);
        }
        Expr::Unary { expr, .. } => collect_expr_names(expr, names),
        Expr::Binary { left, right, .. } => {
            collect_expr_names(left, names);
            collect_expr_names(right, names);
        }
        Expr::Constructor { name, args } => {
            names.insert(name.clone());
            match args {
                ConstructorArgs::Named(args) => {
                    for (_, arg) in args {
                        collect_expr_names(arg, names);
                    }
                }
                ConstructorArgs::Positional(args) => {
                    for arg in args {
                        collect_expr_names(arg, names);
                    }
                }
            }
        }
        Expr::Bool(_) | Expr::Number(_) | Expr::RawString(_) | Expr::Operator(_) => {}
    }
}

/// Performs `parse_name_template` behavior.
fn parse_name_template(source: &str) -> Option<Vec<NameTemplatePart>> {
    let mut parts = Vec::new();
    let mut index = 0;
    let mut found_placeholder = false;
    while let Some(relative_start) = source[index..].find('{') {
        let start = index + relative_start;
        if start > index {
            parts.push(NameTemplatePart::Literal(source[index..start].to_string()));
        }
        let name_start = start + 1;
        let relative_end = source[name_start..].find('}')?;
        let end = name_start + relative_end;
        let name = &source[name_start..end];
        parse_generic_name(name)?;
        parts.push(NameTemplatePart::Placeholder(name.to_string()));
        found_placeholder = true;
        index = end + 1;
    }
    if index < source.len() {
        parts.push(NameTemplatePart::Literal(source[index..].to_string()));
    }
    found_placeholder.then_some(parts)
}

/// Performs `match_name_template` behavior.
fn match_name_template(
    parts: &[NameTemplatePart],
    candidate: &str,
) -> Option<HashMap<String, usize>> {
    let mut captures = HashMap::new();
    let mut index = 0;
    for (part_index, part) in parts.iter().enumerate() {
        match part {
            NameTemplatePart::Literal(literal) => {
                if !candidate[index..].starts_with(literal) {
                    return None;
                }
                index += literal.len();
            }
            NameTemplatePart::Placeholder(name) => {
                let next_literal = parts[part_index + 1..].iter().find_map(|part| match part {
                    NameTemplatePart::Literal(literal) if !literal.is_empty() => {
                        Some(literal.as_str())
                    }
                    _ => None,
                });
                let end = if let Some(next_literal) = next_literal {
                    index + candidate[index..].find(next_literal)?
                } else {
                    candidate.len()
                };
                let value = candidate[index..end].parse::<usize>().ok()?;
                if captures
                    .get(name)
                    .is_some_and(|existing| *existing != value)
                {
                    return None;
                }
                captures.insert(name.clone(), value);
                index = end;
            }
        }
    }
    (index == candidate.len()).then_some(captures)
}

/// Performs `template_substitution_variants` behavior.
fn template_substitution_variants(
    ty: &Type,
    body: &FuncBody,
    captures: &HashMap<String, usize>,
) -> Vec<HashMap<String, usize>> {
    let mut missing_dims = BTreeSet::new();
    collect_unbound_generic_dims(ty, captures, &mut missing_dims);
    let missing_dims = missing_dims.into_iter().collect::<Vec<_>>();
    let min_dim = template_minimum_vector_dimension(body, captures);
    let mut variants = Vec::new();
    build_template_substitution_variants(
        &missing_dims,
        0,
        captures.clone(),
        min_dim,
        &mut variants,
    );
    variants
}

/// Performs `build_template_substitution_variants` behavior.
fn build_template_substitution_variants(
    names: &[String],
    index: usize,
    current: HashMap<String, usize>,
    min_dim: usize,
    out: &mut Vec<HashMap<String, usize>>,
) {
    if index == names.len() {
        out.push(current);
        return;
    }
    let mut next = current;
    next.insert(names[index].clone(), min_dim.max(2));
    build_template_substitution_variants(names, index + 1, next, min_dim, out);
}

/// Performs `collect_unbound_generic_dims` behavior.
fn collect_unbound_generic_dims(
    ty: &Type,
    captures: &HashMap<String, usize>,
    out: &mut BTreeSet<String>,
) {
    match ty {
        Type::VecGeneric(dim) => collect_unbound_generic_dim(dim, captures, out),
        Type::MatGeneric(rows, columns) => {
            collect_unbound_generic_dim(rows, captures, out);
            collect_unbound_generic_dim(columns, captures, out);
        }
        Type::Power(base, dim) => {
            collect_unbound_generic_dims(base, captures, out);
            collect_unbound_generic_dim(dim, captures, out);
        }
        Type::Array(element) => collect_unbound_generic_dims(element, captures, out),
        Type::Product(parts) => {
            for part in parts {
                collect_unbound_generic_dims(part, captures, out);
            }
        }
        Type::Func(input, output) => {
            collect_unbound_generic_dims(input, captures, out);
            collect_unbound_generic_dims(output, captures, out);
        }
        Type::Bool
        | Type::Float
        | Type::Int
        | Type::Complex
        | Type::Quat
        | Type::Isom2
        | Type::Isom3
        | Type::Custom { .. }
        | Type::Object
        | Type::Object2D
        | Type::Vec2
        | Type::Vec3
        | Type::Vec4
        | Type::Mat(_, _)
        | Type::Unit
        | Type::Generic(_) => {}
    }
}

/// Performs `collect_unbound_generic_dim` behavior.
fn collect_unbound_generic_dim(
    dim: &GenericDim,
    captures: &HashMap<String, usize>,
    out: &mut BTreeSet<String>,
) {
    if let GenericDim::Var(name) = dim {
        if !captures.contains_key(name) {
            out.insert(name.clone());
        }
    }
}

/// Performs `template_minimum_vector_dimension` behavior.
fn template_minimum_vector_dimension(body: &FuncBody, captures: &HashMap<String, usize>) -> usize {
    match body {
        FuncBody::Expr(expr) => expr_minimum_vector_dimension(expr, captures),
        FuncBody::RawGlsl(body) | FuncBody::RawGlslClosure { body, .. } => {
            string_minimum_vector_dimension(body, captures)
        }
    }
}

/// Performs `expr_minimum_vector_dimension` behavior.
fn expr_minimum_vector_dimension(expr: &Expr, captures: &HashMap<String, usize>) -> usize {
    match expr {
        Expr::FieldAccess { object, field } => expr_minimum_vector_dimension(object, captures)
            .max(field_minimum_vector_dimension(field, captures)),
        Expr::Closure { body, .. } => expr_minimum_vector_dimension(body, captures),
        Expr::Tuple(items) | Expr::Array(items) => items
            .iter()
            .map(|item| expr_minimum_vector_dimension(item, captures))
            .max()
            .unwrap_or(2),
        Expr::Call { callee, args } => args
            .iter()
            .map(|arg| expr_minimum_vector_dimension(arg, captures))
            .fold(expr_minimum_vector_dimension(callee, captures), usize::max),
        Expr::Conditional {
            condition,
            then_branch,
            else_branch,
        } => {
            let else_min = else_branch
                .as_ref()
                .map(|expr| expr_minimum_vector_dimension(expr, captures))
                .unwrap_or(2);
            expr_minimum_vector_dimension(condition, captures)
                .max(expr_minimum_vector_dimension(then_branch, captures))
                .max(else_min)
        }
        Expr::Index { array, index } => expr_minimum_vector_dimension(array, captures)
            .max(expr_minimum_vector_dimension(index, captures)),
        Expr::Unary { expr, .. } => expr_minimum_vector_dimension(expr, captures),
        Expr::Binary { left, right, .. } => expr_minimum_vector_dimension(left, captures)
            .max(expr_minimum_vector_dimension(right, captures)),
        Expr::Constructor { args, .. } => match args {
            ConstructorArgs::Named(args) => args
                .iter()
                .map(|(_, arg)| expr_minimum_vector_dimension(arg, captures))
                .max()
                .unwrap_or(2),
            ConstructorArgs::Positional(args) => args
                .iter()
                .map(|arg| expr_minimum_vector_dimension(arg, captures))
                .max()
                .unwrap_or(2),
        },
        Expr::RawString(source) => string_minimum_vector_dimension(source, captures),
        Expr::Bool(_) | Expr::Number(_) | Expr::Ident(_) | Expr::Operator(_) => 2,
    }
}

/// Performs `field_minimum_vector_dimension` behavior.
fn field_minimum_vector_dimension(field: &str, captures: &HashMap<String, usize>) -> usize {
    let Some(index) = field.strip_prefix('x') else {
        return 2;
    };
    let Some(placeholder) = index
        .strip_prefix('{')
        .and_then(|rest| rest.strip_suffix('}'))
    else {
        return index.parse::<usize>().map_or(2, |value| value + 1);
    };
    captures.get(placeholder).map_or(2, |value| value + 1)
}

/// Performs `string_minimum_vector_dimension` behavior.
fn string_minimum_vector_dimension(source: &str, captures: &HashMap<String, usize>) -> usize {
    captures
        .iter()
        .filter_map(|(name, value)| {
            source
                .contains(&format!("x{{{name}}}"))
                .then_some(value + 1)
        })
        .max()
        .unwrap_or(2)
}

/// Performs `instantiate_name_template_func` behavior.
fn instantiate_name_template_func(
    func: &FuncDecl,
    substitutions: &HashMap<String, usize>,
) -> FuncDecl {
    let generic_substitutions = GenericSubstitution {
        types: HashMap::new(),
        dims: substitutions.clone(),
    };
    FuncDecl {
        name: substitute_name_template_text(&func.name, substitutions),
        ty: substitute_type(&func.ty, &generic_substitutions),
        body: substitute_func_body_templates(&func.body, substitutions),
        generated: func.generated,
        line: func.line,
    }
}

/// Performs `substitute_func_body_templates` behavior.
fn substitute_func_body_templates(
    body: &FuncBody,
    substitutions: &HashMap<String, usize>,
) -> FuncBody {
    match body {
        FuncBody::Expr(expr) => FuncBody::Expr(substitute_expr_templates(expr, substitutions)),
        FuncBody::RawGlsl(body) => {
            FuncBody::RawGlsl(substitute_name_template_text(body, substitutions))
        }
        FuncBody::RawGlslClosure { params, body } => FuncBody::RawGlslClosure {
            params: params.clone(),
            body: substitute_name_template_text(body, substitutions),
        },
    }
}

/// Performs `substitute_expr_templates` behavior.
fn substitute_expr_templates(expr: &Expr, substitutions: &HashMap<String, usize>) -> Expr {
    match expr {
        Expr::Ident(name) => Expr::Ident(substitute_name_template_text(name, substitutions)),
        Expr::Closure { params, body } => Expr::Closure {
            params: params.clone(),
            body: Box::new(substitute_expr_templates(body, substitutions)),
        },
        Expr::Tuple(items) => Expr::Tuple(
            items
                .iter()
                .map(|item| substitute_expr_templates(item, substitutions))
                .collect(),
        ),
        Expr::Array(items) => Expr::Array(
            items
                .iter()
                .map(|item| substitute_expr_templates(item, substitutions))
                .collect(),
        ),
        Expr::Call { callee, args } => Expr::Call {
            callee: Box::new(substitute_expr_templates(callee, substitutions)),
            args: args
                .iter()
                .map(|arg| substitute_expr_templates(arg, substitutions))
                .collect(),
        },
        Expr::FieldAccess { object, field } => Expr::FieldAccess {
            object: Box::new(substitute_expr_templates(object, substitutions)),
            field: substitute_name_template_text(field, substitutions),
        },
        Expr::Conditional {
            condition,
            then_branch,
            else_branch,
        } => Expr::Conditional {
            condition: Box::new(substitute_expr_templates(condition, substitutions)),
            then_branch: Box::new(substitute_expr_templates(then_branch, substitutions)),
            else_branch: else_branch
                .as_ref()
                .map(|expr| Box::new(substitute_expr_templates(expr, substitutions))),
        },
        Expr::Index { array, index } => Expr::Index {
            array: Box::new(substitute_expr_templates(array, substitutions)),
            index: Box::new(substitute_expr_templates(index, substitutions)),
        },
        Expr::Unary { op, expr } => Expr::Unary {
            op: *op,
            expr: Box::new(substitute_expr_templates(expr, substitutions)),
        },
        Expr::Binary { op, left, right } => Expr::Binary {
            op: *op,
            left: Box::new(substitute_expr_templates(left, substitutions)),
            right: Box::new(substitute_expr_templates(right, substitutions)),
        },
        Expr::Constructor { name, args } => Expr::Constructor {
            name: substitute_name_template_text(name, substitutions),
            args: match args {
                ConstructorArgs::Named(args) => ConstructorArgs::Named(
                    args.iter()
                        .map(|(name, arg)| {
                            (name.clone(), substitute_expr_templates(arg, substitutions))
                        })
                        .collect(),
                ),
                ConstructorArgs::Positional(args) => ConstructorArgs::Positional(
                    args.iter()
                        .map(|arg| substitute_expr_templates(arg, substitutions))
                        .collect(),
                ),
            },
        },
        Expr::RawString(source) => {
            Expr::RawString(substitute_name_template_text(source, substitutions))
        }
        Expr::Bool(value) => Expr::Bool(*value),
        Expr::Number(value) => Expr::Number(*value),
        Expr::Operator(op) => Expr::Operator(*op),
    }
}

/// Performs `substitute_name_template_text` behavior.
fn substitute_name_template_text(source: &str, substitutions: &HashMap<String, usize>) -> String {
    let mut out = String::with_capacity(source.len());
    let mut index = 0;
    while let Some(relative_start) = source[index..].find('{') {
        let start = index + relative_start;
        out.push_str(&source[index..start]);
        let name_start = start + 1;
        let Some(relative_end) = source[name_start..].find('}') else {
            out.push_str(&source[start..]);
            return out;
        };
        let end = name_start + relative_end;
        let name = &source[name_start..end];
        if let Some(value) = substitutions.get(name) {
            out.push_str(&value.to_string());
        } else {
            out.push_str(&source[start..=end]);
        }
        index = end + 1;
    }
    out.push_str(&source[index..]);
    out
}

/// Performs `empty_program_like` behavior.
fn empty_program_like(program: &Program) -> Program {
    Program {
        ambient_dimension: program.ambient_dimension,
        derivative_epsilon: program.derivative_epsilon,
        gradient_epsilon: program.gradient_epsilon,
        is_module: false,
        imports: Vec::new(),
        product_types: Vec::new(),
        category_types: Vec::new(),
        inputs: Vec::new(),
        funcs: Vec::new(),
        value_bindings: Vec::new(),
        bindings: Vec::new(),
        inferred_bindings: Vec::new(),
    }
}

/// Performs `append_program` behavior.
fn append_program(target: &mut Program, mut source: Program, line_offset: usize) {
    bump_program_lines(&mut source, line_offset);
    target.product_types.extend(source.product_types);
    target.category_types.extend(source.category_types);
    target.inputs.extend(source.inputs);
    target.funcs.extend(source.funcs);
    target.value_bindings.extend(source.value_bindings);
    target.bindings.extend(source.bindings);
    target.inferred_bindings.extend(source.inferred_bindings);
}

/// Performs `bump_program_lines` behavior.
fn bump_program_lines(program: &mut Program, offset: usize) {
    for item in &mut program.product_types {
        item.line += offset;
    }
    for item in &mut program.category_types {
        item.line += offset;
    }
    for item in &mut program.inputs {
        item.line += offset;
    }
    for item in &mut program.funcs {
        item.line += offset;
    }
    for item in &mut program.value_bindings {
        item.line += offset;
    }
    for item in &mut program.bindings {
        item.line += offset;
    }
    for item in &mut program.inferred_bindings {
        item.line += offset;
    }
}

/// Performs `reject_duplicate_product_types` behavior.
fn reject_duplicate_product_types(program: &Program) -> Result<(), Error> {
    let mut names = HashSet::new();
    for decl in &program.product_types {
        if !names.insert(decl.name.clone()) {
            return Err(
                Error::new(format!("duplicate product type '{}'", decl.name)).with_line(decl.line),
            );
        }
    }
    for decl in &program.category_types {
        if !names.insert(decl.name.clone()) {
            return Err(
                Error::new(format!("duplicate category type '{}'", decl.name)).with_line(decl.line),
            );
        }
    }
    Ok(())
}

/// Performs `mangle_private_module_names` behavior.
fn mangle_private_module_names(program: &mut Program, module_key: &str) {
    let renames = private_module_renames(program, module_key);
    for decl in &mut program.product_types {
        rename_type_refs(&mut decl.components, &renames);
        rename_decl_name(&mut decl.name, &renames);
    }
    for decl in &mut program.inputs {
        rename_type(&mut decl.ty, &renames);
    }
    for decl in &mut program.funcs {
        rename_type(&mut decl.ty, &renames);
        rename_func_body(&mut decl.body, &renames);
        rename_decl_name(&mut decl.name, &renames);
    }
    for decl in &mut program.value_bindings {
        rename_type(&mut decl.ty, &renames);
        rename_expr(&mut decl.expr, &renames);
        rename_decl_name(&mut decl.name, &renames);
    }
    for decl in &mut program.bindings {
        rename_type(&mut decl.ty, &renames);
        rename_expr(&mut decl.expr, &renames);
        rename_decl_name(&mut decl.name, &renames);
    }
    for decl in &mut program.inferred_bindings {
        rename_expr(&mut decl.expr, &renames);
        rename_decl_name(&mut decl.name, &renames);
    }
}

/// Renames one declaration name when a private-module mapping exists.
fn rename_decl_name(name: &mut String, renames: &HashMap<String, String>) {
    if let Some(replacement) = renames.get(name.as_str()) {
        *name = replacement.clone();
    }
}

/// Builds private rename mappings for module-local declarations.
fn private_module_renames(program: &Program, module_key: &str) -> HashMap<String, String> {
    let mut renames = HashMap::new();
    for decl in &program.product_types {
        add_private_module_rename_if(
            &mut renames,
            &decl.name,
            !decl.eager_ops && !decl.provided,
            module_key,
        );
    }
    for decl in &program.funcs {
        add_private_module_rename_if(&mut renames, &decl.name, !decl.generated, module_key);
    }
    for decl in &program.value_bindings {
        add_private_module_rename_if(&mut renames, &decl.name, !decl.generated, module_key);
    }
    for decl in &program.bindings {
        add_private_module_rename_if(&mut renames, &decl.name, !decl.generated, module_key);
    }
    for decl in &program.inferred_bindings {
        add_private_module_rename_if(&mut renames, &decl.name, !decl.generated, module_key);
    }
    renames
}

/// Adds a rename entry only when the declaration should be privatized.
fn add_private_module_rename_if(
    renames: &mut HashMap<String, String>,
    original: &str,
    should_add: bool,
    module_key: &str,
) {
    if should_add {
        add_private_module_rename(renames, original, module_key);
    }
}

/// Adds one private module rename entry, if needed.
fn add_private_module_rename(
    renames: &mut HashMap<String, String>,
    original: &str,
    module_key: &str,
) {
    renames.insert(
        original.to_string(),
        private_module_name(module_key, original),
    );
}

/// Reports a module import cycle if one is present.
fn import_cycle_error(stack: &[PathBuf], canonical: &PathBuf) -> Option<Error> {
    let index = stack.iter().position(|entry| entry == canonical)?;
    let mut cycle = stack[index..]
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>();
    cycle.push(canonical.display().to_string());
    Some(Error::new(format!(
        "module import cycle: {}",
        cycle.join(" -> ")
    )))
}

/// Performs `private_module_name` behavior.
fn private_module_name(module_key: &str, name: &str) -> String {
    format!("__lane_mod_{}_{}", sanitize_module_ident(module_key), name)
}

/// Performs `sanitize_module_ident` behavior.
fn sanitize_module_ident(source: &str) -> String {
    source
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

/// Performs `rename_type_refs` behavior.
fn rename_type_refs(types: &mut [Type], renames: &HashMap<String, String>) {
    for ty in types {
        rename_type(ty, renames);
    }
}

/// Performs `rename_type` behavior.
fn rename_type(ty: &mut Type, renames: &HashMap<String, String>) {
    match ty {
        Type::Custom { name, .. } => {
            if let Some(replacement) = renames.get(name) {
                *name = replacement.clone();
            }
        }
        Type::Array(element) => rename_type(element, renames),
        Type::Product(parts) => rename_type_refs(parts, renames),
        Type::Func(input, output) => {
            rename_type(input, renames);
            rename_type(output, renames);
        }
        _ => {}
    }
}

/// Performs `rename_func_body` behavior.
fn rename_func_body(body: &mut FuncBody, renames: &HashMap<String, String>) {
    match body {
        FuncBody::Expr(expr) => rename_expr(expr, renames),
        FuncBody::RawGlsl(body) => *body = rename_raw_glsl_placeholders(body, renames),
        FuncBody::RawGlslClosure { body, .. } => {
            *body = rename_raw_glsl_placeholders(body, renames)
        }
    }
}

/// Performs `rename_raw_glsl_placeholders` behavior.
fn rename_raw_glsl_placeholders(body: &str, renames: &HashMap<String, String>) -> String {
    rewrite_raw_glsl_placeholders(body, |name| {
        if let Some((base, field)) = name.split_once('.') {
            let base = renames.get(base).map(String::as_str).unwrap_or(base);
            format!("{base}.{field}")
        } else {
            renames
                .get(name)
                .cloned()
                .unwrap_or_else(|| name.to_string())
        }
    })
}

/// Performs `rewrite_raw_glsl_placeholders` behavior.
fn rewrite_raw_glsl_placeholders(body: &str, mut rewrite: impl FnMut(&str) -> String) -> String {
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
        if is_placeholder_ident(name) {
            out.push_str("${");
            out.push_str(&rewrite(name));
            out.push('}');
        } else {
            out.push_str(&body[start..=end]);
        }
        index = end + 1;
    }
    out.push_str(&body[index..]);
    out
}

/// Performs `is_placeholder_ident` behavior.
pub(crate) fn is_placeholder_ident(name: &str) -> bool {
    if let Some((base, field)) = name.split_once('.') {
        return is_placeholder_ident(base) && is_placeholder_ident(field);
    }
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

/// Performs `rename_expr` behavior.
pub(crate) fn rename_expr(expr: &mut Expr, renames: &HashMap<String, String>) {
    match expr {
        Expr::Bool(_) | Expr::Number(_) => {}
        Expr::RawString(_) => {}
        Expr::Closure { params, body } => {
            for param in params {
                rename_ident(param, renames);
            }
            rename_expr(body, renames);
        }
        Expr::Ident(name) => rename_ident(name, renames),
        Expr::Operator(_) => {}
        Expr::Tuple(items) | Expr::Array(items) => {
            for item in items {
                rename_expr(item, renames);
            }
        }
        Expr::Call { callee, args } => {
            rename_expr(callee, renames);
            for arg in args {
                rename_expr(arg, renames);
            }
        }
        Expr::FieldAccess { object, .. } => rename_expr(object, renames),
        Expr::Conditional {
            condition,
            then_branch,
            else_branch,
        } => {
            rename_expr(condition, renames);
            rename_expr(then_branch, renames);
            if let Some(else_branch) = else_branch {
                rename_expr(else_branch, renames);
            }
        }
        Expr::Index { array, index } => {
            rename_expr(array, renames);
            rename_expr(index, renames);
        }
        Expr::Unary { expr, .. } => rename_expr(expr, renames),
        Expr::Binary { left, right, .. } => {
            rename_expr(left, renames);
            rename_expr(right, renames);
        }
        Expr::Constructor { name, args } => {
            rename_ident(name, renames);
            match args {
                ConstructorArgs::Named(args) => {
                    for (_, expr) in args {
                        rename_expr(expr, renames);
                    }
                }
                ConstructorArgs::Positional(args) => {
                    for expr in args {
                        rename_expr(expr, renames);
                    }
                }
            }
        }
    }
}

/// Performs `rename_ident` behavior.
fn rename_ident(name: &mut String, renames: &HashMap<String, String>) {
    if let Some(replacement) = renames.get(name) {
        *name = replacement.clone();
    }
}
