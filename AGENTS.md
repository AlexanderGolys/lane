# Repository Guidelines

## Project Structure
- Compiler code lives in the root `lane` crate under `src/`.
- Compiler internals are split by pass: `src/parser.rs`, `src/typecheck.rs`, `src/emit.rs`, and `src/registry.rs`.
- The Language Server Protocol binary lives in the separate `crates/lane-lsp` workspace crate.
- Integration tests live in `tests/`.
- Example DSL programs can live at the repository root until a dedicated examples directory exists.

## Design Rules
- Treat shapes as objects denoting SDFs.
- Primitive definitions are based on `ParamShape` records and `sdf0_Shape` local-space evaluators.
- Keep transforms outside primitive parameter records.
- Prefer small typed passes: parse to surface AST, preprocess/desugar into the current Lane core syntax, typecheck, postprocess into GLSL-oriented core syntax, then emit GLSL.
- When adding a Rust-defined object, classify it as either syntactically essential core machinery or as a temporary `std`-movable object. Prefer moving secondary objects into Lane modules instead of expanding Rust registries.
- Keep the compiler/std split similar to C++: the compiler should provide only minimal syntax, typechecking, overload/module machinery, directives, and backend hooks. Objects constructible in Lane belong in `std` and must be imported explicitly rather than hardcoded in Rust.
- Keep `ROADMAP.md` as a progress tracker for desired language/compiler features. When a feature works only partially, record both an example that works now and an example that should work in the final general version but does not yet. After design discussions or implementation work that changes feature scope, update the roadmap status and examples in the same pass.
- After making a change in tree-sitter grammar, always make sure to generate new grammar with `tree-sitter generate`
- If you've finished making changes make sure to install the new version with:
```bash
  cargo install --path .
  cargo install --path crates/lane-lsp
  ```

## Development Commands
- `cargo run -- test.lane`
- `cargo run -p lane-lsp`
- `cargo test`


## Editor Tooling
- When changing Tree-sitter grammar, queries, highlights, or Neovim integration, verify that nvim is not using stale cached parser/query state.
- After any Tree-sitter-related change, explicitly reload/reinstall the parser or restart nvim as needed, then confirm highlight queries load correctly before considering the work done.
- The bundled Neovim Lane plugin also registers `lane-lsp` by default; when changing editor integration, keep the LSP README examples and `tree-sitter-lane/lua/lane/init.lua` setup behavior aligned.
- LSP diagnostics should compile with the opened file's directory as the import base so editor diagnostics match CLI compilation from a file path.

## Style
- Keep functions short and explicit.
- Prefer nominal Lane types with simple internal representations.
- In Lane type signatures, put spaces around product signs: write `R3 × R3`, not `R3×R3`.
- Preserve explicit product syntax in function signatures: `R × R` means two real-valued arguments, while `R2` means a Euclidean-plane/vector argument. They may be treated as isomorphic for compatibility where needed, but parsing or formatting must not silently replace one with the other, especially in larger domains such as `R × R × A` versus `R2 × A`.
- When resolving adjacent numeric generic placeholders in a name, insert `_` if either resolved number has more than one digit: `E{1}{2}` may become `E12`, but `E{12}{3}` becomes `E12_3`.
- Add focused tests for each new DSL feature.
- Only write the lane code using raw GLSL constructors when the fuinction is impossible to implement in pure lane, e.g. has a for loop
- If a std/helper definition can be expressed as ordinary Lane expressions, write it in Lane; do not use raw GLSL as a shortcut for formulas that Lane can model.
- In modules, declare named reusable values, functions, and type/category declarations as `const`; non-const module declarations may receive generated private names and should not be used for public helpers.
- Avoid lambdas when the helper can be written as a pure function composition without naming arguments; prefer point-free composition especially in public module helpers.
- Avoid hardcoding in compiler syntax reserved for specific objects, try to make it as general as possible
- Avoid hardcoding in compiler specific objects
- Try to hardcode in Rust as little GLSL raw code as possible 
- Always update the tree-sitter and LSP after making a changhe to the grammar 
- Always make sure the tree-sitter and LSP after the change are rebuilt and ready to use in nvim
