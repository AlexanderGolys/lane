use std::env;
use std::fs;
use std::io::{self, IsTerminal, Read};
use std::process;

const COLOR_FUNCTION: &str = "34";
const COLOR_TYPE: &str = "33";
const COLOR_CATEGORY: &str = "92";
const COLOR_CAT_METATYPE: &str = "38;2;255;255;255";
const COLOR_ERROR: &str = "31";

const HELP: &str = "lane compiles lane source files into GLSL.\n\nUsage:\n  lane [SOURCE [TARGET]] [--show]\n  lane -l, --list [NAME]\n  lane -l2, --list2d\n  lane -l3, --list3d\n  lane -lo, --list-objects [NAME]\n  lane list-all\n  lane -la, --list-all\n  lane -pc, --print-completion <bash|zsh|fish>\n  lane -h, --help\n\nWhen SOURCE is omitted, lane reads source from stdin. When TARGET is present, lane writes GLSL to that path instead of stdout. Use --show or -s with SOURCE TARGET to also print the compiled GLSL.";

const BASH_COMPLETION_TEMPLATE: &str = r#"_lane() {
    local cur prev
    COMPREPLY=()
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev="${COMP_WORDS[COMP_CWORD-1]}"

    if [[ "$prev" == "--print-completion" || "$prev" == "-pc" ]]; then
        COMPREPLY=( $(compgen -W "bash zsh fish" -- "$cur") )
        return
    fi

    if [[ "$prev" == "--list" || "$prev" == "-l" ]]; then
        COMPREPLY=( $(compgen -W "__PRIMITIVES__" -- "$cur") )
        return
    fi

    if [[ "$prev" == "--list-objects" || "$prev" == "-lo" ]]; then
        COMPREPLY=( $(compgen -W "__OBJECTS__" -- "$cur") )
        return
    fi

    if [[ "$cur" == -* ]]; then
        COMPREPLY=( $(compgen -W "--show -s --list -l --list2d -l2 --list3d -l3 --list-objects -lo --list-all -la --print-completion -pc --help -h" -- "$cur") )
        return
    fi

    COMPREPLY=( $(compgen -W "list-all" -- "$cur") )
}

complete -F _lane lane
"#;

const ZSH_COMPLETION_TEMPLATE: &str = r#"#compdef lane

_lane() {
    _arguments \
        '1:command or file:_files' \
        '(-l --list)'{-l,--list}'[list known primitives or show one primitive]:name:(__PRIMITIVES__)' \
        '(-l2 --list2d)'{-l2,--list2d}'[list only 2D primitives]' \
        '(-l3 --list3d)'{-l3,--list3d}'[list only 3D primitives]' \
        '(-lo --list-objects)'{-lo,--list-objects}'[list known builtin Lane objects or show one builtin]:name:(__OBJECTS__)' \
        '(-la --list-all)'{-la,--list-all}'[list every builtin object, function, type, and constructor]' \
        '(-pc --print-completion)'{-pc,--print-completion}'[print a completion script]:shell:(bash zsh fish)' \
        '(-s --show)'{-s,--show}'[print compiled GLSL while also writing TARGET]' \
        '(-h --help)'{-h,--help}'[show help]'
}

_lane "$@"
"#;

const FISH_COMPLETION_TEMPLATE: &str = r#"complete -c lane -f
complete -c lane -s l -l list -d 'List known primitives'
complete -c lane -s l -l list -r -a '__PRIMITIVES__' -d 'Show one primitive'
complete -c lane -o l2 -l list2d -d 'List only 2D primitives'
complete -c lane -o l3 -l list3d -d 'List only 3D primitives'
complete -c lane -o lo -l list-objects -d 'List builtin Lane objects'
complete -c lane -o lo -l list-objects -r -a '__OBJECTS__' -d 'Show one builtin object'
complete -c lane -o la -l list-all -d 'List every builtin object, function, type, and constructor'
complete -c lane -f -a 'list-all' -d 'List every builtin object, function, type, and constructor'
complete -c lane -o pc -l print-completion -r -a 'bash zsh fish' -d 'Print a completion script'
complete -c lane -s s -l show -d 'Print compiled GLSL while also writing TARGET'
complete -c lane -s h -l help -d 'Show help'
"#;

