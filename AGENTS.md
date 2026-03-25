# Repository Guidelines

## Project Structure
- Compiler code lives in `src/`.
- Integration tests live in `tests/`.
- Example DSL programs can live at the repository root until a dedicated examples directory exists.

## Design Rules
- Treat shapes as objects denoting SDFs.
- Primitive definitions are based on `ParamShape` records and `sdf0_Shape` local-space evaluators.
- Keep transforms outside primitive parameter records.
- Prefer small typed passes: parse, typecheck, desugar, then emit GLSL.

## Development Commands
- `cargo run -- test.lane`
- `cargo test`

## Style
- Keep functions short and explicit.
- Prefer nominal surface types with simple internal representations.
- Add focused tests for each new DSL feature.
