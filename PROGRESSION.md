# Origin IR: Progression

## The Thesis

The sort must be established before the first operation is defined. Not before the first program. Not before the first compilation. Before the first *operation*.

In mathematics, Val α is defined before `+` and `×` exist. The operations are defined *on* Val α. The result: 98% of zero-management infrastructure dissolves.

In a sort-native IR, every value is a sorted value before the first `add` is emitted. The operations are defined *on* sorted values. The prediction: the same pattern — infrastructure that exists to manage undefined behavior, NaN propagation, null handling, and boundary confusion is never generated.

## The Core Question

Does a constitutive sort at the IR level produce the same dramatic reduction in computational infrastructure that Val α produces in mathematical infrastructure?

The annotative approach (origin-mlir, origin-llvm) proved the sorts are useful. The constitutive approach (origin-mathlib) proved the sorts are transformative. origin-ir tests whether the constitutive approach transfers from mathematics to computation.

## The Analogy

| | Mathematics | Computation |
|---|---|---|
| The collapsed system | Mathlib's 17 typeclasses | Flat IR values (`i32`, `f32`) |
| What the collapse generates | 9,682 `≠ 0` hypotheses | NaN propagation, null checks, UB handlers, traps |
| The annotative fix | Adding an 18th typeclass | origin-llvm pass, origin-mlir dialect |
| The constitutive fix | Val α before arithmetic | Sort-native IR before instructions |
| Predicted result | 98% less foundational infrastructure | Significant reduction in boundary-handling code |

## Levels

### Level 1 — Value representation ✓

**Spec:** [SPEC.md](SPEC.md)

Every value in origin-ir has type `val<T>` — one of three sorts: `origin`, `container<T>`, or `contents<T>`. There is no bare `T`. The sort is what a value *is*, not metadata about it. The sort lattice (origin at top, contents at bottom) enables standard dataflow analysis. IEEE 754 maps exactly: NaN is origin, infinity is container, normal values are contents.

### Level 2 — Operations on sorted values ✓

**Spec:** [OPERATIONS.md](OPERATIONS.md)

Every operation follows the universal pattern: origin absorbs, container propagates, contents computes. Exceptions are exhaustive: division (three cases where traditional IR has one), sqrt, log, and comparison involving origin. The critical distinction: `contents(0) × contents(5) = contents(0)` is arithmetic, `origin × contents(5) = origin` is absorption. Same result in a flat IR. Different sorts in origin-ir.

Tensors are sorted as a whole — one bucket, one sort. The answer fell out of the foundation without a special case.

### Level 3 — Optimizer ✓

**Spec:** [OPTIMIZER.md](OPTIMIZER.md)

Five sort-specific passes, all using existing compiler infrastructure (lattice-based dataflow, DCE, branch folding). The sort lattice is three elements — it converges trivially. The new capability: origin folding eliminates entire subgraphs whose results are predetermined. A traditional optimizer can't do this because it has no vocabulary for "this chain is predetermined." The sort gives it that vocabulary.

### Level 4 — Benchmarks ✓

**Spec:** [BENCHMARKS.md](BENCHMARKS.md)

Four test programs (transformer forward pass, Cramer's rule, ODE integrator, stb_image), each with prior data from origin-mlir or origin-llvm. Three paths measured: contents (must be free), origin (should be dramatically faster), mixed (the real-world case). Static metrics, compiled output comparison, and runtime performance counters.

The kill switch: if contents-in-contents-out ever costs more than flat-in-flat-out, the approach fails.

### Level 5 — Lowering ✓

**Spec:** [LOWERING.md](LOWERING.md) (stub), [LOWERING_FULL.md](LOWERING_FULL.md) (complete)

Statically resolved values (85-95%) lower transparently — `contents<i32>` becomes `i32`, zero overhead. Origin-folded chains lower to nothing. Dynamic sort checks lower to compare + branch on current hardware (Option B: separate register, value untouched). Option A (packed 2-bit tag) is the upgrade path to origin-isa — when sort-aware hardware exists, the residual branch cost drops to zero.

Register pressure is the honest cost of bridging to hardware that doesn't speak sorts. Dynamic sort tags are GPRs, subject to spill logic. Re-check is cheaper than spill+reload.

The IR eliminates ~90% of sort checks at compile time. The ISA makes the residual ~10% cheap in hardware. Neither replaces the other. Together they close the gap completely.

### Level 6 — Language frontends ✓

**Spec:** [FRONTENDS.md](FRONTENDS.md)

C's undefined behavior maps to three sorts without new concepts: division by zero → origin/container, null deref → origin, uninitialized read → origin (nothing was ever placed), use after free → origin (returned to the ground), buffer overflow → container, signed overflow → α's problem (not a sort question). The C frontend is simpler than clang's UB handling.

Rust's `Option<T>` maps directly (two of three sorts). Container adds vocabulary Rust doesn't have. Functional languages (`Maybe`/`Option`) map trivially. JAX/StableHLO connects through the origin-mlir bridge.

No frontend required new machinery. Every case mapped to the three sorts the IR already has.

---

## Status

**Design phase: complete.** All six levels specified. Three constructors, four rules, no new concepts at any level.

**Phase 1: complete.** Proof of concept in Rust. `val.rs` (the Foundation.lean moment), `ops.rs` (universal pattern + exceptions), `ir.rs` (SSA graph), `pass_resolve.rs` (static sort resolution), `pass_fold.rs` (origin folding). 61 tests, zero failures.

**Phase 2: complete.** All four benchmarks confirm the Mathlib parallel:

| Benchmark | Result |
|---|---|
| Transformer | 93.8% safe by construction, 12 checks, 126 ops fold from single origin |
| Cramer's rule | 0 hypotheses (standard: 8), checks only at division |
| ODE integrator | 249 ops never emitted, container preserves recovery |
| stb_image | 64/64 pixels traced, real bug caught at cause |

The kill switch was live at every level and never triggered.

## What Comes Next

### Phase 3 — The C frontend

Build the C frontend that maps UB to sorts. Compile real C programs (stb_image, tinyexpr, kiss_fft — the same codebases origin-llvm already analyzed). Compare the sort-aware compiled output against clang's output on the same code.

The FRONTENDS.md spec is complete. The UB mapping table is validated by the stb_image benchmark. The implementation follows the design.

### Phase 4 — The JAX/StableHLO path

Connect origin-ir to JAX through the StableHLO bridge. Compile a real training loop. Measure whether gradient clipping and loss scaling dissolve the way `≠ 0` hypotheses dissolved in Mathlib.

This is the path back to the original question: the AI water consumption problem. If sort-aware compilation reduces the energy cost of training and inference, the DRY principle applied to the boundary check produces a measurable result.

## The Kill Switch

Find a program where the sort-native IR produces more instructions, slower execution, or less information than the traditional IR. Not at the boundary (where the sort check fires). In the common case. If contents-in-contents-out ever costs more than flat-value-in-flat-value-out, that's information.

The sort should be free in the common case. If it isn't, that kills the approach at this level.

If something becomes more complex than traditional, we missed something at a more foundational level.
