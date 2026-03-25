# lane

A small Rust prototype for the lane DSL that compiles object expressions into GLSL.

Current slice:

- pre-registered `Ball3D(r=...)`, `Simplex3D(p0=..., p1=..., p2=..., p3=...)`, `Halfspace3D(n=..., h=...)`, and `Torus3D(major=..., minor=...)` primitives
- pre-registered 2D `Box2D(a=..., b=...)`, `Segment2D`, `Triangle2D`, `Polygon2D`, and `Point2D` primitives in the XY plane
- pre-registered `Union`, `Intersection`, `Difference`, `Xor`, and smooth parametric variants such as `SmoothUnion(k)` and `SmoothDifference(k)`
- value functions `sin`, `cos`, `pow2`
- unary function composition with `f @ g`, meaning `x |-> f(g(x))`
- `vec2`, `vec3`, and nested-row `mat3` tuple literals in value expressions
- ambient object actions with `Obj3 + vec3` translation sugar and `mat3 * Obj3` orthogonal action
- 2D and 3D primitives stay distinct semantic families even though the current object surface type is `Obj3`
- top-level `in`, `func`, typed object bindings, and `out`
- C-style line comments starting with `//`
- primitive constructor arguments can be passed positionally in field order, e.g. `Box2D(2, 1)`
- named declarations use `type name = value` syntax, for example `Obj3 A = Ball3D(r=3)`
- output uses `out: value`, for example `out: C`
- polygons use `Polygon2D(points=((0, 0), (2, 0), (2, 1), (0, 1)))` and currently support up to 16 vertices

Example inputs live in `test.lane` and `showcase.lane`. The showcase file combines all current primitives, object operators, value functions, and ambient action sugars in one scene.

Run it with:

```sh
cargo run -- test.lane
cargo run -- showcase.lane
```

Run tests with:

```sh
cargo test
```

List the known primitives with their Lane-level field shapes with:

```sh
cargo run -- --list
cargo run -- -l
```

Show the full field shape, struct definition, and evaluator definition for one primitive with:

```sh
cargo run -- --list Box2D
cargo run -- -l Box2D
```

List only 2D or only 3D primitives with:

```sh
cargo run -- --list2d
cargo run -- -l2
cargo run -- --list3d
cargo run -- -l3
```

List known predefined GLSL functions or generated parameter structs with:

```sh
cargo run -- --list-functions
cargo run -- -lf
cargo run -- --list-types
cargo run -- -lt
```

Print shell completion scripts with:

```sh
cargo run -- --print-completion bash
cargo run -- -pc bash
cargo run -- --print-completion zsh
cargo run -- --print-completion fish
```

Show CLI usage with:

```sh
cargo run -- --help
```

CLI commands are flag-based: `lane help` is treated as an input path named `help`, while `lane -h` and `lane --help` show usage. Primitive listings show the Lane-level field shape, `lane --list NAME` prints the generated GLSL for that primitive, and 2D primitives expose local `vec2` evaluators while 3D primitives expose local `vec3` evaluators.
