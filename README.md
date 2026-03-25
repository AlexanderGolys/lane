# sdf-compiler

A small Rust prototype for a functional SDF DSL with a `lane` CLI that compiles object expressions into GLSL.

Current slice:

- pre-registered `Ball3D(r=...)`, `Simplex3D(size=...)`, `Halfspace3D(n=..., h=...)`, and `Torus3D(major=..., minor=...)` primitives
- pre-registered 2D `Box2D`, `Segment2D`, `Triangle2D`, `Polygon2D`, and `Point2D` primitives in the XY plane
- pre-registered `SmoothUnion(k)` operator
- value functions `sin`, `cos`, `pow2`
- unary function composition with `f @ g`, meaning `x |-> f(g(x))`
- `vec2` and `vec3` tuple literals in value expressions
- `Obj3 + vec3` placement sugar
- top-level `in`, `func`, typed object bindings, and `out`
- named declarations use `type name = value` syntax, for example `Obj3 A = Ball3D(r=3)`
- output uses `out: value`, for example `out: C`
- polygons use `Polygon2D(points=((0, 0), (2, 0), (2, 1), (0, 1)))` and currently support up to 16 vertices

Example input lives in `test.sdfdsl`.

Run it with:

```sh
cargo run --bin lane -- test.sdfdsl
```

Run tests with:

```sh
cargo test
```

List the known primitives, including each generated SDF signature and parameter domain, with:

```sh
cargo run --bin lane -- --list-primitives
```

Inspect preregistered GLSL support objects with:

```sh
cargo run --bin lane -- --list-preregistered
cargo run --bin lane -- --show-preregistered ParamBall3D
cargo run --bin lane -- --show-preregistered sdf0_Ball3D
```

Show CLI usage with:

```sh
cargo run --bin lane -- --help
```
