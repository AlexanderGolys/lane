# lane

Lane is a small DSL for signed distance field (SDF) scenes. It typechecks Lane
source and emits GLSL containing the scene SDF, gradient helpers, and only the
support code needed by the program.

## Install

Install Rust with Cargo first. From this repository:

```sh
cargo build --workspace
cargo test --workspace
```

Install the compiler CLI:

```sh
cargo install --path .
```

Install the LSP server:

```sh
cargo install --path crates/lane-lsp
```

During development:

```sh
cargo run -- test.lane
cargo run -- test.lane test.glsl
cargo run -p lane-lsp
```

## CLI

```text
lane compiles lane source files into GLSL.

Usage:
  lane [SOURCE [TARGET]] [--show]
  lane SOURCE [--frag=FRAG] [--vert=VERT] [--version=VERSION] [--target=opengl|vulkan]
  lane SOURCE [--frag-spv=SPV] [--vert-spv=SPV]
  lane repl
  lane preview SOURCE
  lane list [NAME]
  lane list 2d
  lane list 3d
  lane list all
  lane -pc, --print-completion <bash|zsh|fish>
  lane -h, --help
```

- `lane [SOURCE]` compiles Lane to GLSL on stdout. Without `SOURCE`, Lane opens
  the interactive shell when stdin is a terminal and reads source from stdin
  otherwise.
- `lane repl` opens the interactive shell explicitly. The shell accumulates
  valid Lane declarations, rejects `#module`, and emits GLSL when a submitted
  line is a `const` declaration. After the first emission, later `const` lines
  show only GLSL lines added since the previous emission. The REPL displays
  submitted Lane code, REPL messages, generated GLSL, and the current input
  linearly in one bottom-anchored, consistently padded transcript, with one
  character of inner left padding and different background colors for user code
  and output code. The current input block leaves one blank terminal row above
  and below it, and empty input shows gray placeholder text. Submitted Lane entries
  include source line numbers in their transcript gutter, while the current
  input remains unnumbered and widens its blank gutter to stay aligned to the
  same source column as line numbers grow. Matching completions appear as gray
  inline hint text after the current token without changing the submitted input.
  Adjacent submitted Lane entries render inside one shared feed box, while
  separate boxes still have a blank row between them. Errors are decided after
  submission: only the
  submitted Lane block for that failed submission is marked red, and the error
  message is shown above that code inside the same red block with aligned left
  padding. Failing submitted code uses an error marker in the gutter instead
  of source line numbers, and inline REPL error messages omit compiler line
  prefixes. Shell commands are
  recognized only at the start of a line and render as plain one-line messages
  (the initial welcome banner also renders as a plain line without box margins)
  that stay attached to their command response while leaving one blank row
  before the next submitted code block. REPL commands ignore trailing spaces:
  `/help`, `/help   `, and similar forms behave the same.
  Command lines and error boxes keep a one-row vertical gap.
  `/help` prints REPL command help,
  `/info` shows loaded modules, used directives, and provided objects, `/show`
  opens a native Vulkan preview window for the current session (preview failures
  are shown as REPL error blocks without exiting the shell), `/split` toggles
  a split view where submitted Lane code and generated GLSL are rendered in
  separate panes, `/clear` clears the transcript but keeps the session,
  `/restart` starts from an empty session, and `/exit` leaves the shell.
  Toggling `/split` off restores the full linear transcript, including generated
  GLSL chunks. Clicking a submitted Lane entry or its generated GLSL highlights
  both parts of that submission. Enter submits the current input, Shift-Enter
  (or Alt-Enter fallback) inserts a newline when supported by the terminal, Up and Down recall submitted
  input history across sessions, Tab completes to the longest unambiguous prefix using `lane-lsp` language items
  for Lane source and REPL command items for slash commands, Ctrl-F formats the
  current input, and Ctrl-C exits.
- `lane SOURCE TARGET` writes generated GLSL to `TARGET`.
- `lane --show SOURCE TARGET`, `lane -s SOURCE TARGET`, or
  `lane SOURCE TARGET --show` writes `TARGET` and prints the GLSL.
- `lane SOURCE --frag=PATH --vert=PATH` writes complete preview fragment and
  vertex shaders. `--target=vulkan` emits Vulkan GLSL.
- Auto-generated preview shading (when `main` is not explicitly defined) needs
  a scene object and material lookup: define `const Object scene = ...` and
  define `const Hom(R3, Material) scene_material = ...`.
  When these requirements are missing, preview generation reports a clear
  requirement error instead of failing with a lower-level unknown-function
  message.
