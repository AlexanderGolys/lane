use std::env;
use std::fs;
use std::io::{self, Read};
use std::process;

const HELP: &str = "lane compiles lane source files into GLSL.\n\nUsage:\n  lane [PATH]\n  lane list\n  lane --list2d\n  lane --list3d\n  lane --list-preregistered\n  lane --show-preregistered <NAME>\n  lane -h\n  lane --help\n\nWhen PATH is omitted, lane reads source from stdin.";

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
        [command] if command == "list" => {
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
        [flag, name] if flag == "--show-preregistered" => print_preregistered_object(name),
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
