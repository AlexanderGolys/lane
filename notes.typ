#set page(numbering: "1")
// #set text(font: "New Computer Modern")

#let theorem_block(label, title: none, fill: none, inset: (x: 1.2em, y: 1em), body) = {
  block(
    inset: inset,
    fill: fill,
    stroke: luma(200),
    radius: 6pt,
    width: 100%,
    [
      #strong(label)
      #if title != none [#sym.space.nobreak (#title)]
      #parbreak()
      #body
    ],
  )
}

#let theorem(title: none, body) = theorem_block("Theorem", title: title, fill: rgb("#eeffee"), body)
#let proposition(title: none, body) = theorem_block("Proposition", title: title, body)
#let lemma(title: none, body) = theorem_block("Lemma", title: title, body)
#let definition(title: none, body) = theorem_block("Definition", title: title, fill: rgb("#f7f7ff"), body)
#let remark(title: none, body) = theorem_block("Remark", title: title, body)
#let admonition(title: none, body) = theorem_block("Note", title: title, fill: rgb("#ffffee"), body)

= Signed Distance Functions: Theory and Design

== Definition

A *Signed Distance Function* (SDF) is a map $f: X -> RR$ where $X$ is typically $RR^2$ or $RR^3$, satisfying:

$
f(p) < 0 &<==> p\ "is inside the shape" \
f(p) = 0 &<==> p\ "is on the boundary" \
f(p) > 0 &<==> p\ "is outside the shape"
$

The *distance property*: $|f(p)|$ equals the Euclidean distance from $p$ to the nearest point on the boundary. This is what enables sphere tracing.

== The Hierarchy of "SDF-like" Functions

This is the most important classification for practical shader/dSL design.

#definition(title: "Exactness Classes")[
Let $f$ approximate the true SDF of a region $Omega$. We distinguish:
]

#table(
  columns: (1.2fr, 2.8fr, 2.6fr),
  gutter: 0.6em,
  inset: 0.4em,
  align: (x, y) => if y == 0 { center + horizon } else { left + top },
  [*Class*], [*Properties*], [*Use cases*],
  [*A. Exact SDF*], [Correct zero set, correct sign, exact magnitude, 1-Lipschitz, $|nabla f| = 1$ a.e. away from cut locus], [Best sphere tracing, offsets/dilations/erosions by adding constants, reliable thickness/shell computation, good curvature approximation, reinitialization target],
  [*B. One-sided exact*], 
     [Exact on one side only. Union via `min`: exact outside. Intersection via `max`: exact inside. Difference `max(a,-b)`: exact inside result. Complement `-a`: exact.], 
     [Union safe for sphere tracing from outside. Intersection/difference need care. ],
  [*C. Conservative DE*], 
     [Correct sign on marching side, magnitude is lower bound on true distance: $f(x) <= d(x, partial Omega)$ outside. ], 
     [ Sufficient for sphere tracing. You need only conservative lower bound, not exactness.],
  [*D. Sign-correct implicit*], 
      [Same inside/outside classification, same zero set, not a distance. ], 
      [Root finding, bisection/secant/Newton along rays, marching cubes/contouring, normals from gradients. NOT for naive sphere tracing.],
  [*E. Zero-set only*], 
      [Only $f^{-1}(0)$ is correct surface. Sign may be weird or absent.], 
      [Pure symbolic geometry, not for robust rendering.],
)

#remark[
Key practical insight: sphere tracing needs only a *conservative distance estimate* on the side you march from. Exactness is a stronger requirement than strictly necessary for basic rendering.
]

#proposition[
For physically meaningful methods using actual path lengths to boundary (subsurface scattering, etc.), you need true or conservative inside distances. But "only exact SDF works for SSS" is too strong—conservative inside distances suffice.
]

#theorem(title: "CSG Preservation Rules")[
If $a, b$ are exact SDFs, then:
- `min(a, b)` (union): outside-exact
- `max(a, b)` (intersection): inside-exact
- `max(a, -b)` (difference): inside-exact
- `-a` (complement): exact
]

#remark[
1-Lipschitz composition preserves 1-Lipschitzness but *not* automatically the "distance estimator" semantics—you need extra monotonicity/underestimation conditions.
]

== Characterization of True SDFs