- `lane SOURCE --frag-spv=PATH --vert-spv=PATH` writes Vulkan SPIR-V shaders
  through `glslc`; intermediate files are placed under `target/lane-preview`.
- `lane preview SOURCE` opens the native Vulkan previewer. It uses `glslc`,
  FIFO presentation, and a conservative frame cap.
- `lane list` lists builtin objects, type aliases, and algebraic categories.
- `lane list NAME` shows one builtin object's type and support body.
- `lane list 2d` and `lane list 3d` list only 2D or 3D primitives.
- `lane list all` lists every builtin item on one line, including primitive
  constructors, GLSL functions, type aliases, object operators, and algebraic
  categories. Repeated scalar/vector overload families are compacted with `Rn`,
  matrix families use `Mat{n}x{m}`, and algebraic helper operations such as
  component-wise matrix multiplication are omitted from this broad list.
- `lane -pc SHELL` prints completion code for `bash`, `zsh`, or `fish`.
- `lane help` is treated as an input path. Use `lane -h` or `lane --help`.
- CLI failures are printed on stderr with an error type prefix such as
  `lane::Error:` or `std::io::Error:`. In an interactive terminal, the whole
  diagnostic is colored red.

## LSP

The LSP server provides diagnostics by compiling the whole document after open,
change, and save events. Diagnostics resolve `#import` paths relative to the
open file, so local modules work the same way in the editor and the CLI. The
server also provides formatting plus basic completion and hover entries for Lane
keywords, built-in modules, primitive constructors, type aliases, categories,
and built-in functions. Standalone type completions stay concrete (for example,
`R`, `R2`, `R3`) and avoid generic placeholders such as `R{n}` that are not
valid as direct `provided` type declarations. The REPL uses the same completion, formatting, and
submitted-error handling.

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

When using the bundled `tree-sitter-lane` Neovim plugin, `require("lane").setup()`
registers the filetype, tree-sitter parser/query, and LSP config. Pass
`{ lsp = false }` to disable the LSP hookup, or pass `{ lsp = { cmd = { ... } } }`
to override the server command.

## Program Structure

Lane source is line-oriented. Each non-empty, non-comment line is one
declaration. `const` emits a value, function, or object even if nothing later
references it. For objects, `const` exports object helper functions.
`const Object output` also keeps the legacy `scene_sdf` and `scene_grad`
entrypoints, but a program does not need an explicit scene.

```lane
provided R time
Func(R, R) pulse = pow2 @ sin
Object shape = Ball3D(r=1 + pulse(time))
const Object output = shape
```

Line comments start with `//`:

```lane
provided R time // animation clock
// final scene
const Object output = Ball3D(r=1 + time)
```

## Directives

Directives are line-oriented declarations that start with `#`. They must appear
before all non-directive declarations.

### `#2D`

`#2D` switches the ambient SDF space from 3D to 2D. It must appear before other
declarations. Under `#2D`, unqualified `Object` means `Object2D`, 3D primitives
and 3D object operators are rejected, and generated entry points use `vec2`.

```lane
#2D
R2 offset = [1, 2]
Object shape = Box2D(a=2, b=1) + offset
const Object output = shape
```

Other `#...` lines are not language directives; the compiler treats unknown
directives as errors.

### `#prec VALUE`

`#prec` sets the finite-difference epsilon used by generated SDF gradients and
all differential builtins. The default is `0.01`. The value must be a positive
float literal.

```lane
#prec 0.002
provided Hom(R3, R) density
provided R3 p
R3 normal = gradient(density)(p)
Func(R, R) slope = grad(sin)
const Object output = Ball3D(r=slope(0) + density(normal))
```

Differential operators do not take per-call epsilon arguments; use `#prec` when
a program needs a different precision.

### `#import NAME`

Imports a Lane module from the local `modules/` directory or the installed Lane
module directory. Imported modules must start with `#module`, may define Lane
helpers and raw GLSL functions, and cannot contain `provided` declarations. The
shipped modules include `std` and `raytracing`.

```lane
#import std
R z = projection_1([3, 4])
```

The `raytracing` module provides the `Ray`, `Hit`, `Material`, `Camera`,
`RaytraceConfig`, and `RaycolorConfig` product types plus helpers for preview
shaders. `raycolor_from_hit_with` contains the raw GLSL reflection loop and is
generic over the hit shading callbacks. Material lookup and material shape stay
in Lane: compose a hit projection with any point-to-material function and
material accessors. The default `Material` type ships with `material_color`,
`material_emission`, and `material_reflectiveness` accessors. `shade` composes a
screen-coordinate ray function with a ray-color function; its ray color input is
generic, with the preview `R4` output context deducing the concrete color type.

