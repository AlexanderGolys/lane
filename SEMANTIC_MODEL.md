# Lane Semantic Model Roadmap

This document records the semantic model we want for Lane and how it differs
from the compiler model that exists today. It is an architecture roadmap, not
user-facing syntax documentation.

## Current State

Lane currently has useful pieces of the desired model, but they are spread
across syntax, typechecking, and emission.

- `Type` represents Lane types, including scalars, vectors, matrices, custom
  types, arrays, products, functions, SDF objects, and generic placeholders.
- `ValueExpr` represents typed value expressions such as variables, calls,
  binary expressions, vectors, matrices, products, arrays, field access, and
  derivative-like values.
- `FunctionExpr` represents Lane function expressions separately from
  `ValueExpr`. It records domain, codomain, and function forms such as named
  functions, operators, composition, pointwise operations, and products.
- `ObjectExpr` currently means an SDF shape expression. This is a separate
  concept from "Lane object" as an element of a type.
- Named value/function/object declarations carry a `generated` flag, mainly
  derived from `const` or `construct`.
- GLSL dependencies are mostly rediscovered by walking `ValueExpr`,
  `FunctionExpr`, and `ObjectExpr` trees in the emitter.
- Raw GLSL template references are tracked separately through placeholder
  collection.
- Generic support exists in small pieces: generic type variables, generic
  vector/matrix dimensions, type unification, type substitution, and textual
  name-template expansion.

The main limitation is that Lane does not yet have a first-class semantic
object table. Because of that, codegen has to infer whether something is
available, computable, virtual, or needed by repeatedly walking enum trees.
This makes dependency pruning, library imports, higher-order functions, and
generic specialization harder than they should be.

## Target Model

A Lane object is any typed element of a Lane type: a real value, matrix,
function, product value, operator, generic specialization, or SDF shape binding.
Every semantic object should eventually be represented by a table entry:

```rust
struct LaneObject {
    id: ObjectId,
    name: Option<String>,
    ty: Type,
    generic_params: Vec<GenericParam>,
    glsl: GlslBody,
    deps: Vec<ObjectId>,
    exported: bool,
}
```

The important fields are:

- `ty`: the Lane type inhabited by the object.
- `generic_params`: compile-time parameters for generic schemes.
- `glsl`: either no GLSL form, an external GLSL reference, or a raw GLSL body
  template with typed placeholders.
- `deps`: direct semantic dependencies as object IDs.
- `exported`: true when the symbol is provided by the host or already exists
  in the target GLSL environment.
- `name`: optional user-facing or compiler-generated name.

`exported` objects are dependency leaves. This includes host-provided Lane
values/functions and GLSL-native symbols such as `sin`. A built-in helper that
requires emitted support code is not exported; it is a normal object with a
GLSL body and dependencies.

Computability should be derived rather than stored as a permanent class:

- no GLSL body means the object is virtual or type-only and cannot be emitted
  directly;
- an external GLSL reference means the object is already available;
- a raw body template becomes emit-ready once all placeholders and dependencies
  resolve to emit-ready or exported objects.

Repeated constructed objects and specializations should be interned. For
example, repeated uses of `d(f)` should resolve to the same semantic object, so
the emitter can either inline it once or emit one helper function.

## Function Semantics

Lane functions are mathematical objects, not just generated GLSL functions.

For an object `f` of type `Hom(X, Y)`:

- given an object `x: X`, Lane can construct `f(x): Y`;
- if `X` and `Y` have GLSL representations, Lane can emit a GLSL definition
  for `f` where evaluation becomes a GLSL call;
- given `g: Hom(Y, Z)`, Lane can construct `g @ f`;
- `Hom(X, Y x Z)` is identified with `Hom(X, Y) x Hom(X, Z)` in the usual
  function-product way;
- from `X x Y`, Lane can construct projections such as `p{0}` and `p{1}`;
- a value `v: X x Y` is identified with `(p{0}(v), p{1}(v))`;
- if `X` and `Y` are GLSL-representable, `X x Y` must also have a GLSL
  representation or a lowering strategy.

The emitter should treat GLSL function definitions as one possible normal form
for first-order functions, not as the source of function semantics.

Higher-order examples illustrate the distinction:

```lane
df: End(End(R))
```

`df` is an object of type `Hom(Hom(R, R), Hom(R, R))`. It is not directly a
GLSL function because its argument is itself a Lane function. It can still have
a raw body template with typed placeholders:

