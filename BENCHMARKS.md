# Origin IR: Benchmark Specification

*Level 4: Does the prediction hold?*

---

## What We're Measuring

One question: **does a constitutive sort at the IR level produce the same dramatic reduction in computational infrastructure that Val α produces in mathematical infrastructure?**

The Mathlib evidence: 98% less foundational code, 53% of theorems dissolved or simplified, 3.5x faster typeclass inference. All from defining the sort before arithmetic.

The prediction: the same pattern transfers to computation. Fewer instructions, fewer branches, smaller binaries, faster execution on the origin path, identical execution on the contents path.

The benchmark proves or disproves this.

---

## The Three Paths

Every benchmark measures three scenarios. The sort system's value is different on each path.

### The contents path (happy path)

All inputs are contents. No boundary is crossed. No origin is encountered. This is normal program execution — the common case.

**Prediction:** identical to traditional compilation. Zero overhead. The sort was resolved statically. The wrapper disappeared at lowering. The machine code is the same.

**Kill switch:** if the contents path is *slower* than the traditional path, the approach fails. The sort must be free in the common case.

### The origin path (something went wrong)

Origin enters the computation. In a traditional compiler, NaN propagates silently through every downstream operation. Every instruction executes. You find out at the output — or you don't.

**Prediction:** dramatically faster than traditional. The optimizer folded the downstream chain. At runtime, the moment origin is detected, the result is known. No downstream computation.

**Measurement:** how many instructions were eliminated? How much faster is the origin path?

### The mixed path (real-world)

Some inputs are statically contents, some are dynamic. This is the realistic scenario — most values are known safe, a few need runtime checks.

**Prediction:** faster than traditional. The static values have zero overhead. The dynamic checks are predicted branches (one comparison each). The total cost is the branch cost on the residual checks — a small fraction of the program.

**Measurement:** how many runtime checks were emitted? What's the branch prediction hit rate? What's the net cost?

---

## The Test Programs

Four programs, chosen because the project already has data on each from prior work.

### Test 1: Transformer forward pass

**Source:** 4-layer transformer, 128 hidden, 4 heads. Exported from JAX as StableHLO.

**Prior data (origin-mlir, annotative):** 499 operations. 458 statically safe (91.8%). 41 needing runtime checks. 107 operations foldable from a single origin entry.

**What origin-ir should show:**

| Metric | Traditional | Origin-ir | How measured |
|---|---|---|---|
| Total IR instructions | 499 | ≤499 | Count after optimization |
| Statically resolved sorts | 0 (no sort concept) | ≥458 | Count after Pass 1 |
| Origin-foldable instructions | 0 (NaN propagates) | ≥107 | Count after Pass 2 |
| Runtime sort checks | 0 (silent propagation) | ≤41 | Count after Pass 4 |
| Machine instructions (contents path) | N | ≤N | Count after lowering |
| Machine instructions (origin path) | N (full propagation) | N − folded | Count after lowering |

**The honest comparison:** the traditional compiler emits zero checks and zero sort overhead — but it also catches zero errors and folds zero origin chains. Origin-ir emits up to 41 checks but eliminates up to 107 downstream operations when origin is detected. The question is whether the net is positive.

### Test 2: Matrix solver (Cramer's rule)

**Source:** 2×2 and 3×3 linear system solvers using Cramer's rule.

**Prior data (original-arithmetic, Lean):** 8 `≠ 0` hypothesis instances across 6 theorems in the standard approach. Zero on the Val side. Every Val proof is `rfl`.

**What origin-ir should show:**

| Metric | Traditional | Origin-ir |
|---|---|---|
| Division guards (det ≠ 0 checks) | 8 runtime checks or caller-side assertions | 0 static, 1 dynamic (at the determinant division) |
| Code after determinant is known contents | Identical in both | Identical in both |
| Code when determinant is zero | Undefined / trap / NaN propagation | Origin. Chain folds. No wasted division. |

This is the smallest test case. If origin-ir can't win here — the exact scenario the Lean benchmarks proved — something is wrong with the implementation.

### Test 3: ODE integrator (projectile simulation)

**Source:** 20-step projectile simulation with a bad sensor reading at step 5.

**Prior data (origin-llvm):** Traditional: 14 steps of NaN, final answer looks fine. Sort-aware: detected at step 5, 14 tainted, 6 clean. Sort-aware with recovery: 0 tainted, all clean.

**What origin-ir should show:**

| Metric | Traditional | Origin-ir |
|---|---|---|
| Steps computed after bad reading | 14 (all NaN) | 0 (origin folds the chain) |
| Recovery possible | No (values are NaN) | Yes (container carries last values) |
| Total instructions (origin path) | 14 × step_cost | 1 × origin detection |

This test measures the origin folding cascade. A single bad input at step 5 should fold steps 6-20 instantly. The traditional compiler computes all 14 NaN propagation steps.

### Test 4: stb_image HDR pipeline

**Source:** stb_image v2.30. 7,988 lines of real C code.

**Prior data (origin-llvm):** 2,757 sort findings. One verified bug: `stbi_hdr_to_ldr_gamma(0.0f)` stores infinity in a global, corrupts every subsequent pixel conversion.

**What origin-ir should show:**