#theorem(title: "SDF Characterization (Unsigned)")[
A function `u: X -> [0, infinity)` is the distance to a closed set $K$ if and only if:
+ $u >= 0$ everywhere
+ $u = 0$ on $K$
+ $u$ is the unique viscosity solution of the eikonal equation `|nabla u| = 1` on `X \ K` with boundary condition `u|_K = 0`.
]

Geometrically: $u$ is 1-Lipschitz, every point has a minimizing geodesic to the zero set, and $u$ drops with slope 1 along that geodesic.

#admonition(title: "Not every 1-Lipschitz vanishing-at-zero is a distance function")[
$
u(x) = min(|x|, 1)
$
is 1-Lipschitz and vanishes at 0, but it is *not* the distance to its zero set.
]

For signed distance: same idea on both sides, with sign fixed by the chosen domain.

== Unsigned vs Signed Distance: The Key Distinction

#proposition[
There are really two different objects:
+ *Distance to a set* $M$: always nonnegative. Natural in Lawvere-enriched setting.
+ *Signed distance to a region* $Omega$: requires choice of inside/outside. Sign is extra geometric/topological structure, not intrinsic to the Lawvere metric.
]

If `M = partial Omega` for a nice domain:
```text
|s_Omega(x)| = d(x, partial Omega)
```

So `abs` forgets the side and gives distance to boundary.

#admonition(title: "Caveat on topology")[
For non-orientable, self-intersecting, or non-separating situations, global signed distance is not canonical without extra conventions.
]

== The Categorical Structure: Lawvere Metric Spaces

=== Distance to Set as Enriched Kan Extension

In *Lawvere metric spaces* --- categories enriched over `([0, infinity], >=, +)` --- distance functions have natural interpretations.

#definition(title: "Lawvere Metric Space")[
A category enriched over `([0, infinity], >=, +)` where:
+ Objects are points
+ Hom-sets: `X(a, b) = d(a, b) in [0, infinity]`
+ Composition: $d(a, c) <= d(a, b) + d(b, c)$ (triangle inequality)
+ Identity: $d(a, a) = 0$
]

For a subset `A subseteq X`, the unsigned distance is:

```text
d_A(x) = inf_{a in A} d(x, a) = /
  _{a in A} X(x, a)
```

#theorem(title: "Distance as Module Composition")[
For subset `A`, define the characteristic module `chi_A: X -> {ast}` by:
```text
chi_A(a, ast) = cases(0 if a in A, infinity otherwise)
```

Then:
```text
(d_A = X millions chi_A)(x, ast)
  = inf_{a in X} (X(x, a) + chi_A(a, ast))
  = inf_{a in A} X(x, a)
```

This is *exactly* the distance to $A$, expressed as composite of the hom-distributor with the characteristic module.
]

#remark[
Equivalently: distance-to-set is an *inf-convolution* with the metric kernel. In tropical/idempotent language, this is the continuous shortest-path closure of the zero-set.
]

=== When Distance Becomes Representable

#theorem[
Distance to $A$ is representable (in the enriched sense) if and only if $A$ has been collapsed to a single point in the quotient.

More precisely: if $A$ has diameter 0 in the Lawvere metric (all $d(a, a') = 0$ for $a, a' in A$), then every representable $X(-, b)$ with $b in A$ coincides with $d_A$.
]

#remark(title: "Syntactic vs Structural")[
If $A$ is already "point-like" internally, the quotient is syntactic sugar—it makes explicit that the formula doesn't depend on the choice of $b in A$. For general $A$, the quotient is genuinely structural: it creates a new representable that didn't exist before.
]

=== The Profunctor Formulation

#proposition[
In the quotient $X / A$ collapsing $A$ to point $ast$:
+ $X/{A}(-, ast)$ is "distance from point to $A$"
+ $X/{A}(ast, -)$ is "distance from $A$ to point"

In symmetric metrics these agree. In asymmetric Lawvere spaces, they differ.
]

=== The Key Insight: SDF is Polarized

The enriched construction captures *unsigned* distance. For signed distance:

#theorem(title: "SDF as Difference of Modules")[
For a region $Omega$ with boundary `partial Omega`:
$
s_Omega(x) = d(x, Omega^c) - d(x, Omega)
$

This is the difference of two ordinary distance transforms—one for each side.

Equivalently: choose two collapsed classes, `partial Omega_in` and `partial Omega_out`, and take the difference of distances to these two points.
]