```lane
#import raytracing
const Hom(R2, Ray) rays = camera_ray(camera)
const Hom(Hit, R3) color_at = material_color @ material @ hit_position
const Hom(Hit, R3) emission_at = material_emission @ material @ hit_position
const Hom(Hit, R) reflectiveness_at = material_reflectiveness @ material @ hit_position
const Hom(Ray, R3) colors = raycolor_from_hit_with(default_raycolor_config, ambientColor, hit, color_at, emission_at, reflectiveness_at)
const Hom(R2, R4) pixels = shade(rays, colors)
const Hom(*, *) main = fragment_main(pixels)
```

## Declarations

### `provided TYPE name`

Declares an external GLSL value, such as a uniform or global constant. Generated
SDF helpers and scene entrypoints reference provided values by name; they do not
thread provided values through helper parameters.

```lane
provided R time
provided R3 center
const Object output = Ball3D(r=1 + time) + center
```

### `provided CATEGORY TypeName`

Declares an external nominal type. Lane knows its algebraic category, but the
host GLSL environment must provide the representation and operations.

```lane
provided Grp G
provided G a
provided G b
provided Hom(G, R) measure
R radius = measure(a * b)
const Object output = Ball3D(r=radius)
```

This emits calls such as `mult_G(a, b)`. For neutral literals, Lane expects
globals such as `zero_G`, `one_G`, and `e_G` when those operations are needed.

### `[const] CATEGORY TypeName = TYPE x TYPE <field, field>`

Constructs a nominal product type and emits a GLSL struct. Every component must
satisfy the declared category or a subcategory. `DivRing` products are rejected.

```lane
Grp G = Isom3 x Isom2 <m, n>
provided G a
provided G b
G product = a * b
const Object output = Ball3D(r=1)
```

Emitted support includes:

```glsl
struct G {
    Isom3 m;
    Isom2 n;
};

G mult_G(G a, G b) {
    return G(mult_Isom3(a.m, b.m), mult_Isom2(a.n, b.n));
}
```

DivRing names are optional:

```lane
Ab Pair = R2 x R3
Pair p = Pair([1, 2], 0)
```

Default field names are `x`, `y`, `z`, `w` up to four components and `x0`,
`x1`, ... after that. Positional aliases are accepted for default fields:
`x`, `y`, `z`, `w` and `x0`, `x1`, `x2`, `x3` refer to the same first four
components when present.

Without `const`, product operations are emitted only when used. With `const`,
all operations for the category are emitted:

```lane
const Grp G = Isom3 x Isom2
```

### `TYPE name = expression`

Declares a typed value, function, or object binding. The right hand side must
match the annotated type.

```lane
R radius = 1.5
R3 offset = [1, 0, 0]
Object ball = Ball3D(r=radius) + offset
Func(R, R) wobble = pow2 @ sin
```

### `name = expression`

Declares a local binding with an inferred type. Inference is accepted only when
the expression has one clear type.

```lane
radius = 1 + 2
R3 offset = [1, 0, 0]
shape = Ball3D(r=radius) + offset
const Object output = shape
```

### `construct Object name = object_expression`

Exports a reusable SDF helper named `sdf_name` and a gradient helper named
`grad_sdf_name`. Later uses call the helper instead of inlining the object.

```lane
provided R radius
R3 offset = [1, 0, 0]
construct Object shell = Ball3D(r=radius) + offset
const Object output = shell
```

Object bindings also expose function getters:

```lane
Object shell = Ball3D(r=2)
R d = shell.sdf([0, 0, 0])
R3 normal = shell.grad([0, 0, 0])
R3 finite_diff_normal = gradient(shell.sdf)([0, 0, 0])
const Object output = Ball3D(r=d + length(normal + finite_diff_normal))
```

`obj.sdf` has type `Hom(R3, R)` for 3D objects and `Hom(R2, R)` for 2D
objects. `obj.grad` returns the matching ambient vector type. Using either
getter emits the same helper functions as `construct`, even for a plain
`Object` binding. In a generated value/function expression, bare object getters
lift over the ambient point:

```lane
#2D
const rect = Box2D(a=1, b=2)
R4 tint = [.5, .5, .9, 1]
const color = rect.sdf * tint
```

Here `color` is emitted as `Hom(R2, R4)`.

### `const name = expression`

Emits an inferred value, function, or object. Object-valued declarations emit
SDF helpers; function-valued declarations emit GLSL helper functions; values
that can be represented as top-level GLSL constants are emitted as `const`
globals.

