use std::env;
use std::fs;
use std::io::{self, IsTerminal, Read};
use std::process::{self, Command};

mod preview;
mod repl;

const COLOR_FUNCTION: &str = "34";
const COLOR_TYPE: &str = "33";
const COLOR_CATEGORY: &str = "92";
const COLOR_CAT_METATYPE: &str = "38;2;255;255;255";
const COLOR_ERROR: &str = "31";
const GLSL_KEYWORDS: &[&str] = &[
    "float", "int", "bool", "void", "return", "if", "else", "for", "while", "const", "struct",
];
const GLSL_TYPES: &[&str] = &["vec2", "vec3", "vec4", "mat2", "mat3", "mat4"];
const GLSL_HELPER_PREFIXES: &[&str] = &["sdf_", "op_", "Param"];

const HELP: &str = "lane compiles lane source files into GLSL.\n\nUsage:\n  lane [SOURCE [TARGET]] [--show]\n  lane SOURCE [--frag=FRAG] [--vert=VERT] [--version=VERSION] [--target=opengl|vulkan]\n  lane SOURCE [--frag-spv=SPV] [--vert-spv=SPV]\n  lane repl\n  lane preview SOURCE\n  lane list [NAME]\n  lane list 2d\n  lane list 3d\n  lane list all\n  lane -pc, --print-completion <bash|zsh|fish>\n  lane -h, --help\n\nWhen SOURCE is omitted, lane opens the interactive shell when stdin is a terminal and reads source from stdin otherwise. `lane repl` opens the same shell explicitly. The REPL keeps submitted Lane code, REPL messages, and generated GLSL in one padded colored transcript by default, with one character of inner left padding, source line numbers on submitted Lane entries, adjacent submitted Lane entries and adjacent generated GLSL outputs merged into continuous boxes, blank rows between boxes, and errors shown as red boxes with one blank row between command lines and error boxes; later const submissions show newly added GLSL lines even when support structs are inserted before older output. `/info` shows loaded modules, used directives, and provided objects. `/code` shows the full session source, `/save <filename>` writes that source to a file, and `/export <filename>` writes generated GLSL to a file. `/show` opens a native Vulkan preview window for the current session. `/split` toggles a split view where generated GLSL is shown only in its separate pane, mouse-wheel scrolling is independent per pane, and toggling back restores the linear transcript without adding toggle messages. REPL commands ignore trailing spaces. Up and Down recall submitted input history across sessions, Left and Right move through the current input, and PageUp/PageDown or the mouse wheel scrolls the transcript. Clicking a submitted Lane entry or its generated GLSL highlights both parts of that submission, and right-clicking a transcript block copies that block's text to the terminal clipboard. When TARGET is present, lane writes GLSL to that path instead of stdout. Use --show or -s with SOURCE TARGET to also print the compiled GLSL. Preview shader flags write complete fragment and/or vertex shaders; VERSION defaults to 300es for OpenGL/WebGL. Vulkan preview SPIR-V output and `lane preview` use glslc.";

const BASH_COMPLETION_TEMPLATE: &str = r#"_lane() {
    local cur prev
    COMPREPLY=()
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev="${COMP_WORDS[COMP_CWORD-1]}"

    if [[ "$prev" == "--print-completion" || "$prev" == "-pc" ]]; then
        COMPREPLY=( $(compgen -W "bash zsh fish" -- "$cur") )
        return
    fi

    if [[ "${COMP_WORDS[1]}" == "list" && "$COMP_CWORD" == "2" ]]; then
        COMPREPLY=( $(compgen -W "all 2d 3d __OBJECTS__" -- "$cur") )
        return
    fi

    if [[ "$cur" == -* ]]; then
        COMPREPLY=( $(compgen -W "--show -s --frag= --vert= --frag-spv= --vert-spv= --version= --target= --print-completion -pc --help -h" -- "$cur") )
        return
    fi

    COMPREPLY=( $(compgen -W "preview repl list" -- "$cur") )
}

complete -F _lane lane
"#;

