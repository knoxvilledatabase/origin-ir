# Origin IR: Operations on Sorted Values

*Level 2: Every operation defined by sort combination.*

---

## The Principle

Operations are not checked. They are defined. The sort determines behavior the same way Val α's constructors determine behavior in Foundation.lean.

Every operation follows one rule: **resolve the sort first, then compute if you're in contents.**

---

## The Universal Pattern

Most operations follow the sort lattice directly:

```
op(origin,       anything)     = origin              ; I1: left absorption
op(anything,     origin)       = origin              ; I2: right absorption
op(container(a), contents(b))  = container(a)        ; boundary propagates
op(contents(a),  container(b)) = container(b)        ; boundary propagates
op(container(a), container(b)) = container(a)        ; first boundary preserved
op(contents(a),  contents(b))  = contents(op(a, b))  ; normal arithmetic
```

This is the default. Most operations — add, sub, mul, bitwise, shifts — follow this pattern exactly. The exceptions are documented explicitly below.

---

## Arithmetic Operations

### Addition: `val.add`

Follows the universal pattern. No exceptions.

```
val.add contents(a), contents(b)  = contents(a + b)
val.add contents(a), contents(0)  = contents(a)        ; additive identity — within contents
val.add contents(0), contents(b)  = contents(b)        ; additive identity — within contents
val.add origin,      contents(b)  = origin              ; absorption
val.add contents(a), origin       = origin              ; absorption
```

Note what this means: `contents(0)` is the additive identity. It is a quantity — no apples. It is not origin. The additive identity stays in contents. This is the core distinction of the entire project expressed as an IR operation.

### Subtraction: `val.sub`

Follows the universal pattern. No exceptions.

```
val.sub contents(a), contents(b)  = contents(a - b)
val.sub contents(a), contents(a)  = contents(0)        ; not origin. quantity zero.
val.sub origin,      contents(b)  = origin
```

`a - a = contents(0)`, not origin. The counting reached zero. The counting didn't reach the ground.

### Multiplication: `val.mul`

Follows the universal pattern. No exceptions.

```
val.mul contents(a), contents(b)  = contents(a * b)
val.mul contents(0), contents(b)  = contents(0)        ; multiplicative zero — within contents
val.mul contents(a), contents(0)  = contents(0)        ; multiplicative zero — within contents
val.mul origin,      contents(b)  = origin              ; absorption — different from above
```

This is the distinction that Mathlib's 17 typeclasses exist to manage. `contents(0) * contents(5) = contents(0)` is arithmetic — zero times five is zero. `origin * contents(5) = origin` is absorption — the ground absorbs the fish. Same result in a flat type system. Different sorts in origin-ir. The type carries the difference.

### Division: `val.div`

Division is where the sort system earns its keep. This is the operation that generates most of the 97 patches.

```
val.div contents(a), contents(b)  = contents(a / b)    ; b ≠ 0, normal division
val.div contents(a), contents(0)  = container(a)       ; n/0: boundary. last value preserved.
val.div contents(0), contents(0)  = origin              ; 0/0: the ground. nothing to retrieve.
val.div origin,      anything     = origin              ; absorption
val.div anything,    origin       = origin              ; absorption
```

Three cases where traditional IR has one ("undefined" or "trap"):

| Case | Traditional IR | Origin IR | IEEE 754 equivalent |
|---|---|---|---|
| `a / b` (b ≠ 0) | Normal division | `contents(a/b)` | Normal result |
| `a / 0` (a ≠ 0) | Trap / UB | `container(a)` — last value preserved | ±Infinity |
| `0 / 0` | Trap / UB | `origin` — nothing to retrieve | NaN |

The traditional IR makes one decision for all three cases. Origin-ir names each one. The sort carries the distinction.

### Remainder: `val.rem`

Same divisor rules as division.

```
val.rem contents(a), contents(b)  = contents(a % b)    ; b ≠ 0
val.rem contents(a), contents(0)  = container(a)       ; boundary
val.rem contents(0), contents(0)  = origin              ; ground
```

### Negation: `val.neg`

Unary operation. Sort preserved.

```
val.neg contents(a)   = contents(-a)
val.neg container(a)  = container(a)     ; boundary preserved, value preserved
val.neg origin        = origin            ; absorption
```

---

## Floating-Point Arithmetic

### `val.fadd`, `val.fsub`, `val.fmul`

Follow the universal pattern, identical to their integer counterparts. The inner type is floating-point but the sort rules are the same.