```lane
const radius = 1.5
const pulse = sin
const shell = Ball3D(r=radius)
```

### `const Object name = object_expression`

Exports a reusable SDF helper like `construct`. For 3D objects, Lane emits
`sdf_name` and `grad_sdf_name`; for 2D objects, Lane emits `sdf_name`. The name
does not need to be `output`.

```lane
const Object shell = Ball3D(r=2)
const Object scene = Ball3D(r=1)
```

## Types

Lane type names are nominal and case-sensitive.

| Lane type | Alias | GLSL value |
| --- | --- | --- |
| `Bool` | | `bool` |
| `Float` | `R` | `float` |
| `Int` | `Z` | `int` |
| `Complex` | `C` | `vec2` |
| `H` | | `vec4` |
| `Vec2` | `R2` | `vec2` |
| `Vec3` | `R3` | `vec3` |
| `Vec4` | `R4` | `vec4` |
| `Mat2`, `Mat3`, `Mat4` | | `mat2`, `mat3`, `mat4` |
| `MatNxM` | `N,M in {2,3,4}` | GLSL `matMxN` |
| `Isom2` | | 2D isometry struct |
| `Isom3` | | 3D isometry struct |
| `Object` | `Object3D` | ambient 3D SDF object, or `Object2D` under `#2D` |
| `Object2D` | | 2D SDF object |

### `Func(input, output)`

Function type.

```lane
Func(R, R) pulse = sin
Func(R, R3) path = (sin, cos, 0)
```

User-defined function bodies support value inputs such as `R`, `R2`, and `R3`.
Inside a `Func(R, T)` binding, bare unary functions are implicitly applied to
the generated parameter `t`:

```lane
Func(R, R) wobble = pow2 @ sin + .5
```

### `Hom(input, output)`

Function type alias used for mathematical maps and external functions.

```lane
provided Hom(R3, R) density
provided Hom(R3 x R3, R3) cross
R3 c = cross([1, 0, 0], [0, 1, 0])
```

If the domain is a product type, function calls use multiple positional
arguments. `R x R` is an explicit product domain with two real-valued
arguments; `R2` is a Euclidean vector type. Lane may use the GLSL isomorphism
between them for selected built-in interop, but it does not collapse one syntax
into the other in user function signatures.

### `End(T)`

Endomorphism type. Equivalent to `Hom(T, T)`.

```lane
provided End(R) loop
R y = loop(0.5)
```

### `Array(T)`

Array type. Array values are fixed-size at construction time, and indexing uses
integer indices.

```lane
Array(R) weights = Array(1, 2, 3)
R first = weights[0]
R count = size(weights)
```

Use `concat(left, right)` to concatenate arrays:

```lane
Array(R) a = Array(1, 2)
Array(R) b = Array(3)
Array(R) c = concat(a, b)
```

### Product Types In Type Positions

`T × U` and spaced ASCII `T x U` create anonymous product domains, mainly for
function types.

```lane
provided Hom(R3 × R3, R3) cross_unicode
provided Hom(R3 x R3, R3) cross_ascii
```

ASCII `x` is a product separator only when surrounded by whitespace.

`A^n` is shorthand for an `n`-fold product `A × A × ... × A` when `n` is a
positive integer.

```lane
const Hom(R^3, R) sum3 = v |-> v.x + v.y + v.z
Set Triple = R^{3}
```

### Parenthesized Types

Parentheses group types.

```lane
provided Hom((R3 x R3), R3) cross
```

## Categories

Categories classify which algebraic operations are available.

| Category | Meaning | Operations used by Lane |
| --- | --- | --- |
| `Set` | plain values | no algebraic operations |
| `Ab` | additive abelian group | `0`, `+`, `-` |
| `Mon` | monoid | `1`, `*` |
| `Grp` | group | `e`, `*`, inverse helpers |
| `Ring` | ring | `0`, `1`, `+`, `-`, `*` |
| `DivRing` | division ring | ring operations and `/` |
| `VectR` | real vector space | `0`, `+`, `-`, scalar `*` and `/` |
| `RAlg` | real algebra | ring operations and scalar multiplication |

The category order used by Lane is:

```text
DivRing < Grp < Mon < Set
DivRing < Ring
Ring < Mon
Ring < Ab
RAlg < Ring
RAlg < VectR < Ab
RAlg < Mon
```

Category names are reserved and cannot be used as value type names.

## Value Expressions

### Numbers

Numbers are `R` by default unless an expected type casts them.

