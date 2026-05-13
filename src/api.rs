//! Exposes the public API used by the CLI, REPL, LSP, tests, and embedding callers.
//! This layer is separated from the compiler passes so external entrypoints can compile, format, inspect, list, and preview Lane programs without knowing pass internals.
//! It orchestrates the full processing pipeline from source loading through parse, module loading, preprocessing, semantic analysis, postprocessing, and emission.

use std::fs;
use std::path::{Path, PathBuf};

use super::*;

/// Compiles source text as Lane using the current working directory as import base.
pub fn compile_program(source: &str) -> Result<String, Error> {
    compile_program_with_base_dir(source, current_directory())
}

/// Reads a source file path from disk and compiles the contained Lane source.
pub fn compile_program_from_path(path: impl AsRef<Path>) -> Result<String, Error> {
    let (source, base_dir) = load_source_file(path.as_ref())?;
    compile_program_with_base_dir(&source, base_dir)
}

/// Compiles Lane source text using an explicit import base directory.
pub fn compile_program_with_base_dir(
    source: &str,
    base_dir: impl AsRef<Path>,
) -> Result<String, Error> {
    compile_program_with_base(source, base_dir.as_ref())
}

fn current_directory() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn load_source_file(path: &Path) -> Result<(String, PathBuf), Error> {
    let source = fs::read_to_string(path).map_err(|err| Error::new(err.to_string()))?;
    let base_dir = path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    Ok((source, base_dir))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProgramInfo {
    pub loaded_modules: Vec<String>,
    pub directives: Vec<String>,
    pub provided_objects: Vec<String>,
}

/// Collects module and directive metadata for a source snippet.
pub fn program_info(source: &str) -> Result<ProgramInfo, Error> {
    program_info_with_base_dir(source, current_directory())
}

/// Collects metadata for a source snippet using an explicit base path.
pub fn program_info_with_base_dir(
    source: &str,
    base_dir: impl AsRef<Path>,
) -> Result<ProgramInfo, Error> {
    let program = ModuleLoader::new(base_dir.as_ref()).load_main(source)?;
    Ok(ProgramInfo {
        loaded_modules: source_directive_values(source, "#import"),
        directives: source_info_directives(source),
        provided_objects: program
            .inputs
            .iter()
            .map(|input| format!("{} {}", format_type(&input.ty), input.name))
            .collect(),
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaneDiagnostic {
    pub line: usize,
    pub message: String,
}

/// Runs parse/typecheck and returns diagnostics collected from one validation pass.
pub fn lane_diagnostics_with_base_dir(
    source: &str,
    base_dir: impl AsRef<Path>,
) -> Vec<LaneDiagnostic> {
    match check_diagnostic_document(source, base_dir.as_ref()) {
        Ok(_) => Vec::new(),
        Err(error) => vec![LaneDiagnostic {
            line: error.line().unwrap_or(1),
            message: error.to_string(),
        }],
    }
}

/// Performs `check_diagnostic_document` behavior.
fn check_diagnostic_document(source: &str, base_dir: &Path) -> Result<(), Error> {
    let registry = Registry::default();
    let program = ModuleLoader::new(base_dir).load_document(source)?;
    TypedProgram::from_program(&program, &registry)?;
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaneCompletionKind {
    Keyword,
    Module,
    Constructor,
    Function,
    Type,
    Category,
    Constant,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaneCompletionItem {
    pub label: String,
    pub kind: LaneCompletionKind,
    pub detail: Option<String>,
    pub documentation: Option<String>,
}

impl LaneCompletionItem {
    /// Performs `new` behavior.
    fn new(label: impl Into<String>, kind: LaneCompletionKind) -> Self {
        Self {
            label: label.into(),
            kind,
            detail: None,
            documentation: None,
        }
    }

    /// Performs `with_detail` behavior.
    fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Performs `with_documentation` behavior.
    fn with_documentation(mut self, documentation: impl Into<String>) -> Self {
        self.documentation = Some(documentation.into());
        self
    }
}

/// Builds the shared completion catalogue used by CLI and LSP completion endpoints.
pub fn lane_completion_items() -> Vec<LaneCompletionItem> {
    let mut items = Vec::new();
    for (label, detail) in [
        (
            "const",
            "Emit a Lane binding even when only referenced by generated code",
        ),
        ("provided", "Declare a host-provided shader input"),
        ("Hom", "Function type constructor"),
        ("Func", "Function type constructor alias"),
        ("Object", "Current ambient SDF object type"),
        ("Object2D", "2D SDF object type"),
        ("Object3D", "3D SDF object type"),
        ("Type", "Type metatype"),
        ("Cat", "Category metatype"),
        ("#import", "Import a Lane module"),
        ("#prec", "Set default differential precision"),
        ("#2D", "Switch the program to 2D SDF mode"),
    ] {
        items.push(LaneCompletionItem::new(label, LaneCompletionKind::Keyword).with_detail(detail));
    }
    for module in ["std", "raytracing"] {
        items.push(
            LaneCompletionItem::new(module, LaneCompletionKind::Module)
                .with_detail("built-in Lane module"),
        );
    }
    for (label, detail, kind) in [
        (
            "Mat{n}x{m}",
            "generic real matrix type",
            LaneCompletionKind::Type,
        ),
        (
            "Mat{n}",
            "generic square real matrix type",
            LaneCompletionKind::Type,
        ),
        (
            "E{n}{m}",
            "generic matrix basis element",
            LaneCompletionKind::Constant,
        ),
    ] {
        items.push(LaneCompletionItem::new(label, kind).with_detail(detail));
    }
    for primitive in known_primitives() {
        let fields = primitive
            .fields
            .iter()
            .map(|field| format!("{}: {}", field.name, field.domain))
            .collect::<Vec<_>>()
            .join(", ");
        items.push(
            LaneCompletionItem::new(primitive.name, LaneCompletionKind::Constructor)
                .with_detail(format!("{}({fields})", primitive.parameter_space))
                .with_documentation(format!(
                    "{} primitive constructor",
                    primitive.dimension.label()
                )),
        );
    }
    for object in known_builtin_objects() {
        let kind = match object.kind {
            KnownBuiltinObjectKind::Function => LaneCompletionKind::Function,
            KnownBuiltinObjectKind::Type => LaneCompletionKind::Type,
            KnownBuiltinObjectKind::Category => LaneCompletionKind::Category,
        };
        items.push(LaneCompletionItem::new(object.name, kind).with_detail(object.ty));
    }
    items
}

/// Returns hover text for known primitives, built-in objects, and keywords.
pub fn lane_hover_for_word(word: &str) -> Option<String> {
    if let Some(primitive) = known_primitive(word) {
        let fields = primitive
            .fields
            .iter()
            .map(|field| format!("{}: {}", field.name, field.domain))
            .collect::<Vec<_>>()
            .join(", ");
        return Some(format!(
            "{}: {}\n\n{} primitive constructor with fields: {}",
            primitive.name,
            primitive.parameter_space,
            primitive.dimension.label(),
            fields
        ));
    }
    if let Some(object) = known_builtin_object(word) {
        return Some(format!("{}: {}", object.name, object.ty));
    }
    match word {
        "const" => Some("const emits a Lane value, function, or object binding.".to_string()),
        "provided" => Some("provided declares a host-provided shader input.".to_string()),
        "Hom" | "Func" => Some(format!("{word}(A, B) is a function type from A to B.")),
        "Object" => Some("Object is the current ambient SDF object type.".to_string()),
        "Object2D" => Some("Object2D is a 2D SDF object type.".to_string()),
        "Object3D" => Some("Object3D is a 3D SDF object type.".to_string()),
        "R" => {
            Some("R is the real scalar type; R{n} denotes generic real vector spaces.".to_string())
        }
        "Mat" => {
            Some("Mat{n} denotes generic square matrices; Mat{n}x{m} denotes generic rectangular matrix types.".to_string())
        }
        "E" => Some("E{n}{m} denotes generic matrix basis elements.".to_string()),
        _ => None,
    }
}

/// Formats source with compact separator rules while preserving program structure.
pub fn format_lane_source(source: &str) -> String {
    let mut lines = Vec::new();
    let mut previous_blank = false;
    for line in source.lines() {
        let line = line.trim_end();
        let blank = line.trim().is_empty();
        if blank && previous_blank {
            continue;
        }
        lines.push(format_lane_line(line));
        previous_blank = blank;
    }
    if lines.is_empty() {
        return String::new();
    }
    let mut formatted = lines.join("\n");
    formatted.push('\n');
    formatted
}

/// Performs `format_lane_line` behavior.
fn format_lane_line(line: &str) -> String {
    let Some(equal_index) = find_top_level_equal(line) else {
        return format_declaration_type_head(line);
    };
    if is_product_type_assignment_head(&line[..equal_index]) {
        let mut formatted = String::new();
        formatted.push_str(&line[..=equal_index]);
        formatted.push_str(&format_product_type_assignment_tail(
            &line[equal_index + 1..],
        ));
        return formatted;
    }
    let mut formatted = format_declaration_type_head(&line[..equal_index]);
    formatted.push_str(&line[equal_index..]);
    formatted
}

/// Performs `format_product_type_assignment_tail` behavior.
fn format_product_type_assignment_tail(tail: &str) -> String {
    let Some(field_index) = tail.find('<') else {
        return format_type_product_separators(tail);
    };
    let mut formatted = format_type_product_separators(&tail[..field_index]);
    formatted.push_str(&tail[field_index..]);
    formatted
}

/// Performs `format_declaration_type_head` behavior.
fn format_declaration_type_head(line: &str) -> String {
    let leading_len = line.len() - line.trim_start().len();
    let (leading, body) = line.split_at(leading_len);
    if let Some(rest) = body.strip_prefix("provided ") {
        return format_prefixed_declaration_type_head(leading, "provided ", rest);
    }
    if let Some(rest) = body.strip_prefix("const ") {
        return format_prefixed_declaration_type_head(leading, "const ", rest);
    }
    format_type_before_name(line)
}

/// Performs `format_prefixed_declaration_type_head` behavior.
fn format_prefixed_declaration_type_head(leading: &str, prefix: &str, rest: &str) -> String {
    let mut formatted = String::new();
    formatted.push_str(leading);
    formatted.push_str(prefix);
    if let Some(colon_index) = find_top_level_colon(rest) {
        formatted.push_str(&rest[..=colon_index]);
        formatted.push_str(&format_type_product_separators(&rest[colon_index + 1..]));
        return formatted;
    }
    formatted.push_str(&format_type_before_name(rest));
    formatted
}

/// Performs `format_type_before_name` behavior.
fn format_type_before_name(source: &str) -> String {
    let trimmed_end_len = source.trim_end().len();
    let trailing = &source[trimmed_end_len..];
    let head = &source[..trimmed_end_len];
    let Some(name_end) = head.rfind(|ch: char| !ch.is_ascii_whitespace()) else {
        return source.to_string();
    };
    let Some(space_before_name) = head[..=name_end].rfind(|ch: char| ch.is_ascii_whitespace())
    else {
        return source.to_string();
    };
    let mut formatted = format_type_product_separators(&head[..space_before_name]);
    formatted.push_str(&head[space_before_name..]);
    formatted.push_str(trailing);
    formatted
}

/// Performs `format_type_product_separators` behavior.
fn format_type_product_separators(source: &str) -> String {
    let mut formatted = String::new();
    for (index, ch) in source.char_indices() {
        if ch == 'x' {
            let prev_space = index > 0 && source.as_bytes()[index - 1].is_ascii_whitespace();
            let next_index = index + ch.len_utf8();
            let next_space = source
                .as_bytes()
                .get(next_index)
                .is_some_and(u8::is_ascii_whitespace);
            if prev_space && next_space {
                formatted.push('×');
                continue;
            }
        }
        formatted.push(ch);
    }
    formatted
}

/// Performs `find_top_level_equal` behavior.
fn find_top_level_equal(source: &str) -> Option<usize> {
    find_top_level_delimiter(source, '=')
}

/// Performs `find_top_level_colon` behavior.
fn find_top_level_colon(source: &str) -> Option<usize> {
    find_top_level_delimiter(source, ':')
}

/// Finds a top-level delimiter outside of parenthesized/bracketed groups.
fn find_top_level_delimiter(source: &str, delimiter: char) -> Option<usize> {
    let mut depth = 0usize;
    for (index, ch) in source.char_indices() {
        match ch {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            ch if ch == delimiter && depth == 0 => return Some(index),
            _ => {}
        }
    }
    None
}

/// Performs `is_product_type_assignment_head` behavior.
fn is_product_type_assignment_head(head: &str) -> bool {
    let head = head.trim();
    let head = head
        .strip_prefix("provided ")
        .or_else(|| head.strip_prefix("const "))
        .unwrap_or(head)
        .trim();
    let Some((category, name)) = head.split_once(char::is_whitespace) else {
        return false;
    };
    if category_by_name(category).is_none() {
        return false;
    }
    let name = name.trim();
    if name.is_empty() {
        return false;
    }
    if let Some(fields) = name
        .strip_suffix('>')
        .and_then(|name| name.rsplit_once('<'))
    {
        !fields.0.trim().is_empty()
    } else {
        !name.chars().any(char::is_whitespace)
    }
}

/// Loads a file, resolves imports, and emits an OpenGL/WebGL preview fragment.
pub fn compile_preview_fragment_from_path(
    path: impl AsRef<Path>,
    version: &str,
) -> Result<String, Error> {
    let (source, base_dir) = load_source_file(path.as_ref())?;
    compile_preview_fragment(&source, &base_dir, version, PreviewShaderTarget::OpenGl)
}

/// Loads a file and compiles the Vulkan preview fragment shader.
pub fn compile_vulkan_preview_fragment_from_path(path: impl AsRef<Path>) -> Result<String, Error> {
    let (source, base_dir) = load_source_file(path.as_ref())?;
    compile_preview_fragment(&source, &base_dir, "450", PreviewShaderTarget::Vulkan)
}

/// Compiles source as a Vulkan preview fragment shader using default version 450.
pub fn compile_vulkan_preview_fragment(source: &str) -> Result<String, Error> {
    let base_dir = current_directory();
    compile_preview_fragment(source, &base_dir, "450", PreviewShaderTarget::Vulkan)
}

/// Builds the OpenGL preview vertex shader for fullscreen triangle rendering.
pub fn compile_preview_vertex(version: &str) -> String {
    format!(
        "{}\nprecision highp float;\n\nconst vec2 vertices[3] = vec2[3](\n    vec2(-1.0, -1.0),\n    vec2(3.0, -1.0),\n    vec2(-1.0, 3.0)\n);\n\nvoid main() {{\n    gl_Position = vec4(vertices[gl_VertexID], 0.0, 1.0);\n}}\n",
        glsl_version_directive(version)
    )
}

/// Builds the Vulkan preview vertex shader for fullscreen triangle rendering.
pub fn compile_vulkan_preview_vertex() -> String {
    "#version 450\n\nconst vec2 vertices[3] = vec2[3](\n    vec2(-1.0, -1.0),\n    vec2(3.0, -1.0),\n    vec2(-1.0, 3.0)\n);\n\nvoid main() {\n    gl_Position = vec4(vertices[gl_VertexIndex], 0.0, 1.0);\n}\n"
        .to_string()
}

/// Performs `compile_program_with_base` behavior.
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

/// Performs `compile_preview_fragment` behavior.
fn compile_preview_fragment(
    source: &str,
    base_dir: &Path,
    version: &str,
    target: PreviewShaderTarget,
) -> Result<String, Error> {
    let preview = preview_context(source);
    validate_preview_requirements(&preview)?;
    let source = prepare_preview_source(source, &preview);
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

/// Rewrites source for preview generation by injecting defaults only when missing.
fn prepare_preview_source(source: &str, preview: &PreviewContext) -> String {
    let mut out = String::new();
    if !source
        .lines()
        .any(|line| line.trim() == "#import raytracing")
    {
        out.push_str("#import raytracing\n");
    }
    out.push_str(source);
    if !preview.has_root_object() {
        if let Some(name) = preview.last_object_name.as_ref() {
            out.push('\n');
            out.push_str(&format!("const Object scene = {name}\n"));
        }
    }
    if !preview.has_main {
        append_preview_provided_values(preview, &mut out);
        out.push_str(
            "\nconst Hom(R2, Ray) preview_camera_ray = camera_ray(Camera(cameraPosition, cameraForward, cameraGlobalUp, resolution))\n\
const Hom(Ray, Hit) preview_hit = raytrace_with(default_raytrace_config, scene)\n\
const Hom(Hit, R3) preview_material_color = hit |-> material_color(scene_material(hit.position))\n\
const Hom(Hit, R3) preview_material_emission = hit |-> material_emission(scene_material(hit.position))\n\
const Hom(Hit, R) preview_material_reflectiveness = hit |-> material_reflectiveness(scene_material(hit.position))\n\
const Hom(Ray, R3) preview_color = raycolor_from_hit_with(default_raycolor_config, ambientColor, preview_hit, preview_material_color, preview_material_emission, preview_material_reflectiveness)\n\
const Hom(R2, R4) preview_shade = shade(preview_camera_ray, preview_color)\n\
const Hom(*, *) main = fragment_main(preview_shade)\n",
        );
    }
    out
}

/// Extracts directive tokens relevant to preview shader compilation.
fn source_info_directives(source: &str) -> Vec<String> {
    source
        .lines()
        .filter_map(|line| {
            let line = strip_source_line_comment(line).trim();
            if line == "#2D" || line.starts_with("#prec ") {
                Some(line.to_string())
            } else {
                None
            }
        })
        .collect()
}

/// Collects stripped values for a named preview directive.
fn source_directive_values(source: &str, directive: &str) -> Vec<String> {
    source
        .lines()
        .filter_map(|line| {
            let line = strip_source_line_comment(line).trim();
            line.strip_prefix(directive)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| value.trim_matches('"').to_string())
        })
        .collect()
}

/// Removes `//`-style comments from one line, preserving string literals.
fn strip_source_line_comment(line: &str) -> &str {
    let mut in_string = false;
    let mut chars = line.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        if ch == '"' {
            in_string = !in_string;
        }
        if !in_string && ch == '/' && chars.peek().is_some_and(|(_, next)| *next == '/') {
            return &line[..index];
        }
    }
    line
}

/// Appends required default provided values only when user source omitted them.
fn append_preview_provided_values(context: &PreviewContext, out: &mut String) {
    let provided = [
        ("R3", "cameraPosition"),
        ("R3", "cameraForward"),
        ("R3", "cameraGlobalUp"),
        ("R2", "resolution"),
        ("R3", "ambientColor"),
    ];
    for (ty, name) in provided {
        if !context.has_provided_value(name) {
            out.push_str(&format!("\nprovided {ty} {name}"));
        }
    }
}

/// Parses once and returns `None` on parse errors to keep preview diagnostics lazy.
fn parse_preview_program(source: &str) -> Option<Program> {
    parser::Parser::new(source).parse_program().ok()
}

#[derive(Default)]
struct PreviewContext {
    has_main: bool,
    has_scene_object_binding: bool,
    has_scene_material_function: bool,
    last_object_name: Option<String>,
    provided_values: BTreeSet<String>,
}

impl PreviewContext {
    /// Returns true when source has either explicit `scene` binding or object literal.
    fn has_root_object(&self) -> bool {
        self.has_scene_object_binding || self.last_object_name.is_some()
    }

    /// Builds preview metadata from a parsed program in one pass.
    fn from_program(program: &Program) -> Self {
        let object_bindings = preview_object_bindings(program);
        let has_scene_object_binding = object_bindings.iter().any(|(_, name)| name == "scene");
        let last_object_name = object_bindings
            .into_iter()
            .max_by_key(|(line, _)| *line)
            .map(|(_, name)| name);
        let mut provided_values = BTreeSet::new();
        for input in &program.inputs {
            provided_values.insert(input.name.clone());
        }
        Self {
            has_main: program.funcs.iter().any(|func| func.name == "main"),
            has_scene_object_binding,
            has_scene_material_function: program
                .funcs
                .iter()
                .any(|func| func.name == "scene_material"),
            last_object_name,
            provided_values,
        }
    }

    /// Checks whether a value was provided/declared in source.
    fn has_provided_value(&self, value_name: &str) -> bool {
        self.provided_values.contains(value_name)
    }
}

/// Builds preview context by parsing once and collecting only data used by defaults.
fn preview_context(source: &str) -> PreviewContext {
    parse_preview_program(source)
        .map(|program| PreviewContext::from_program(&program))
        .unwrap_or_default()
}

/// Ensures preview source includes required scene/material entry points.
fn validate_preview_requirements(context: &PreviewContext) -> Result<(), Error> {
    if context.has_main {
        return Ok(());
    }
    let mut missing = Vec::new();
    if !context.has_root_object() {
        missing.push("`const Object scene = ...` or `const Object output = ...`".to_string());
    }
    if !context.has_scene_material_function {
        missing.push("`const Hom(R3, Material) scene_material = ...`".to_string());
    }
    if missing.is_empty() {
        return Ok(());
    }
    let mut details = String::from(
        "preview generation requirements were not met. Add an explicit `main`, or define:\n",
    );
    for item in &missing {
        details.push_str("- ");
        details.push_str(item);
        details.push('\n');
    }
    Err(Error::new(details.trim_end().to_string()))
}

/// Returns object binding declarations and inferred object constructions with line numbers.
fn preview_object_bindings(program: &Program) -> Vec<(usize, String)> {
    let mut bindings = program
        .bindings
        .iter()
        .filter(|binding| matches!(binding.ty, Type::Object))
        .map(|binding| (binding.line, binding.name.clone()))
        .collect::<Vec<_>>();
    bindings.extend(
        program
            .inferred_bindings
            .iter()
            .filter(|binding| binding.construct)
            .map(|binding| (binding.line, binding.name.clone())),
    );
    bindings
}

/// Rejects provided function inputs because preview runtime does not support them.
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

/// Produces preview uniform declarations for OpenGL or Vulkan emission.
fn preview_uniforms(program: &Program, target: PreviewShaderTarget) -> String {
    let uniform_names = preview_uniform_names(program);
    match target {
        PreviewShaderTarget::OpenGl => uniform_names
            .iter()
            .map(|uniform| format!("uniform {};", uniform))
            .collect::<Vec<_>>()
            .join("\n"),
        PreviewShaderTarget::Vulkan => {
            if uniform_names.is_empty() {
                String::new()
            } else {
                format!(
                    "layout(std140, push_constant) uniform PreviewUniforms {{\n{}\n}};",
                    uniform_names
                        .iter()
                        .map(|uniform| format!("    {uniform};"))
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            }
        }
    }
}

/// Collects uniform declarations (without trailing semicolon).
fn preview_uniform_names(program: &Program) -> Vec<String> {
    program
        .inputs
        .iter()
        .filter(|input| !matches!(input.ty, Type::Object | Type::Object2D))
        .map(|input| format!("{} {}", input.ty.glsl_name(), input.name))
        .collect()
}

/// Emits a GLSL version directive normalized for ES vs non-ES forms.
fn glsl_version_directive(version: &str) -> String {
    let trimmed = version.trim();
    if let Some(number) = trimmed.strip_suffix("es") {
        format!("#version {} es", number.trim())
    } else {
        format!("#version {trimmed}")
    }
}
