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
- Prefer small typed passes: parse, typecheck, desugar, then emit GLSL.

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