```lane
R a = 1
R b = .5
R c = 1e-2
Z i = 3
```

Generated GLSL writes float literals with an `f` suffix in normal compiler
output, for example `1.0f`.

### Identifiers

Use previously declared values, functions, objects, or provided inputs.

```lane
provided R radius
provided R x, y, z
Object ball = Ball3D(r=radius)
const Object output = ball
```

### Neutral Literals: `0`, `1`, `e`

In expected-type contexts, Lane casts neutral literals:

```lane
R3 origin = 0
Mat3 identity_matrix = e
Isom3 identity_motion = Isom3(e, 0)
```

- `0` casts to the additive neutral element.
- `1` casts to the multiplicative neutral element.
- `e` casts to group or square-matrix identity elements.
- Matrix identities can also be written explicitly as `I{n}` or `eye{n}`.

When overloads conflict, Lane prefers the uncast numeric type if that resolves
the call; otherwise an explicit expected type may be required.

`Bool` values cast to `Z` or `R` when a numeric type is expected. `true` becomes
`1` and `false` becomes `0`; this applies to literals, variables, and function
results.

```lane
provided Bool enabled
R weight = enabled
Z count = enabled
```

### Product And Vector Literals

Parentheses group expressions and are used for tuple-shaped function products.
Explicit product domains such as `R x R` are called with multiple positional
arguments. Brackets construct vectors, complex numbers, quaternions, and
matrices when a vector or matrix type is expected.

```lane
R2 uv = [0.5, 1]
C z = [1, 0]
R3 p = [1, 2, 3]
H q = [1, 0, 0, 0]
Mat3 m = [[1, 0, 0], [0, 1, 0], [0, 0, 1]]
```

Matrix names use row-by-column shape. GLSL matrix constructors are emitted in
the correct column-major form.

### Arrays

Array values use `Array(...)`. All elements must have the same type. Brackets
are reserved for vector and matrix literals.

```lane
Array(R3) points = Array([0, 0, 0], [1, 0, 0])
R3 first = points[0]
```

### Calls

Function calls use parentheses.

```lane
R y = sin(time)
R z = pow2(y)
R3 c = cross([1, 0, 0], [0, 1, 0])
```

Operators can be referenced as binary functions with `&`. Operator references
use the same type deduction and emission as infix syntax. Their natural domain
is an explicit two-argument product such as `R x R`.

```lane
R sum = &+(x, y)
Bool ordered = &<(x, y)
const Hom(R x R, R) wave = sin @ &+
```

Constructor calls use the same syntax:

```lane
Isom3 g = Isom3(e, [1, 2, 3])
Pair p = Pair([1, 2], 0)
```

### Named Arguments

Primitive constructors accept named arguments.

```lane
Ball3D(r=1)
Box3D(a=1, b=2, c=3)
Triangle2D(p0=[0, 0], p1=[1, 0], p2=[0, 1])
```

### Positional Arguments

Primitive constructors also accept positional arguments in field order.

```lane
Ball3D(1)
Box3D(1, 2, 3)
Segment3D([0, 0, 0], [1, 0, 0])
```

Named and positional arguments cannot be mixed in one call.

### Unary `-`

Numeric negation.

```lane
R x = -1
R3 p = [-1, -2, -3]
```

### Binary `+` And `-`

Addition and subtraction for supported additive types. On objects, `+` with a
vector translates the object.

```lane
R a = 1 + 2
Bool toggled = true + false
R3 p = [1, 0, 0] - [0, 1, 0]
Object moved = Ball3D(r=1) + [2, 0, 0]
```

### Binary `*`

Multiplication, scalar scaling, group composition, function/object action, or
matrix action depending on operand types.

```lane
R area = 2 * 3
Bool both = true * false
R3 p = 2 * [1, 0, 0]
Isom3 composed = a * b
R3 moved = composed * [1, 0, 0]
Object rotated = rot([0, 0, 1], 0) * Ball3D(r=1)
```

### Binary `/`

Division for fields and scalar division for vector spaces. It is not accepted
for plain `Grp` values.

```lane
C z = [1, 2]
C normalized = z / z
R3 half = [1, 2, 3] / 2
```

### Comparisons

`==` and `!=` compare `Bool`, `R`, and `Z` values. `<`, `<=`, `>`, and `>=`
compare `R` and `Z` values. All comparison operators return `Bool`.

```lane
Bool same = time == 0
Bool inside = 0 <= time
Bool ordered = count < 4
```

### Conditional Values

