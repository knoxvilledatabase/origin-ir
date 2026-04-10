# Origin IR: Language Frontends

*Level 6: Every language that compiles through origin-ir gets the sort for free.*

---

## The Principle

The frontend should simplify, not patch. If a language frontend requires more machinery than mapping the language's existing concepts to the three sorts, that's a signal — either the IR foundation is missing something, or the frontend is doing work the IR should handle.

The test: does the frontend emit operations and let the IR handle the sort? Or does the frontend make policy decisions the IR doesn't resolve?

---

## C: Naming the Unnamed

C has undefined behavior — situations where the standard says "anything can happen." The compiler is allowed to assume UB doesn't occur and optimize accordingly. The programmer is responsible for preventing it.

Origin-ir names what C left unnamed. Every case of C undefined behavior maps to a sort the IR already has. No new concepts. No new operations. The frontend maps.

### The UB table

| C undefined behavior | Sort | Why |
|---|---|---|
| Division by zero (0/0) | `origin` | Asked the part to be the whole. Nothing to retrieve. |
| Division by zero (n/0) | `container(n)` | Boundary crossed. Last value preserved. |
| Null dereference | `origin` | Load from nothing. Nothing to retrieve. |
| Signed overflow | α's problem | Bucket too small. Not a sort transition. |
| Uninitialized read | `origin` | Nothing was ever placed. Nothing to retrieve. |
| Use after free | `origin` | The value was returned to the ground. Nothing to retrieve. |
| Buffer overflow (write) | `container` | Boundary of the allocation crossed. Last valid value preserved. |

Five map to sorts the IR already has. Two are α questions the frontend was already deciding. The frontend doesn't make policy decisions. It accurately maps what happened.

### What the C frontend does

```c
// C source
int x = a / b;
```

```
; Traditional clang output — must choose: trap? UBSan? assume no UB?
; The decision tree is in the frontend.

; Origin-ir frontend — emits the operation, IR handles the sort
%result = val.div contents(%a), %b
; If %b is contents(0) and %a is contents(0): origin
; If %b is contents(0) and %a is nonzero: container(%a)
; If %b is contents(nonzero): contents(a/b)
; The frontend didn't decide. The IR knew.
```

```c
// C source
int *p = NULL;
int x = *p;
```

```
; Traditional clang — UB. "Anything can happen."

; Origin-ir frontend
%p = origin                        ; NULL is origin
%x = val.load %p                   ; load from origin = origin
; The chain folds. No segfault. No "anything can happen." Named.
```

```c
// C source
int x;          // uninitialized
int y = x + 1;  // UB in C
```

```
; Origin-ir frontend
%x = origin                        ; nothing was ever placed. nothing to retrieve.
%y = val.add %x, contents(1)       ; origin + anything = origin
; The chain folds. The compiler knows at compile time.
```

```c
// C source
free(p);
int x = *p;    // use after free — UB in C
```

```
; Origin-ir frontend
; after free: %p's sort transitions to origin (returned to the ground)
%x = val.load %p                   ; load from origin = origin
; Named. Traceable. Not "anything can happen."
```

### What disappears from clang

| Clang today | Origin-ir frontend |
|---|---|
| UB decision tree per operation | Emit the operation. IR handles the sort. |
| `-fwrapv` / `-ftrapv` / UBSan flags | α's overflow semantics. Frontend picks the inner type. Already doing this. |
| Assume-no-UB optimizations | Sort-aware optimizations. Stronger — the sort proves more than "assumed not UB." |
| Silent UB propagation | Origin propagation. Named. Traceable. Foldable. |

The C frontend for origin-ir is simpler than clang's UB handling. Clang has a decision tree for every UB case. Origin-ir has three sorts. The frontend maps. The IR resolves.

### Signed overflow: α's problem, not the sort's problem

The frontend picks the inner type's overflow semantics. This is the same choice clang already makes:

| Clang flag | α behavior | Sort |
|---|---|---|
| Default (UB) | Compiler assumes no overflow | `contents` (optimizer exploits this — same as today) |
| `-fwrapv` | Two's complement wraparound | `contents(wrapped_result)` |
| `-ftrapv` | Trap on overflow | α signals → `container(last_value)` |
| UBSan overflow check | Check and report | α signals → `container(last_value)` |

The sort doesn't change. The sort says `contents + contents = contents`. What `a + b` means when it exceeds α's representable range is α's business. The frontend was always choosing. Origin-ir doesn't add a new choice. It stops conflating the overflow question with the sort question.

---

## Rust: Already There

Rust's type system already speaks two of the three sorts.

| Rust | Origin-ir | Mapping |
|---|---|---|
| `None` | `origin` | Direct. Nothing to retrieve. |
| `Some(T)` | `contents(T)` | Direct. Safe territory. |
| — | `container(T)` | Rust doesn't have this. `Option<T>` has no "boundary with last value." |

The Rust frontend maps `Option<T>` to `val<T>`:

```rust
// Rust source
let x: Option<i32> = Some(42);
let y: Option<i32> = None;
```

```
; Origin-ir
%x = contents.i32 42
%y = origin
```

Direct. No machinery. No policy decisions. `match` on `Option` maps to `val.is_contents` + branch. Pattern matching maps to sort dispatch.

### What Rust gains

Rust already prevents null dereference and use-after-free at compile time through the borrow checker. Origin-ir doesn't replace that. What it adds:

**Container.** Rust's `Option<T>` is `Some | None`. There's no middle ground — no "something went wrong but here's the last value." Origin-ir's container gives Rust a vocabulary for partial failure that preserves information. A Rust library could return `Val<T>` instead of `Option<T>` when the last known value matters.