#admonition(title: "Categorical Summary")[
+ *Ordinary DF* is a single presheaf/module (enriched Kan extension)
+ *Signed DF* is a *polarized pair* of such modules

The sign is not intrinsic to the Lawvere base—it requires subtraction in $RR$, not `[0, infinity]`.
]

== Generating SDF from Arbitrary Functions

#proposition[
Given a smooth $f: RR^n -> RR$ with transverse zero set, the canonical signed distance is:
```text
SD(f)(x) = sgn(f(x)) * d(x, f^(-1)(0))
```

The zero set is $Z(f) = f^{-1}(0)$, and the distance to $Z(f)$ is computed via the eikonal equation or level-set reinitialization.
]

#definition(title: "Reinitialization PDE")[
```text
partial_t u + sgn(f) (|nabla u| - 1) = 0,
u(., 0) = f
```

The steady state is the signed distance with same zero set as $f$.
]

This is:
+ Infimal convolution
+ Hopf-Lax / Lax-Oleinik semigroup
+ Enriched Kan extension / tropical closure

#remark[
There is no globally smooth exact SDF in general, even if the boundary is smooth—the medial axis/cut locus creates nonsmoothness. The operator is canonical but not "smoothifying."
]

== Non-Euclidean Raymarching

Sphere tracing generalizes to *any geodesic metric*.

#theorem[
If $F(p)$ is a lower bound on geodesic distance from $p$ to a surface, and you move along a geodesic by length $F(p)$, you cannot cross the surface.
]

For hyperbolic geometry:
+ *Poincaré models*: conformal (angles preserved), geodesics are circles/lines
+ *Klein model*: geodesics are straight lines (convenient for traversal), but not conformal
+ *Hyperboloid model*: cleanest for isometries and numerics

#admonition(title: "Metric Normals")[
For a level-set $F = 0$ in Riemannian metric $g$, the geometric normal is:
```text
n prop g^(-1) nabla F
```
normalized using $g$, not Euclidean norm. Non-conformality doesn't break tracing—it changes angle/normal interpretation.
]

== Graph SDF from 2D SDF

#proposition[
Given $g: RR^2 -> RR$, the graph $Gamma_g = {(x, y, g(x,y))}$ has implicit function $F(x,y,z) = z - g(x,y)$. This is *not* generally an exact SDF for $Gamma_g$.
]

#theorem(title: "Conservative Estimator for Graph")[
If $g$ is $L$-Lipschitz, then:
$
F(x,y,z) / sqrt{1 + L^2}
$
is a conservative signed distance estimator for $Gamma_g$.

In particular, if $g$ is an SDF (1-Lipschitz), then $(z - g(x,y))/sqrt{2}$ is conservative.
]

This is a very useful compiler rule for DSLs lifting 2D fields to 3D graphs.

== Ruled Surfaces and SDFs

The observation "all SDFs are ruled" is not literally true—a sphere is not ruled.

#proposition[
What *is* true:
+ Away from the medial axis, signed distance is linear along normal rays
+ The neighborhood of a smooth surface is parameterized by the normal bundle
+ SDF geometry is about normal congruences, offset/parallel surfaces, and medial axis/cut locus
]

Two related "ruled-like" facts:
+ The graph of an unsigned distance function is the lower envelope of cones centered on the set — tropical/envelope viewpoint
+ The characteristics of the eikonal equation are normal rays, which are straight in Euclidean space

== Reference: The Guarantee Types for DSL Design

For a practical SDF-generating DSL, the key abstraction is:

```typescript
type Guarantee =
| { kind: "ExactSD" }
| { kind: "SideExact"; outsideExact: boolean; insideExact: boolean; }
| { kind: "ConservativeDE"; side: "outside" | "inside" | "both"; lip: number; }
| { kind: "SignCorrectImplicit"; lip?: number }
| { kind: "ZeroSetOnly" };

type Metric =
| { kind: "Euclidean" }
| { kind: "HyperbolicHalfSpace" }
| { kind: "HyperbolicKlein" }
| { kind: "CustomRiemannian"; g: MetricTensor };
```

Every node should carry:
+ Ambient dimension
+ Metric
+ Sign convention
+ Guarantee
+ Lipschitz bound
+ (Optional) differentiability class
+ (Optional) transform Jacobian bounds

=== Compiler Propagation Rules