```
val.fadd contents(1.0), contents(2.0)  = contents(3.0)
val.fmul contents(0.0), contents(5.0)  = contents(0.0)   ; not origin
val.fadd origin, contents(3.14)        = origin            ; absorption
```

### `val.fdiv`

Same three cases as integer division, but maps directly to IEEE 754:

```
val.fdiv contents(a), contents(b)    = contents(a / b)    ; b ≠ 0.0
val.fdiv contents(a), contents(0.0)  = container(a)       ; ±infinity — direction preserved
val.fdiv contents(0.0), contents(0.0) = origin             ; NaN — nothing to retrieve
```

### `val.sqrt`

```
val.sqrt contents(a)  = contents(√a)      ; a ��� 0
val.sqrt contents(a)  = origin             ; a < 0, result is NaN
val.sqrt container(a) = container(a)       ; boundary preserved
val.sqrt origin       = origin             ; absorption
```

### `val.log`

```
val.log contents(a)  = contents(log(a))    ; a > 0
val.log contents(0)  = container(0)        ; log(0) = -∞, boundary, value preserved
val.log contents(a)  = origin              ; a < 0, result is NaN
val.log origin       = origin              ; absorption
```

---

## Comparison Operations

Comparisons produce a `val<i1>` (sorted boolean). The sort of the result depends on the sorts of the operands.

### `val.cmp`

```
val.cmp eq  contents(a), contents(b) = contents(a == b)   ; normal comparison
val.cmp lt  contents(a), contents(b) = contents(a < b)    ; normal comparison
val.cmp eq  origin, origin           = origin              ; can't compare the ground to itself
val.cmp eq  origin, contents(a)      = contents(false)     ; origin is not any quantity
val.cmp eq  contents(a), origin      = contents(false)     ; no quantity is origin
```

Note: `origin == origin` is `origin`, not `contents(true)`. This matches IEEE 754's `NaN != NaN`. The ground is not a value that can be compared — not even to itself.

But `origin == contents(a)` is `contents(false)`. We *can* say that origin is not any quantity. That's a definite answer. The sort tells us.

### Branching on sort

Comparison enables sort-aware branching:

```
%is_contents = val.is_contents %x        ; contents(true) if %x is contents, contents(false) otherwise
val.br %is_contents, label %safe, label %boundary
```

This is the runtime sort check. It's emitted only where the sort is dynamic. Where the sort is static, the optimizer eliminates the branch entirely.

---

## Memory Operations

### Load: `val.load`

Loading from memory produces a sorted value. The sort of the pointer determines behavior:

```
val.load contents(ptr)    = val<T>          ; load from valid pointer, result sort depends on value
val.load container(ptr)   = container(...)  ; loading from boundary pointer preserves boundary
val.load origin           = origin          ; loading from null/origin: absorption, not a segfault
```

Loading from an origin pointer is not a trap. It's origin. The program knows. The chain folds. No segfault, no crash — a named state.

### Store: `val.store`

```
val.store contents(v), contents(ptr)    ; normal store
val.store contents(v), origin           ; store to null: origin. no-op. nothing to store to.
val.store origin, contents(ptr)         ; store origin to valid address: the address now holds origin
```

### Allocation

```
%p = val.alloc T              = contents(ptr)    ; fresh allocation is always contents
%p = val.alloc_checked T      = val<ptr>         ; may fail: contents if success, origin if OOM
```

---

## Control Flow

### Branch: `val.br`

Conditional branch on a sorted boolean:

```
val.br contents(true),  label %then, label %else   ; branch to %then
val.br contents(false), label %then, label %else   ; branch to %else
val.br origin,          label %then, label %else   ; branch to a dedicated %origin handler
val.br container(cond), label %then, label %else   ; branch to a dedicated %boundary handler
```

When the condition is origin or container, the branch doesn't guess. It goes to a handler that knows the sort. This is the structural equivalent of Val α's pattern matching.

### Phi: `val.phi`

Phi nodes at branch merges preserve sort information:

```
%result = val.phi [contents(%a), %bb1], [contents(%b), %bb2]   ; both contents → result is contents
%result = val.phi [contents(%a), %bb1], [origin, %bb2]          ; mixed → result is val<T> (dynamic)
%result = val.phi [origin, %bb1], [origin, %bb2]                ; both origin → result is origin
```

The optimizer narrows phi sorts where possible. If all incoming edges are the same sort, the phi result has that sort statically.

### Call: `val.call`

