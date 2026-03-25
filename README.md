# sdf-compiler

A small Rust prototype for a functional SDF DSL that compiles object expressions into GLSL.

Current slice:

- pre-registered `Ball3D(r=...)` primitive
- pre-registered `SmoothUnion(k)` operator
- value functions `sin`, `cos`, `pow2`
- `Obj3 + vec3` placement sugar
- top-level `in`, `func`, typed object bindings, and `out`
- named declarations use `type name = value` syntax, for example `Obj3 A = Ball3D(r=3)`

Example input lives in `test.sdfdsl`.

Run it with:

```sh
cargo run -- test.sdfdsl
```

Run tests with:

```sh
cargo test
```