const ZSH_COMPLETION_TEMPLATE: &str = r#"#compdef lane

_lane() {
    _arguments \
        '1:command or file:(preview repl list)' \
        '2:list target:(all 2d 3d __OBJECTS__)' \
        '(-pc --print-completion)'{-pc,--print-completion}'[print a completion script]:shell:(bash zsh fish)' \
        '(-s --show)'{-s,--show}'[print compiled GLSL while also writing TARGET]' \
        '--frag=[write complete preview fragment shader]:file:_files' \
        '--vert=[write complete preview vertex shader]:file:_files' \
        '--frag-spv=[write Vulkan preview fragment SPIR-V]:file:_files' \
        '--vert-spv=[write Vulkan preview vertex SPIR-V]:file:_files' \
        '--version=[preview shader GLSL version]:version:' \
        '--target=[preview shader target]:target:(opengl vulkan)' \
        '(-h --help)'{-h,--help}'[show help]'
}

_lane "$@"
"#;

const FISH_COMPLETION_TEMPLATE: &str = r#"complete -c lane -f
complete -c lane -f -a 'repl' -d 'Open the interactive Lane shell'
complete -c lane -f -a 'list' -d 'List builtin Lane objects'
complete -c lane -n '__fish_seen_subcommand_from list' -f -a 'all' -d 'List every builtin object, function, type, and constructor'
complete -c lane -n '__fish_seen_subcommand_from list' -f -a '2d' -d 'List only 2D primitives'
complete -c lane -n '__fish_seen_subcommand_from list' -f -a '3d' -d 'List only 3D primitives'
complete -c lane -n '__fish_seen_subcommand_from list' -f -a '__OBJECTS__' -d 'Show one builtin object'
complete -c lane -o pc -l print-completion -r -a 'bash zsh fish' -d 'Print a completion script'
complete -c lane -s s -l show -d 'Print compiled GLSL while also writing TARGET'
complete -c lane -l frag -r -d 'Write complete preview fragment shader'
complete -c lane -l vert -r -d 'Write complete preview vertex shader'
complete -c lane -l frag-spv -r -d 'Write Vulkan preview fragment SPIR-V'
complete -c lane -l vert-spv -r -d 'Write Vulkan preview vertex SPIR-V'
complete -c lane -l version -r -d 'Set preview shader GLSL version'
complete -c lane -l target -r -a 'opengl vulkan' -d 'Set preview shader target'
complete -c lane -s h -l help -d 'Show help'
complete -c lane -f -a 'preview' -d 'Open native Vulkan preview'
"#;

/// CLI process entrypoint: parses args, runs interactive mode, or invokes preview/compile paths.
fn main() {
    if let Err(err) = run() {
        print_error(err.as_ref());
        process::exit(1);
    }
}

