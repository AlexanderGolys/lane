# lane

Lane is a small Rust prototype for a DSL that describes signed distance field
(SDF) scenes and emits GLSL. A Lane program is a sequence of declarations that
introduce external inputs, value functions, object bindings, and one final
`generate` expression.

The workspace contains the root `lane` compiler crate and a separate
`crates/lane-lsp` crate for the Language Server Protocol server. Compiler
internals are split by pass under `src/`: parser, typechecker, emitter, and
registry.

## Install

Install Rust with Cargo first. From a clone of this repository:

```sh
cargo build --workspace
cargo test --workspace
```

To install the CLI binary into Cargo's bin directory:

```sh
cargo install --path .
```

To install the LSP binary too:

```sh
cargo install --path crates/lane-lsp
```

During development you can also run both binaries directly from the workspace:

```sh
cargo run -- test.lane
cargo run -p lane-lsp
```

## CLI

```text
lane compiles lane source files into GLSL.

Usage:
  lane [PATH]
  lane -l, --list [NAME]
  lane -l2, --list2d
  lane -l3, --list3d
  lane -lo, --list-objects [NAME]
  lane -pc, --print-completion <bash|zsh|fish>
  lane -h, --help
```

Commands are flag-based. `lane help` is treated as an input path named `help`;
use `lane -h` or `lane --help` for help.

- `lane [PATH]` compiles a `.lane` source file to GLSL. If `PATH` is omitted,
  Lane reads source from stdin.
- `lane -l` or `lane --list` lists all primitive constructors and their GLSL
  parameter structs.
- `lane -l NAME` or `lane --list NAME` shows one primitive's visible parameter
  shape and generated GLSL support code.
- `lane -l2` or `lane --list2d` lists only 2D primitives.
- `lane -l3` or `lane --list3d` lists only 3D primitives.
- `lane -lo` or `lane --list-objects` lists known builtin Lane objects and type
  aliases.
- `lane -lo NAME` or `lane --list-objects NAME` shows one builtin object's GLSL
  implementation.
- List output is syntax-highlighted when stdout is a terminal: function objects
  are blue, type objects and type names are yellow, `Type` is bright yellow,
  `Hom` and `×` are red, and punctuation is white.
- `lane -pc SHELL` or `lane --print-completion SHELL` prints shell completion
  code for `bash`, `zsh`, or `fish`; primitive and builtin object candidates
  are generated from the compiler registry.

Add this directive comment to request a minimal fullscreen fragment shader
wrapper instead of a bare GLSL SDF snippet:

```lane
// fragment-shader: #version 330 core
generate Ball3D(r=1)
```

The wrapper currently requires `scene_sdf(vec3 p)` with no extra Lane inputs.

## LSP

The LSP server provides full-document sync and compile diagnostics by re-running
the Lane compiler when documents are opened, changed, or saved.

Neovim built-in LSP example:

```lua
vim.filetype.add({ extension = { lane = "lane" } })

vim.lsp.config("lane_lsp", {
    cmd = { "cargo", "run", "-p", "lane-lsp" },
    filetypes = { "lane" },
    root_markers = { "Cargo.toml", ".git" },
})

vim.lsp.enable("lane_lsp")
```

## Language Syntax

### File Structure

Lane source is line-oriented. Each non-empty line is one declaration. `//`
starts a comment that runs to the end of the line.

```lane
provided R time
Func(R, R) pulse = pow2 @ sin
Solid ball = Ball3D(r=1 + pulse(time))
generate ball
```

Supported declaration forms:

```lane
provided TYPE name
TYPE name = expression
construct Solid name = object_expression
const Solid name = object_expression
generate object_expression
gen object_expression
```

`provided` declares an external GLSL input. A typed binding declares either a
value binding, a value function, or a `Solid` object binding depending on its
type. `construct` and `const` are only valid for `Solid` bindings and export
stable helper functions named `sdf_name` and `grad_sdf_name` in the generated
GLSL. A program must contain exactly one final output declaration using
`generate` or `gen`.

### Types

Lane has nominal surface types that lower to GLSL scalar, vector, matrix, or SDF
object representations.

| Lane type | Alias | GLSL value |
| --- | --- | --- |
| `Float` | `R` | `float` |
| `Int` | `Z` | `int` |
| `Complex` | `C` | `vec2` |
| `Vec2` | `R2` | `vec2` |
| `Vec3` | `R3` | `vec3` |
| `Vec4` | `R4` | `vec4` |
| `Mat2` | | `mat2` |
| `Mat3` | | `mat3` |
| `Mat4` | | `mat4` |
| `Solid` | | SDF object |

