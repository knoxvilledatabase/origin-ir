# origin-ir

## What is This? 

Is a quantity of zero and the thing the numberline sits on the same thing? 

We asked: what would happen if they weren't.

The result? A sort-native intermediate representation where:

- 93.8% of a transformer's operations are safe by construction. No runtime check needed.
- 8 `≠ 0` hypotheses that mathematicians write by hand dissolved to 0.
- 249 operations that a traditional compiler executes — computing NaN through 15 steps — were never emitted.
- A real bug in production C code (stb_image, 7,988 lines) that UBSan missed was caught at the operation that caused it.



```
origin      × contents(5) = origin           // absorption. the ground took it.
contents(0) × contents(5) = contents(0)     // arithmetic. zero apples. still apples.
```

If you see why those are different, run this:

```bash
git clone https://github.com/knoxvilledatabase/origin-ir.git
cd origin-ir
cargo run
```

---

A sort-native intermediate representation. Three constructors, four rules, before the first instruction.

- **Origin** — the system hit its absolute boundary. Nothing to retrieve. Everything downstream folds.
- **Container** — the boundary was crossed. The last known value is preserved. You know what you were holding.
- **Contents** — safe territory. Arithmetic lives here. `contents(a) + contents(b) = contents(a + b)`.

The sort isn't metadata about a value. It's what a value *is*.

## What We Found

Does a constitutive sort at the IR level produce the same dramatic reduction in computational infrastructure that Val α produces in mathematical infrastructure?

Yes.

| Benchmark | What it tested | Result |
|---|---|---|
| Transformer forward pass | Depth, scale, fold | 93.8% safe by construction, 12 runtime checks, 126 ops fold from single origin |
| Cramer's rule | Hypothesis dissolution | 0 hypotheses (standard: 8), checks only at division |
| ODE integrator | Time propagation, recovery | 249 ops never emitted, container preserves last good values |
| stb_image HDR pipeline | Real C code, real bug | 64/64 pixels traced, bug caught at cause not output |

61 tests. Zero failures. The kill switch was live at every level and never triggered.

### The transformer result

4-layer transformer, 128 hidden, 4 heads. 193 operations.

The annotative approach (origin-mlir) discovered 458/499 operations were safe by analyzing the graph. The constitutive approach (origin-ir) has 181/193 operations safe by *definition* — `val_add`, `val_mul`, `val_matmul` preserve contents because that's how they're defined, not because a pass proved they do.

93.8% safe by construction vs 91.8% safe by analysis. 12 runtime checks (the exception ops: div, rsqrt) vs 41.

When origin enters at the first layer norm, 126 of 193 operations fold. A traditional compiler executes all 126 — computing NaN × weight + bias × weight through every layer. Origin-ir never emits them.

### The Cramer's rule result

The Lean benchmark showed 8 `≠ 0` hypothesis instances across 6 theorems dissolving to 0 with Val α. The IR reproduces this exactly: every mul, sub, and add in the Cramer computation resolves to contents by construction. The only unknowns are the divisions by determinant — the exact operations where the sort question is real.

The 8 hypotheses weren't 8 independent questions. They were the same question asked 8 times — "is the determinant zero?" — because the flat type system had no way to propagate the answer. Origin-ir asks once, at the division. The sort propagates.

### The ODE integrator result

20-step projectile simulation. Bad sensor reading at step 5.

Traditional: all 366 operations execute. 270 produce NaN silently through 15 steps. No detection. No recovery.

Origin-ir: 111 operations execute (contents). 249 are origin (never emitted). The moment origin entered at step 5, the result of steps 6-19 was predetermined. The chain folded. The last good velocity and position are preserved in container for recovery.

### The stb_image result

stb_image v2.30. 7,988 lines of production C code. The verified bug: `stbi_hdr_to_ldr_gamma(0.0f)` stores infinity in a global, corrupts every subsequent pixel conversion. UBSan (default) produces zero warnings.

Origin-ir: `val.div(contents(1.0), contents(0.0)) = container(1.0)`. The sort says boundary crossed. The value 1.0 is preserved. Container propagates through all 385 downstream operations to every output pixel. 64 of 64 pixels carry the container sort — every one named, every one traceable. The bug is caught at the operation that causes it, not after 64 corrupted pixels.