| Metric | Traditional | Origin-ir |
|---|---|---|
| Silent NaN/inf propagation paths | 2,757 (unknown to compiler) | 0 (each one is sort-classified) |
| The gamma bug | Undetected — inf stored silently | `container` — boundary named, value preserved |
| Runtime checks needed | 0 (no awareness) | Only at division/sqrt/log points |
| Downstream corruption from gamma bug | Every subsequent pixel | Folds at the container — no corruption |

This test measures real-world impact. The bug exists in production code today. A traditional compiler doesn't see it. Origin-ir names it.

---

## Static Metrics (No Execution Needed)

For each test program, compiled through both a traditional pipeline and origin-ir:

| Metric | What it measures | How |
|---|---|---|
| Total IR instructions emitted | Raw IR size | Count instructions after optimization |
| Statically resolved sorts | How much the compiler proved at compile time | Count after Pass 1 |
| Origin-folded instructions | Subgraphs that never need to execute | Count after Pass 2 |
| Dead checks eliminated | Redundant runtime checks removed | Count after Pass 4 |
| Sort-aware DCE eliminations | Instructions with predetermined results | Count after Pass 5 |
| Runtime sort checks remaining | The residual dynamic cost | Count after all passes |
| Machine instructions emitted | Final compiled output size | Count after lowering |
| Branch instructions emitted | The branch cost of residual checks | Count after lowering |

### The key ratios

**Sort resolution ratio:** statically resolved / total values. Predicted: 85-95%.

**Folding ratio:** origin-folded instructions / total instructions on origin path. Predicted: proportional to chain depth downstream of origin entry points.

**Check density:** runtime checks / total instructions. Predicted: 5-15%. These are the only instructions that cost anything at runtime.

**Instruction reduction:** (traditional instructions − origin-ir instructions) / traditional instructions. The headline number.

---

## Runtime Metrics (Execution Required)

Same test programs, same inputs, same hardware. Measured with hardware performance counters where available.

| Metric | What it measures | How |
|---|---|---|
| Wall clock time | End-to-end speed | `time` / `perf stat` |
| Instructions retired | How many instructions the CPU actually executed | `perf stat` — instructions |
| Branch instructions | Total branches in the compiled output | `perf stat` — branches |
| Branch mispredictions | Cost of dynamic sort checks | `perf stat` — branch-misses |
| Cache misses | Memory behavior difference | `perf stat` — cache-misses |
| Energy per operation | The original question (AI water consumption) | Same methodology as Rust/Python benchmark |

### Per-path measurements

Each test program is run three times:

1. **All-contents input.** Every value is safe. Measures the common-case overhead (should be zero).
2. **Origin-triggering input.** A value causes origin. Measures chain folding benefit.
3. **Realistic input.** Mix of safe and unsafe values. Measures net real-world impact.

---

## The Predictions

Based on the pattern across every level of the project:

| Metric | Predicted | Evidence |
|---|---|---|
| Contents path overhead | Zero | Val α within contents is `rfl` — zero overhead by construction |
| Origin path speedup | Proportional to chain depth | origin-mlir: 107/499 ops foldable from single origin |
| Sort resolution ratio | 85-95% | origin-mlir: 458/499 = 91.8% on transformer |
| Instruction reduction (origin path) | 20-50% fewer instructions | Depends on chain depth and origin frequency |
| Branch cost of residual checks | <1% of total branches | Residual checks are 5-15% of instructions, each is one branch |
| Energy per operation | Measurably less on origin path | Fewer instructions = less energy. Already measured in Rust/Python. |

### The Mathlib parallel

| | Mathlib | Computation (predicted) |
|---|---|---|
| What dissolves | 9,682 `≠ 0` hypotheses | Runtime NaN checks, null guards, UB handlers |
| Reduction in infrastructure | 98% less foundational code | 85-95% of sort checks eliminated at compile time |
| What remains | α's own arithmetic properties | Residual dynamic checks at division/sqrt/log |
| What's unchanged | Pure math (47% of theorems) | Pure computation on contents values |

---

## How to Run

When the implementation exists:

```bash
# Compile test program through both pipelines
origin-ir-compile test_transformer.c -o test_origin --emit-stats
traditional-compile test_transformer.c -o test_traditional

# Static comparison
diff-stats test_origin.stats test_traditional.stats

# Runtime comparison (contents path)
perf stat ./test_origin --input=safe_data
perf stat ./test_traditional --input=safe_data

# Runtime comparison (origin path)
perf stat ./test_origin --input=bad_sensor_data
perf stat ./test_traditional --input=bad_sensor_data

# Runtime comparison (mixed path)
perf stat ./test_origin --input=realistic_data
perf stat ./test_traditional --input=realistic_data
```

---

## The Kill Switch

Two conditions that end the approach:

1. **Contents path is slower.** If `contents(a) + contents(b)` costs more than `a + b` on any test program, the sort wrapper has overhead. The approach fails at this level.

2. **Instruction count is higher.** If origin-ir emits more total instructions than the traditional compiler on any test program (contents path), the sort system is adding complexity instead of removing it. The approach fails.

The origin path and mixed path are where origin-ir should win. The contents path is where it must not lose. That's the honest test.

---

*Level 1 defined the value. Level 2 defined the operations. Level 3 defined the optimizer. Level 4 defines how to measure whether the prediction holds. Level 5 specifies the full lowering. Level 6 connects language frontends.*
