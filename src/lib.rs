use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

mod emit;
mod parser;
mod registry;
mod typecheck;

pub fn compile_program(source: &str) -> Result<String, Error> {
    compile_program_with_base_dir(
        source,
        &std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    )
}

pub fn compile_program_from_path(path: impl AsRef<Path>) -> Result<String, Error> {
    let path = path.as_ref();
    let source = fs::read_to_string(path).map_err(|err| Error::new(err.to_string()))?;
    let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
    compile_program_with_base_dir(&source, base_dir)
}

pub fn compile_program_with_base_dir(
    source: &str,
    base_dir: impl AsRef<Path>,
) -> Result<String, Error> {
    compile_program_with_base(source, base_dir.as_ref())
}

pub fn compile_preview_fragment_from_path(
    path: impl AsRef<Path>,
    version: &str,
) -> Result<String, Error> {
    let path = path.as_ref();
    let source = fs::read_to_string(path).map_err(|err| Error::new(err.to_string()))?;
    let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
    compile_preview_fragment(&source, base_dir, version, PreviewShaderTarget::OpenGl)
}

pub fn compile_vulkan_preview_fragment_from_path(path: impl AsRef<Path>) -> Result<String, Error> {
    let path = path.as_ref();
    let source = fs::read_to_string(path).map_err(|err| Error::new(err.to_string()))?;
    let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
    compile_preview_fragment(&source, base_dir, "450", PreviewShaderTarget::Vulkan)
}

pub fn compile_vulkan_preview_fragment(source: &str) -> Result<String, Error> {
    compile_preview_fragment(
        source,
        &std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        "450",
        PreviewShaderTarget::Vulkan,
    )
}

pub fn compile_preview_vertex(version: &str) -> String {
    format!(
        "{}\nprecision highp float;\n\nconst vec2 vertices[3] = vec2[3](\n    vec2(-1.0, -1.0),\n    vec2(3.0, -1.0),\n    vec2(-1.0, 3.0)\n);\n\nvoid main() {{\n    gl_Position = vec4(vertices[gl_VertexID], 0.0, 1.0);\n}}\n",
        glsl_version_directive(version)
    )
}

pub fn compile_vulkan_preview_vertex() -> String {
    "#version 450\n\nconst vec2 vertices[3] = vec2[3](\n    vec2(-1.0, -1.0),\n    vec2(3.0, -1.0),\n    vec2(-1.0, 3.0)\n);\n\nvoid main() {\n    gl_Position = vec4(vertices[gl_VertexIndex], 0.0, 1.0);\n}\n"
        .to_string()
}

fn compile_program_with_base(source: &str, base_dir: &Path) -> Result<String, Error> {
    let registry = Registry::default();
    let program = ModuleLoader::new(base_dir).load_main(source)?;
    let typed = TypedProgram::from_program(&program, &registry)?;
    Ok(typed.emit_glsl(&registry))
}

#[derive(Clone, Copy)]
enum PreviewShaderTarget {
    OpenGl,
    Vulkan,
}

fn compile_preview_fragment(
    source: &str,
    base_dir: &Path,
    version: &str,
    target: PreviewShaderTarget,
) -> Result<String, Error> {
    let source = prepare_preview_source(source);
    let registry = Registry::default();
    let program = ModuleLoader::new(base_dir).load_main(&source)?;
    reject_preview_provided_functions(&program)?;
    let uniforms = preview_uniforms(&program, target);
    let typed = TypedProgram::from_program(&program, &registry)?;
    let body = typed.emit_glsl(&registry);
    let output = match target {
        PreviewShaderTarget::OpenGl => "out vec4 outColor;",
        PreviewShaderTarget::Vulkan => "layout(location = 0) out vec4 outColor;",
    };
    let precision = match target {
        PreviewShaderTarget::OpenGl => "precision highp float;\n\n",
        PreviewShaderTarget::Vulkan => "",
    };
    Ok(format!(
        "{}\n\n{}{}\n\n{}\n{}",
        glsl_version_directive(version),
        precision,
        output,
        uniforms,
        body
    ))
}

fn prepare_preview_source(source: &str) -> String {
    let mut out = String::new();
    if !source
        .lines()
        .any(|line| line.trim() == "#import raytracing")
    {
        out.push_str("#import raytracing\n");
    }
    out.push_str(source);
    if !has_const_object(source, "scene") {
        if let Some(name) = last_const_object_name(source) {
            out.push('\n');
            out.push_str(&format!("const Object scene = {name}\n"));
        }
    }
    if !has_const_main(source) {
        append_preview_provided_values(source, &mut out);
        out.push_str(
            "\nconst Hom(R2, Ray) preview_camera_ray = camera_ray(Camera(cameraPosition, cameraForward, cameraGlobalUp, resolution))\n\
const Hom(Ray, Hit) preview_hit = raytrace_with(default_raytrace_config, scene)\n\
const Hom(Hit, R3) preview_material_color = hit -> material_color(scene_material(hit.position))\n\
const Hom(Hit, R3) preview_material_emission = hit -> material_emission(scene_material(hit.position))\n\
const Hom(Hit, R) preview_material_reflectiveness = hit -> material_reflectiveness(scene_material(hit.position))\n\
const Hom(Ray, R3) preview_color = raycolor_from_hit_with(default_raycolor_config, ambientColor, preview_hit, preview_material_color, preview_material_emission, preview_material_reflectiveness)\n\
const Hom(R2, R4) preview_shade = shade(preview_camera_ray, preview_color)\n\
const Hom(*, *) main = fragment_main(preview_shade)\n",
        );
    }
    out
}

fn append_preview_provided_values(source: &str, out: &mut String) {
    let provided = [
        ("R3", "cameraPosition"),
        ("R3", "cameraForward"),
        ("R3", "cameraGlobalUp"),
        ("R2", "resolution"),
        ("R3", "ambientColor"),
    ];
    for (ty, name) in provided {
        if !has_provided_value(source, name) {
            out.push_str(&format!("\nprovided {ty} {name}"));
        }
    }
}

fn has_const_object(source: &str, object_name: &str) -> bool {
    source.lines().any(|line| {
        line.trim_start()
            .starts_with(&format!("const Object {object_name}"))
    })
}

fn last_const_object_name(source: &str) -> Option<String> {
    source
        .lines()
        .rev()
        .filter_map(|line| {
            let line = line
                .split_once("//")
                .map_or(line, |(before, _)| before)
                .trim();
            let rest = line.strip_prefix("const Object ")?;
            let (name, _) = rest.split_once('=')?;
            Some(name.trim().to_string())
        })
        .next()
}

fn has_const_main(source: &str) -> bool {
    source
        .lines()
        .map(|line| {
            line.split_once("//")
                .map_or(line, |(before, _)| before)
                .trim()
        })
        .any(|line| {
            line.starts_with("const Hom(*, *) main")
                || line.starts_with("const Hom(*,*) main")
                || line.starts_with("const Func(*, *) main")
                || line.starts_with("const Func(*,*) main")
        })
}

fn has_provided_value(source: &str, value_name: &str) -> bool {
    source
        .lines()
        .map(|line| {
            line.split_once("//")
                .map_or(line, |(before, _)| before)
                .trim()
        })
        .any(|line| {
            let Some(rest) = line.strip_prefix("provided ") else {
                return false;
            };
            rest.split_whitespace().nth(1) == Some(value_name)
        })
}

fn reject_preview_provided_functions(program: &Program) -> Result<(), Error> {
    for input in &program.inputs {
        if matches!(input.ty, Type::Func(_, _)) {
            return Err(Error::new(format!(
                "preview shaders do not support provided function '{}'",
                input.name
            ))
            .with_line(input.line));
        }
    }
    Ok(())
}

fn preview_uniforms(program: &Program, target: PreviewShaderTarget) -> String {
    let uniforms = program
        .inputs
        .iter()
        .filter(|input| !matches!(input.ty, Type::Object | Type::Object2D))
        .map(|input| format!("    {} {};", input.ty.glsl_name(), input.name))
        .collect::<Vec<_>>();
    match target {
        PreviewShaderTarget::OpenGl => uniforms
            .into_iter()
            .map(|uniform| format!("uniform {};", uniform.trim_end_matches(';').trim()))
            .collect::<Vec<_>>()
            .join("\n"),
        PreviewShaderTarget::Vulkan => {
            if uniforms.is_empty() {
                String::new()
            } else {
                format!(
                    "layout(std140, push_constant) uniform PreviewUniforms {{\n{}\n}};",
                    uniforms.join("\n")
                )
            }
        }
    }
}

fn glsl_version_directive(version: &str) -> String {
    let trimmed = version.trim();
    if let Some(number) = trimmed.strip_suffix("es") {
        format!("#version {} es", number.trim())
    } else {
        format!("#version {trimmed}")
    }
}

struct ModuleLoader {
    base_dir: PathBuf,
}

impl ModuleLoader {
    fn new(base_dir: &Path) -> Self {
        Self {
            base_dir: base_dir.to_path_buf(),
        }
    }

    fn load_main(&self, source: &str) -> Result<Program, Error> {
        let mut stack = Vec::new();
        let parsed = parser::Parser::new(source).parse_program()?;
        if parsed.is_module {
            return Err(Error::new("#module is only valid in imported module files"));
        }
        let mut merged = self.expand_program(parsed, false, "main", &mut stack)?;
        reject_duplicate_product_types(&merged)?;
        merged.imports.clear();
        Ok(merged)
    }

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
        let mut line_offset = 0usize;
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
        append_program(&mut merged, program, line_offset);
        Ok(merged)
    }

    fn load_import(&self, import_path: &str, stack: &mut Vec<PathBuf>) -> Result<Program, Error> {
        let path = self.resolve_import(import_path)?;
        let canonical = path.canonicalize().unwrap_or(path.clone());
        if let Some(index) = stack.iter().position(|entry| entry == &canonical) {
            let mut cycle = stack[index..]
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>();
            cycle.push(canonical.display().to_string());
            return Err(Error::new(format!(
                "module import cycle: {}",
                cycle.join(" -> ")
            )));
        }

        stack.push(canonical);
        let source = fs::read_to_string(&path).map_err(|err| Error::new(err.to_string()))?;
        let parsed = parser::Parser::new(&source).parse_program()?;
        let module_key = import_path.replace(['/', '\\', '.', '-'], "_");
        let expanded = self.expand_program(parsed, true, &module_key, stack);
        stack.pop();
        expanded
    }

    fn resolve_import(&self, import_path: &str) -> Result<PathBuf, Error> {
        let relative = import_candidate(import_path);
        let mut roots = Vec::new();
        if let Ok(paths) = std::env::var("LANE_MODULE_PATH") {
            roots.extend(std::env::split_paths(&paths));
        }
        roots.push(self.base_dir.clone());
        roots.push(self.base_dir.join("modules"));
        roots.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("modules"));

        for root in roots {
            let candidate = root.join(&relative);
            if candidate.exists() {
                return Ok(candidate);
            }
        }
        Err(Error::new(format!("module '{import_path}' not found")))
    }
}