## Why a New IR?

The project built the sort system at every software layer:

| Repo | Layer | Approach | Result |
|---|---|---|---|
| [origin-mathlib](https://github.com/knoxvilledatabase/origin-mathlib4) | Mathematical library | **Constitutive.** Val α defined before arithmetic. | 98% reduction in foundational infrastructure |
| [origin-lang](https://github.com/knoxvilledatabase/origin-lang) | Application | Enforcement at runtime | Sort-aware values in Rust and Python |
| [origin-mlir](https://github.com/knoxvilledatabase/origin-mlir) | ML compiler | **Annotative.** Sort dialect alongside existing dialects. | Real bugs found, 458/499 ops proven safe |
| [origin-llvm](https://github.com/knoxvilledatabase/origin-llvm) | Systems compiler | **Annotative.** Pass on existing LLVM IR. | Real bugs found, zero hot-path cost |

The pattern was clear: when the sort is established *before* operations are defined (constitutive), the results are dramatic. When the sort is applied *after* operations exist (annotative), the results are useful but modest.

In origin-mathlib, Val α is defined before addition, before multiplication, before any operation. The 17 typeclasses that Mathlib uses to manage zero simply never get generated. 98% of the foundational infrastructure dissolves — not because it was compressed, but because the condition that created it was eliminated at the root.

In origin-llvm, the instructions already exist. The pass discovers sorts and annotates them. It catches real bugs. But the infrastructure was already generated. It's the equivalent of adding an 18th typeclass to Mathlib instead of replacing the foundation.

The IR is the right level because it's the last place in the compilation stack where "what is a value?" is still an open question. The language has decided. The hardware has decided. But the IR is where the compiler decides how to represent values for optimization and code generation.

## The critical distinction

`contents(0) × contents(5) = contents(0)` — arithmetic. Zero is a quantity. The smallest bound. The count that stopped at nothing. The sort stays contents. No check.

`origin × contents(5) = origin` — absorption. Origin is the ground. Not a quantity at all. The sort propagates. Everything downstream folds.

Same result in a flat IR. Different sorts in origin-ir. This distinction — between the quantity zero and the ground — is what the 17 typeclasses manage, what Val α separates, and what origin-ir's `resolve_sort` implements in one function that every non-exception operation calls.

## How origin-ir and origin-isa fit together

The IR eliminates ~90% of sort checks at compile time — static resolution proves most values are contents. The [ISA](https://github.com/knoxvilledatabase/original-arithmetic/tree/main/origin-isa) makes the residual ~10% cheap in hardware — 2-bit register reads instead of comparison and branch.

The IR is where the sort is established constitutively. The ISA is where the residual cost disappears. Neither replaces the other. The IR without the ISA still works — residual checks are branches on current hardware. The ISA without the IR is the previous approach — annotating after arithmetic exists, modest results. Together they close the gap completely.

## The type

```rust
enum Val<T> {
    Origin,
    Container(T),
    Contents(T),
}
```

Three constructors carry what 17 typeclasses guard. The sort is in the type. No hypothesis needed. No typeclass resolution. No convention. (Rust convention capitalizes enum variants — `Origin`, `Container`, `Contents` — the spec uses lowercase throughout.)

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

The three constructors and four rules are formally verified in [original-arithmetic](https://github.com/knoxvilledatabase/original-arithmetic) (509 Lean 4 theorems, zero errors, zero sorries). The [origin-mathlib](https://github.com/knoxvilledatabase/origin-mathlib4) fork proves the constitutive approach works inside the largest formal math library. origin-mlir and origin-llvm proved the annotative approach works at the compiler layer. origin-ir is the step from annotation to constitution.

Same three constructors. Same four rules. The level where they become the foundation, not an overlay.

## How this was built

This is a Human-AI collaborated effort. The human held the philosophy and identified the level — the IR is where the sort can be constitutive in computation the same way Val α is constitutive in mathematics. Claude Code did the specification and implementation. Claude Web stress-tested every design decision — the signed integer overflow question, the uninitialized read question, the register pressure question — each one a potential kill switch moment that resolved without new concepts.

The design phase produced something with the right property: it got simpler at every level instead of more complex. That's the signal.