+ Primitive sphere/circle: `ExactSD`
+ Rigid transform: preserve exactness
+ Uniform scale by $s$: preserve exactness, multiply value by $s$
+ Non-uniform linear map $M$: usually degrade to conservative DE using singular-value bounds
+ Union `min`: `outsideExact = true`
+ Intersection `max`: `insideExact = true`
+ Difference `max(a, -b)`: `insideExact = true`
+ Smooth blends: default to `SignCorrectImplicit` unless proven otherwise
+ Graph-lift from $L$-Lipschitz field: conservative DE with factor $sqrt{1 + L^2}$

== The SDF DSL in Categorical Terms

#definition(title: "Semantics")[
+ `Shape` = region (with interior specification)
+ `Boundary` = hypersurface (codimension-1)
+ `DF(B)` = distance to boundary `-> [0, infinity)`
+ `SDF(Omega)` = signed distance `-> RR` (derived from two DFs)
]

The signed distance is:

```text
SDF(Omega) = DF(Omega^c) - DF(Omega)
```

#remark[
This matters when mixing:
+ Curves in 3D
+ Surfaces in 3D
+ Solids
+ Shells
+ Open sets
+ Zero-sets of implicit functions

Only region objects genuinely have global signed distance.
]

== Bibliography

=== Foundational (Lawvere-enriched)

+ F. W. Lawvere, "Metric spaces, generalized logic, and closed categories," *Rend. Sem. Mat. Fis. Milano* 43 (1973), 135–166. Reprinted: *TAC Reprints* 1.

+ G. M. Kelly, *Basic Concepts of Enriched Category Theory*, Cambridge 1982. TAC Reprints 10.

+ I. Stubbe, "Categorical structures enriched in a quantaloid," *TAC* 14 (2005), 1–45.

+ I. Stubbe, "An introduction to quantaloid-enriched categories," *Fuzzy Sets and Systems* 256 (2014), 95–116.

+ M. M. Clementino, D. Hofmann, W. Tholen, "One setting for all: metric, topology, uniformity, approach structure."

+ A. Balan, A. Kurz, J. Velebil, "Extending set functors to generalised metric spaces," *LMCS* 15 (2019).

=== Distance Functions and Level Sets

+ J. A. Sethian, *Level Set Methods and Fast Marching Methods*, Cambridge 1999.

+ S. Osher, R. Fedkiw, *Level Set Methods and Dynamic Implicit Surfaces*, Springer 2003.

+ L. C. Evans, *Partial Differential Equations*, AMS 2010.

+ M. G. Crandall, H. Ishii, P.-L. Lions, "User's guide to viscosity solutions," *Bull. AMS* 27 (1992).

=== Geometric Measure / Regularity

+ H. Federer, "Curvature measures," *J. Diff. Geom.* (1959).

+ P. Cannarsa, C. Sinestrari, *Semiconcave Functions, Hamilton–Jacobi Equations, and Optimal Control*, Birkhäuser 2004.

+ J. M. Lee, *Introduction to Riemannian Manifolds*, Springer 2018.

+ M. do Carmo, *Riemannian Geometry*, Birkhäuser 1992.

=== Tropical / Idempotent Analysis

+ M. Akian, S. Gaubert, V. Kolokoltsov, various papers on max-plus/idempotent analysis.

=== Conversation Notes

+ Deep discussion on SDF categorical structure, guarantee propagation for DSLs, and enriched formulation: *t3.chat conversation*, March 2026. https://t3.chat/share/qdt247hd3h

---

== Appendix: The Precise Enriched Formulation

For a Lawvere metric space $(X, d)$ and subset `A subseteq X`:

#definition(title: "Characteristic Module")[
Define `chi_A: X toptopiangleright 1` by:
```text
chi_A(a, ast) = cases(0 if a in A, infinity otherwise)
```
]

#theorem[
The distance to $A$ is:
```text
d_A = X millions chi_A
```

where `millions` is enriched profunctor composition (inf-convolution in `([0, infinity], +, >=)`).
]

#proposition[
If $A$ is collapsed to a point $ast$ in the quotient $X/A$, then $d_A$ becomes the representable $(X/A)(-, ast)$.
]

For a region $Omega$ with boundary `partial Omega`:

```text
delta_(partial Omega)(x) := d(x, partial Omega)
delta_Omega(x) := d(x, Omega)
```

and the signed distance is:
```text
s_Omega(x) = delta_(partial Omega)(x) - d(x, Omega) * sgn(sad(x in Omega))
```