fn import_candidate(import_path: &str) -> PathBuf {
    let path = PathBuf::from(import_path);
    if path.extension().is_some() {
        path
    } else {
        path.with_extension("lane")
    }
}

fn empty_program_like(program: &Program) -> Program {
    Program {
        ambient_dimension: program.ambient_dimension,
        derivative_epsilon: program.derivative_epsilon,
        gradient_epsilon: program.gradient_epsilon,
        is_module: false,
        imports: Vec::new(),
        product_types: Vec::new(),
        inputs: Vec::new(),
        funcs: Vec::new(),
        value_bindings: Vec::new(),
        bindings: Vec::new(),
        inferred_bindings: Vec::new(),
        output: None,
    }
}

fn append_program(target: &mut Program, mut source: Program, line_offset: usize) {
    bump_program_lines(&mut source, line_offset);
    target.product_types.extend(source.product_types);
    target.inputs.extend(source.inputs);
    target.funcs.extend(source.funcs);
    target.value_bindings.extend(source.value_bindings);
    target.bindings.extend(source.bindings);
    target.inferred_bindings.extend(source.inferred_bindings);
    if let Some(output) = source.output {
        target.output = Some(output);
    }
}

fn bump_program_lines(program: &mut Program, offset: usize) {
    for item in &mut program.product_types {
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
    if let Some(output) = &mut program.output {
        output.line += offset;
    }
}

fn reject_duplicate_product_types(program: &Program) -> Result<(), Error> {
    let mut names = HashSet::new();
    for decl in &program.product_types {
        if !names.insert(decl.name.clone()) {
            return Err(
                Error::new(format!("duplicate product type '{}'", decl.name)).with_line(decl.line),
            );
        }
    }
    Ok(())
}

fn mangle_private_module_names(program: &mut Program, module_key: &str) {
    let mut renames = HashMap::new();
    for decl in &program.product_types {
        if !decl.eager_ops && !decl.provided {
            renames.insert(
                decl.name.clone(),
                private_module_name(module_key, &decl.name),
            );
        }
    }
    for decl in &program.funcs {
        if !decl.generated {
            renames.insert(
                decl.name.clone(),
                private_module_name(module_key, &decl.name),
            );
        }
    }
    for decl in &program.value_bindings {
        if !decl.generated {
            renames.insert(
                decl.name.clone(),
                private_module_name(module_key, &decl.name),
            );
        }
    }
    for decl in &program.bindings {
        if !decl.generated {
            renames.insert(
                decl.name.clone(),
                private_module_name(module_key, &decl.name),
            );
        }
    }
    for decl in &program.inferred_bindings {
        if !decl.generated {
            renames.insert(
                decl.name.clone(),
                private_module_name(module_key, &decl.name),
            );
        }
    }

    for decl in &mut program.product_types {
        rename_type_refs(&mut decl.components, &renames);
        if let Some(name) = renames.get(&decl.name) {
            decl.name = name.clone();
        }
    }
    for decl in &mut program.inputs {
        rename_type(&mut decl.ty, &renames);
    }
    for decl in &mut program.funcs {
        rename_type(&mut decl.ty, &renames);
        rename_func_body(&mut decl.body, &renames);
        if let Some(name) = renames.get(&decl.name) {
            decl.name = name.clone();
        }
    }
    for decl in &mut program.value_bindings {
        rename_type(&mut decl.ty, &renames);
        rename_expr(&mut decl.expr, &renames);
        if let Some(name) = renames.get(&decl.name) {
            decl.name = name.clone();
        }
    }
    for decl in &mut program.bindings {
        rename_type(&mut decl.ty, &renames);
        rename_expr(&mut decl.expr, &renames);
        if let Some(name) = renames.get(&decl.name) {
            decl.name = name.clone();
        }
    }
    for decl in &mut program.inferred_bindings {
        rename_expr(&mut decl.expr, &renames);
        if let Some(name) = renames.get(&decl.name) {
            decl.name = name.clone();
        }
    }
}

fn private_module_name(module_key: &str, name: &str) -> String {
    format!("__lane_mod_{}_{}", sanitize_module_ident(module_key), name)
}

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

fn rename_type_refs(types: &mut [Type], renames: &HashMap<String, String>) {
    for ty in types {
        rename_type(ty, renames);
    }
}

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

fn rename_func_body(body: &mut FuncBody, renames: &HashMap<String, String>) {
    match body {
        FuncBody::Expr(expr) => rename_expr(expr, renames),
        FuncBody::RawGlsl(body) => *body = rename_raw_glsl_placeholders(body, renames),
        FuncBody::RawGlslClosure { body, .. } => {
            *body = rename_raw_glsl_placeholders(body, renames)
        }
    }
}

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

fn is_placeholder_ident(name: &str) -> bool {
    if let Some((base, field)) = name.split_once('.') {
        return is_placeholder_ident(base) && is_placeholder_ident(field);
    }
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

fn rename_expr(expr: &mut Expr, renames: &HashMap<String, String>) {
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

fn rename_ident(name: &mut String, renames: &HashMap<String, String>) {
    if let Some(replacement) = renames.get(name) {
        *name = replacement.clone();
    }
}

pub(crate) fn suffix_glsl_float_literals(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let chars = source.chars().collect::<Vec<_>>();
    let mut index = 0;

    while index < chars.len() {
        let ch = chars[index];
        if ch.is_ascii_digit()
            && (index == 0
                || !(chars[index - 1].is_ascii_alphanumeric()
                    || matches!(chars[index - 1], '_' | '.')))
        {
            let start = index;
            while index < chars.len() && chars[index].is_ascii_digit() {
                index += 1;
            }

            let mut is_float = false;
            if index < chars.len()
                && chars[index] == '.'
                && index + 1 < chars.len()
                && chars[index + 1].is_ascii_digit()
            {
                is_float = true;
                index += 1;
                while index < chars.len() && chars[index].is_ascii_digit() {
                    index += 1;
                }
            }

            if index < chars.len() && matches!(chars[index], 'e' | 'E') {
                let exponent_index = index;
                let mut scan = index + 1;
                if scan < chars.len() && matches!(chars[scan], '+' | '-') {
                    scan += 1;
                }
                let digits_start = scan;
                while scan < chars.len() && chars[scan].is_ascii_digit() {
                    scan += 1;
                }
                if scan > digits_start {
                    is_float = true;
                    index = scan;
                } else {
                    index = exponent_index;
                }
            }

            let literal = chars[start..index].iter().collect::<String>();
            out.push_str(&literal);
            if is_float && !matches!(chars.get(index), Some('f' | 'F')) {
                out.push('f');
            }
            continue;
        }

        out.push(ch);
        index += 1;
    }

    out
}

pub fn known_primitives() -> Vec<KnownPrimitive> {
    let registry = Registry::default();
    registry.known_primitives()
}

pub fn known_primitives_by_dimension(dimension: ShapeDimension) -> Vec<KnownPrimitive> {
    known_primitives()
        .into_iter()
        .filter(|primitive| primitive.dimension == dimension)
        .collect()
}

pub fn known_primitive(name: &str) -> Option<KnownPrimitive> {
    let registry = Registry::default();
    registry.known_primitive(name)
}

pub fn known_preregistered_objects() -> Vec<PreregisteredObject> {
    let registry = Registry::default();
    registry.preregistered_objects()
}

pub fn known_builtin_objects() -> Vec<KnownBuiltinObject> {
    let registry = Registry::default();
    registry.known_builtin_objects()
}

pub fn known_builtin_object(name: &str) -> Option<KnownBuiltinObjectDetail> {
    let registry = Registry::default();
    registry.known_builtin_object(name)
}

pub fn preregistered_object(name: &str) -> Option<PreregisteredObject> {
    let registry = Registry::default();
    registry.preregistered_object(name)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    message: String,
    line: Option<usize>,
}

impl Error {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            line: None,
        }
    }

    fn with_line(mut self, line: usize) -> Self {
        if self.line.is_none() {
            self.line = Some(line);
        }
        self
    }

    pub fn line(&self) -> Option<usize> {
        self.line
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(line) = self.line {
            write!(f, "line {line}: {}", self.message)
        } else {
            f.write_str(&self.message)
        }
    }
}

impl std::error::Error for Error {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KnownPrimitive {
    pub name: String,
    pub dimension: ShapeDimension,
    pub parameter_space: String,
    pub fields: Vec<KnownPrimitiveField>,
    pub type_body: Option<String>,
    pub function_body: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeDimension {
    D2,
    D3,
}

impl ShapeDimension {
    pub fn label(self) -> &'static str {
        match self {
            Self::D2 => "2D",
            Self::D3 => "3D",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KnownPrimitiveField {
    pub name: String,
    pub domain: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KnownBuiltinObject {
    pub name: String,
    pub ty: String,
    pub kind: KnownBuiltinObjectKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KnownBuiltinObjectDetail {
    pub name: String,
    pub ty: String,
    pub kind: KnownBuiltinObjectKind,
    pub body: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KnownBuiltinObjectKind {
    Function,
    Type,
    Category,
}

pub const TYPE_METATYPE_NAME: &str = "Type";
pub const CATEGORY_METATYPE_NAME: &str = "Cat";

pub fn known_type_names() -> Vec<&'static str> {
    let mut names = BUILTIN_TYPE_DEFS
        .iter()
        .flat_map(|def| def.aliases.iter().copied())
        .collect::<Vec<_>>();
    names.extend(MATRIX_TYPE_NAMES.iter().copied());
    names
}

pub fn is_known_type_name(name: &str) -> bool {
    parse_builtin_type_name(name).is_some()
}

pub fn known_category_names() -> Vec<&'static str> {
    ALGEBRAIC_CATEGORY_DEFS.iter().map(|def| def.name).collect()
}

pub fn is_known_category_name(name: &str) -> bool {
    category_by_name(name).is_some()
}

const BUILTIN_TYPE_DETAILS: [(&str, &str); 5] = [
    ("Bool", ""),
    ("C", "#define Complex vec2"),
    ("Isom2", ""),
    ("Isom3", ""),
    ("H", "#define H vec4"),
];

const COMPLEX_FIELD_SUPPORT_GLSL: &str = "vec2 mult_C(vec2 a, vec2 b) {\n    return vec2((a.x * b.x) - (a.y * b.y), (a.x * b.y) + (a.y * b.x));\n}\n\nvec2 div_C(vec2 a, vec2 b) {\n    return mult_C(a, vec2(b.x, -b.y) / dot(b, b));\n}";

const COMPLEX_OVERLOAD_NAMES: [&str; 10] = [
    "inv", "exp", "log", "sqrt", "sin", "cos", "tan", "sinh", "cosh", "tanh",
];

fn complex_overload_name(name: &str) -> Option<&'static str> {
    match name {
        "cinv" => Some("inv"),
        "cexp" => Some("exp"),
        "clog" => Some("log"),
        "csqrt" => Some("sqrt"),
        "csin" => Some("sin"),
        "ccos" => Some("cos"),
        "ctan" => Some("tan"),
        "csinh" => Some("sinh"),
        "ccosh" => Some("cosh"),
        "ctanh" => Some("tanh"),
        "inv" => Some("inv"),
        "exp" => Some("exp"),
        "log" => Some("log"),
        "sqrt" => Some("sqrt"),
        "sin" => Some("sin"),
        "cos" => Some("cos"),
        "tan" => Some("tan"),
        "sinh" => Some("sinh"),
        "cosh" => Some("cosh"),
        "tanh" => Some("tanh"),
        _ => None,
    }
}

fn complex_overload_support_glsl(name: &str) -> Option<&'static str> {
    match name {
        "inv" => Some("vec2 inv(vec2 z) {\n    return vec2(z.x, -z.y) / dot(z, z);\n}"),
        "exp" => Some("vec2 exp(vec2 z) {\n    float scale = exp(z.x);\n    return scale * vec2(cos(z.y), sin(z.y));\n}"),
        "log" => Some("vec2 log(vec2 z) {\n    return vec2(log(length(z)), atan(z.y, z.x));\n}"),
        "pow" => Some("vec2 pow(vec2 z, vec2 w) {\n    return exp(mult_C(w, log(z)));\n}"),
        "sqrt" => Some("vec2 sqrt(vec2 z) {\n    float r = length(z);\n    float a = sqrt(max((r + z.x) * 0.5, 0.0));\n    float b = sqrt(max((r - z.x) * 0.5, 0.0));\n    return vec2(a, sign(z.y) * b);\n}"),
        "sin" => Some("vec2 sin(vec2 z) {\n    return vec2(sin(z.x) * cosh(z.y), cos(z.x) * sinh(z.y));\n}"),
        "cos" => Some("vec2 cos(vec2 z) {\n    return vec2(cos(z.x) * cosh(z.y), -sin(z.x) * sinh(z.y));\n}"),
        "tan" => Some("vec2 tan(vec2 z) {\n    float d = cos(2.0 * z.x) + cosh(2.0 * z.y);\n    return vec2(sin(2.0 * z.x), sinh(2.0 * z.y)) / d;\n}"),
        "sinh" => Some("vec2 sinh(vec2 z) {\n    return vec2(sinh(z.x) * cos(z.y), cosh(z.x) * sin(z.y));\n}"),
        "cosh" => Some("vec2 cosh(vec2 z) {\n    return vec2(cosh(z.x) * cos(z.y), sinh(z.x) * sin(z.y));\n}"),
        "tanh" => Some("vec2 tanh(vec2 z) {\n    float d = cosh(2.0 * z.x) + cos(2.0 * z.y);\n    return vec2(sinh(2.0 * z.x), sin(2.0 * z.y)) / d;\n}"),
        _ => None,
    }
}

const ISOM2_GROUP_SUPPORT_GLSL: &str = "struct Isom2 {\n    mat2 A;\n    vec2 t;\n};\n\nvec2 act_Isom2(Isom2 g, vec2 p) {\n    return (g.A * p) + g.t;\n}\n\nIsom2 mult_Isom2(Isom2 a, Isom2 b) {\n    return Isom2(a.A * b.A, (a.A * b.t) + a.t);\n}\n\nIsom2 inv_Isom2(Isom2 g) {\n    mat2 inverse_linear = transpose(g.A);\n    return Isom2(inverse_linear, -(inverse_linear * g.t));\n}";

const QUAT_FIELD_SUPPORT_GLSL: &str = "vec4 mult_H(vec4 a, vec4 b) {\n    return vec4(\n        a.x * b.x - a.y * b.y - a.z * b.z - a.w * b.w,\n        a.x * b.y + a.y * b.x + a.z * b.w - a.w * b.z,\n        a.x * b.z - a.y * b.w + a.z * b.x + a.w * b.y,\n        a.x * b.w + a.y * b.z - a.z * b.y + a.w * b.x\n    );\n}\n\nvec4 inv_H(vec4 q) {\n    return vec4(q.x, -q.y, -q.z, -q.w) / dot(q, q);\n}\n\nvec4 div_H(vec4 a, vec4 b) {\n    return mult_H(a, inv_H(b));\n}";

const ISOM3_GROUP_SUPPORT_GLSL: &str = "struct Isom3 {\n    mat3 A;\n    vec3 t;\n};\n\nvec3 act_Isom3(Isom3 g, vec3 p) {\n    return (g.A * p) + g.t;\n}\n\nIsom3 mult_Isom3(Isom3 a, Isom3 b) {\n    return Isom3(a.A * b.A, (a.A * b.t) + a.t);\n}\n\nIsom3 inv_Isom3(Isom3 g) {\n    mat3 inverse_linear = transpose(g.A);\n    return Isom3(inverse_linear, -(inverse_linear * g.t));\n}\n\nmat3 rot_Isom3_matrix(vec3 binormal, float angle) {\n    vec3 axis = normalize(binormal);\n    float c = cos(angle);\n    float s = sin(angle);\n    float oc = 1.0 - c;\n    return mat3(\n        vec3((axis.x * axis.x * oc) + c, (axis.y * axis.x * oc) + (axis.z * s), (axis.z * axis.x * oc) - (axis.y * s)),\n        vec3((axis.x * axis.y * oc) - (axis.z * s), (axis.y * axis.y * oc) + c, (axis.z * axis.y * oc) + (axis.x * s)),\n        vec3((axis.x * axis.z * oc) + (axis.y * s), (axis.y * axis.z * oc) - (axis.x * s), (axis.z * axis.z * oc) + c)\n    );\n}\n\nIsom3 rot(vec3 binormal, vec3 anchor, float angle) {\n    mat3 A = rot_Isom3_matrix(binormal, angle);\n    return Isom3(A, anchor - (A * anchor));\n}";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AlgebraicCategory {
    Ab,
    Mon,
    Grp,
    Ring,
    DivRing,
    VectR,
    RAlg,
    Set,
}

struct AlgebraicCategoryDef {
    category: AlgebraicCategory,
    name: &'static str,
}

const ALGEBRAIC_CATEGORY_DEFS: [AlgebraicCategoryDef; 8] = [
    AlgebraicCategoryDef {
        category: AlgebraicCategory::Ab,
        name: "Ab",
    },
    AlgebraicCategoryDef {
        category: AlgebraicCategory::Mon,
        name: "Mon",
    },
    AlgebraicCategoryDef {
        category: AlgebraicCategory::Grp,
        name: "Grp",
    },
    AlgebraicCategoryDef {
        category: AlgebraicCategory::Ring,
        name: "Ring",
    },
    AlgebraicCategoryDef {
        category: AlgebraicCategory::DivRing,
        name: "DivRing",
    },
    AlgebraicCategoryDef {
        category: AlgebraicCategory::VectR,
        name: "VectR",
    },
    AlgebraicCategoryDef {
        category: AlgebraicCategory::RAlg,
        name: "RAlg",
    },
    AlgebraicCategoryDef {
        category: AlgebraicCategory::Set,
        name: "Set",
    },
];

fn category_by_name(name: &str) -> Option<AlgebraicCategory> {
    ALGEBRAIC_CATEGORY_DEFS
        .iter()
        .find(|def| def.name == name)
        .map(|def| def.category)
}

fn category_name(category: AlgebraicCategory) -> &'static str {
    ALGEBRAIC_CATEGORY_DEFS
        .iter()
        .find(|def| def.category == category)
        .map(|def| def.name)
        .unwrap()
}

fn type_category_signature(name: &str) -> Option<String> {
    let ty = parse_builtin_type_name(name)?;
    let categories = minimal_categories(type_direct_categories(&ty));
    if categories.is_empty() {
        return Some(TYPE_METATYPE_NAME.to_string());
    }
    Some(format_categories(&categories))
}

fn minimal_categories(categories: Vec<AlgebraicCategory>) -> Vec<AlgebraicCategory> {
    categories
        .iter()
        .copied()
        .filter(|category| {
            !categories
                .iter()
                .any(|other| other != category && category_implies(*other, *category))
        })
        .collect()
}

fn type_direct_categories(ty: &Type) -> Vec<AlgebraicCategory> {
    if let Type::Mat(rows, columns) = ty {
        let mut categories = Vec::new();
        if rows == columns {
            categories.push(AlgebraicCategory::Ring);
        }
        categories.push(AlgebraicCategory::VectR);
        return categories;
    }

    BUILTIN_TYPE_DEFS
        .iter()
        .find(|def| &def.ty == ty)
        .map(|def| def.categories.to_vec())
        .unwrap_or_default()
}

fn format_categories(categories: &[AlgebraicCategory]) -> String {
    categories
        .iter()
        .map(|category| category_name(*category))
        .collect::<Vec<_>>()
        .join(", ")
}

struct BuiltinTypeDef {
    ty: Type,
    aliases: &'static [&'static str],
    display_name: &'static str,
    support_glsl: Option<&'static str>,
    categories: &'static [AlgebraicCategory],
}

const MATRIX_TYPE_NAMES: [&str; 9] = [
    "Mat2", "Mat2x3", "Mat2x4", "Mat3x2", "Mat3", "Mat3x4", "Mat4x2", "Mat4x3", "Mat4",
];

const BUILTIN_TYPE_DEFS: [BuiltinTypeDef; 12] = [
    BuiltinTypeDef {
        ty: Type::Bool,
        aliases: &["Bool"],
        display_name: "Bool",
        support_glsl: None,
        categories: &[AlgebraicCategory::DivRing],
    },
    BuiltinTypeDef {
        ty: Type::Float,
        aliases: &["Float", "R"],
        display_name: "R",
        support_glsl: None,
        categories: &[
            AlgebraicCategory::DivRing,
            AlgebraicCategory::Grp,
            AlgebraicCategory::RAlg,
            AlgebraicCategory::VectR,
        ],
    },
    BuiltinTypeDef {
        ty: Type::Int,
        aliases: &["Int", "Z"],
        display_name: "Z",
        support_glsl: None,
        categories: &[AlgebraicCategory::Ring],
    },
    BuiltinTypeDef {
        ty: Type::Complex,
        aliases: &["Complex", "C"],
        display_name: "C",
        support_glsl: Some(COMPLEX_FIELD_SUPPORT_GLSL),
        categories: &[
            AlgebraicCategory::DivRing,
            AlgebraicCategory::Grp,
            AlgebraicCategory::RAlg,
            AlgebraicCategory::VectR,
        ],
    },
    BuiltinTypeDef {
        ty: Type::Vec2,
        aliases: &["Vec2", "R2"],
        display_name: "R2",
        support_glsl: None,
        categories: &[AlgebraicCategory::VectR],
    },
    BuiltinTypeDef {
        ty: Type::Vec3,
        aliases: &["Vec3", "R3"],
        display_name: "R3",
        support_glsl: None,
        categories: &[AlgebraicCategory::VectR],
    },
    BuiltinTypeDef {
        ty: Type::Vec4,
        aliases: &["Vec4", "R4"],
        display_name: "R4",
        support_glsl: None,
        categories: &[AlgebraicCategory::VectR],
    },
    BuiltinTypeDef {
        ty: Type::Quat,
        aliases: &["H"],
        display_name: "H",
        support_glsl: Some(QUAT_FIELD_SUPPORT_GLSL),
        categories: &[
            AlgebraicCategory::DivRing,
            AlgebraicCategory::Grp,
            AlgebraicCategory::RAlg,
            AlgebraicCategory::VectR,
        ],
    },
    BuiltinTypeDef {
        ty: Type::Object,
        aliases: &["Object", "Object3D"],
        display_name: "Object",
        support_glsl: None,
        categories: &[],
    },
    BuiltinTypeDef {
        ty: Type::Object2D,
        aliases: &["Object2D"],
        display_name: "Object2D",
        support_glsl: None,
        categories: &[],
    },
    BuiltinTypeDef {
        ty: Type::Isom2,
        aliases: &["Isom2"],
        display_name: "Isom2",
        support_glsl: Some(ISOM2_GROUP_SUPPORT_GLSL),
        categories: &[AlgebraicCategory::Grp],
    },
    BuiltinTypeDef {
        ty: Type::Isom3,
        aliases: &["Isom3"],
        display_name: "Isom3",
        support_glsl: Some(ISOM3_GROUP_SUPPORT_GLSL),
        categories: &[AlgebraicCategory::Grp],
    },
];

fn builtin_type_support_glsl(name: &str) -> Option<&'static str> {
    BUILTIN_TYPE_DEFS
        .iter()
        .find(|def| def.aliases.contains(&name))
        .and_then(|def| def.support_glsl)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PreregisteredObjectKind {
    Function,
    Type,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreregisteredObject {
    pub name: String,
    pub kind: PreregisteredObjectKind,
    pub body: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum GenericDim {
    Known(usize),
    Var(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Type {
    Unit,
    Bool,
    Float,
    Int,
    Complex,
    Quat,
    Isom2,
    Isom3,
    Custom {
        name: String,
        categories: Vec<AlgebraicCategory>,
    },
    Vec2,
    Vec3,
    Vec4,
    Mat(usize, usize),
    Generic(String),
    VecGeneric(GenericDim),
    MatGeneric(GenericDim, GenericDim),
    Array(Box<Type>),
    Object,
    Object2D,
    Product(Vec<Type>),
    Func(Box<Type>, Box<Type>),
}

impl Type {
    fn func(input: Type, output: Type) -> Self {
        Self::Func(Box::new(input), Box::new(output))
    }

    fn glsl_name(&self) -> String {
        match self {
            Self::Unit => "void".to_string(),
            Self::Bool => "bool".to_string(),
            Self::Float => "float".to_string(),
            Self::Int => "int".to_string(),
            Self::Complex => "vec2".to_string(),
            Self::Quat => "vec4".to_string(),
            Self::Isom2 => "Isom2".to_string(),
            Self::Isom3 => "Isom3".to_string(),
            Self::Custom { name, .. } => name.clone(),
            Self::Vec2 => "vec2".to_string(),
            Self::Vec3 => "vec3".to_string(),
            Self::Vec4 => "vec4".to_string(),
            Self::Mat(rows, columns) => matrix_glsl_type(*rows, *columns),
            Self::Generic(_) | Self::VecGeneric(_) | Self::MatGeneric(_, _) => String::new(),
            Self::Array(element) => format!("{}[]", element.glsl_name()),
            Self::Object | Self::Object2D | Self::Product(_) | Self::Func(_, _) => "".to_string(),
        }
    }

    fn type_name(&self) -> String {
        match self {
            Self::Mat(rows, columns) => return matrix_type_name(*rows, *columns),
            Self::Generic(name) => return format!("{{{name}}}"),
            Self::VecGeneric(dim) => return format!("R{{{}}}", format_generic_dim(dim)),
            Self::MatGeneric(rows, columns) => {
                return format!(
                    "Mat{{{}}}x{{{}}}",
                    format_generic_dim(rows),
                    format_generic_dim(columns)
                );
            }
            _ => {}
        }
        BUILTIN_TYPE_DEFS
            .iter()
            .find(|def| &def.ty == self)
            .map(|def| def.display_name.to_string())
            .unwrap_or_else(|| match self {
                Self::Unit => "*".to_string(),
                Self::Custom { name, .. } => name.clone(),
                Self::Product(_) => "Product".to_string(),
                Self::Func(_, _) => "Func".to_string(),
                Self::Array(element) => format!("Array({})", format_type(element)),
                _ => unreachable!(),
            })
    }
}

fn parse_builtin_type_name(name: &str) -> Option<Type> {
    if let Some(ty) = parse_generic_type_name(name) {
        return Some(ty);
    }
    BUILTIN_TYPE_DEFS
        .iter()
        .find(|def| def.aliases.contains(&name))
        .map(|def| def.ty.clone())
        .or_else(|| parse_matrix_type_name(name))
}

fn parse_generic_type_name(name: &str) -> Option<Type> {
    if let Some(inner) = name
        .strip_prefix('R')
        .and_then(|rest| rest.strip_prefix('{'))
    {
        let dim = inner.strip_suffix('}')?;
        return Some(vector_type_for_generic_dim(parse_generic_dim(dim)?));
    }
    if let Some(inner) = name.strip_prefix("Mat") {
        if let Some(square) = parse_braced_generic_dim(inner) {
            return Some(matrix_type_for_generic_dims(square.clone(), square));
        }
        let (rows, columns) = split_matrix_generic_dims(inner)?;
        return Some(matrix_type_for_generic_dims(rows, columns));
    }
    let inner = name.strip_prefix('{')?.strip_suffix('}')?;
    parse_generic_name(inner).map(Type::Generic)
}

fn custom_type(name: &str, category: AlgebraicCategory) -> Type {
    Type::Custom {
        name: name.to_string(),
        categories: vec![category],
    }
}

fn product_type_decl_type(decl: &ProductTypeDecl) -> Type {
    custom_type(&decl.name, decl.category)
}

fn parse_matrix_type_name(name: &str) -> Option<Type> {
    let suffix = name.strip_prefix("Mat")?;
    if suffix.len() == 1 {
        let dimension = parse_matrix_dimension(suffix)?;
        return Some(Type::Mat(dimension, dimension));
    }

    let (rows, columns) = suffix.split_once('x')?;
    Some(Type::Mat(
        parse_matrix_dimension(rows)?,
        parse_matrix_dimension(columns)?,
    ))
}

fn parse_matrix_dimension(source: &str) -> Option<usize> {
    source
        .parse::<usize>()
        .ok()
        .filter(|dimension| *dimension > 0)
}

fn parse_generic_dim(source: &str) -> Option<GenericDim> {
    parse_matrix_dimension(source)
        .map(GenericDim::Known)
        .or_else(|| parse_generic_name(source).map(GenericDim::Var))
}

fn parse_generic_name(source: &str) -> Option<String> {
    let mut chars = source.chars();
    let first = chars.next()?;
    if !(first.is_ascii_alphabetic() || first == '_') {
        return None;
    }
    chars
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        .then(|| source.to_string())
}

fn parse_braced_generic_dim(source: &str) -> Option<GenericDim> {
    let inner = source.strip_prefix('{')?.strip_suffix('}')?;
    parse_generic_dim(inner)
}

fn split_matrix_generic_dims(source: &str) -> Option<(GenericDim, GenericDim)> {
    let rows_end = source.strip_prefix('{')?.find('}')? + 1;
    let rows = parse_braced_generic_dim(&source[..=rows_end])?;
    let columns = parse_braced_generic_dim(source[rows_end + 1..].strip_prefix('x')?)?;
    Some((rows, columns))
}

fn format_generic_dim(dim: &GenericDim) -> String {
    match dim {
        GenericDim::Known(value) => value.to_string(),
        GenericDim::Var(name) => name.clone(),
    }
}

fn vector_type_for_generic_dim(dim: GenericDim) -> Type {
    match dim {
        GenericDim::Known(2) => Type::Vec2,
        GenericDim::Known(3) => Type::Vec3,
        GenericDim::Known(4) => Type::Vec4,
        dim => Type::VecGeneric(dim),
    }
}

fn matrix_type_for_generic_dims(rows: GenericDim, columns: GenericDim) -> Type {
    match (&rows, &columns) {
        (GenericDim::Known(rows), GenericDim::Known(columns)) => Type::Mat(*rows, *columns),
        _ => Type::MatGeneric(rows, columns),
    }
}

fn matrix_type_name(rows: usize, columns: usize) -> String {
    if rows == columns {
        format!("Mat{rows}")
    } else {
        format!("Mat{rows}x{columns}")
    }
}

fn matrix_glsl_type(rows: usize, columns: usize) -> String {
    if rows == columns {
        format!("mat{rows}")
    } else {
        format!("mat{columns}x{rows}")
    }
}

fn matrix_constructor_type(rows: usize, columns: usize) -> String {
    if rows == columns {
        format!("mat{rows}")
    } else {
        format!("mat{rows}x{columns}")
    }
}

fn has_category(ty: &Type, category: AlgebraicCategory) -> bool {
    if category == AlgebraicCategory::Set {
        return !matches!(ty, Type::Object | Type::Object2D | Type::Func(_, _));
    }
    if matches!(ty, Type::Generic(_)) {
        return true;
    }
    if matches!(ty, Type::VecGeneric(_)) {
        return matches!(
            category,
            AlgebraicCategory::VectR
                | AlgebraicCategory::Ab
                | AlgebraicCategory::Mon
                | AlgebraicCategory::Grp
        );
    }
    if let Type::MatGeneric(rows, columns) = ty {
        return category == AlgebraicCategory::VectR
            || (unify_symbolic_dims(rows, columns, &mut GenericSubstitution::default())
                && (category == AlgebraicCategory::Ring
                    || category_implies(AlgebraicCategory::Ring, category)));
    }
    if let Type::Mat(rows, columns) = ty {
        return category == AlgebraicCategory::VectR
            || (rows == columns
                && (category == AlgebraicCategory::Ring
                    || category_implies(AlgebraicCategory::Ring, category)));
    }
    if let Type::Custom { categories, .. } = ty {
        return categories
            .iter()
            .any(|candidate| *candidate == category || category_implies(*candidate, category));
    }
    let Some(def) = BUILTIN_TYPE_DEFS.iter().find(|def| &def.ty == ty) else {
        return false;
    };
    def.categories
        .iter()
        .any(|candidate| *candidate == category || category_implies(*candidate, category))
}

fn category_implies(source: AlgebraicCategory, target: AlgebraicCategory) -> bool {
    matches!(
        (source, target),
        (AlgebraicCategory::RAlg, AlgebraicCategory::Ring)
            | (AlgebraicCategory::RAlg, AlgebraicCategory::VectR)
            | (AlgebraicCategory::RAlg, AlgebraicCategory::Ab)
            | (AlgebraicCategory::RAlg, AlgebraicCategory::Mon)
            | (AlgebraicCategory::Ring, AlgebraicCategory::Ab)
            | (AlgebraicCategory::Ring, AlgebraicCategory::Mon)
            | (AlgebraicCategory::DivRing, AlgebraicCategory::Grp)
            | (AlgebraicCategory::DivRing, AlgebraicCategory::Ring)
            | (AlgebraicCategory::DivRing, AlgebraicCategory::Ab)
            | (AlgebraicCategory::DivRing, AlgebraicCategory::Mon)
            | (AlgebraicCategory::Grp, AlgebraicCategory::Mon)
            | (AlgebraicCategory::VectR, AlgebraicCategory::Ab)
            | (AlgebraicCategory::Ab, AlgebraicCategory::Set)
            | (AlgebraicCategory::Mon, AlgebraicCategory::Set)
            | (AlgebraicCategory::Grp, AlgebraicCategory::Set)
            | (AlgebraicCategory::Ring, AlgebraicCategory::Set)
            | (AlgebraicCategory::DivRing, AlgebraicCategory::Set)
            | (AlgebraicCategory::VectR, AlgebraicCategory::Set)
            | (AlgebraicCategory::RAlg, AlgebraicCategory::Set)
    )
}

#[derive(Clone, Debug)]
struct InputDecl {
    name: String,
    ty: Type,
    line: usize,
}

#[derive(Clone, Debug)]
struct ProvidedTypeDecl {
    name: String,
    category: AlgebraicCategory,
}

#[derive(Clone, Debug)]
struct ProductTypeDecl {
    name: String,
    category: AlgebraicCategory,
    components: Vec<Type>,
    field_names: Vec<String>,
    eager_ops: bool,
    provided: bool,
    line: usize,
}

#[derive(Clone, Debug)]
struct FuncDecl {
    name: String,
    ty: Type,
    body: FuncBody,
    generated: bool,
    line: usize,
}

#[derive(Clone, Debug)]
enum FuncBody {
    Expr(Expr),
    RawGlsl(String),
    RawGlslClosure { params: Vec<String>, body: String },
}

#[derive(Clone, Debug)]
struct BindingDecl {
    name: String,
    ty: Type,
    expr: Expr,
    generated: bool,
    final_output: bool,
    line: usize,
}

#[derive(Clone, Debug)]
struct ValueBindingDecl {
    name: String,
    ty: Type,
    expr: Expr,
    generated: bool,
    line: usize,
}

#[derive(Clone, Debug)]
struct InferredBindingDecl {
    name: String,
    expr: Expr,
    generated: bool,
    construct: bool,
    final_output: bool,
    line: usize,
}

#[derive(Clone, Debug)]
struct OutputDecl {
    expr: Expr,
    line: usize,
}

#[derive(Clone, Debug)]
struct Program {
    ambient_dimension: ShapeDimension,
    derivative_epsilon: f64,
    gradient_epsilon: f64,
    is_module: bool,
    imports: Vec<ImportDecl>,
    product_types: Vec<ProductTypeDecl>,
    inputs: Vec<InputDecl>,
    funcs: Vec<FuncDecl>,
    value_bindings: Vec<ValueBindingDecl>,
    bindings: Vec<BindingDecl>,
    inferred_bindings: Vec<InferredBindingDecl>,
    output: Option<OutputDecl>,
}

#[derive(Clone, Debug)]
struct ImportDecl {
    path: String,
    line: usize,
}

#[derive(Clone, Debug)]
enum Decl {
    ProvidedType(ProvidedTypeDecl),
    ProductType(ProductTypeDecl),
    Input(InputDecl),
    Func(FuncDecl),
    ValueBinding(ValueBindingDecl),
    Binding(BindingDecl),
    InferredBinding(InferredBindingDecl),
}

#[derive(Clone, Debug)]
enum Expr {
    Bool(bool),
    Number(f64),
    RawString(String),
    Closure {
        params: Vec<String>,
        body: Box<Expr>,
    },
    Ident(String),
    Operator(BinOp),
    Tuple(Vec<Expr>),
    Array(Vec<Expr>),
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },
    FieldAccess {
        object: Box<Expr>,
        field: String,
    },
    Conditional {
        condition: Box<Expr>,
        then_branch: Box<Expr>,
        else_branch: Option<Box<Expr>>,
    },
    Index {
        array: Box<Expr>,
        index: Box<Expr>,
    },
    Binary {
        op: BinOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Constructor {
        name: String,
        args: ConstructorArgs,
    },
}

#[derive(Clone, Debug)]
enum ConstructorArgs {
    Named(Vec<(String, Expr)>),
    Positional(Vec<Expr>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Product,
    Compose,
}

#[derive(Clone, Debug)]
enum ValueExpr {
    Bool(bool),
    Float(f64),
    Int(i64),
    Neutral {
        kind: NeutralKind,
        ty: Type,
    },
    Var {
        name: String,
        ty: Type,
        array_len: Option<usize>,
    },
    Call {
        func: String,
        args: Vec<ValueExpr>,
        ty: Type,
    },
    MonoidPow {
        exponent: Box<ValueExpr>,
        base: Box<ValueExpr>,
        ty: Type,
    },
    BoolToNumberCast {
        value: Box<ValueExpr>,
        ty: Type,
    },
    Conditional {
        condition: Box<ValueExpr>,
        then_branch: Box<ValueExpr>,
        else_branch: Box<ValueExpr>,
        ty: Type,
    },
    ObjectGetterCall {
        object: String,
        getter: ObjectGetter,
        point: Box<ValueExpr>,
        captures: Vec<ValueExpr>,
        ty: Type,
    },
    FieldAccess {
        value: Box<ValueExpr>,
        field: String,
        ty: Type,
    },
    Array {
        element_ty: Type,
        elements: Vec<ValueExpr>,
    },
    Index {
        array: Box<ValueExpr>,
        index: Box<ValueExpr>,
        ty: Type,
    },
    Concat {
        element_ty: Type,
        left: Box<ValueExpr>,
        right: Box<ValueExpr>,
    },
    Binary {
        op: BinOp,
        left: Box<ValueExpr>,
        right: Box<ValueExpr>,
        ty: Type,
    },
    Vec2(Box<ValueExpr>, Box<ValueExpr>),
    Vec3(Box<ValueExpr>, Box<ValueExpr>, Box<ValueExpr>),
    Vec4(
        Box<ValueExpr>,
        Box<ValueExpr>,
        Box<ValueExpr>,
        Box<ValueExpr>,
    ),
    Matrix {
        columns: usize,
        rows: Vec<ValueExpr>,
    },
    MatrixBasis {
        row: usize,
        column: usize,
        ty: Type,
    },
    UnitVectorBasis {
        dimension: usize,
        index: usize,
        ty: Type,
    },
    Derivative {
        epsilon: Box<ValueExpr>,
        func: FunctionExpr,
        at: Box<ValueExpr>,
        ty: Type,
    },
    Partial {
        axis: usize,
        epsilon: Box<ValueExpr>,
        func: FunctionExpr,
        at: Box<ValueExpr>,
        ty: Type,
    },
    Gradient {
        epsilon: Box<ValueExpr>,
        func: FunctionExpr,
        at: Box<ValueExpr>,
        ty: Type,
    },
    Divergence {
        epsilon: Box<ValueExpr>,
        func: FunctionExpr,
        at: Box<ValueExpr>,
    },
}

impl ValueExpr {
    fn ty(&self) -> Type {
        match self {
            Self::Bool(_) => Type::Bool,
            Self::Float(_) => Type::Float,
            Self::Int(_) => Type::Int,
            Self::Neutral { ty, .. } => ty.clone(),
            Self::Var { ty, .. } => ty.clone(),
            Self::Call { ty, .. } => ty.clone(),
            Self::MonoidPow { ty, .. } => ty.clone(),
            Self::BoolToNumberCast { ty, .. } => ty.clone(),
            Self::Conditional { ty, .. } => ty.clone(),
            Self::ObjectGetterCall { ty, .. } => ty.clone(),
            Self::FieldAccess { ty, .. } => ty.clone(),
            Self::Array { element_ty, .. } => Type::Array(Box::new(element_ty.clone())),
            Self::Index { ty, .. } => ty.clone(),
            Self::Concat { element_ty, .. } => Type::Array(Box::new(element_ty.clone())),
            Self::Binary { ty, .. } => ty.clone(),
            Self::Vec2(_, _) => Type::Vec2,
            Self::Vec3(_, _, _) => Type::Vec3,
            Self::Vec4(_, _, _, _) => Type::Vec4,
            Self::Matrix { columns, rows } => Type::Mat(rows.len(), *columns),
            Self::MatrixBasis { ty, .. } => ty.clone(),
            Self::UnitVectorBasis { ty, .. } => ty.clone(),
            Self::Derivative { ty, .. } => ty.clone(),
            Self::Partial { ty, .. } => ty.clone(),
            Self::Gradient { ty, .. } => ty.clone(),
            Self::Divergence { .. } => Type::Float,
        }
    }

    fn array_len(&self) -> Option<usize> {
        match self {
            Self::Var { array_len, .. } => *array_len,
            Self::Array { elements, .. } => Some(elements.len()),
            Self::Concat { left, right, .. } => Some(left.array_len()? + right.array_len()?),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NeutralKind {
    Zero,
    One,
    Identity,
}

#[derive(Clone, Debug)]
struct FunctionExpr {
    input: Type,
    output: Type,
    kind: FunctionExprKind,
}

#[derive(Clone, Debug)]
enum FunctionExprKind {
    Named(String),
    Operator(BinOp),
    ObjectGetter {
        object: String,
        getter: ObjectGetter,
        captures: Vec<ValueExpr>,
    },
    Compose(Box<FunctionExpr>, Box<FunctionExpr>),
    PointwiseBinary {
        op: BinOp,
        left: PointwiseCallArg,
        right: PointwiseCallArg,
    },
    PointwiseCall {
        func: String,
        args: Vec<PointwiseCallArg>,
    },
    PointwiseConditional {
        condition: PointwiseCallArg,
        then_branch: PointwiseCallArg,
        else_branch: PointwiseCallArg,
    },
    ProductSameDomain(Vec<FunctionExpr>),
    ProductTensor(Box<FunctionExpr>, Box<FunctionExpr>),
}

#[derive(Clone, Debug)]
enum PointwiseCallArg {
    Function {
        func: Box<FunctionExpr>,
        expected: Type,
    },
    Value(Box<ValueExpr>),
}

fn apply_function_expr(func: &FunctionExpr, arg: ValueExpr) -> ValueExpr {
    match &func.kind {
        FunctionExprKind::Named(name) => ValueExpr::Call {
            func: name.clone(),
            args: vec![arg],
            ty: func.output.clone(),
        },
        FunctionExprKind::Operator(op) => {
            let (left, right) = operator_function_args(func, arg);
            ValueExpr::Binary {
                op: *op,
                left: Box::new(left),
                right: Box::new(right),
                ty: func.output.clone(),
            }
        }
        FunctionExprKind::ObjectGetter {
            object,
            getter,
            captures,
        } => ValueExpr::ObjectGetterCall {
            object: object.clone(),
            getter: *getter,
            point: Box::new(arg),
            captures: captures.clone(),
            ty: func.output.clone(),
        },
        FunctionExprKind::Compose(outer, inner) => {
            let inner_value = apply_function_expr(inner, arg);
            apply_function_expr(outer, inner_value)
        }
        FunctionExprKind::PointwiseBinary { op, left, right } => ValueExpr::Binary {
            op: *op,
            left: Box::new(apply_pointwise_call_arg(left, arg.clone())),
            right: Box::new(apply_pointwise_call_arg(right, arg)),
            ty: func.output.clone(),
        },
        FunctionExprKind::PointwiseCall { func: name, args } => ValueExpr::Call {
            func: name.clone(),
            args: args
                .iter()
                .map(|call_arg| apply_pointwise_call_arg(call_arg, arg.clone()))
                .collect(),
            ty: func.output.clone(),
        },
        FunctionExprKind::PointwiseConditional {
            condition,
            then_branch,
            else_branch,
        } => ValueExpr::Conditional {
            condition: Box::new(apply_pointwise_call_arg(condition, arg.clone())),
            then_branch: Box::new(apply_pointwise_call_arg(then_branch, arg.clone())),
            else_branch: Box::new(apply_pointwise_call_arg(else_branch, arg)),
            ty: func.output.clone(),
        },
        FunctionExprKind::ProductSameDomain(funcs) => product_value(
            funcs
                .iter()
                .map(|func| apply_function_expr(func, arg.clone()))
                .collect(),
        ),
        FunctionExprKind::ProductTensor(left, right) => {
            let left_arg = ValueExpr::Index {
                array: Box::new(arg.clone()),
                index: Box::new(ValueExpr::Int(0)),
                ty: left.input.clone(),
            };
            let right_arg = ValueExpr::Index {
                array: Box::new(arg),
                index: Box::new(ValueExpr::Int(1)),
                ty: right.input.clone(),
            };
            product_value(vec![
                apply_function_expr(left, left_arg),
                apply_function_expr(right, right_arg),
            ])
        }
    }
}

fn operator_function_args(func: &FunctionExpr, arg: ValueExpr) -> (ValueExpr, ValueExpr) {
    match &func.input {
        Type::Product(parts) if parts.len() == 2 => (
            ValueExpr::Var {
                name: "_t0".to_string(),
                ty: parts[0].clone(),
                array_len: None,
            },
            ValueExpr::Var {
                name: "_t1".to_string(),
                ty: parts[1].clone(),
                array_len: None,
            },
        ),
        Type::Vec2 if func.output == Type::Float || func.output == Type::Bool => (
            ValueExpr::FieldAccess {
                value: Box::new(arg.clone()),
                field: "x".to_string(),
                ty: Type::Float,
            },
            ValueExpr::FieldAccess {
                value: Box::new(arg),
                field: "y".to_string(),
                ty: Type::Float,
            },
        ),
        _ => unreachable!("operator function expects a binary domain"),
    }
}

fn apply_pointwise_call_arg(call_arg: &PointwiseCallArg, arg: ValueExpr) -> ValueExpr {
    match call_arg {
        PointwiseCallArg::Function {
            func,
            expected: expected_ty,
        } => cast_value_for_expected_type(apply_function_expr(func, arg), expected_ty),
        PointwiseCallArg::Value(value) => value.as_ref().clone(),
    }
}

fn cast_value_for_expected_type(value: ValueExpr, expected_ty: &Type) -> ValueExpr {
    if value.ty() == *expected_ty {
        return value;
    }
    if value.ty() == Type::Bool && matches!(expected_ty, Type::Float | Type::Int) {
        return ValueExpr::BoolToNumberCast {
            value: Box::new(value),
            ty: expected_ty.clone(),
        };
    }
    value
}

fn product_value(values: Vec<ValueExpr>) -> ValueExpr {
    match values.as_slice() {
        [x, y] => ValueExpr::Vec2(Box::new(x.clone()), Box::new(y.clone())),
        [x, y, z] => ValueExpr::Vec3(
            Box::new(x.clone()),
            Box::new(y.clone()),
            Box::new(z.clone()),
        ),
        [x, y, z, w] => ValueExpr::Vec4(
            Box::new(x.clone()),
            Box::new(y.clone()),
            Box::new(z.clone()),
            Box::new(w.clone()),
        ),
        _ => unreachable!("function products only support R2, R3, and R4 outputs"),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ObjectGetter {
    Sdf,
    Grad,
}

#[derive(Clone, Debug)]
enum PrimitiveArgExpr {
    Value(ValueExpr),
    Vec2List(Vec<ValueExpr>),
}

#[derive(Clone, Debug)]
enum ObjectExpr {
    Var(String),
    Primitive {
        name: String,
        kind: PrimitiveKind,
        fields: Vec<(String, PrimitiveArgExpr)>,
    },
    AmbientTransform {
        object: Box<ObjectExpr>,
        translation: ValueExpr,
        linear: ValueExpr,
    },
    IsometryTransform {
        object: Box<ObjectExpr>,
        transform: ValueExpr,
    },
    RegisteredOp {
        name: String,
        glsl_name: String,
        value_args: Vec<ValueExpr>,
        object_args: Vec<ObjectExpr>,
    },
}

#[derive(Clone, Debug)]
struct TypedFunc {
    name: String,
    input: Type,
    output: Type,
    body: TypedFuncBody,
    param_bindings: Vec<TypedFuncParamBinding>,
    raw_glsl_refs: RawGlslRefs,
    generated: bool,
    line: usize,
}

#[derive(Clone, Debug, Default)]
struct RawGlslRefs {
    funcs: BTreeSet<String>,
    values: BTreeSet<String>,
    object_getters: BTreeSet<String>,
}

#[derive(Clone, Debug)]
struct TypedFuncParamBinding {
    ty: Type,
    name: String,
    expr: String,
}

#[derive(Clone, Debug)]
enum TypedFuncBody {
    Expr(ValueExpr),
    RawGlsl(String),
    RawGlslTemplate,
}

#[derive(Clone, Debug)]
struct TypedBinding {
    name: String,
    expr: ObjectExpr,
    generated: bool,
    dimension: Option<ShapeDimension>,
    line: usize,
}

#[derive(Clone, Debug)]
struct TypedValueBinding {
    name: String,
    ty: Type,
    expr: ValueExpr,
    generated: bool,
}

#[derive(Clone, Debug)]
struct TypedProgram {
    ambient_dimension: ShapeDimension,
    gradient_epsilon: f64,
    product_types: Vec<ProductTypeDecl>,
    inputs: Vec<InputDecl>,
    funcs: Vec<TypedFunc>,
    value_bindings: Vec<TypedValueBinding>,
    bindings: Vec<TypedBinding>,
    output: Option<ObjectExpr>,
}

#[derive(Clone, Debug)]
struct EmitLocals {
    point: String,
    func_param: String,
    eps: String,
}

#[derive(Clone, Debug)]
struct PrimitiveDef {
    kind: PrimitiveKind,
    fields: Vec<PrimitiveFieldDef>,
    support_glsl: &'static str,
}

#[derive(Clone, Debug)]
struct PrimitiveFieldDef {
    name: &'static str,
    kind: PrimitiveFieldKind,
}

#[derive(Clone, Debug)]
enum PrimitiveFieldKind {
    Value(Type),
    Vec2List,
}

#[derive(Clone, Debug)]
enum PrimitiveKind {
    ParamStruct(&'static str),
    Polygon2D,
}

#[derive(Clone, Debug)]
struct ObjectOpDef {
    name: &'static str,
    value_arg_types: Vec<Type>,
    object_arg_count: usize,
    associative_binary: bool,
    glsl_name: &'static str,
    support_glsl: &'static str,
}

#[derive(Clone, Debug)]
struct ValueFuncDef {
    ty: Type,
    support_glsl: Option<&'static str>,
    listed: bool,
}

#[derive(Clone, Debug)]
struct Registry {
    primitives: HashMap<&'static str, PrimitiveDef>,
    object_ops: HashMap<&'static str, ObjectOpDef>,
    value_funcs: HashMap<&'static str, ValueFuncDef>,
}

fn object_op_type(op: &ObjectOpDef) -> Type {
    let object_domain = if op.object_arg_count == 1 {
        object_op_arg_type(op, 0)
    } else {
        Type::Product(
            (0..op.object_arg_count)
                .map(|index| object_op_arg_type(op, index))
                .collect(),
        )
    };
    let output = Type::func(object_domain, Type::Object);
    match op.value_arg_types.as_slice() {
        [] => output,
        [value_arg] => Type::func(value_arg.clone(), output),
        value_args => Type::func(Type::Product(value_args.to_vec()), output),
    }
}

fn object_op_arg_type(op: &ObjectOpDef, _index: usize) -> Type {
    if op.name == "revolution" {
        Type::Object2D
    } else {
        Type::Object
    }
}

fn ensure_type(actual: &Type, expected: &Type, context: &str) -> Result<(), Error> {
    if types_match(actual, expected) {
        return Ok(());
    }
    if let (Type::Array(actual), Type::Array(expected)) = (actual, expected) {
        return ensure_type(actual, expected, context);
    }
    if matches!(
        (actual, expected),
        (Type::Vec2, Type::Complex)
            | (Type::Complex, Type::Vec2)
            | (Type::Vec4, Type::Quat)
            | (Type::Quat, Type::Vec4)
    ) {
        return Ok(());
    }
    if matches!(
        (actual, expected),
        (Type::Custom { name: actual, .. }, Type::Custom { name: expected, .. })
            if actual == expected
    ) {
        return Ok(());
    }
    Err(Error::new(format!(
        "{} expected {}, got {}",
        context,
        format_type(expected),
        format_type(actual)
    )))
}

fn types_match(actual: &Type, expected: &Type) -> bool {
    unify_types(actual, expected, &mut GenericSubstitution::default())
}

#[derive(Clone, Debug, Default)]
struct GenericSubstitution {
    types: HashMap<String, Type>,
    dims: HashMap<String, usize>,
}

fn unify_types(left: &Type, right: &Type, substitutions: &mut GenericSubstitution) -> bool {
    if left == right {
        return true;
    }
    match (left, right) {
        (Type::Generic(name), other) | (other, Type::Generic(name)) => {
            unify_type_var(name, other, substitutions)
        }
        (Type::VecGeneric(dim), other) | (other, Type::VecGeneric(dim)) => {
            vector_type_dimension(other)
                .is_some_and(|dimension| unify_generic_dim(dim, dimension, substitutions))
        }
        (Type::MatGeneric(rows, columns), Type::Mat(actual_rows, actual_columns))
        | (Type::Mat(actual_rows, actual_columns), Type::MatGeneric(rows, columns)) => {
            unify_generic_dim(rows, *actual_rows, substitutions)
                && unify_generic_dim(columns, *actual_columns, substitutions)
        }
        (
            Type::MatGeneric(left_rows, left_columns),
            Type::MatGeneric(right_rows, right_columns),
        ) => {
            unify_symbolic_dims(left_rows, right_rows, substitutions)
                && unify_symbolic_dims(left_columns, right_columns, substitutions)
        }
        (Type::Array(left), Type::Array(right)) => unify_types(left, right, substitutions),
        (Type::Product(left), Type::Product(right)) if left.len() == right.len() => left
            .iter()
            .zip(right.iter())
            .all(|(left, right)| unify_types(left, right, substitutions)),
        (Type::Func(left_input, left_output), Type::Func(right_input, right_output)) => {
            unify_types(left_input, right_input, substitutions)
                && unify_types(left_output, right_output, substitutions)
        }
        _ => false,
    }
}

fn unify_type_var(name: &str, ty: &Type, substitutions: &mut GenericSubstitution) -> bool {
    if let Some(bound) = substitutions.types.get(name).cloned() {
        return unify_types(&bound, ty, substitutions);
    }
    substitutions.types.insert(name.to_string(), ty.clone());
    true
}

fn unify_symbolic_dims(
    left: &GenericDim,
    right: &GenericDim,
    substitutions: &mut GenericSubstitution,
) -> bool {
    match (left, right) {
        (GenericDim::Known(left), GenericDim::Known(right)) => left == right,
        (GenericDim::Var(name), GenericDim::Known(value))
        | (GenericDim::Known(value), GenericDim::Var(name)) => {
            unify_generic_dim_var(name, *value, substitutions)
        }
        (GenericDim::Var(_), GenericDim::Var(_)) => true,
    }
}

fn unify_generic_dim(
    dim: &GenericDim,
    concrete: usize,
    substitutions: &mut GenericSubstitution,
) -> bool {
    match dim {
        GenericDim::Known(value) => *value == concrete,
        GenericDim::Var(name) => unify_generic_dim_var(name, concrete, substitutions),
    }
}

fn unify_generic_dim_var(
    name: &str,
    concrete: usize,
    substitutions: &mut GenericSubstitution,
) -> bool {
    match substitutions.dims.get(name) {
        Some(bound) => *bound == concrete,
        None => {
            substitutions.dims.insert(name.to_string(), concrete);
            true
        }
    }
}

fn vector_type_dimension(ty: &Type) -> Option<usize> {
    match ty {
        Type::Vec2 => Some(2),
        Type::Vec3 => Some(3),
        Type::Vec4 => Some(4),
        Type::VecGeneric(GenericDim::Known(value)) => Some(*value),
        _ => None,
    }
}

fn substitute_type(ty: &Type, substitutions: &GenericSubstitution) -> Type {
    match ty {
        Type::Generic(name) => substitutions
            .types
            .get(name)
            .cloned()
            .unwrap_or_else(|| ty.clone()),
        Type::VecGeneric(dim) => match substitute_dim(dim, substitutions) {
            GenericDim::Known(2) => Type::Vec2,
            GenericDim::Known(3) => Type::Vec3,
            GenericDim::Known(4) => Type::Vec4,
            resolved => Type::VecGeneric(resolved),
        },
        Type::MatGeneric(rows, columns) => {
            let rows = substitute_dim(rows, substitutions);
            let columns = substitute_dim(columns, substitutions);
            match (&rows, &columns) {
                (GenericDim::Known(rows), GenericDim::Known(columns)) => Type::Mat(*rows, *columns),
                _ => Type::MatGeneric(rows, columns),
            }
        }
        Type::Array(element) => Type::Array(Box::new(substitute_type(element, substitutions))),
        Type::Product(parts) => Type::Product(
            parts
                .iter()
                .map(|part| substitute_type(part, substitutions))
                .collect(),
        ),
        Type::Func(input, output) => Type::func(
            substitute_type(input, substitutions),
            substitute_type(output, substitutions),
        ),
        _ => ty.clone(),
    }
}

fn substitute_dim(dim: &GenericDim, substitutions: &GenericSubstitution) -> GenericDim {
    match dim {
        GenericDim::Known(_) => dim.clone(),
        GenericDim::Var(name) => substitutions
            .dims
            .get(name)
            .copied()
            .map(GenericDim::Known)
            .unwrap_or_else(|| dim.clone()),
    }
}

fn format_type(ty: &Type) -> String {
    match ty {
        Type::Array(element) => format!("Array({})", format_type(element)),
        Type::Product(parts) => parts
            .iter()
            .map(format_type)
            .collect::<Vec<_>>()
            .join(" × "),
        Type::Func(_, _) => {
            let (inputs, output) = flatten_func_type(ty);
            let domain = if inputs.len() == 1 {
                format_type(inputs[0])
            } else {
                format_type(&Type::Product(inputs.into_iter().cloned().collect()))
            };
            format!("Func({}, {})", domain, format_type(output))
        }
        _ => ty.type_name().to_string(),
    }
}

fn format_object_type(ty: &Type) -> String {
    match ty {
        Type::Product(parts) => parts
            .iter()
            .map(format_object_type)
            .collect::<Vec<_>>()
            .join(" × "),
        Type::Func(input, output) => {
            format!(
                "Hom({}, {})",
                format_object_type(input),
                format_object_type(output)
            )
        }
        _ => ty.type_name().to_string(),
    }
}

fn format_overload_set(overloads: &[Type]) -> String {
    compact_overload_set(overloads).unwrap_or_else(|| {
        overloads
            .iter()
            .map(format_object_type)
            .collect::<Vec<_>>()
            .join(" | ")
    })
}

fn compact_overload_set(overloads: &[Type]) -> Option<String> {
    let mut remaining = overloads.to_vec();
    let mut parts = Vec::new();

    take_pattern(
        &mut remaining,
        &transpose_matrix_overloads(),
        "Hom(Mat{n}x{m}, Mat{m}x{n})",
        &mut parts,
    );
    take_pattern(
        &mut remaining,
        &same_matrix_overloads(),
        "Hom(Mat{n}x{m} × Mat{n}x{m}, Mat{n}x{m})",
        &mut parts,
    );
    take_pattern(
        &mut remaining,
        &square_matrix_to_float_overloads(),
        "Hom(Mat{n}x{n}, R)",
        &mut parts,
    );
    take_pattern(
        &mut remaining,
        &square_matrix_overloads(),
        "Hom(Mat{n}x{n}, Mat{n}x{n})",
        &mut parts,
    );

    for arity in [3, 2] {
        let label = compact_same_type_label(arity);
        take_pattern(
            &mut remaining,
            &same_type_float_gen_overloads(arity),
            &label,
            &mut parts,
        );

        let label = compact_vector_scalar_last_label(arity);
        take_pattern(
            &mut remaining,
            &vector_scalar_last_overloads(arity),
            &label,
            &mut parts,
        );
        if arity > 2 {
            let label = compact_vectors_then_scalar_label(arity);
            take_pattern(
                &mut remaining,
                &vectors_then_scalar_overloads(arity),
                &label,
                &mut parts,
            );
        }

        let label = compact_scalar_vector_first_label(arity);
        take_pattern(
            &mut remaining,
            &scalar_vector_first_overloads(arity),
            &label,
            &mut parts,
        );
        if arity > 2 {
            let label = compact_scalars_then_vector_label(arity);
            take_pattern(
                &mut remaining,
                &scalars_then_vector_overloads(arity),
                &label,
                &mut parts,
            );
        }
    }

    take_pattern(
        &mut remaining,
        &unary_float_gen_type_overloads(),
        "Hom(Rn, Rn)",
        &mut parts,
    );
    take_pattern(
        &mut remaining,
        &vector_measure_overloads(2),
        "Hom(Rn × Rn, R)",
        &mut parts,
    );
    take_pattern(
        &mut remaining,
        &vector_measure_overloads(1),
        "Hom(Rn, R)",
        &mut parts,
    );

    for ty in remaining {
        parts.push(format_object_type(&ty));
    }

    (!parts.is_empty()).then(|| parts.join(" | "))
}

fn take_pattern(remaining: &mut Vec<Type>, pattern: &[Type], label: &str, parts: &mut Vec<String>) {
    if pattern.is_empty() || !contains_pattern(remaining, pattern) {
        return;
    }
    for ty in pattern {
        let index = remaining
            .iter()
            .position(|candidate| candidate == ty)
            .unwrap();
        remaining.remove(index);
    }
    parts.push(label.to_string());
}

fn contains_pattern(remaining: &[Type], pattern: &[Type]) -> bool {
    let mut scratch = remaining.to_vec();
    for ty in pattern {
        let Some(index) = scratch.iter().position(|candidate| candidate == ty) else {
            return false;
        };
        scratch.remove(index);
    }
    true
}

fn compact_same_type_label(arity: usize) -> String {
    format!("Hom({}, Rn)", vec!["Rn"; arity].join(" × "))
}

fn compact_vector_scalar_last_label(arity: usize) -> String {
    let mut args = vec!["Rn"];
    args.extend(vec!["R"; arity - 1]);
    format!("Hom({}, Rn)", args.join(" × "))
}

fn compact_vectors_then_scalar_label(arity: usize) -> String {
    let mut args = vec!["Rn"; arity - 1];
    args.push("R");
    format!("Hom({}, Rn)", args.join(" × "))
}

fn compact_scalar_vector_first_label(arity: usize) -> String {
    let mut args = vec!["R"];
    args.extend(vec!["Rn"; arity - 1]);
    format!("Hom({}, Rn)", args.join(" × "))
}

fn compact_scalars_then_vector_label(arity: usize) -> String {
    let mut args = vec!["R"; arity - 1];
    args.push("Rn");
    format!("Hom({}, Rn)", args.join(" × "))
}

fn flatten_func_type<'a>(ty: &'a Type) -> (Vec<&'a Type>, &'a Type) {
    let mut inputs = Vec::new();
    let mut current = ty;
    while let Type::Func(input, output) = current {
        inputs.push(input.as_ref());
        current = output.as_ref();
    }
    (inputs, current)
}

fn flatten_call<'a>(expr: &'a Expr) -> Result<(String, Vec<&'a Expr>), Error> {
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
            Expr::Ident(name) => {
                args.reverse();
                return Ok((name.clone(), args));
            }
            Expr::Constructor {
                name,
                args: constructor_args,
            } => match constructor_args {
                ConstructorArgs::Positional(constructor_args) => {
                    for arg in constructor_args.iter().rev() {
                        args.push(arg);
                    }
                    args.reverse();
                    return Ok((name.clone(), args));
                }
                ConstructorArgs::Named(_) => {
                    return Err(Error::new("unsupported callable object expression"))
                }
            },
            Expr::Index { .. } | Expr::Array(_) => {
                return Err(Error::new("unsupported callable object expression"))
            }
            _ => return Err(Error::new("unsupported callable object expression")),
        }
    }
}

fn listed_builtin_value_func_overload_types(name: &str) -> Option<Vec<Type>> {
    let mut overloads = glsl_builtin_value_func_overload_types(name).unwrap_or_default();
    if COMPLEX_OVERLOAD_NAMES.contains(&name) {
        overloads.push(Type::func(Type::Complex, Type::Complex));
    }
    (!overloads.is_empty()).then_some(overloads)
}

fn listed_builtin_value_func_overloads(name: &str) -> Option<String> {
    if name == "pow" {
        return Some(format!(
            "Hom(Z × Mon, Mon) | {}",
            format_overload_set(&listed_builtin_value_func_overload_types(name)?)
        ));
    }
    listed_builtin_value_func_overload_types(name).map(|overloads| format_overload_set(&overloads))
}

fn glsl_builtin_value_func_overload_types(name: &str) -> Option<Vec<Type>> {
    let mut result = Vec::new();
    for (candidate, overloads) in glsl_builtin_value_func_overloads() {
        if candidate == name {
            result.extend(overloads);
        }
    }
    (!result.is_empty()).then_some(result)
}

fn glsl_builtin_value_func_overloads() -> Vec<(&'static str, Vec<Type>)> {
    let mut funcs = Vec::new();

    for name in [
        "radians",
        "degrees",
        "sin",
        "cos",
        "tan",
        "asin",
        "acos",
        "sinh",
        "cosh",
        "tanh",
        "asinh",
        "acosh",
        "atanh",
        "exp",
        "log",
        "exp2",
        "log2",
        "sqrt",
        "inversesqrt",
        "abs",
        "sign",
        "floor",
        "trunc",
        "round",
        "roundEven",
        "ceil",
        "fract",
        "normalize",
        "dFdx",
        "dFdy",
        "fwidth",
    ] {
        funcs.push((name, unary_float_gen_type_overloads()));
    }

    funcs.extend([
        ("atan", same_type_float_gen_overloads(2)),
        ("atan", unary_float_gen_type_overloads()),
        ("pow", same_type_float_gen_overloads(2)),
        (
            "pow",
            vec![func_type(vec![Type::Complex, Type::Complex], Type::Complex)],
        ),
        ("mod", same_or_scalar_float_gen_overloads(2)),
        ("min", same_or_scalar_symmetric_float_gen_overloads()),
        ("max", same_or_scalar_symmetric_float_gen_overloads()),
        ("clamp", same_or_scalar_float_gen_overloads(3)),
        ("mix", same_prefix_scalar_last_float_gen_overloads()),
        ("step", scalar_or_same_first_float_gen_overloads()),
        ("smoothstep", scalar_or_same_prefix_float_gen_overloads()),
        ("fma", same_type_float_gen_overloads(3)),
        ("length", vector_measure_overloads(1)),
        ("distance", vector_measure_overloads(2)),
        ("dot", vector_measure_overloads(2)),
        (
            "cross",
            vec![func_type(vec![Type::Vec3, Type::Vec3], Type::Vec3)],
        ),
        ("faceforward", same_type_float_gen_overloads(3)),
        ("reflect", same_type_float_gen_overloads(2)),
        ("refract", gen_type_with_scalar_last_overloads(3)),
        ("matrixCompMult", generic_same_matrix_overloads()),
        ("transpose", generic_transpose_matrix_overloads()),
        ("determinant", generic_square_matrix_to_float_overloads()),
        ("inverse", generic_square_matrix_overloads()),
    ]);

    funcs.extend([
        ("abs", vec![Type::func(Type::Int, Type::Int)]),
        ("sign", vec![Type::func(Type::Int, Type::Int)]),
        (
            "min",
            vec![func_type(vec![Type::Int, Type::Int], Type::Int)],
        ),
        (
            "max",
            vec![func_type(vec![Type::Int, Type::Int], Type::Int)],
        ),
        (
            "clamp",
            vec![func_type(vec![Type::Int, Type::Int, Type::Int], Type::Int)],
        ),
    ]);

    funcs
}

fn unary_float_gen_type_overloads() -> Vec<Type> {
    float_gen_types()
        .into_iter()
        .map(|ty| Type::func(ty.clone(), ty))
        .collect()
}

fn generic_dim_var(name: &str) -> GenericDim {
    GenericDim::Var(name.to_string())
}

fn generic_matrix_type(rows: &str, columns: &str) -> Type {
    Type::MatGeneric(generic_dim_var(rows), generic_dim_var(columns))
}

fn generic_square_matrix_type(dim: &str) -> Type {
    let dim = generic_dim_var(dim);
    Type::MatGeneric(dim.clone(), dim)
}

fn generic_transpose_matrix_overloads() -> Vec<Type> {
    vec![Type::func(
        generic_matrix_type("n", "m"),
        generic_matrix_type("m", "n"),
    )]
}

fn generic_same_matrix_overloads() -> Vec<Type> {
    let matrix = generic_matrix_type("n", "m");
    vec![func_type(vec![matrix.clone(), matrix.clone()], matrix)]
}

fn generic_square_matrix_to_float_overloads() -> Vec<Type> {
    vec![Type::func(generic_square_matrix_type("n"), Type::Float)]
}

fn generic_square_matrix_overloads() -> Vec<Type> {
    let matrix = generic_square_matrix_type("n");
    vec![Type::func(matrix.clone(), matrix)]
}

fn same_type_float_gen_overloads(arity: usize) -> Vec<Type> {
    float_gen_types()
        .into_iter()
        .map(|ty| func_type(vec![ty.clone(); arity], ty))
        .collect()
}

fn same_or_scalar_float_gen_overloads(arity: usize) -> Vec<Type> {
    let mut overloads = same_type_float_gen_overloads(arity);
    overloads.extend(vector_scalar_last_overloads(arity));
    overloads
}

fn same_or_scalar_symmetric_float_gen_overloads() -> Vec<Type> {
    let mut overloads = same_or_scalar_float_gen_overloads(2);
    overloads.extend(scalar_vector_first_overloads(2));
    overloads
}

fn vector_scalar_last_overloads(arity: usize) -> Vec<Type> {
    vector_types()
        .into_iter()
        .map(|ty| {
            let mut args = vec![ty.clone()];
            args.extend(vec![Type::Float; arity - 1]);
            func_type(args, ty)
        })
        .collect()
}

fn scalar_vector_first_overloads(arity: usize) -> Vec<Type> {
    vector_types()
        .into_iter()
        .map(|ty| {
            let mut args = vec![Type::Float];
            args.extend(vec![ty.clone(); arity - 1]);
            func_type(args, ty)
        })
        .collect()
}

fn vectors_then_scalar_overloads(arity: usize) -> Vec<Type> {
    vector_types()
        .into_iter()
        .map(|ty| {
            let mut args = vec![ty.clone(); arity - 1];
            args.push(Type::Float);
            func_type(args, ty)
        })
        .collect()
}

fn scalars_then_vector_overloads(arity: usize) -> Vec<Type> {
    vector_types()
        .into_iter()
        .map(|ty| {
            let mut args = vec![Type::Float; arity - 1];
            args.push(ty.clone());
            func_type(args, ty)
        })
        .collect()
}

fn scalar_or_same_first_float_gen_overloads() -> Vec<Type> {
    let mut overloads = same_type_float_gen_overloads(2);
    for ty in vector_types() {
        overloads.push(func_type(vec![Type::Float, ty.clone()], ty));
    }
    overloads
}

fn scalar_or_same_prefix_float_gen_overloads() -> Vec<Type> {
    let mut overloads = same_type_float_gen_overloads(3);
    for ty in vector_types() {
        overloads.push(func_type(vec![Type::Float, Type::Float, ty.clone()], ty));
    }
    overloads
}

fn same_prefix_scalar_last_float_gen_overloads() -> Vec<Type> {
    let mut overloads = same_type_float_gen_overloads(3);
    for ty in vector_types() {
        overloads.push(func_type(vec![ty.clone(), ty.clone(), Type::Float], ty));
    }
    overloads
}

fn gen_type_with_scalar_last_overloads(arity: usize) -> Vec<Type> {
    float_gen_types()
        .into_iter()
        .map(|ty| {
            let mut args = vec![ty.clone(); arity - 1];
            args.push(Type::Float);
            func_type(args, ty)
        })
        .collect()
}

fn vector_measure_overloads(arity: usize) -> Vec<Type> {
    float_gen_types()
        .into_iter()
        .map(|ty| func_type(vec![ty; arity], Type::Float))
        .collect()
}

fn same_matrix_overloads() -> Vec<Type> {
    matrix_types()
        .into_iter()
        .map(|ty| func_type(vec![ty.clone(), ty.clone()], ty))
        .collect()
}

fn transpose_matrix_overloads() -> Vec<Type> {
    matrix_shapes()
        .into_iter()
        .map(|(rows, columns)| Type::func(Type::Mat(rows, columns), Type::Mat(columns, rows)))
        .collect()
}

fn square_matrix_to_float_overloads() -> Vec<Type> {
    square_matrix_types()
        .into_iter()
        .map(|ty| Type::func(ty, Type::Float))
        .collect()
}

fn square_matrix_overloads() -> Vec<Type> {
    square_matrix_types()
        .into_iter()
        .map(|ty| Type::func(ty.clone(), ty))
        .collect()
}

fn func_type(inputs: Vec<Type>, output: Type) -> Type {
    let input = match inputs.as_slice() {
        [single] => single.clone(),
        _ => Type::Product(inputs),
    };
    Type::func(input, output)
}

fn float_gen_types() -> Vec<Type> {
    let mut types = vec![Type::Float];
    types.extend(vector_types());
    types
}

fn vector_types() -> Vec<Type> {
    vec![Type::Vec2, Type::Vec3, Type::Vec4]
}

fn matrix_types() -> Vec<Type> {
    matrix_shapes()
        .into_iter()
        .map(|(rows, columns)| Type::Mat(rows, columns))
        .collect()
}

fn square_matrix_types() -> Vec<Type> {
    (2..=4)
        .map(|dimension| Type::Mat(dimension, dimension))
        .collect()
}

fn matrix_shapes() -> Vec<(usize, usize)> {
    let mut shapes = Vec::new();
    for rows in 2..=4 {
        for columns in 2..=4 {
            shapes.push((rows, columns));
        }
    }
    shapes
}

impl BinOp {
    fn symbol(self) -> &'static str {
        match self {
            Self::Add => "+",
            Self::Sub => "-",
            Self::Mul => "*",
            Self::Div => "/",
            Self::Eq => "==",
            Self::Ne => "!=",
            Self::Lt => "<",
            Self::Le => "<=",
            Self::Gt => ">",
            Self::Ge => ">=",
            Self::Product => "x",
            Self::Compose => "@",
        }
    }
}
