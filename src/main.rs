use std::env;
use std::fs;
use std::io::{self, Read};
use std::process;

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.as_slice() {
        [flag] if flag == "--list-primitives" => {
            print_known_primitives();
            return Ok(());
        }
        [flag] if flag == "--list-preregistered" => {
            print_preregistered_objects();
            return Ok(());
        }
        [flag, name] if flag == "--show-preregistered" => {
            if let Some(object) = sdf_dsl::preregistered_object(name) {
                println!("{} {}", kind_name(object.kind), object.name);
                println!();
                println!("{}", object.body);
                return Ok(());
            }
            return Err(format!("unknown preregistered object '{name}'").into());
        }
        _ => {}
    }

    let source = if let Some(path) = args.first() {
        fs::read_to_string(path)?
    } else {
        let mut source = String::new();
        io::stdin().read_to_string(&mut source)?;
        source
    };

    let glsl = sdf_dsl::compile_program(&source)?;
    println!("{glsl}");
    Ok(())
}

fn print_known_primitives() {
    for primitive in sdf_dsl::known_primitives() {
        let fields = primitive
            .fields
            .iter()
            .map(|field| format!("{}: {}", field.name, field.domain))
            .collect::<Vec<_>>()
            .join(", ");
        match &primitive.parameter_type {
            Some(parameter_type) => {
                println!(
                    "{}: {} | params {} {{{}}}",
                    primitive.name, primitive.domain, parameter_type, fields
                );
            }
            None => {
                println!(
                    "{}: {} | fields {{{}}}",
                    primitive.name, primitive.domain, fields
                );
            }
        }
    }
}

fn print_preregistered_objects() {
    for kind in [
        sdf_dsl::PreregisteredObjectKind::Function,
        sdf_dsl::PreregisteredObjectKind::Type,
    ] {
        println!("{}:", kind_name(kind));
        for object in sdf_dsl::known_preregistered_objects() {
            if object.kind == kind {
                println!("{}", object.name);
            }
        }
        println!();
    }
}

fn kind_name(kind: sdf_dsl::PreregisteredObjectKind) -> &'static str {
    match kind {
        sdf_dsl::PreregisteredObjectKind::Function => "function",
        sdf_dsl::PreregisteredObjectKind::Type => "type",
    }
}
