fn main() {
    println!("cargo:rerun-if-changed=tree-sitter-lane/src/parser.c");
    println!("cargo:rerun-if-changed=tree-sitter-lane/src/tree_sitter/parser.h");

    cc::Build::new()
        .include("tree-sitter-lane/src")
        .file("tree-sitter-lane/src/parser.c")
        .compile("tree-sitter-lane");
}
