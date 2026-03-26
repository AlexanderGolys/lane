# lane

A small Rust prototype for the lane DSL that compiles object expressions into GLSL.

Current slice:

- pre-registered 3D `Ball3D(r=...)`, `Box3D(a=..., b=..., c=...)`, `Triangle3D(p1=..., p2=..., p3=...)`, `Quad3D(p1=..., p2=..., p3=..., p4=...)`, `Plane3D(n=..., origin=...)`, `Line3D(x0=..., dir=...)`, `Simplex3D(p0=..., p1=..., p2=..., p3=...)`, `Halfspace3D(n=..., h=...)`, `Segment3D(a=..., b=...)`, and `Torus3D(major=..., minor=...)` primitives
- pre-registered 2D `Ball2D(r=...)`, `Box2D(a=..., b=...)`, `Segment2D`, `Triangle2D`, `Quad2D`, `Polygon2D`, and `Point2D` primitives in the XY plane
- pre-registered `Union`, `Intersection`, `Difference`, `Xor`, and smooth parametric variants such as `SmoothUnion(k)` and `SmoothDifference(k)`
- pre-registered `Revolution(offset)` and `Extrusion(height)` object lifts from 2D XY-plane fields into `Obj3`
- associative binary operators such as `Union`, `Intersection`, and `Xor` accept any arity `>= 2` and are lowered to balanced binary calls
- custom value functions such as `pow2` and holomorphic `Complex -> Complex` helpers including `cexp`, `clog`, `csqrt`, `csin`, `ccos`, `ctan`, `csinh`, `ccosh`, `ctanh`, and `cinv`
- unary minus in value expressions emits direct negative GLSL terms instead of `(0.0 - x)` wrappers
- differential builtin objects such as `derivative`, `partialX`, `partialY`, `partialZ`, `directionalDerivative`, `gradient`, and `divergence`
- unary function composition with `f @ g`, meaning `x |-> f(g(x))`
- `construct Obj3 name = expr` exports stable helper names `sdf_name` and `grad_sdf_name` without changing the scene semantics, and `const` is accepted as an alias
- `Complex`, `Vec2`, `Vec3`, and nested-row `Mat3` surface types in value expressions, with aliases such as `C`, `R`, `R2`, and `R3`; complex values still lower to GLSL `vec2`
- ambient object actions with `Obj3 + R3` translation sugar and `Mat3 * Obj3` orthogonal action
- 2D and 3D primitives stay distinct semantic families even though the current object surface type is `Obj3`
- top-level `provided`, `func`, typed object bindings, `construct`, and `generate`
- C-style line comments starting with `//`
- primitive constructor arguments can be passed positionally in field order, e.g. `Box2D(2, 1)`
- `Segment2D` and `Segment3D` also accept centered length constructors as `Segment2D(length=2)`, `Segment2D(2)`, and `Segment3D(2)`
- named declarations use `type name = value` syntax, for example `Obj3 A = Ball3D(r=3)`
- output uses `generate value`, for example `generate C`, and `gen value` is accepted as a shorthand alias
- emitted GLSL always includes both `scene_sdf` and a numerically approximated `scene_grad`
- emitted GLSL renames generated local identifiers when they would collide with user-defined value names such as `p` or `eps`
- polygons use `Polygon2D(points=((0, 0), (2, 0), (2, 1), (0, 1)))` and currently support up to 16 vertices
- `Plane3D(n=..., origin=...)` lowers to a local `ParamPlane3D { vec3 n; float h; }` representation before GLSL emission
- `Revolution(offset)(shape)` lowers with the standard `vec2(length(p.xz) - offset, p.y)` profile, while `Extrusion(height)(shape)` uses the standard exact extrusion formula along `z`

Example inputs live in `test.lane` and `showcase.lane`. The showcase file combines all current primitives, object operators, value functions, and ambient action sugars in one scene.

Run it with:

```sh
cargo run -- test.lane
cargo run -- showcase.lane
```

Add a directive comment such as `// fragment-shader: #version 330 core` to wrap the emitted scene GLSL in a minimal fullscreen fragment shader for the no-extra-input case.

Run tests with:

```sh
cargo test
```

Run the minimal Language Server Protocol server with:

```sh
cargo run --bin lane-lsp
```

The LSP currently provides full-document sync and basic compile diagnostics by re-running the Lane compiler on open, change, and save.

To hook the server up to Neovim's built-in LSP client, add a config such as:

```lua
vim.filetype.add({ extension = { lane = "lane" } })

vim.lsp.config("lane_lsp", {
    cmd = { "cargo", "run", "--bin", "lane-lsp" },
    filetypes = { "lane" },
    root_markers = { "Cargo.toml", ".git" },
})

vim.lsp.enable("lane_lsp")
```

List the known primitives with their generated GLSL parameter structs with:

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

List known builtin Lane objects with their Lane types with:

```sh
cargo run -- --list-objects
cargo run -- -lo
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

CLI commands are flag-based: `lane help` is treated as an input path named `help`, while `lane -h` and `lane --help` show usage. Primitive listings print one primitive name per block followed by the generated GLSL parameter struct when one exists, with the `Polygon2D` fallback still showing its Lane-level field shape because it has no generated struct. `lane --list NAME` prints the generated GLSL for that primitive with ANSI syntax highlighting on interactive terminals, `lane --list-objects` prints interpreter-known custom Lane objects as `name: type` using curried `Hom(...)` notation, excludes raw GLSL builtins such as `sin`, and omits trivial derived combinators such as `gradient`, and 2D primitives expose local `Vec2` evaluators while 3D primitives expose local `Vec3` evaluators.