fn main() {
    if let Err(err) = run() {
        print_error(err.as_ref());
        process::exit(1);
    }
}

fn print_error(err: &(dyn std::error::Error + 'static)) {
    let message = format_error(err);
    if io::stderr().is_terminal() {
        eprintln!("{}", color(COLOR_ERROR, &message));
        return;
    }
    eprintln!("{message}");
}

fn format_error(err: &(dyn std::error::Error + 'static)) -> String {
    format!("{}: {}", error_type(err), err)
}

fn error_type(err: &(dyn std::error::Error + 'static)) -> &'static str {
    if err.is::<lane::Error>() {
        return "lane::Error";
    }
    if err.is::<io::Error>() {
        return "std::io::Error";
    }
    "error"
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.as_slice() {
        [] => compile_from_stdin(),
        [command] if is_help(command) => {
            print_help();
            Ok(())
        }
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

fn print_compile_path(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let source = fs::read_to_string(path)?;
    print_compiled_program(&source)
}

fn write_compile_path(
    source_path: &str,
    target_path: &str,
    show: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let source = fs::read_to_string(source_path)?;
    let output = compile_program_output(&source)?;
    fs::write(target_path, &output)?;
    if show {
        print_glsl(&output);
    }
    Ok(())
}

fn compile_from_stdin() -> Result<(), Box<dyn std::error::Error>> {
    let mut source = String::new();
    io::stdin().read_to_string(&mut source)?;
    print_compiled_program(&source)
}

fn print_compiled_program(source: &str) -> Result<(), Box<dyn std::error::Error>> {
    let output = compile_program_output(source)?;
    print_glsl(&output);
    Ok(())
}

fn compile_program_output(source: &str) -> Result<String, Box<dyn std::error::Error>> {
    Ok(lane::compile_program(source)?)
}

fn print_help() {
    println!("{HELP}");
}

fn is_help(arg: &str) -> bool {
    matches!(arg, "-h" | "--help")
}

fn is_show(arg: &str) -> bool {
    matches!(arg, "-s" | "--show")
}

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

fn completion_script(template: &str) -> String {
    template
        .replace("__PRIMITIVES__", &completion_primitive_names())
        .replace("__OBJECTS__", &completion_object_names())
}

fn completion_primitive_names() -> String {
    lane::known_primitives()
        .into_iter()
        .map(|primitive| primitive.name)
        .collect::<Vec<_>>()
        .join(" ")
}

fn completion_object_names() -> String {
    lane::known_builtin_objects()
        .into_iter()
        .map(|object| object.name)
        .collect::<Vec<_>>()
        .join(" ")
}

fn print_known_primitives() {
    let primitives = lane::known_primitives();
    for (index, primitive) in primitives.iter().enumerate() {
        print_known_primitive(primitive, index > 0);
    }
}

fn print_known_primitives_for_dimension(dimension: lane::ShapeDimension) {
    let primitives = lane::known_primitives_by_dimension(dimension);
    for (index, primitive) in primitives.iter().enumerate() {
        print_known_primitive(primitive, index > 0);
    }
}

fn print_known_builtin_objects() {
    for object in lane::known_builtin_objects() {
        print_known_builtin_object_line(&object.name, &object.ty, object.kind);
    }
}

fn include_in_list_all(name: &str) -> bool {
    !matches!(name, "matrixCompMult")
}

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

fn print_known_builtin_object_line(name: &str, ty: &str, kind: lane::KnownBuiltinObjectKind) {
    if io::stdout().is_terminal() {
        println!("{}", highlight_builtin_object_line(name, ty, kind));
        return;
    }

    println!("{name}: {ty}");
}

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

fn highlight_builtin_object_name(name: &str, kind: lane::KnownBuiltinObjectKind) -> String {
    match kind {
        lane::KnownBuiltinObjectKind::Function => color(COLOR_FUNCTION, name),
        lane::KnownBuiltinObjectKind::Type => color(COLOR_TYPE, name),
        lane::KnownBuiltinObjectKind::Category => color(COLOR_CATEGORY, name),
    }
}

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

fn highlight_ident(token: &str) -> String {
    if matches!(
        token,
        "float"
            | "int"
            | "bool"
            | "void"
            | "return"
            | "if"
            | "else"
            | "for"
            | "while"
            | "const"
            | "struct"
    ) {
        return color("35", token).to_string();
    }
    if matches!(token, "vec2" | "vec3" | "vec4" | "mat2" | "mat3" | "mat4") {
        return color("34", token).to_string();
    }
    if token.starts_with("sdf")
        || token.starts_with("op_")
        || token.starts_with("scene_")
        || token.starts_with("Param")
    {
        return color("33", token).to_string();
    }
    token.to_string()
}

fn color<'a>(code: &'a str, text: &'a str) -> String {
    format!("\x1b[{}m{}\x1b[0m", code, text)
}

#[cfg(test)]
mod tests {
    use super::{color, format_error, highlight_builtin_object_line, highlight_glsl, COLOR_ERROR};

    #[test]
    fn highlights_glsl_keywords_types_and_numbers() {
        let highlighted = highlight_glsl("float scene_sdf(vec3 p) { return 1.0f-2e-3f; }");

        assert!(highlighted.contains("\x1b[35mfloat\x1b[0m"));
        assert!(highlighted.contains("\x1b[34mvec3\x1b[0m"));
        assert!(highlighted.contains("\x1b[33mscene_sdf\x1b[0m"));
        assert!(highlighted.contains("\x1b[36m1.0f\x1b[0m-\x1b[36m2e-3f\x1b[0m"));
    }

    #[test]
    fn highlights_builtin_object_names_and_lane_types() {
        let highlighted = highlight_builtin_object_line(
            "union",
            "Hom(Object × Object, Object)",
            lane::KnownBuiltinObjectKind::Function,
        );

        assert!(highlighted.contains("\x1b[34munion\x1b[0m"));
        assert!(highlighted.contains("\x1b[97m:\x1b[0m"));
        assert!(highlighted.contains("\x1b[35mHom\x1b[0m"));
        assert!(highlighted.contains("\x1b[33mObject\x1b[0m"));
        assert!(highlighted.contains("\x1b[97m(\x1b[0m"));
        assert!(highlighted.contains("\x1b[97m,\x1b[0m"));
        assert!(highlighted.contains("\x1b[35m×\x1b[0m"));
    }

    #[test]
    fn highlights_builtin_type_names_as_types() {
        let highlighted =
            highlight_builtin_object_line("H", "DivRing, RAlg", lane::KnownBuiltinObjectKind::Type);

        assert!(highlighted.contains("\x1b[33mH\x1b[0m"));
        assert!(highlighted.contains("\x1b[92mDivRing\x1b[0m"));
        assert!(highlighted.contains("\x1b[92mRAlg\x1b[0m"));
        assert!(!highlighted.contains("\x1b[33mDivRing\x1b[0m"));
        assert!(!highlighted.contains("\x1b[33mRAlg\x1b[0m"));
    }

    #[test]
    fn highlights_categories_as_bright_yellow_and_cat_as_white() {
        let highlighted =
            highlight_builtin_object_line("DivRing", "Cat", lane::KnownBuiltinObjectKind::Category);

        assert!(highlighted.contains("\x1b[92mDivRing\x1b[0m"));
        assert!(highlighted.contains("\x1b[38;2;255;255;255mCat\x1b[0m"));
        assert!(!highlighted.contains("\x1b[33mDivRing\x1b[0m"));
        assert!(!highlighted.contains("\x1b[92mCat\x1b[0m"));
    }

    #[test]
    fn highlights_type_metatype_as_type_not_category() {
        let highlighted =
            highlight_builtin_object_line("Object", "Type", lane::KnownBuiltinObjectKind::Type);

        assert!(highlighted.contains("\x1b[33mObject\x1b[0m"));
        assert!(highlighted.contains("\x1b[33mType\x1b[0m"));
        assert!(!highlighted.contains("\x1b[92mType\x1b[0m"));
    }

    #[test]
    fn formats_lane_errors_with_error_type() {
        let err = lane::compile_program("const Object output = Unknown3D(r=1)\n").unwrap_err();

        assert!(format_error(&err).contains("lane::Error: line 1: unknown primitive 'Unknown3D'"));
    }

    #[test]
    fn colors_error_messages_red() {
        assert_eq!(
            color(COLOR_ERROR, "lane::Error: bad"),
            "\x1b[31mlane::Error: bad\x1b[0m"
        );
    }
}
