# Origin IR: Value Representation Specification

*Level 1: What is a value in origin-ir?*

---

## The Principle

In Val α, the sort is the outermost constructor. You don't have a value and then ask what sort it is. You have a sorted value. The sort comes first. The value lives inside.

```lean
inductive Val (α : Type u) where
  | origin : Val α
  | container : α → Val α
  | contents : α → Val α
```

The IR representation follows the same structure. The sort is not metadata. The sort is not a tag. The sort is the type.

---

## Value Representation

### The three sorts

```
origin                — nothing to retrieve. absorbs everything downstream.
container<T>          — boundary crossed. carries a value of type T. the last known state.
contents<T>           — safe territory. arithmetic lives here. T is the inner type.
```

`T` is any scalar or aggregate type the IR supports: `i32`, `f64`, `tensor<8x128xf32>`, etc. Origin has no inner type — there is nothing inside.

### The unified value type

Every value in origin-ir has type `val<T>`, which is one of the three sorts:

```
val<T> = origin | container<T> | contents<T>
```

There is no bare `T` in origin-ir. A bare `i32` does not exist. Every integer is `val<i32>`. Every float is `val<f64>`. Every tensor is `val<tensor<...>>`. The sort is always present.

This is the constitutive choice. In a traditional IR, a value is `T`. In origin-ir, a value is `val<T>`. The sort wraps the type the same way Val α wraps α.

### What each sort carries

| Sort | Inner value | Meaning |
|---|---|---|
| `origin` | none | The system hit its absolute boundary. Nothing to retrieve. Everything downstream that depends on this is predetermined — it's origin. |
| `container<T>` | one value of type T | The boundary was crossed. The value inside is the last known good state. You know what you were holding when it happened. |
| `contents<T>` | one value of type T | Safe territory. The value inside is a normal arithmetic value. Operations proceed normally. |

### Static vs dynamic sorts

A value's sort can be:

**Statically known** — the compiler can prove at compile time which sort the value is. This is the common case. Most values in a well-formed program are statically `contents`. When the sort is statically known, no runtime check is emitted. The optimizer operates directly on the known sort.

```
%x = contents.i32 42              ; statically contents
%y = contents.f64 3.14            ; statically contents
%z = origin                       ; statically origin
```

**Dynamically resolved** — the compiler cannot prove the sort at compile time. This happens at boundaries: division where the divisor could be zero, external input, function calls with unknown callers. A runtime sort check is emitted at this point and only at this point.

```
%result = val.div %a, %b          ; sort of result depends on sort of %b
                                   ; if %b is contents(0): result is origin
                                   ; if %b is contents(nonzero): result is contents
                                   ; resolved at runtime if %b's value isn't known
```

---

## Sort Semantics

### Origin

Origin absorbs. Any operation involving origin produces origin. This is interaction axiom I1 and I2 from Foundation.lean.

```
op(origin, anything) = origin          ; I1: left absorption
op(anything, origin) = origin          ; I2: right absorption
op(origin, origin)   = origin          ; follows from I1 or I2
```

Origin carries no value. There is nothing to retrieve. This is not an error. This is not a trap. This is not undefined behavior. It is a known, named, propagating state. The system hit the ground. The ground has a name.

When the optimizer sees origin, it folds. Everything downstream that depends on an origin value is origin. The instructions are never emitted. This is the subgraph folding that a traditional optimizer cannot do — it doesn't know that NaN propagation means the entire chain is predetermined.

### Container

Container preserves. It carries the last known value and a reason for the boundary crossing. This is the traceability that silent NaN propagation destroys.

```
container<T> = { value: T, reason: string }
```

When a boundary is crossed — a value goes out of range, a sensor gives a bad reading, a computation produces infinity — the result is container, not origin. The value before the boundary is preserved. You know what you were holding.

Container propagates through operations:

```
op(container(a), contents(b)) = container(a)    ; boundary propagates, value preserved
op(contents(a), container(b)) = container(b)    ; boundary propagates, value preserved
op(container(a), container(b)) = container(a)   ; first boundary preserved
```

### Contents

Contents computes. This is normal arithmetic. The inner values interact according to their type's rules.

```
op(contents(a), contents(b)) = contents(op(a, b))    ; arithmetic inside containers
```

`contents(a) + contents(b) = contents(a + b)` is the IR equivalent of `rfl` in Lean. The sort is preserved. The operation proceeds on the inner values. Zero overhead.

Within contents, the wrapper is invisible. The optimizer can treat contents operations identically to traditional operations. The sort only becomes visible when it changes — which is exactly when you want to know about it.

---

## Sort Lattice

The three sorts form a lattice for optimization:

