# origin-ir

## What is This? 

Is a quantity of zero and the thing the number line sits on the same thing? 

We asked: what would happen if they weren't.

A sort-native intermediate representation. Three constructors, four rules, before the first instruction.

- **Origin** — nothing to retrieve. Everything downstream folds.
- **Container** — the last known value is preserved. You know what you were holding.
- **Contents** — safe territory. Arithmetic lives here.

The sort isn't metadata about a value. It's what a value *is*.

The result?

- 93.8% of a transformer's operations are safe by construction. No runtime check needed.
- 8 zero-checks in a linear solver dissolved to 1 — the compiler now asks once, at the division.
- 249 operations that a traditional compiler executes — computing NaN through 15 steps — were never emitted.
- A real bug in production C code (stb_image, 7,988 lines) that UBSan missed was caught at the operation that caused it.

If you see why those are different, run this:

```bash
git clone https://github.com/knoxvilledatabase/origin-ir.git
cd origin-ir
cargo run
```

---

## What We Found

Four benchmarks. Each tests a different claim. All four confirmed.

| Benchmark | What it tested | Result |
|---|---|---|
| Transformer forward pass | Can the compiler prove operations safe at scale? | 93.8% safe by construction, 12 runtime checks, 127 ops fold from single origin |
| Linear solver (Cramer's rule) | Do redundant zero-checks dissolve? | 8 checks → 1. The compiler asks once, at the division. |
| Projectile simulation (ODE) | Does the compiler stop computing when the result is predetermined? | 249 ops never emitted. Last good values preserved for recovery. |
| stb_image HDR pipeline | Does it catch real bugs in real code? | 64/64 pixels traced to the cause. UBSan found nothing. |

61 tests. Zero failures. The kill switch was live at every level and never triggered.

### The transformer result

4-layer transformer, 128 hidden, 4 heads. 193 operations.

Traditional compiler: every operation executes. NaN propagates silently. You find out at the output — or you don't.

Origin-ir: 181 of 193 operations are safe by construction — `val_add`, `val_mul`, `val_matmul` preserve contents because that's how they're defined. 12 runtime checks at the operations that can actually produce a sort transition (div, rsqrt). Everything else is guaranteed. Not analyzed. Defined.

When origin enters at the first layer norm, 127 of 193 operations fold. The traditional compiler computes NaN × weight + bias × weight through every layer. Origin-ir never emits those instructions.

### The linear solver result

Cramer's rule for a 2×2 system. 17 operations.

Traditional approach: 8 zero-checks scattered across the computation. Every operation that touches the determinant has to ask "is it zero?" because the type system can't propagate the answer.

Origin-ir: 15 operations resolve to contents by construction. 2 runtime checks — the two divisions by determinant. The compiler asks "is the determinant zero?" once, at the division. The answer propagates. Every downstream operation inherits the sort without checking again.

### The ODE integrator result

20-step projectile simulation. Bad sensor reading at step 5.

Traditional: all 366 operations execute. 270 produce NaN silently through 15 steps. No detection. No recovery. The final answer may look plausible.

Origin-ir: 111 operations execute (contents). 249 are origin (never emitted). The moment origin entered at step 5, the result of steps 6-19 was predetermined. The chain folded. The last good velocity and position are preserved in container — recovery is possible because you know what you were holding when the sensor failed.

### The stb_image result

stb_image v2.30. 7,988 lines of production C code. The verified bug: `stbi_hdr_to_ldr_gamma(0.0f)` stores infinity in a global, corrupts every subsequent pixel conversion. UBSan (default) produces zero warnings.

Origin-ir: `val.div(contents(1.0), contents(0.0)) = container(1.0)`. The sort says boundary crossed. The value 1.0 is preserved. Container propagates through all 385 downstream operations to every output pixel. 64 of 64 pixels carry the container sort — every one named, every one traceable. The bug is caught at the operation that causes it, not after 64 corrupted pixels.

## The critical distinction

`contents(0) × contents(5) = contents(0)` — arithmetic. Zero is a quantity. The sort stays contents. No check.

`origin × contents(5) = origin` — absorption. Origin is the ground. Not a quantity. The sort propagates. Everything downstream folds.

Same result in a traditional compiler. Different sorts here. This distinction is what `resolve_sort` implements in one function that every non-exception operation calls.

## Why a new IR?

Traditional compilers have no concept of sort. A value is an `i32` or an `f64`. The number zero, a null pointer, and a meaningful quantity are all the same bit pattern. When something goes wrong — division by zero, NaN, null dereference — the compiler either traps, propagates silently, or calls it undefined behavior.

Origin-ir makes every value a sorted value before the first instruction is emitted. The sort determines behavior:

1. **Statically contents?** Normal arithmetic. Zero overhead. No check emitted.
2. **Statically origin?** Everything downstream folds. Don't emit the instructions.
3. **Sort unknown?** Emit a runtime sort check at this point — and only at this point.

The traditional compiler emits all instructions and relies on runtime NaN propagation. Every operation executes even when the result is predetermined. The sort-native compiler knows the result before emission. The instructions that would compute a predetermined answer are never generated.

## How origin-ir and origin-isa fit together

The IR eliminates ~90% of sort checks at compile time. The [ISA extension](https://github.com/knoxvilledatabase/original-arithmetic/tree/main/origin-isa) makes the residual ~10% cheap in hardware — 2-bit register reads instead of comparison and branch.

The IR is where the sort is established. The ISA is where the residual cost disappears. Neither replaces the other. Together they close the gap completely.

## The type

```rust
enum Val<T> {
    Origin,
    Container(T),
    Contents(T),
}
```

Three constructors. The sort is in the type. (Rust convention capitalizes enum variants — `Origin`, `Container`, `Contents` — the spec uses lowercase throughout.)

The universal pattern — implemented once, called by every non-exception operation:

```rust
fn resolve_sort<T, F>(a: Val<T>, b: Val<T>, op: F) -> Val<T>
where F: Fn(T, T) -> T {
    match (a, b) {
        (Val::Origin, _) | (_, Val::Origin) => Val::Origin,
        (Val::Container(x), _) => Val::Container(x),
        (_, Val::Container(y)) => Val::Container(y),
        (Val::Contents(x), Val::Contents(y)) => Val::Contents(op(x, y)),
    }
}
```

One rule. Not seventeen.

## Build

```bash
cd origin-ir
cargo test    # 61 tests
cargo run     # all four benchmarks
```

## The specification

| File | Level | What it answers |
|---|---|---|
| [SPEC.md](SPEC.md) | 1 | What is a value (`val<T>`, sort lattice, IEEE 754 mapping) |
| [OPERATIONS.md](OPERATIONS.md) | 2 | What happens when sorted values interact (universal pattern + exceptions) |
| [OPTIMIZER.md](OPTIMIZER.md) | 3 | What the compiler can see (5 passes, lattice-based dataflow) |
| [BENCHMARKS.md](BENCHMARKS.md) | 4 | How to measure the prediction (4 test programs, 3 paths) |
| [LOWERING.md](LOWERING.md) | stub | Quick reference for residual check cost |
| [LOWERING_FULL.md](LOWERING_FULL.md) | 5 | How sorted values become machine code (ABI, register strategy, FFI) |
| [FRONTENDS.md](FRONTENDS.md) | 6 | How languages connect (C UB table, Rust, Python/JAX, functional) |

## Where this came from

The three constructors and four rules are formally verified in [original-arithmetic](https://github.com/knoxvilledatabase/original-arithmetic) (509 Lean 4 theorems, zero errors, zero sorries). The formal proof established the foundation. [origin-mathlib](https://github.com/knoxvilledatabase/origin-mathlib4) demonstrated it inside the largest formal math library. [origin-mlir](https://github.com/knoxvilledatabase/origin-mlir) and [origin-llvm](https://github.com/knoxvilledatabase/origin-llvm) proved the sorts work at the compiler layer. origin-ir is where the sort becomes the foundation of the IR itself — not an annotation on existing operations, but the type that operations are defined on.

## How this was built

This is a Human-AI collaborated effort. The human held the philosophy and identified the level — the IR is where the sort can be constitutive, established before the first operation is defined. Claude Code did the specification and implementation. Claude Web stress-tested every design decision — the signed integer overflow question, the uninitialized read question, the register pressure question — each one a potential kill switch moment that resolved without new concepts.

The design phase produced something with the right property: it got simpler at every level instead of more complex. That's the signal.