`if(cond) x else y` selects between two values of the same type and emits a GLSL
conditional expression. `cond` must be `Bool`. The shorthand `if(cond) x` uses
zero for the else branch when `0` is valid for the type of `x`.

```lane
R clipped = if(inside) distance else 0
R masked = if(inside) distance
Hom(R2, R4) color = if(shape.sdf > 0) foreground else background
```

### Function Composition `@`

`f @ g` means `f(g(t))` for unary functions.

```lane
Func(R, R) wave = pow2 @ sin
R y = wave(time)
```

Functions with matching domains support pointwise arithmetic when their codomain
supports the corresponding operation.

```lane
Hom(R2, R) h = f + g
```

Pointwise arithmetic and value function calls can mix functions with ordinary
values. When a call overload expects a value type and one or more arguments are
functions, Lane lifts the call over the shared function domain and treats value
arguments as constants over that domain. Function-typed parameters are passed as
functions rather than lifted.

```lane
Hom(R2, R) clipped = max(shape.sdf, 0)
Hom(R2, R4) color = blend * (shape.sdf > 0)
```

### Closures

`t |-> expr` builds a function by lifting `expr` over the parameter `t`. Product
domains can name each component explicitly.

```lane
Hom(R, R) shifted = t |-> sin(t + 1)
Hom(R x R, R) diagonal = (x, y) |-> sin(x + y)
```

The unit type `*` is used for shader entry functions. Raw GLSL module templates
that instantiate to `Hom(*, *)` emit as `void main()` regardless of the Lane
binding name.

### Function Products

Tuples of functions with the same domain form a vector-valued function when the
expected codomain is `R2`, `R3`, or `R4`. The domains must match as written:
`Hom(R2, R)` and `Hom(R x R, R)` are different signatures.

```lane
Hom(R, R2) circle = (sin, cos)
R2 p = circle(time)
```

`f x g` forms a product map for scalar functions, applying the left function to
the first coordinate and the right function to the second.

```lane
Hom(R2, R2) warp = sin x cos
R2 q = warp([1, 2])
```

### Indexing `[]`

Array indexing uses square brackets and integer indices.

```lane
Array(R) xs = Array(1, 2, 3)
R x = xs[1]
```

## Builtin Value Functions

### GLSL Math Builtins

Lane pre-registers GLSL math builtins whose signatures fit Lane's current value
types: `Bool`, `R`, `Z`, `R2`, `R3`, `R4`, and generic matrix families such as
`Mat{n}x{m}`. Calls emit as direct GLSL calls and do not add support bodies.
`lane list all` prints complete scalar/vector families compactly, for example
`Hom(Rn, Rn)` for functions available on `R`, `R2`, `R3`, and `R4`, and
`transpose` as `Hom(Mat{n}x{m}, Mat{m}x{n})`.

```lane
provided Mat3 frame
R y = sin(time) + cos(time)
R3 n = normalize([1, 2, 3])
R3 reflected = reflect(n, [0, 1, 0])
R3 color = mix(clamp(reflected, 0, 1), [1, 0, 0], 0.25)
Mat3 adjusted = matrixCompMult(frame, inverse(transpose(frame)))
```

Matrix identities can be written as `I{n}` or `eye{n}`. Matrix basis literals
use `E{i}{j}` with one-based row and column indices, and the expected `Mat...`
type sets the full matrix size. Unit vector literals use `e{N}{n}`, with `N` as
the vector dimension and `n` as the one-based component index.

```lane
Mat3 eye = I{3}
Mat3 also_eye = eye{3}
Mat3 ez = E{1}{3}
Mat12x3 large = E12_3
R3 y_axis = e{3}{2}
```

Registered GLSL functions include:

- Angle and trigonometry: `radians`, `degrees`, `sin`, `cos`, `tan`, `asin`,
  `acos`, `atan`, `sinh`, `cosh`, `tanh`, `asinh`, `acosh`, `atanh`.
- Exponential: `pow`, `exp`, `log`, `exp2`, `log2`, `sqrt`, `inversesqrt`.
  Besides GLSL-style `pow(Rn, Rn)`, `pow` has category overloads
  `pow(Z, Mon)` for repeated monoid multiplication and `pow(C, C)` for complex
  exponentiation.
- Common math: `abs`, `sign`, `floor`, `trunc`, `round`, `roundEven`, `ceil`,
  `fract`, `mod`, `min`, `max`, `clamp`, `mix`, `step`, `smoothstep`, `fma`.
  `min` and `max` accept scalar/vector operands in either order.
- Geometric functions: `length`, `distance`, `dot`, `cross`, `normalize`,
  `faceforward`, `reflect`, `refract`.
