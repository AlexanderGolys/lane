use std::env;
use std::fs;
use std::io::{self, Read};
use std::process;

const HELP: &str = "lane compiles lane source files into GLSL.\n\nUsage:\n  lane [PATH]\n  lane --list [NAME]\n  lane --list2d\n  lane --list3d\n  lane --list-functions\n  lane --list-types\n  lane --print-completion <bash|zsh|fish>\n  lane -h\n  lane --help\n\nWhen PATH is omitted, lane reads source from stdin.";

const BASH_COMPLETION: &str = r#"_lane() {
    local cur prev
    COMPREPLY=()
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev="${COMP_WORDS[COMP_CWORD-1]}"

    if [[ "$prev" == "--print-completion" ]]; then
        COMPREPLY=( $(compgen -W "bash zsh fish" -- "$cur") )
        return
    fi

    if [[ "$prev" == "--list" ]]; then
        COMPREPLY=( $(compgen -W "Ball3D Box2D Halfspace3D Point2D Polygon2D Segment2D Simplex3D Torus3D Triangle2D" -- "$cur") )
        return
    fi

    if [[ "$cur" == -* ]]; then
        COMPREPLY=( $(compgen -W "--list --list2d --list3d --list-functions --list-types --print-completion --help -h" -- "$cur") )
        return
    fi
}

complete -F _lane lane
"#;

const ZSH_COMPLETION: &str = r#"#compdef lane

_lane() {
    _arguments \
        '1:command or file:_files' \
        '--list[list known primitives or show one primitive]:name:(Ball3D Box2D Halfspace3D Point2D Polygon2D Segment2D Simplex3D Torus3D Triangle2D)' \
        '--list2d[list only 2D primitives]' \
        '--list3d[list only 3D primitives]' \
        '--list-functions[list known predefined functions]' \
        '--list-types[list known predefined types]' \
        '--print-completion[print a completion script]:shell:(bash zsh fish)' \
        '(-h --help)'{-h,--help}'[show help]'
}

_lane "$@"
"#;

const FISH_COMPLETION: &str = r#"complete -c lane -f
complete -c lane -l list -d 'List known primitives'
complete -c lane -l list -r -a 'Ball3D Box2D Halfspace3D Point2D Polygon2D Segment2D Simplex3D Torus3D Triangle2D' -d 'Show one primitive'
complete -c lane -l list2d -d 'List only 2D primitives'
complete -c lane -l list3d -d 'List only 3D primitives'
complete -c lane -l list-functions -d 'List predefined functions'
complete -c lane -l list-types -d 'List predefined types'
complete -c lane -l print-completion -r -a 'bash zsh fish' -d 'Print a completion script'
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
        [flag] if flag == "--list" => {
            print_known_primitives();
            Ok(())
        }
        [flag, name] if flag == "--list" => print_known_primitive_detail(name),
        [flag] if flag == "--list2d" => {
            print_known_primitives_for_dimension(lane::ShapeDimension::D2);
            Ok(())
        }
        [flag] if flag == "--list3d" => {
            print_known_primitives_for_dimension(lane::ShapeDimension::D3);
            Ok(())
        }
        [flag] if flag == "--list-functions" => {
            print_preregistered_objects_of_kind(lane::PreregisteredObjectKind::Function);
            Ok(())
        }
        [flag] if flag == "--list-types" => {
            print_preregistered_objects_of_kind(lane::PreregisteredObjectKind::Type);
            Ok(())
        }
        [flag, shell] if flag == "--print-completion" => print_completion(shell),
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
    println!("{glsl}");
    Ok(())
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
            println!("{type_body}");
            println!();
        }
        println!("{}", primitive.function_body);
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
    for primitive in lane::known_primitives() {
        print_known_primitive(&primitive);
    }
}

fn print_known_primitives_for_dimension(dimension: lane::ShapeDimension) {
    for primitive in lane::known_primitives_by_dimension(dimension) {
        print_known_primitive(&primitive);
    }
}

fn print_preregistered_objects_of_kind(kind: lane::PreregisteredObjectKind) {
    for object in lane::known_preregistered_objects() {
        if object.kind == kind {
            println!("{}", object.name);
        }
    }
}

fn print_known_primitive(primitive: &lane::KnownPrimitive) {
    println!("{}: {}", primitive.name, visible_parameter_space(primitive));
}