```text
(({f}({x} - eps) - {f}({x} + eps)) * 0.5 / eps)
```

After evaluating `df(sin)`, the result has type `Hom(R, R)` and can become a
first-order GLSL function:

```glsl
float df_sin(float x) {
    return (sin(x - eps) - sin(x + eps)) * 0.5 / eps;
}
```

The same semantic machinery should handle partial evaluation, currying, and
uncurrying. An object of type `Hom(X, Hom(Y, Z))` has two typed placeholders:
Lane can evaluate either placeholder, evaluate both, or lower the object to
`Hom(X x Y, Z)` when that is the desired first-order GLSL form.

## Generic Semantics

Generics are separate from higher-order functions. A generic is a compile-time
scheme parameterized by types, dimensions, literals, or possibly objects. It is
resolved by symbolic substitution and constraint solving, not by Lane function
composition.

Generics should support:

- explicit parameters;
- omitted parameters that can be deduced later;
- type, literal, and object parameters;
- simultaneous binding of type and value-level parameters;
- deferred evaluation;
- recursive specialization;
- backtracking when multiple substitutions are possible;
- unbounded symbolic parameters until a concrete specialization requires them.

Desired examples:

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

The projection example needs both generic values and generic types. If
`f: Hom(R, R^7)`, then:

```lane
p{3}{} @ f
```

should deduce `X = R`, `n = 7`, and `k = 3`. The omitted `{}` is a hole solved
from composition constraints.

If `f: Hom(R, R^n)`, then:

```lane
p{3}{7} @ f
```

should deduce the type of `f` from the explicit dimension. By contrast, `{k}`
cannot be deduced from the type alone, and `{X}` cannot be deduced from an
object-only use of `p{n}{k}` unless additional constraints mention it.

This should replace today's textual name-template expansion with semantic
generic schemes and a specialization cache keyed by the generic object plus its
resolved substitution.

## Roadmap

### Phase 1: Documentation and Terminology

- Keep this document as the canonical architecture roadmap.
- Use "Lane object" for a typed element of a Lane type.
- Use "SDF object" or `ObjectExpr` for shape expressions.
- Record current compiler limitations without changing behavior.

### Phase 2: Semantic Object Table

- Add an internal table keyed by stable `ObjectId`.
- Store type, dependencies, export status, optional name, and GLSL body/ref.
- Initially keep `Type`, `ValueExpr`, `FunctionExpr`, and `ObjectExpr` as IR,
  but resolve named references through object IDs.
- Treat provided symbols and GLSL-native symbols as exported leaves.
- Represent emitted helper/support bodies as normal non-exported objects.

### Phase 3: Dependency-Driven Emission

- Choose roots from explicit outputs, `const` declarations, preview targets,
  and requested helper functions.
- Emit each root by recursively emitting non-exported dependencies first.
- Replace broad emitter scans with reachability from the object table.
- Intern repeated constructed objects such as `d(f)`.
- Let the emitter decide inline versus helper emission from use count, user
  name, body size, and whether the object is function-shaped.

### Phase 4: Semantic Generic Schemes

- Represent generic declarations as schemes with explicit parameters and
  constraints.
- Support omitted generic arguments as holes.
- Deduce substitutions from explicit generic args, expected type, actual
  argument types, composition constraints, and object usage.
- Cache each successful specialization.
- Keep symbolic parameters unresolved until a concrete operation requires a
  concrete value.

### Phase 5: Product and Function Normalization

- Make projections, product values, function products, currying, and uncurrying
  explicit semantic rewrites.
- Use these rewrites for projection templates, function tuples, and
  `Hom(X, Hom(Y, Z))` to `Hom(X x Y, Z)` lowering.
- Ensure every GLSL-representable product type has either a real GLSL type or a
  clear lowering strategy such as destructured arguments.

## Future Test Scenarios

When implementing the roadmap, add focused tests for:

- unused imported library declarations do not emit support code;
- `d(f) + d(f)` interns one specialization instead of creating duplicates;
- `df(sin)` emits one first-order GLSL helper when needed;
- `p{3}{} @ f` deduces `n = 7` from `f: Hom(R, R^7)`;
- explicit `p{3}{7} @ f` constrains `f` to `Hom(R, R^7)`;
- unresolved generic parameters produce precise diagnostics;
- higher-order objects stay virtual until specialization makes them
  GLSL-representable;
- generated names cannot collide with user names.