- Matrix functions: `matrixCompMult`, `transpose`, `determinant`, `inverse`.
- Fragment derivative functions: `dFdx`, `dFdy`, `fwidth`.
- Complex overloads: `inv`, `exp`, `log`, `sqrt`, `sin`, `cos`, `tan`, `sinh`,
  `cosh`, `tanh` on `C`.
- Bool helpers: `not`, `and`, `or`, `xor`.

Sampler, image, atomic, packing, and out-parameter GLSL builtins are not
registered yet because Lane does not have the corresponding value types or
parameter passing forms.

`pow2` remains available as a Lane helper:

```lane
R squared = pow2(y)
```

Complex overloads are available for functions such as `exp`, `log`, `pow`, and
`sin` on `C` inputs. Their GLSL overloads are emitted only when used.

```lane
C seed = [1, 0]
C z = exp(seed)
```

Monoid powers take the integer exponent first:

```lane
Mon Pair = R x Z
provided Pair p
const Pair cubed = pow(3, p)
```

### `rot`

Value-level `rot(axis, anchor, angle)` constructs an `Isom3` isometry.

```lane
R3 axis = [0, 0, 1]
Isom3 r = rot(axis, 0, time)
R3 p = r * [1, 0, 0]
const Object output = Ball3D(r=1) + p
```

Object-level `rot(...)` rotates an object by applying the inverse transform to
the sampled point. Defaults are accepted:

```lane
const Object output = rot([0, 1, 0], [1, 0, 0], 0.5)(Ball3D(r=1))
const Object output = rot(0.5)(Ball3D(r=1)) // axis=(0,0,1), anchor=0
const Object output = rot()(Ball3D(r=1))    // angle=0
```

### `rot2D`

Value-level `rot2D(anchor, angle)` constructs an `Isom2` isometry.

```lane
#2D
Isom2 r = rot2D([0, 0], time)
const Object output = r * Box2D(a=1, b=.5)
```

Object-level `rot2D(...)` rotates 2D objects:

```lane
const Object output = rot2D([1, 0], 0.5)(Box2D(a=1, b=.5))
const Object output = rot2D(0.5)(Box2D(a=1, b=.5)) // anchor=0
const Object output = rot2D()(Box2D(a=1, b=.5))    // angle=0
```

### Differential Builtins

Differential builtins lower directly during emission.

```lane
Func(R, R) slope = derivative(sin)
Func(R, R) slope_default = grad(sin)
```

For scalar fields, `derivative`, `gradient`, and `grad` all produce the
finite-difference gradient in the domain dimension.

```lane
provided Hom(R3, R) density
provided R3 p
R3 n = gradient(density)(p)
```

Partial derivatives are named by axis and are available where the input
dimension includes that axis:

```lane
provided Hom(R3, R) density
provided R3 p
R along_x = dfdx(density)(p)
R along_y = dfdy(density)(p)
R along_z = dfdz(density)(p)
```

For vector-valued functions, `derivative` returns the corresponding Jacobian
matrix. Divergence is available for same-dimensional vector fields:

```lane
provided Hom(R2, R3) field
provided Hom(R3, R3) flow
provided R2 uv
provided R3 p
Mat2x3 jacobian = derivative(field)(uv)
R outflow = divergence(flow)(p)
```

## Object Expressions

Objects denote SDFs. Primitive constructors, object operators, and ambient
actions produce `Object` or `Object2D` values.

```lane
Object a = Ball3D(r=2)
R3 offset = [2, 0, 0]
Object b = Box3D(1, .5, .25) + offset
Object c = smoothUnion(.2)(a, b)
const Object output = c
```

### 3D Primitive Constructors

```lane
Ball3D(r=1)
Box3D(a=1, b=2, c=3)
Halfspace3D(n=(0, 1, 0), h=0)
Line3D(x0=[0, 0, 0], dir=[1, 0, 0])
Plane3D(n=[0, 1, 0], origin=[0, 2, 0])
Quad3D(p1=[0, 0, 0], p2=[1, 0, 0], p3=[1, 1, 0], p4=[0, 1, 0])
Segment3D(a=[0, 0, 0], b=[1, 0, 0])
Segment3D(2) // centered length constructor
Simplex3D(p0=[0, 0, 0], p1=[1, 0, 0], p2=[0, 1, 0], p3=[0, 0, 1])
Torus3D(major=3, minor=.5)
Triangle3D(p1=[0, 0, 0], p2=[1, 0, 0], p3=[0, 1, 0])
```

### 2D Primitive Constructors

