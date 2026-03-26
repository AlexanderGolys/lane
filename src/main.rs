use std::env;
use std::fs;
use std::io::{self, IsTerminal, Read};
use std::process;

const HELP: &str = "lane compiles lane source files into GLSL.\n\nUsage:\n  lane [PATH]\n  lane --list [NAME]\n  lane -l [NAME]\n  lane --list2d\n  lane -l2\n  lane --list3d\n  lane -l3\n  lane --list-objects\n  lane -lo\n  lane --print-completion <bash|zsh|fish>\n  lane -pc <bash|zsh|fish>\n  lane -h\n  lane --help\n\nWhen PATH is omitted, lane reads source from stdin. Add `// fragment-shader: #version 330 core` to wrap the generated GLSL in a minimal fullscreen fragment shader.";

const BASH_COMPLETION: &str = r#"_lane() {
    local cur prev
    COMPREPLY=()
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev="${COMP_WORDS[COMP_CWORD-1]}"

    if [[ "$prev" == "--print-completion" || "$prev" == "-pc" ]]; then
        COMPREPLY=( $(compgen -W "bash zsh fish" -- "$cur") )
        return
    fi

    if [[ "$prev" == "--list" || "$prev" == "-l" ]]; then
        COMPREPLY=( $(compgen -W "Ball3D Box2D Halfspace3D Point2D Polygon2D Segment2D Segment3D Simplex3D Torus3D Triangle2D" -- "$cur") )
        return
    fi

    if [[ "$cur" == -* ]]; then
        COMPREPLY=( $(compgen -W "--list -l --list2d -l2 --list3d -l3 --list-objects -lo --print-completion -pc --help -h" -- "$cur") )
        return
    fi
}

complete -F _lane lane
"#;

const ZSH_COMPLETION: &str = r#"#compdef lane

_lane() {
    _arguments \
        '1:command or file:_files' \
        '(-l --list)'{-l,--list}'[list known primitives or show one primitive]:name:(Ball3D Box2D Halfspace3D Point2D Polygon2D Segment2D Segment3D Simplex3D Torus3D Triangle2D)' \
        '(-l2 --list2d)'{-l2,--list2d}'[list only 2D primitives]' \
        '(-l3 --list3d)'{-l3,--list3d}'[list only 3D primitives]' \
        '(-lo --list-objects)'{-lo,--list-objects}'[list known builtin Lane objects]' \
        '(-pc --print-completion)'{-pc,--print-completion}'[print a completion script]:shell:(bash zsh fish)' \
        '(-h --help)'{-h,--help}'[show help]'
}

_lane "$@"
"#;

const FISH_COMPLETION: &str = r#"complete -c lane -f
complete -c lane -s l -l list -d 'List known primitives'
complete -c lane -s l -l list -r -a 'Ball3D Box2D Halfspace3D Point2D Polygon2D Segment2D Segment3D Simplex3D Torus3D Triangle2D' -d 'Show one primitive'
complete -c lane -o l2 -l list2d -d 'List only 2D primitives'
complete -c lane -o l3 -l list3d -d 'List only 3D primitives'
complete -c lane -o lo -l list-objects -d 'List builtin Lane objects'
complete -c lane -o pc -l print-completion -r -a 'bash zsh fish' -d 'Print a completion script'
complete -c lane -s h -l help -d 'Show help'
"#;

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        process::exit(1);
    }
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
        [flag, shell] if matches!(flag.as_str(), "--print-completion" | "-pc") => {
            print_completion(shell)
        }
        [path] => compile_path(path),
        _ => Err("unexpected arguments; run `lane --help` for usage".into()),
    }
}

fn compile_path(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let source = fs::read_to_string(path)?;
    print_compiled_program(&source)
}

fn compile_from_stdin() -> Result<(), Box<dyn std::error::Error>> {
    let mut source = String::new();
    io::stdin().read_to_string(&mut source)?;
    print_compiled_program(&source)
}

