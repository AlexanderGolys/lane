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
