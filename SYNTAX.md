# Lane Syntax Notes

This file describes the intended shape of Lane syntax and how it relates to the
current implementation. For the complete current user-facing reference, see
`README.md`. For the deeper compiler roadmap, see `SEMANTIC_MODEL.md`.

## Current Surface

Lane source is line-oriented. Each non-empty, non-comment line is one
declaration. Continuation lines start with whitespace.

```lane
provided R time
Func(R, R) pulse = pow2 @ sin
Object shape = Ball3D(r=1 + pulse(time))
const Object output = shape
```

The current declaration forms are:

- `provided TYPE name`: declares an external value or function.
- `provided name: A -> B`: declares an external function.
- `provided CATEGORY TypeName`: declares an external nominal type.
- `CATEGORY TypeName = BaseType {op: name, ...}`: promotes a type into a
  category using provided operations.
- `[const] CATEGORY TypeName<field, ...> = TYPE x TYPE`: declares a product
  type with optional field names.
- `TYPE name = expression`: declares a typed value, function, or SDF object.
- `name = expression`: declares an inferred binding.
- `const name = expression`: marks a binding as generated/emitted.
- `construct Object name = expression`: currently exports an SDF helper for an
  object binding.

Directives:

- `#2D`: switches ambient SDF space to 2D.
- `#prec VALUE`: sets finite-difference epsilon.
- `#import NAME`: imports a module from `modules/` or the installed module
  path.
- `#module`: marks an imported module file and is invalid in scene files.

## Types

Lane uses nominal and structural types.

Common primitive and built-in types:

```lane
Bool
R
Z
C
H
R2
R3
R4
Mat2
Mat3
Mat4
Mat{n}
Mat{n}x{m}
Object
Object2D
```

Function types can be written with either `Func` or `Hom`:

```lane
Func(R, R)
Hom(R3, R)
End(R)      // Hom(R, R)
```

Products are structural:

```lane
R x R
R × R
R3 x R
Hom(R x R, R)
```

Concrete power types are supported today and lower to products:

```lane
R^3
R^{3}
```

Current generic placeholders are supported in limited places:

```lane
{X}
R{n}
Mat{n}
Mat{n}x{m}
```

They are currently handled by type unification and textual name-template
expansion, not by a full semantic generic system.

## Lane Objects

In the semantic model, a Lane object means any typed element of a Lane type:
a real value, vector, matrix, function, product, operator, generic
specialization, or SDF shape binding.

This is different from the current compiler type `ObjectExpr`, which means an
SDF shape expression.

Every Lane object should eventually be describable by:

- its Lane type;
- its optional generic parameters;
- its GLSL body, template, or external reference;
- its direct dependencies;
- whether it is exported/external.

The current compiler does not store this information in one place. It mostly
rediscovers it by walking expression enums during typechecking and emission.

## Functions

An object of type `Hom(X, Y)` is a Lane function. It has a domain, codomain, and
supports evaluation and composition.

```lane
Hom(R, R) f = sin
R y = f(0)
Hom(R, R) g = pow2 @ f
```

Pointwise function arithmetic is surface sugar for operations in the codomain.

```lane
provided Hom(R, R) f
Hom(R, R) h = sin + 2 * sin
Hom(R, R) inv = ~f
```

Semantically this is a function that, when evaluated at `x`, behaves like:

```text
sin(x) + 2 * sin(x)
```

The current unary inverse operator is written `~`. For values, `~a` requires
`a` to have a `Grp` type and lowers to that type's inverse helper. For
functions, `~f` is interpreted pointwise in the codomain when a function type is
expected.

Function products are also semantic. Use brackets for vector-valued functions
and parentheses for structural product-valued functions:

```lane
Hom(R, R2) circle = [sin, cos]
Hom(R, R x R) pair = (sin, cos)
```

The intended identification is:

```text
Hom(X, Y x Z) = Hom(X, Y) x Hom(X, Z)
```

Products should also provide projections:

```lane
p{0}: Hom({X} x {Y}, {X})
p{1}: Hom({X} x {Y}, {Y})
```

Every `v: {X} x {Y}` can be identified with:

```text
(p{0}(v), p{1}(v))
```

## Higher-Order Functions

Higher-order functions are Lane objects, but not necessarily GLSL functions.

For example:

```lane
df: End(End(R))
```

means:

```text
df: Hom(Hom(R, R), Hom(R, R))
```

This cannot be emitted directly as a GLSL function because its argument is a
Lane function. After specialization, it may become first-order:

```lane
df(sin): Hom(R, R)
```

Then the compiler can emit a normal GLSL helper such as:

```glsl
float df_sin(float x) {
    return (sin(x - eps) - sin(x + eps)) * 0.5 / eps;
}
```

The intended core operation is typed placeholder substitution. A higher-order
body can contain placeholders such as `{f}` and `{x}`. Evaluating the object
substitutes those placeholders; GLSL function emission is only one possible
normal form after enough placeholders are concrete.

## GLSL Bodies

Raw GLSL bodies in modules are currently allowed for `const` function
declarations. They are useful for loops and code that cannot be expressed in
pure Lane yet.

The target model is to store raw GLSL as an unnamed body template. Names are
assigned when a user names the object or when the compiler interns a generated
specialization.

For example, derivative can be thought of as a body template:

```text
(({f}({x} - eps) - {f}({x} + eps)) * 0.5 / eps)
```

If `d(f)` appears more than once, the compiler should resolve it to one
specialized object and reuse that object. The emitter can then decide whether
to inline the body or emit a helper function.

## Generics

Generics are compile-time schemes, not Lane functions. They are based on
symbolic substitution and constraint solving. They can parameterize types,
objects, dimensions, and literals, but they cannot be composed or evaluated as
runtime values.

Desired generic forms include:

```lane
swap{X}{Y}: {X} x {Y} -> {Y} x {X}
```

```lane
Hom({B}, {C}) x Hom({A}, {C})
```

```lane
{X}^{n}
```

```lane
Hom({X}^{n}, {X}) p{k}{n}
```

The intended behavior is that explicit generic arguments and contextual type
constraints cooperate.

If:

```lane
f: Hom(R, R^7)
```

then:

```lane
p{3}{} @ f
```

should deduce:

```text
X = R
n = 7
k = 3
```

If:

```lane
f: Hom(R, R^n)
```

then:

```lane
p{3}{7} @ f
```

should constrain `f` to have codomain `R^7`.

Some parameters are not deducible from type alone. For example, `{k}` in a
projection cannot be recovered from `Hom({X}^{n}, {X})` without an explicit
argument or some other constraint that mentions the selected component.

## Current Gaps

These are intentional roadmap items, not current syntax promises:

- Semantic generic schemes with explicit parameter lists.
- Empty generic holes such as `p{3}{}`.
- Symbolic power types such as `{X}^{n}`.
- Backtracking and deferred generic resolution.
- Interned specializations such as one shared object for repeated `d(f)`.
- Dependency-driven emission from a first-class object table.
- First-order GLSL emission for higher-order specializations in general.

The current implementation supports parts of this through `Type`, `ValueExpr`,
`FunctionExpr`, textual name templates, and type unification, but the full
semantic model is still planned.