Function types use `Func(input, output)` or `Hom(input, output)`:

```lane
Func(R, R) pulse = sin
provided Hom(R3, R) density
End(R) loop = sin
```

`End(T)` means `Hom(T, T)`. Product types are written with `×`, for example
`Solid × Solid`, and appear in builtin object listings.

### Value Expressions

Value expressions include numbers, identifiers, tuples, function calls,
function composition, and arithmetic.

```lane
R radius = 1.5
R small = 1e-1
R2 uv = (0.5, 1)
R3 offset = (sin(time), cos(time), 0)
Mat3 identity = ((1, 0, 0), (0, 1, 0), (0, 0, 1))
Func(R, R) wobble = pow2 @ sin + .5
```

Operators:

- `-x` for unary negation.
- `+`, `-`, `*`, `/` for supported scalar, complex, vector, and matrix value
  combinations.
- `f @ g` for unary function composition.
- `f(x)` for value function calls.

User-defined function bodies currently support `R` inputs. Inside a
`Func(R, T)` binding, bare unary function identifiers are implicitly applied to
the generated parameter `t`, so `pow2 @ sin + .5` emits a function of `t`.
Provided functions may use other function types, such as `Hom(R3, R)`.

Tuple rules:

- `(x, y)` creates `R2` or `C` depending on the expected type.
- `(x, y, z)` creates `R3` when all three elements are scalar values.
- `((...), (...), (...))` creates `Mat3` from three `R3` rows.

### Object Expressions

Objects denote SDFs. Primitive constructors, object operators, and ambient
actions produce `Solid` values.

```lane
Solid a = Ball3D(r=2)
Solid b = Box3D(1, .5, .25) + (2, 0, 0)
Solid c = SmoothUnion(.2)(a, b)
generate c
```

Primitive arguments can be named or positional in field order:

```lane
Ball3D(r=1)
Ball3D(1)
Box3D(a=1, b=2, c=3)
Box3D(1, 2, 3)
```

Object actions:

- `Solid + R3` translates an object in ambient 3D space.
- `Mat3 * Solid` applies an orthogonal linear action to an object.

2D primitives live in the XY plane but still produce `Solid` objects in the
current compiler slice. Operators such as `Union`, `Intersection`, and `Xor`
accept two or more object arguments and are lowered to balanced binary GLSL
calls. Other binary operators require exactly two object arguments.

Special primitive forms:

- `Segment2D(length=2)`, `Segment2D(2)`, and `Segment3D(2)` create centered
  segments.
- `Polygon2D(points=((0, 0), (1, 0), (0, 1)))` accepts 3 to 16 `R2` vertices.
- `Plane3D(n=..., origin=...)` lowers to `ParamPlane3D { vec3 n; float h; }`.

### Emitted GLSL

Every compilation emits:

- support structs and helper functions for only the used primitives, operators,
  and value functions;
- user value helper functions named `dsl_name`;
- `float scene_sdf(vec3 p, ...)`;
- `vec3 scene_grad(vec3 p, ...)`, computed by finite differences.

Generated local names are renamed when they would collide with user names such
as `p`, `eps`, `dx`, `dy`, or `dz`.

## Examples

### Minimal Sphere

```lane
generate Ball3D(r=1)
```

Run it:

```sh
printf 'generate Ball3D(r=1)\n' | cargo run --
```

### Animated Union

```lane
provided R time
provided Func(R, R3) center

Func(R, R) pulse = pow2 @ sin + .25
Solid a = Ball3D(r=1 + pulse(time))
Solid b = Box3D(.75, .5, .5) + center(time)
Solid scene = SmoothUnion(.2)(a, b)

generate scene
```

### Constructed Helper

```lane
provided R radius
construct Solid shell = Ball3D(r=radius) + (1, 0, 0)
generate shell
```

This exports `sdf_shell(...)` and `grad_sdf_shell(...)` in addition to the final
`scene_sdf(...)` and `scene_grad(...)`.

### 2D Profile Lifted Into 3D

```lane
Solid profile = Triangle2D(p0=(0, -.5), p1=(.5, 0), p2=(0, .5))
Solid lathe = Revolution(1.25)(profile)
Solid slab = Extrusion(.2)(Box2D(1, .5)) + (0, 0, 1)
generate Union(lathe, slab)
```

The repository includes `test.lane` as a compact sample and `showcase.lane` as a
larger scene using most registered primitives and operators.