Function calls propagate sorts:

```
; Callee with known return sort
declare contents<i32> @pure_function(contents<i32>, contents<i32>)
%result = val.call @pure_function(%a, %b)    ; result is statically contents

; Callee with dynamic return sort
declare val<i32> @external_function(val<i32>)
%result = val.call @external_function(%x)     ; result sort is dynamic, check at use
```

Function signatures carry sort information. A function declared to return `contents<T>` guarantees the sort. The caller emits no check. A function declared to return `val<T>` may return any sort. The caller checks.

This is interprocedural sort propagation. The function boundary carries the sort the same way Val α carries the sort across Lean theorems.

### Return: `val.return`

```
val.return contents(%x)     ; returning safe territory
val.return origin            ; returning origin — caller knows
val.return container(%x)    ; returning boundary — caller knows what was held
```

---

## Conversion Operations

### Type casts: `val.cast`

Sort is preserved across type conversions:

```
val.cast contents(i32 42) to f64    = contents(f64 42.0)
val.cast container(i32 42) to f64   = container(f64 42.0)
val.cast origin to f64              = origin                 ; nothing to convert
```

### Sort transitions: `val.to_contents`

Explicit sort assertion — the programmer or a previous check guarantees the sort:

```
; After a runtime check has confirmed the sort
%checked = val.is_contents %x
val.br %checked, label %safe, label %handle

%safe:
  %x_safe = val.to_contents %x       ; sort assertion: this is contents
  ; optimizer treats %x_safe as statically contents from here forward
```

This is the only way to narrow a dynamic sort to a static sort. It requires a prior check. The optimizer verifies that a check dominates every `val.to_contents`.

---

## Aggregate Operations

### Tensor / vector operations

Sort applies to the entire tensor:

```
val.matmul contents<tensor<4x8xf32>>, contents<tensor<8x2xf32>>
    = contents<tensor<4x2xf32>>                                     ; contents closure

val.matmul origin, contents<tensor<8x2xf32>>
    = origin                                                         ; absorption — entire result folds
```

A tensor is either entirely contents, entirely origin, or container. There is no element-wise sort mixing within a single tensor value. This is a deliberate design choice — element-wise sort tracking would add complexity that the sort lattice doesn't require. If element-wise tracking is needed, the tensor is split into separate values.

### Struct / aggregate access

```
%field = val.extractvalue contents({i32, f64}), 0    = contents(i32)
%field = val.extractvalue origin, 0                   = origin
%field = val.extractvalue container({i32, f64}), 0   = container(i32)
```

Sort propagates through aggregate access. If the struct is contents, every field is contents.

---

## Summary: Which Operations Are Exceptions to the Universal Pattern?

| Operation | Exception | Why |
|---|---|---|
| `val.div`, `val.fdiv` | `contents(a) / contents(0)` produces `container` or `origin`, not contents | Division by zero is a sort transition, not a trap |
| `val.rem` | Same as division | Same reason |
| `val.sqrt` | `contents(negative)` produces `origin` | Square root of negative is a sort transition |
| `val.log` | `contents(0)` produces `container`, `contents(negative)` produces `origin` | Logarithm boundaries |
| `val.cmp` with origin | `origin == origin` is `origin`, not `contents(true)` | The ground can't be compared to itself |
| `val.cmp` origin vs contents | `origin == contents(a)` is `contents(false)` | Origin is definitively not any quantity |
| `val.load` from origin | Produces `origin`, not a segfault | Null dereference becomes a named sort |

Everything else follows the universal pattern: origin absorbs, container propagates, contents computes.

---

## The Test

The operation definitions are correct if:

1. **Every sort combination is defined.** No operation can receive sorted inputs and produce an unsorted output. The sort is always present.

2. **Contents is conservative.** `op(contents(a), contents(b))` produces the same inner value as traditional `op(a, b)` for every operation. The sort adds information. It never changes the arithmetic.

3. **Origin folding is sound.** Every chain that the optimizer folds to origin would have produced NaN, undefined, or a trap in a traditional IR. Folding doesn't change behavior — it avoids computing a predetermined result.

4. **Container preserves.** Every container result carries a value that was the last known good state before the boundary was crossed. No information is lost.

5. **The exceptions are exhaustive.** Every operation that can produce a sort transition (contents → origin, contents → container) is listed. No silent sort transitions exist.

---

*Level 1 defined what a value is. Level 2 defined what happens when values interact. Level 3 builds the optimizer that exploits this information.*