/// Renders any top-level error through terminal-aware formatting.
fn print_error(err: &(dyn std::error::Error + 'static)) {
    let message = format_error(err);
    if io::stderr().is_terminal() {
        eprintln!("{}", color(COLOR_ERROR, &message));
        return;
    }
    eprintln!("{message}");
}

/// Adds the classified error kind prefix used by CLI-facing error strings.
fn format_error(err: &(dyn std::error::Error + 'static)) -> String {
    format!("{}: {}", error_type(err), err)
}

/// Classifies top-level error kinds for user-facing formatting.
fn error_type(err: &(dyn std::error::Error + 'static)) -> &'static str {
    if err.is::<lane::Error>() {
        return "lane::Error";
    }
    if err.is::<io::Error>() {
        return "std::io::Error";
    }
    "error"
}

/// Dispatches to compile, preview, list, help, and REPL handlers from CLI args.
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|arg| is_preview_arg(arg)) {
        return write_preview_shaders(&args);
    }
    match args.as_slice() {
        [] if io::stdin().is_terminal() => repl::run(),
        [] => compile_from_stdin(),
        [command] if command == "repl" => repl::run(),
        [command] if is_help(command) => {
            print_help();
            Ok(())
        }
        [command] if command == "list" => {
            print_known_builtin_objects();
            Ok(())
        }
        [command, target] if command == "list" && target == "2d" => {
            print_known_primitives_for_dimension(lane::ShapeDimension::D2);
            Ok(())
        }
        [command, target] if command == "list" && target == "3d" => {
            print_known_primitives_for_dimension(lane::ShapeDimension::D3);
            Ok(())
        }
        [command, target] if command == "list" && target == "all" => {
            print_all_known_builtin_items();
            Ok(())
        }
        [command, name] if command == "list" => print_known_builtin_object_detail(name),
        [flag] if matches!(flag.as_str(), "--list" | "-l") => {
            print_known_primitives();
            Ok(())
        }
        [flag, name] if matches!(flag.as_str(), "--list" | "-l") => {
            print_known_primitive_detail(name)
        }
        [flag] if matches!(flag.as_str(), "--list2d" | "-l2") => {
            print_known_primitives_for_dimension(lane::ShapeDimension::D2);
            Ok(())
        }
        [flag] if matches!(flag.as_str(), "--list3d" | "-l3") => {
            print_known_primitives_for_dimension(lane::ShapeDimension::D3);
            Ok(())
        }
        [flag] if matches!(flag.as_str(), "--list-objects" | "-lo") => {
            print_known_builtin_objects();
            Ok(())
        }
        [flag, name] if matches!(flag.as_str(), "--list-objects" | "-lo") => {
            print_known_builtin_object_detail(name)
        }
        [command] if matches!(command.as_str(), "list-all" | "--list-all" | "-la") => {
            print_all_known_builtin_items();
            Ok(())
        }
        [command, source_path] if command == "preview" => run_preview(source_path),
        [flag, shell] if matches!(flag.as_str(), "--print-completion" | "-pc") => {
            print_completion(shell)
        }
        [flag, source_path, target_path] if is_show(flag) => {
            write_compile_path(source_path, target_path, true)
        }
        [source_path, target_path, flag] if is_show(flag) => {
            write_compile_path(source_path, target_path, true)
        }
        [flag] if is_show(flag) => Err("--show requires SOURCE and TARGET".into()),
        [flag, _] if is_show(flag) => Err("--show requires SOURCE and TARGET".into()),
        [path] => print_compile_path(path),
        [source_path, target_path] => write_compile_path(source_path, target_path, false),
        _ => Err("unexpected arguments; run `lane --help` for usage".into()),
    }
}

/// Handles preview-related flags and dispatches to GLSL/SPIR-V generation.
fn write_preview_shaders(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let mut source_path = None;
    let mut frag_path = None;
    let mut vert_path = None;
    let mut frag_spv_path = None;
    let mut vert_spv_path = None;
    let mut version = "300es".to_string();
    let mut target = PreviewTarget::OpenGl;

    for arg in args {
        if let Some(path) = arg.strip_prefix("--frag=") {
            frag_path = Some(path.to_string());
        } else if let Some(path) = arg.strip_prefix("--vert=") {
            vert_path = Some(path.to_string());
        } else if let Some(path) = arg.strip_prefix("--frag-spv=") {
            frag_spv_path = Some(path.to_string());
            target = PreviewTarget::Vulkan;
        } else if let Some(path) = arg.strip_prefix("--vert-spv=") {
            vert_spv_path = Some(path.to_string());
            target = PreviewTarget::Vulkan;
        } else if let Some(value) = arg.strip_prefix("--version=") {
            version = value.to_string();
        } else if let Some(value) = arg.strip_prefix("--target=") {
            target = PreviewTarget::parse(value)?;
        } else if arg.starts_with('-') {
            return Err(format!("unsupported preview flag '{arg}'").into());
        } else if source_path.replace(arg.to_string()).is_some() {
            return Err("preview shader generation expects one SOURCE".into());
        }
    }

    let Some(source_path) = source_path else {
        return Err("preview shader generation requires SOURCE".into());
    };
    if frag_path.is_none()
        && vert_path.is_none()
        && frag_spv_path.is_none()
        && vert_spv_path.is_none()
    {
        return Err(
            "preview shader generation requires --frag=PATH, --vert=PATH, --frag-spv=PATH, or --vert-spv=PATH"
                .into(),
        );
    }

    if let Some(path) = frag_path {
        let output = match target {
            PreviewTarget::OpenGl => {
                lane::compile_preview_fragment_from_path(&source_path, &version)?
            }
            PreviewTarget::Vulkan => lane::compile_vulkan_preview_fragment_from_path(&source_path)?,
        };
        fs::write(path, output)?;
    }
    if let Some(path) = vert_path {
        let output = match target {
            PreviewTarget::OpenGl => lane::compile_preview_vertex(&version),
            PreviewTarget::Vulkan => lane::compile_vulkan_preview_vertex(),
        };
        fs::write(path, output)?;
    }
    if let Some(path) = frag_spv_path {
        let output = lane::compile_vulkan_preview_fragment_from_path(&source_path)?;
        write_spirv_shader("frag", &output, &path)?;
    }
    if let Some(path) = vert_spv_path {
        write_spirv_shader("vert", &lane::compile_vulkan_preview_vertex(), &path)?;
    }
    Ok(())
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum PreviewTarget {
    OpenGl,
    Vulkan,
}

impl PreviewTarget {
    /// Parses preview target names from CLI input.
    fn parse(value: &str) -> Result<Self, Box<dyn std::error::Error>> {
        match value {
            "opengl" | "gl" | "webgl" => Ok(Self::OpenGl),
            "vulkan" | "vk" => Ok(Self::Vulkan),
            _ => Err(format!("unsupported preview target '{value}'").into()),
        }
    }
}

/// Detects preview-generation arguments in the top-level CLI parser.
fn is_preview_arg(arg: &str) -> bool {
    arg.starts_with("--frag=")
        || arg.starts_with("--vert=")
        || arg.starts_with("--frag-spv=")
        || arg.starts_with("--vert-spv=")
        || arg.starts_with("--version=")
        || arg.starts_with("--target=")
}

/// Writes a temporary SPIR-V file for a shader source and stage.
fn write_spirv_shader(
    stage: &str,
    source: &str,
    target_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = compile_spirv_shader(stage, source)?;
    fs::write(target_path, bytes)?;
    Ok(())
}

/// Builds and compiles one temporary SPIR-V shader stage.
fn compile_spirv_shader(stage: &str, source: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let source_path = unique_preview_shader_path(stage, "glsl")?;
    let target_path = unique_preview_shader_path(stage, "spv")?;
    fs::write(&source_path, source)?;
    let output = Command::new("glslc")
        .arg(format!("-fshader-stage={stage}"))
        .arg(&source_path)
        .arg("-o")
        .arg(&target_path)
        .output();
    let _ = fs::remove_file(&source_path);
    let output = output.map_err(|err| format!("failed to run glslc: {err}"))?;
    if !output.status.success() {
        let _ = fs::remove_file(&target_path);
        return Err(format!(
            "glslc failed for {stage} shader: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    let bytes = fs::read(&target_path)?;
    let _ = fs::remove_file(&target_path);
    Ok(bytes)
}

/// Compiles and opens Vulkan preview for a source file path.
fn run_preview(source_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let fragment = lane::compile_vulkan_preview_fragment_from_path(source_path)?;
    run_preview_fragment(&fragment)
}

/// Compiles Vulkan preview from inline source text.
fn run_preview_source(source: &str) -> Result<(), Box<dyn std::error::Error>> {
    let fragment = lane::compile_vulkan_preview_fragment(source)?;
    run_preview_fragment(&fragment)
}

/// Runs the preview renderer from already-compiled fragment source.
fn run_preview_fragment(fragment: &str) -> Result<(), Box<dyn std::error::Error>> {
    let vertex = lane::compile_vulkan_preview_vertex();
    let fragment_spv = compile_spirv_shader("frag", fragment)?;
    let vertex_spv = compile_spirv_shader("vert", &vertex)?;
    preview::run(preview::PreviewShaders {
        vertex_spv,
        fragment_spv,
    })
}

/// Generates unique preview staging paths for temporary artifacts.
fn unique_preview_shader_path(
    stage: &str,
    extension: &str,
) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let dir = std::env::current_dir()?.join("target").join("lane-preview");
    fs::create_dir_all(&dir)?;
    Ok(dir.join(format!(
        "lane-preview-{}-{}-{}.{}",
        stage,
        process::id(),
        nanos,
        extension
    )))
}

/// Compiles one source file and prints generated GLSL to stdout.
fn print_compile_path(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let output = lane::compile_program_from_path(path)?;
    print_glsl(&output);
    Ok(())
}

/// Writes compiled GLSL to target path and optionally prints to stdout.
fn write_compile_path(
    source_path: &str,
    target_path: &str,
    show: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let output = lane::compile_program_from_path(source_path)?;
    fs::write(target_path, &output)?;
    if show {
        print_glsl(&output);
    }
    Ok(())
}

/// Reads from stdin and prints compiled GLSL output.
fn compile_from_stdin() -> Result<(), Box<dyn std::error::Error>> {
    let mut source = String::new();
    io::stdin().read_to_string(&mut source)?;
    print_compiled_program(&source)
}

/// Compiles source text and prints formatted GLSL output.
fn print_compiled_program(source: &str) -> Result<(), Box<dyn std::error::Error>> {
    let output = compile_program_output(source)?;
    print_glsl(&output);
    Ok(())
}

/// Compiles source text and returns raw GLSL output.
fn compile_program_output(source: &str) -> Result<String, Box<dyn std::error::Error>> {
    Ok(lane::compile_program(source)?)
}

/// Prints CLI usage text.
fn print_help() {
    println!("{HELP}");
}

/// Checks whether an argument requests help output.
fn is_help(arg: &str) -> bool {
    matches!(arg, "-h" | "--help")
}

/// Checks whether an argument requests `--show` behavior.
fn is_show(arg: &str) -> bool {
    matches!(arg, "-s" | "--show")
}

/// Prints a full primitive definition, including parameter/body formatting.
fn print_known_primitive_detail(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(primitive) = lane::known_primitive(name) {
        println!(
            "{}: {}",
            primitive.name,
            visible_parameter_space(&primitive)
        );
        println!();
        if let Some(type_body) = primitive.type_body {
            print_glsl(&type_body);
            println!();
        }
        print_glsl(&primitive.function_body);
        return Ok(());
    }
    Err(format!("unknown primitive '{name}'").into())
}

/// Returns the user-facing signature used for `known`/`list` output.
fn visible_parameter_space(primitive: &lane::KnownPrimitive) -> String {
    let derived_name = format!("Param{}", primitive.name);
    if primitive.parameter_space == derived_name {
        let fields = primitive
            .fields
            .iter()
            .map(|field| format!("{}: {}", field.name, field.domain))
            .collect::<Vec<_>>()
            .join(", ");
        return format!("{{{fields}}}");
    }

    primitive.parameter_space.clone()
}

/// Prints shell completion script for requested shell.
fn print_completion(shell: &str) -> Result<(), Box<dyn std::error::Error>> {
    let script = match shell {
        "bash" => completion_script(BASH_COMPLETION_TEMPLATE),
        "zsh" => completion_script(ZSH_COMPLETION_TEMPLATE),
        "fish" => completion_script(FISH_COMPLETION_TEMPLATE),
        _ => return Err(format!("unsupported shell '{shell}'").into()),
    };
    print!("{script}");
    Ok(())
}

/// Expands completion template with available primitive names.
fn completion_script(template: &str) -> String {
    template
        .replace("__PRIMITIVES__", &completion_primitive_names())
        .replace("__OBJECTS__", &completion_object_names())
}

/// Builds a whitespace-separated primitive completion list.
fn completion_primitive_names() -> String {
    lane::known_primitives()
        .into_iter()
        .map(|primitive| primitive.name)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Builds a whitespace-separated builtin-object completion list.
fn completion_object_names() -> String {
    lane::known_builtin_objects()
        .into_iter()
        .map(|object| object.name)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Prints all registered primitives to stdout.
fn print_known_primitives() {
    let primitives = lane::known_primitives();
    for (index, primitive) in primitives.iter().enumerate() {
        print_known_primitive(primitive, index > 0);
    }
}

/// Prints primitives filtered by target shape dimension.
fn print_known_primitives_for_dimension(dimension: lane::ShapeDimension) {
    let primitives = lane::known_primitives_by_dimension(dimension);
    for (index, primitive) in primitives.iter().enumerate() {
        print_known_primitive(primitive, index > 0);
    }
}

/// Prints all builtin objects (functions/types/categories) to stdout.
fn print_known_builtin_objects() {
    for object in lane::known_builtin_objects() {
        print_known_builtin_object_line(&object.name, &object.ty, object.kind);
    }
}

/// Excludes internal names from the `list all` command.
fn include_in_list_all(name: &str) -> bool {
    !matches!(name, "matrixCompMult")
}

/// Prints all known primitive/object entries grouped for `list all`.
fn print_all_known_builtin_items() {
    for primitive in lane::known_primitives() {
        print_known_builtin_object_line(
            &primitive.name,
            &visible_parameter_space(&primitive),
            lane::KnownBuiltinObjectKind::Function,
        );
    }
    for object in lane::known_builtin_objects() {
        if !include_in_list_all(&object.name) {
            continue;
        }
        print_known_builtin_object_line(&object.name, &object.ty, object.kind);
    }
}

/// Prints one builtin object by name with optional body.
fn print_known_builtin_object_detail(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(object) = lane::known_builtin_object(name) {
        print_known_builtin_object_line(&object.name, &object.ty, object.kind);
        if !object.body.is_empty() {
            println!();
            print_glsl(&object.body);
        }
        return Ok(());
    }
    Err(format!("unknown builtin object '{name}'").into())
}

/// Prints one object line and uses color if output is terminal.
fn print_known_builtin_object_line(name: &str, ty: &str, kind: lane::KnownBuiltinObjectKind) {
    if io::stdout().is_terminal() {
        println!("{}", highlight_builtin_object_line(name, ty, kind));
        return;
    }

    println!("{name}: {ty}");
}

/// Renders a single object line with syntax highlighting by object kind.
fn highlight_builtin_object_line(
    name: &str,
    ty: &str,
    kind: lane::KnownBuiltinObjectKind,
) -> String {
    format!(
        "{}{} {}",
        highlight_builtin_object_name(name, kind),
        color("97", ":"),
        highlight_lane_signature(ty)
    )
}

/// Highlights builtin name by object kind.
fn highlight_builtin_object_name(name: &str, kind: lane::KnownBuiltinObjectKind) -> String {
    match kind {
        lane::KnownBuiltinObjectKind::Function => color(COLOR_FUNCTION, name),
        lane::KnownBuiltinObjectKind::Type => color(COLOR_TYPE, name),
        lane::KnownBuiltinObjectKind::Category => color(COLOR_CATEGORY, name),
    }
}

/// Highlights lane type signatures for display in terminal output.
fn highlight_lane_signature(source: &str) -> String {
    let mut out = String::new();
    let bytes = source.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let ch = source[i..].chars().next().unwrap();
        if ch.is_ascii_alphabetic() {
            let start = i;
            i += 1;
            while i < bytes.len() {
                let next = bytes[i] as char;
                if next.is_ascii_alphanumeric() {
                    i += 1;
                } else {
                    break;
                }
            }
            out.push_str(&highlight_lane_ident(&source[start..i]));
            continue;
        }
        match ch {
            '(' | ')' => out.push_str(&color("97", &source[i..i + 1])),
            ',' => out.push_str(&color("97", &source[i..i + 1])),
            '×' => out.push_str(&color("35", "×")),
            _ => out.push(ch),
        }
        i += ch.len_utf8();
    }
    out
}

/// Highlights reserved keywords/types/symbol names in a lane signature.
fn highlight_lane_ident(token: &str) -> String {
    if matches!(token, "Func" | "Hom" | "End") {
        return color("35", token);
    }
    if token == lane::TYPE_METATYPE_NAME {
        return color(COLOR_TYPE, token);
    }
    if token == lane::CATEGORY_METATYPE_NAME {
        return color(COLOR_CAT_METATYPE, token);
    }
    if lane::is_known_category_name(token) {
        return color(COLOR_CATEGORY, token);
    }
    if lane::is_known_type_name(token) {
        return color(COLOR_TYPE, token);
    }
    token.to_string()
}

/// Prints one primitive entry used by `list` and keeps spacing.
fn print_known_primitive(primitive: &lane::KnownPrimitive, separate_from_previous: bool) {
    if separate_from_previous {
        println!();
    }
    println!("{}", primitive.name);
    if let Some(type_body) = primitive.type_body.as_deref() {
        print_glsl(type_body);
        return;
    }
    println!("{}", visible_parameter_space(primitive));
}

/// Dispatches to highlighted or plain GLSL emission.
fn print_glsl(source: &str) {
    if io::stdout().is_terminal() {
        print!("{}", highlight_glsl(source));
        if !source.ends_with('\n') {
            println!();
        }
        return;
    }

    println!("{source}");
}

/// Adds simple GLSL highlighting to support readable CLI preview.
fn highlight_glsl(source: &str) -> String {
    let mut out = String::new();
    let bytes = source.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let ch = bytes[i] as char;
        if ch == '/' && i + 1 < bytes.len() && bytes[i + 1] as char == '/' {
            let start = i;
            i += 2;
            while i < bytes.len() && bytes[i] as char != '\n' {
                i += 1;
            }
            out.push_str(&color("90", &source[start..i]));
            continue;
        }
        if ch.is_ascii_digit() {
            let start = i;
            i = scan_glsl_number(bytes, i);
            out.push_str(&color("36", &source[start..i]));
            continue;
        }
        if ch.is_ascii_alphabetic() || ch == '_' {
            let start = i;
            i += 1;
            while i < bytes.len() {
                let next = bytes[i] as char;
                if next.is_ascii_alphanumeric() || next == '_' {
                    i += 1;
                } else {
                    break;
                }
            }
            let token = &source[start..i];
            out.push_str(&highlight_ident(token));
            continue;
        }
        out.push(ch);
        i += 1;
    }
    out
}

/// Advances to the first index after a numeric literal in source bytes.
fn scan_glsl_number(bytes: &[u8], mut i: usize) -> usize {
    while i < bytes.len() && (bytes[i] as char).is_ascii_digit() {
        i += 1;
    }
    if i < bytes.len() && bytes[i] == b'.' {
        i += 1;
        while i < bytes.len() && (bytes[i] as char).is_ascii_digit() {
            i += 1;
        }
    }
    if i < bytes.len() && matches!(bytes[i], b'e' | b'E') {
        let exponent = i;
        i += 1;
        if i < bytes.len() && matches!(bytes[i], b'+' | b'-') {
            i += 1;
        }
        let digits = i;
        while i < bytes.len() && (bytes[i] as char).is_ascii_digit() {
            i += 1;
        }
        if i == digits {
            i = exponent;
        }
    }
    if i < bytes.len() && matches!(bytes[i], b'f' | b'F') {
        i += 1;
    }
    i
}

/// Highlights GLSL keywords/types/helpers for terminal output.
fn highlight_ident(token: &str) -> String {
    if GLSL_KEYWORDS.contains(&token) {
        return color("35", token).to_string();
    }
    if GLSL_TYPES.contains(&token) {
        return color("34", token).to_string();
    }
    if GLSL_HELPER_PREFIXES
        .iter()
        .any(|prefix| token.starts_with(prefix))
    {
        return color("33", token).to_string();
    }
    token.to_string()
}

/// Wraps ANSI color code around one token.
fn color<'a>(code: &'a str, text: &'a str) -> String {
    format!("\x1b[{}m{}\x1b[0m", code, text)
}

#[cfg(test)]
#[path = "../tests/unit/cli.rs"]
mod tests;