```lane
Ball2D(r=1)
Box2D(a=1, b=.5)
Point2D(at=(3, 4))
Polygon2D(points=((0, 0), (2, 0), (2, 1), (0, 1)))
Quad2D(p1=[0, 0], p2=[1, 0], p3=[1, 1], p4=[0, 1])
Segment2D(a=[0, 0], b=[1, 0])
Segment2D(length=2) // centered length constructor
Triangle2D(p0=[0, 0], p1=[1, 0], p2=[0, 1])
```

### Boolean Object Operators

Associative operators accept two or more objects:

```lane
const Object output = union(Ball3D(r=1), Box3D(1, 1, 1), Torus3D(2, .2))
const Object output = intersect(Ball3D(r=2), Box3D(1, 1, 1))
R3 offset = [1, 0, 0]
const Object output = xor(Ball3D(r=1), Ball3D(r=1) + offset)
```

Binary difference accepts exactly two objects:

```lane
const Object output = diff(Box3D(2, 2, 2), Ball3D(r=1))
```

### Smooth Object Operators

Smooth operators are curried by smoothing radius `k`.

```lane
const Object output = smoothUnion(.2)(Ball3D(r=1), Box3D(1, 1, 1))
const Object output = smoothIntersect(.2)(Ball3D(r=2), Box3D(1, 1, 1))
const Object output = smoothDiff(.2)(Box3D(2, 2, 2), Ball3D(r=1))
R3 offset = [1, 0, 0]
const Object output = smoothXor(.2)(Ball3D(r=1), Ball3D(r=1) + offset)
```

### `revolution`

Lifts an `Object2D` profile into a 3D surface of revolution.

```lane
Object2D profile = Segment2D(a=[0, -1], b=[0, 1])
const Object output = revolution(1.5)(profile)
```

### `extrude`

Extrudes a 2D profile along the `z` axis.

```lane
const Object output = extrude(.25)(Box2D(a=1, b=.5))
```

### Ambient Transforms

Translate objects with vector addition:

```lane
R3 offset = [1, 2, 3]
const Object output = Ball3D(r=1) + offset
```

Apply orthogonal matrix actions or isometry actions with `*`:

```lane
provided Mat3 R
const Object output = R * Ball3D(r=1)

Isom3 g = Isom3(e, [1, 2, 3])
const Object output = g * Ball3D(r=1)
```

Under `#2D`, `Isom2 * Object2D` is accepted:

```lane
#2D
Isom2 g = Isom2(e, [1, 2])
const Object output = g * Box2D(a=2, b=1)
```

## Emitted GLSL

Every compilation emits:

- support structs and helper functions for used primitives, operators, value
  functions, built-in algebraic types, and constructed product types;
- user value functions emitted with their Lane name;
- generated object helpers for `construct` or `const Object` bindings:
  `sdf_name` and `grad_sdf_name` for 3D objects, and `sdf_name` for 2D objects;
- legacy `float scene_sdf(vec3 p)` and `vec3 scene_grad(vec3 p)` entrypoints
  only when `const Object output` is present.

Scene-invariant value bindings are emitted as global `const` values when
possible. Generated local names are renamed if they would collide with user
names such as `p`, `eps`, `dx`, `dy`, or `dz`.

## Complete Mini Example

```lane
provided R time
provided Hom(R3, R) density

Grp Motion = Isom3 x Isom3 <left, right>

R3 axis = [0, 0, 1]
Isom3 spin = rot(axis, 0, time)
Motion both = Motion(spin, Isom3(e, [2, 0, 0])) * Motion(Isom3(e, 0), spin)

construct Object ball = Ball3D(r=1 + sin(time))
Object box = Isom3(e, [2, 0, 0]) * Box3D(a=1, b=.5, c=.5)
Object scene = smoothUnion(.2)(spin * ball, box)

const Object output = scene
```

Use the CLI for authoritative registered GLSL bodies:

```sh
lane list 3d
lane list revolution
lane list Isom3
```

## Feature Samples

The repository includes compile-valid feature samples:

- `all_features.lane` uses the 3D language surface: provided inputs, provided
  category types, constructed product types, typed and inferred bindings,
  `construct`, `const Object`, arrays, product domains, categories,
  neutral casts, differential builtins, value-level rotations, object-level
  rotations, primitive constructors, object operators, revolution, extrusion,
  and ambient actions.
- `all_features_2d.lane` covers the `#2D` ambient mode and 2D object actions.

Compile them directly:

```sh
cargo run -- all_features.lane
cargo run -- all_features_2d.lane
```