fn print_compiled_program(source: &str) -> Result<(), Box<dyn std::error::Error>> {
    let glsl = lane::compile_program(source)?;
    let output = match fragment_shader_version(source) {
        Some(version) => wrap_fragment_shader(&glsl, version)?,
        None => glsl,
    };
    print_glsl(&output);
    Ok(())
}

fn fragment_shader_version(source: &str) -> Option<&str> {
    source.lines().find_map(|line| {
        line.trim()
            .strip_prefix("// fragment-shader:")
            .map(str::trim)
            .filter(|version| !version.is_empty())
    })
}

fn wrap_fragment_shader(glsl: &str, version: &str) -> Result<String, Box<dyn std::error::Error>> {
    if !scene_sdf_accepts_only_point(glsl) {
        return Err(
            "fragment shader wrapper currently requires `scene_sdf(vec3 ...)` without extra inputs"
                .into(),
        );
    }

    Ok(format!(
        "{version}\n\nout vec4 fragColor;\nuniform vec2 resolution;\n\n{glsl}\n\nvoid main() {{\n    vec2 uv = ((2.0 * gl_FragCoord.xy) - resolution) / min(resolution.x, resolution.y);\n    float d = scene_sdf(vec3(uv, 0.0));\n    fragColor = d <= 0.0 ? vec4(vec3(1.0), 1.0) : vec4(vec3(0.0), 1.0);\n}}"
    ))
}

fn scene_sdf_accepts_only_point(glsl: &str) -> bool {
    glsl.lines().any(|line| {
        let Some(signature) = line.trim().strip_prefix("float scene_sdf(") else {
            return false;
        };
        let Some((args, _)) = signature.split_once(')') else {
            return false;
        };
        args.starts_with("vec3 ") && !args.contains(',')
    })
}

fn print_help() {
    println!("{HELP}");
}

fn is_help(arg: &str) -> bool {
    matches!(arg, "-h" | "--help")
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
        "bash" => BASH_COMPLETION,
        "zsh" => ZSH_COMPLETION,
        "fish" => FISH_COMPLETION,
        _ => return Err(format!("unsupported shell '{shell}'").into()),
    };
    print!("{script}");
    Ok(())
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
        println!("{}: {}", object.name, object.ty);
    }
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
            i += 1;
            while i < bytes.len() {
                let next = bytes[i] as char;
                if next.is_ascii_alphanumeric() || matches!(next, '.' | '_' | '+' | '-') {
                    i += 1;
                } else {
                    break;
                }
            }
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
        || token.starts_with("dsl_")
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
    use super::{
        fragment_shader_version, highlight_glsl, scene_sdf_accepts_only_point, wrap_fragment_shader,
    };

    #[test]
    fn highlights_glsl_keywords_types_and_numbers() {
        let highlighted = highlight_glsl("float scene_sdf(vec3 p) { return 1.0; }");

        assert!(highlighted.contains("\x1b[35mfloat\x1b[0m"));
        assert!(highlighted.contains("\x1b[34mvec3\x1b[0m"));
        assert!(highlighted.contains("\x1b[33mscene_sdf\x1b[0m"));
        assert!(highlighted.contains("\x1b[36m1.0\x1b[0m"));
    }

    #[test]
    fn detects_fragment_shader_directive() {
        let source = "// fragment-shader: #version 330 core\ngenerate Ball3D(r=1)\n";

        assert_eq!(fragment_shader_version(source), Some("#version 330 core"));
    }

    #[test]
    fn wraps_minimal_fragment_shader() {
        let wrapped = wrap_fragment_shader(
            "float scene_sdf(vec3 p) { return length(p); }",
            "#version 330 core",
        )
        .unwrap();

        assert!(wrapped.starts_with("#version 330 core"));
        assert!(wrapped.contains("uniform vec2 resolution;"));
        assert!(wrapped.contains("float d = scene_sdf(vec3(uv, 0.0));"));
        assert!(wrapped.contains("fragColor = d <= 0.0"));
    }

    #[test]
    fn rejects_fragment_wrapper_for_extra_scene_inputs() {
        assert!(!scene_sdf_accepts_only_point(
            "float scene_sdf(vec3 p, float time) { return time; }"
        ));
    }
}