**Sort propagation through arithmetic.** Rust handles `None` through pattern matching — the programmer checks. Origin-ir propagates sorts through operations — the compiler checks. `None` used in arithmetic without a match is a compile-time error in Rust. `origin` used in arithmetic in origin-ir folds the chain. Same safety, different mechanism.

**Cross-language sort preservation.** When Rust calls C through FFI, the sort information currently stops at the boundary. With origin-ir as the shared IR, the sort propagates across the FFI. Rust's `None` and C's null pointer are the same sort — origin. The compiler sees through the boundary.

---

## Python: Naming None

Python's `None` is origin. Values are contents. There is no container — when something goes wrong in Python, you get an exception, and the value is gone.

```python
# Python source
x = None
y = x + 1    # TypeError at runtime
```

```
; Origin-ir (if Python compiled through origin-ir)
%x = origin
%y = val.add %x, contents(1)      ; origin + anything = origin
; Fold at compile time. No TypeError at runtime.
; The sort knew before execution.
```

Python is interpreted, not compiled. The path to origin-ir is through:

1. **Cython / mypyc** — Python-to-C compilers that could target origin-ir instead of C.
2. **JAX / NumPy** — numerical Python that already compiles through XLA/StableHLO. The origin-mlir bridge connects here.
3. **origin-lang** — the existing `pip install origin-lang` package, which already provides `Origin`, `Boundary`, `Contents` in Python.

The Python frontend is the least direct path but has the highest leverage for AI — JAX models compiled through origin-ir get sort-aware optimization on every tensor operation.

### NaN in NumPy

```python
# NumPy today
a = np.array([1.0, 0.0, 3.0])
b = np.array([0.0, 0.0, 1.0])
c = a / b                          # [inf, nan, 3.0] — silent, mixed
d = c + 1.0                        # [inf, nan, 4.0] — nan propagates silently
```

Through origin-ir, the tensor is one bucket. If any element produces NaN, the tensor's sort transitions. But the per-element behavior is α's problem — NumPy's `errstate` context manager already controls this. The sort system adds the name and the propagation. NumPy already has the policy.

---

## JAX / StableHLO: The ML Path

This is the highest-leverage frontend for the AI water consumption question.

JAX exports models as StableHLO IR. Origin-mlir already tested the four rules against real StableHLO operations. The path:

```
JAX model → StableHLO → origin-ir → optimized sort-aware code → backend
```

### What changes

| JAX today | JAX through origin-ir |
|---|---|
| NaN propagates silently through forward pass | Origin folds the chain at detection |
| Gradient clipping as a training patch | Sort-aware gradient skips boundary steps |
| Loss scaling to prevent overflow | Sort catches overflow at the operation, not at the loss |
| No indication where NaN originated | Container carries the operation, reason, and last value |

### The training loop

```python
# JAX today
loss = model(batch)
grads = jax.grad(loss_fn)(params)
grads = jax.tree.map(lambda g: jnp.clip(g, -1.0, 1.0), grads)  # clip — a patch
params = update(params, grads)
```

```python
# JAX through origin-ir (conceptual)
loss = model(batch)                    # sort-aware forward pass
grads = jax.grad(loss_fn)(params)      # sort-aware backward pass
# no clipping — the sort system caught boundary steps
# grads that hit container were skipped, not clipped
params = update(params, grads)
```

Gradient clipping is a patch for silent NaN propagation. If the sort names the boundary and the training loop can skip boundary steps instead of clipping all gradients, the patch dissolves. The same pattern as `≠ 0` hypotheses dissolving in Mathlib — the infrastructure exists to manage a problem that the sort prevents.

---

## Functional Languages: The Cleanest Mapping

Haskell, OCaml, F#, Scala — languages with `Maybe` / `Option` types.

| Language | Type | Origin-ir mapping |
|---|---|---|
| Haskell | `Nothing` | `origin` |
| Haskell | `Just a` | `contents(a)` |
| OCaml | `None` | `origin` |
| OCaml | `Some a` | `contents(a)` |
| Scala | `None` | `origin` |
| Scala | `Some(a)` | `contents(a)` |

These languages already separate "no value" from "a value." The sort system adds container (boundary with last value) and the propagation rules. The mapping is direct. The frontend is trivial.

---

## The Frontend Test: Summary

| Language | Frontend complexity | Why |
|---|---|---|
| C | **Simpler than clang's UB handling** | UB decision tree replaced by sort mapping |
| Rust | **Direct mapping** | `Option<T>` is two of three sorts. Container is new vocabulary. |
| Python (via JAX) | **Bridge through StableHLO** | origin-mlir already tested the path |
| Functional languages | **Trivial** | `Maybe`/`Option` maps directly |

No language frontend required new machinery. Every frontend either simplified (C), mapped directly (Rust, functional), or connected through an existing bridge (Python/JAX).

The principle held: the frontend simplifies, it doesn't patch.

---

## The Kill Switch

If any language frontend requires the sort system to add a concept it doesn't have — a fourth sort, a new operation category, a policy decision the IR can't resolve — that's information. Either the language has a genuine need the sort system doesn't cover, or the IR foundation needs something added back.

Through C, Rust, Python, and the functional languages, the kill switch was never triggered. Every case mapped to the three sorts. Every UB case was either a sort question the IR handles or an α question the frontend was already deciding.

The sort system didn't need to know about C's UB categories, Rust's borrow checker, Python's exceptions, or Haskell's monads. It needed three constructors and four rules. The languages provided the rest.

---

*Level 1 defined the value. Level 2 defined the operations. Level 3 defined the optimizer. Level 4 defined the benchmarks. Level 5 defined the lowering. Level 6 defined the frontends. The specification is complete.*