```
        origin          (top — absorbs everything)
       /      \
  container  container   (middle — preserves, propagates)
       \      /
       contents          (bottom — normal arithmetic)
```

The optimizer uses this lattice for sort inference:

- If both operands are `contents`, the result is `contents` (bottom meets bottom = bottom)
- If either operand is `origin`, the result is `origin` (top absorbs everything)
- If either operand is `container` and neither is `origin`, the result is `container` (middle propagates)

This is a standard lattice-based dataflow analysis. Existing compiler infrastructure for lattice-based optimization applies directly.

---

## Sort Resolution Rules

### Where sorts originate

Sorts enter the IR at specific points:

| Source | Sort | Why |
|---|---|---|
| Integer/float literals | `contents` | A known value is safe territory |
| Function arguments (known callers) | Inferred from call sites | Sort propagates interprocedurally |
| Function arguments (unknown callers) | `val<T>` (dynamic) | Sort unknown, check at use |
| External input (I/O, sensors, network) | `val<T>` (dynamic) | External data is untrusted |
| Division where divisor is `contents(0)` | `origin` | Structural — not a trap |
| Division where divisor is `contents(nonzero)` | `contents` | Normal arithmetic |
| Division where divisor's value is unknown | Runtime check | The only place a check is emitted |
| Overflow / out-of-range | `container` | Last good value preserved |

### Where sorts resolve

The optimizer resolves sorts as early as possible:

1. **Constant propagation.** If both operands are literal contents, the result is contents with a known value. Standard constant folding, but sort-aware.

2. **Branch narrowing.** If a branch guard proves `%x != 0`, then within that branch, `%x` is `contents(nonzero)`. Division by `%x` is `contents`. No check emitted inside the guard.

3. **Sort propagation.** If a function always returns `contents` (provable from its body), callers know the return sort statically. No check at the call site.

4. **Origin folding.** If a value is `origin`, every value that depends on it is `origin`. The optimizer walks the dependency chain and folds. Instructions are never emitted.

---

## How Traditional IR Patterns Map

### Null pointer

```
; Traditional: the billion-dollar mistake
%p = load ptr, ptr %addr
%is_null = icmp eq ptr %p, null         ; runtime check
br i1 %is_null, label %error, label %ok

; Origin-ir: the sort carries it
%p = load val<ptr>, ptr %addr
; if %p is origin: null. the sort says so.
; if %p is contents: valid pointer. no check needed.
; the branch exists only if the sort is dynamic.
```

### NaN propagation

```
; Traditional: silent propagation through 47 layers
%x = fdiv float %a, %b                  ; might be NaN
%y = fadd float %x, %c                  ; NaN propagates silently
%z = fmul float %y, %d                  ; still NaN, still silent
; you find out at the output. or you don't.

; Origin-ir: the chain folds
%x = val.fdiv %a, %b                    ; if origin: known here
; %y and %z are never emitted.           ; the chain is folded.
; the optimizer knew at %x.
```

### Division by zero

```
; Traditional: trap or undefined behavior
%result = sdiv i32 %a, %b               ; if %b is 0: trap (x86), UB (C)

; Origin-ir: structural
%result = val.div %a, %b
; if %b is contents(0): result is origin. not a trap. a named sort.
; if %b is contents(nonzero): result is contents(a/b). normal division.
; if %b is dynamic: runtime check. the only case that costs anything.
```

### IEEE 754 mapping

The three sorts map exactly to IEEE 754:

| IEEE 754 | Origin-ir sort | What it means |
|---|---|---|
| NaN | `origin` | The system hit its boundary. Nothing to retrieve. |
| Infinity | `container` | The boundary was crossed. The direction (sign) is preserved. |
| Normal / subnormal / zero | `contents` | Safe territory. Arithmetic lives here. |

IEEE 754 built this distinction into every floating-point chip in 1985. The sort-native IR names it.

---

## The Test

The value representation is correct if:

1. **Conservativity.** Within `contents`, origin-ir produces identical results to a traditional IR. The sort wrapper adds zero information loss and zero overhead in the common case.

2. **Sort preservation.** Every operation preserves or correctly transitions the sort. No operation can produce a bare `T` — the sort is always present.

3. **Folding correctness.** Every origin-folded subgraph would have produced NaN/undefined in a traditional IR. The folding doesn't change behavior — it names what was already happening and avoids computing the predetermined result.

4. **Completeness.** Every program expressible in a traditional IR is expressible in origin-ir. The sort system adds information. It doesn't restrict expressiveness.

---

*This is the foundation. Level 2 defines the operations on these sorted values. Level 3 builds the optimizer. Level 4 benchmarks it against the traditional approach.*
