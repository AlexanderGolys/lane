use std::env;
use std::fs;
use std::io::{self, Read};
use std::process;

const HELP: &str = "lane compiles lane source files into GLSL.\n\nUsage:\n  lane [PATH]\n  lane --list\n  lane --list2d\n  lane --list3d\n  lane --list-preregistered\n  lane --show <NAME>\n  lane --print-completion <bash|zsh|fish>\n  lane -h\n  lane --help\n\nWhen PATH is omitted, lane reads source from stdin.";

const BASH_COMPLETION: &str = r#"_lane() {
    local cur prev
    COMPREPLY=()
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev="${COMP_WORDS[COMP_CWORD-1]}"

    if [[ "$prev" == "--print-completion" ]]; then
        COMPREPLY=( $(compgen -W "bash zsh fish" -- "$cur") )
        return
    fi

    if [[ "$cur" == -* ]]; then
        COMPREPLY=( $(compgen -W "--list --list2d --list3d --list-preregistered --show --print-completion --help -h" -- "$cur") )
        return
    fi
}

complete -F _lane lane
"#;

const ZSH_COMPLETION: &str = r#"#compdef lane

_lane() {
    _arguments \
        '1:command or file:_files' \
        '--list2d[list only 2D primitives]' \
        '--list3d[list only 3D primitives]' \
        '--list-preregistered[list preregistered objects]' \
        '--show[show one preregistered object]:name' \
        '--print-completion[print a completion script]:shell:(bash zsh fish)' \
        '(-h --help)'{-h,--help}'[show help]'
}

_lane "$@"
"#;

const FISH_COMPLETION: &str = r#"complete -c lane -f
complete -c lane -l list -d 'List known primitives'
complete -c lane -l list2d -d 'List only 2D primitives'
complete -c lane -l list3d -d 'List only 3D primitives'
complete -c lane -l list-preregistered -d 'List preregistered objects'
complete -c lane -l show -r -d 'Show one preregistered object'
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
        [flag] if flag == "--list2d" => {
            print_known_primitives_for_dimension(lane::ShapeDimension::D2);
            Ok(())
        }
        [flag] if flag == "--list3d" => {
            print_known_primitives_for_dimension(lane::ShapeDimension::D3);
            Ok(())
        }
        [flag] if flag == "--list-preregistered" => {
            print_preregistered_objects();
            Ok(())
        }
        [flag, name] if flag == "--show" => print_preregistered_object(name),
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

fn print_preregistered_object(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(object) = lane::preregistered_object(name) {
        println!("{} {}", kind_name(object.kind), object.name);
        println!();
        println!("{}", object.body);
        return Ok(());
    }
    Err(format!("unknown preregistered object '{name}'").into())
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

fn print_known_primitive(primitive: &lane::KnownPrimitive) {
    let fields = primitive
        .fields
        .iter()
        .map(|field| format!("{}: {}", field.name, field.domain))
        .collect::<Vec<_>>()
        .join(", ");
    match &primitive.parameter_type {
        Some(parameter_type) => {
            println!(
                "[{}] {}: {} | params {} {{{}}}",
                primitive.dimension.label(),
                primitive.name,
                primitive.domain,
                parameter_type,
                fields
            );
        }
        None => {
            println!(
                "[{}] {}: {} | fields {{{}}}",
                primitive.dimension.label(),
                primitive.name,
                primitive.domain,
                fields
            );
        }
    }
}

fn print_preregistered_objects() {
    for kind in [
        lane::PreregisteredObjectKind::Function,
        lane::PreregisteredObjectKind::Type,
    ] {
        println!("{}:", kind_name(kind));
        for object in lane::known_preregistered_objects() {
            if object.kind == kind {
                println!("{}", object.name);
            }
        }
        println!();
    }
}

fn kind_name(kind: lane::PreregisteredObjectKind) -> &'static str {
    match kind {
        lane::PreregisteredObjectKind::Function => "function",
        lane::PreregisteredObjectKind::Type => "type",
    }
}