## Registered Objects

### 3D Primitives

| Object | Fields |
| --- | --- |
| `Ball3D` | `r: R` |
| `Box3D` | `a: R`, `b: R`, `c: R` |
| `Halfspace3D` | `n: R3`, `h: R` |
| `Line3D` | `x0: R3`, `dir: R3` |
| `Plane3D` | `n: R3`, `origin: R3` |
| `Quad3D` | `p1: R3`, `p2: R3`, `p3: R3`, `p4: R3` |
| `Segment3D` | `a: R3`, `b: R3` |
| `Simplex3D` | `p0: R3`, `p1: R3`, `p2: R3`, `p3: R3` |
| `Torus3D` | `major: R`, `minor: R` |
| `Triangle3D` | `p1: R3`, `p2: R3`, `p3: R3` |

### 2D Primitives

| Object | Fields |
| --- | --- |
| `Ball2D` | `r: R` |
| `Box2D` | `a: R`, `b: R` |
| `Point2D` | `at: R2` |
| `Polygon2D` | `points: R2 list` |
| `Quad2D` | `p1: R2`, `p2: R2`, `p3: R2`, `p4: R2` |
| `Segment2D` | `a: R2`, `b: R2` |
| `Triangle2D` | `p0: R2`, `p1: R2`, `p2: R2` |

### Object Operators

| Object | Type | Notes |
| --- | --- | --- |
| `Union` | `Hom(Solid × Solid, Solid)` | Associative, accepts two or more solids. |
| `Intersection` | `Hom(Solid × Solid, Solid)` | Associative, accepts two or more solids. |
| `Difference` | `Hom(Solid × Solid, Solid)` | Binary set difference. |
| `Xor` | `Hom(Solid × Solid, Solid)` | Associative, accepts two or more solids. |
| `SmoothUnion` | `Hom(R, Hom(Solid × Solid, Solid))` | Curried by smoothing radius `k`. |
| `SmoothIntersection` | `Hom(R, Hom(Solid × Solid, Solid))` | Curried by smoothing radius `k`. |
| `SmoothDifference` | `Hom(R, Hom(Solid × Solid, Solid))` | Curried by smoothing radius `k`. |
| `SmoothXor` | `Hom(R, Hom(Solid × Solid, Solid))` | Curried by smoothing radius `k`. |
| `Revolution` | `Hom(R, Hom(Solid, Solid))` | Lifts a 2D profile with `vec2(length(p.xz) - offset, p.y)`. |
| `Extrusion` | `Hom(R, Hom(Solid, Solid))` | Lifts a 2D profile along the `z` axis. |

### Value Functions And Type Aliases

| Object | Type |
| --- | --- |
| `C` | `Type` |
| `E2` | `Type` |
| `E3` | `Type` |
| `H` | `Type` |
| `pow2` | `Hom(R, R)` |
| `ccos` | `Hom(C, C)` |
| `ccosh` | `Hom(C, C)` |
| `cexp` | `Hom(C, C)` |
| `cinv` | `Hom(C, C)` |
| `clog` | `Hom(C, C)` |
| `csin` | `Hom(C, C)` |
| `csinh` | `Hom(C, C)` |
| `csqrt` | `Hom(C, C)` |
| `ctan` | `Hom(C, C)` |
| `ctanh` | `Hom(C, C)` |

Raw GLSL `sin` and `cos` are also available as `Hom(R, R)` value functions, but
they are not listed by `lane --list-objects` because they do not require custom
support code.

### Differential Builtins

These are available in value expressions but are omitted from `--list-objects`
because they lower directly during emission:

| Object | Shape |
| --- | --- |
| `derivative(eps)(f)(x)` | Central derivative for `Hom(R, R)`. |
| `partialX(eps)(f)(p)` | X partial derivative for `Hom(R3, R)`. |
| `partialY(eps)(f)(p)` | Y partial derivative for `Hom(R3, R)`. |
| `partialZ(eps)(f)(p)` | Z partial derivative for `Hom(R3, R)`. |
| `directionalDerivative(eps)(dir)(f)(p)` | Directional derivative for `Hom(R3, R)`. |
| `gradient(eps)(f)(p)` | Gradient of `Hom(R3, R)`, returns `R3`. |
| `divergence(eps)(f)(p)` | Divergence of `Hom(R3, R3)`, returns `R`. |

Use the CLI for the authoritative generated GLSL definitions:

```sh
lane --list Ball3D
lane --list-objects Revolution
```
