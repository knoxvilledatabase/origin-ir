# Origin IR: The Optimizer

*Level 3: What can a compiler see when the sort is in the type?*

---

## The Principle

A traditional optimizer sees values. It can prove things about values — this constant is 42, that branch is always taken, this load is redundant. But it cannot prove things about sorts because sorts don't exist in the representation.

The origin-ir optimizer sees sorted values. It can prove everything a traditional optimizer proves, plus:

- This entire subgraph is predetermined — don't emit it.
- This value is safe territory — don't check it.
- This boundary was crossed here — name it.

The optimizer doesn't add these capabilities. The value representation gives them for free. The optimizer just follows the lattice.

---

## The Sort Lattice as Dataflow Framework

Compilers already know how to do lattice-based dataflow analysis. Constant propagation, range analysis, alias analysis — they all use lattices. The sort lattice is one more, and it's simpler than most.

```
        origin          (top — absorbs everything)
       /      \
  container  container   (middle — preserves, propagates)
       \      /
       contents          (bottom — normal arithmetic)
```

**Meet rules:**

```
meet(origin,    anything)   = origin       ; top wins
meet(anything,  origin)     = origin       ; top wins
meet(container, container)  = container    ; boundary persists
meet(container, contents)   = container    ; boundary persists
meet(contents,  contents)   = contents     ; safe territory
```

Every existing dataflow framework in every existing compiler can run this lattice. It's three elements. It converges in one pass on a DAG, at most loop-depth + 1 passes on a CFG. The infrastructure already exists. The lattice is new.

---

## Pass 1: Static Sort Resolution

The first pass walks the IR and resolves every sort it can prove statically. This is the pass that eliminates the majority of runtime checks.

### What it resolves

**Literals.** Every literal is `contents`. This is the entry point — known values are safe territory.

```
%x = contents.i32 42        ; statically contents
%y = contents.f64 3.14      ; statically contents
```

**Arithmetic on known sorts.** If both operands are statically `contents`, the result is statically `contents`. No check emitted. This is the common case.

```
%a = contents.i32 10
%b = contents.i32 20
%c = val.add %a, %b          ; both contents → result is contents. resolved statically.
```

**Known origin.** If any operand is statically `origin`, the result is statically `origin`.

```
%bad = origin
%y = val.mul %bad, %x        ; origin × anything = origin. resolved statically.
%z = val.add %y, %w          ; origin + anything = origin. resolved statically.
; %y and %z are both origin. the optimizer knows without computing.
```

**Branch narrowing.** If a branch guard tests the sort, the optimizer narrows the sort within the branch.

```
%is_safe = val.is_contents %x
val.br %is_safe, label %then, label %else

%then:
  ; %x is statically contents here. every operation on %x is sort-resolved.
  %result = val.div %a, %x       ; no runtime check — %x is known contents
```

**Function signatures.** If a function is declared to return `contents<T>`, the caller knows the sort statically.

```
declare contents<i32> @pure_add(contents<i32>, contents<i32>)
%result = val.call @pure_add(%a, %b)    ; result is statically contents
```

### What it doesn't resolve

**External input.** Values from I/O, network, sensors — the sort is unknown. A runtime check is emitted.

**Division where the divisor's value is unknown.** The sort depends on whether the divisor is zero. If the optimizer can't prove it either way, a runtime check is emitted at the division.

**Function calls to external code.** If the function's return sort isn't declared, the result is dynamic.

These are the residual checks — the only places in the compiled output where the sort costs anything at runtime. Everything else is resolved at compile time.

### The metric

After this pass, count:
- **Statically resolved:** instructions where the sort is known. Zero runtime cost.
- **Dynamically checked:** instructions where a runtime check was emitted.

The ratio is the first benchmark. The origin-mlir analysis of a transformer forward pass showed 458/499 statically resolved (91.8%). The prediction: a sort-native IR resolves at least as many, likely more, because the sort information propagates further when it's constitutive.

---

## Pass 2: Origin Folding

This is the pass that a traditional optimizer cannot do. It's the compiler equivalent of Val α dissolving `≠ 0` hypotheses.

### What it does

When a value is origin, everything that depends on it is origin. The optimizer walks the dependency graph forward from every origin value and folds the entire downstream chain.

```
; Before origin folding
%x = val.fdiv %a, %b              ; runtime check: if %b is contents(0.0)
                                    ; ...result could be origin
%y = val.fmul %x, %w1             ; if %x is origin, this is origin
%z = val.fadd %y, %bias           ; if %y is origin, this is origin
%out = val.fmul %z, %w2           ; if %z is origin, this is origin

; After origin folding (when origin is detected at %x)
%x = origin                        ; sort check fired
%out = origin                      ; 3 instructions folded. never emitted.
```

Three instructions eliminated. Not optimized. Not simplified. Eliminated. They would have computed NaN × weight + bias × weight, and the result was predetermined the moment origin entered.

### How it scales

The origin-mlir analysis showed:

- A single origin at the first layer norm divide has 16 downstream dependents
- Across a 4-layer transformer, 107 operations (21.4% of all ops) are directly foldable from a single origin entry point
- The chain depth averages 4-6 operations, with the longest chains spanning entire layers

In a traditional compiler, all 107 operations execute. NaN propagates silently through every one. In origin-ir, the optimizer folds them at compile time. At runtime, the moment origin is detected, the chain returns origin immediately. No computation.

### The interaction with static resolution

Origin folding and static resolution work together:

1. Static resolution proves most values are contents — no checks needed.
2. The few remaining runtime checks are at division, sqrt, log — the operations that can produce origin.
3. When a runtime check detects origin, origin folding kicks in — everything downstream folds.

The common path (all contents) has zero overhead. The origin path (something went wrong) computes nothing — it folds. The traditional compiler pays full cost on both paths.

---

## Pass 3: Container Propagation

Container is the traceability pass. When a boundary is crossed, the last known value propagates forward.

### What it does

```
; Division produces container (a / 0 where a ≠ 0)
%x = val.fdiv contents(7.0), contents(0.0)    ; container(7.0), reason: "fdiv by zero"

; Container propagates — the value 7.0 is carried forward
%y = val.fadd %x, %bias                        ; container(7.0)
%z = val.fmul %y, %w                           ; container(7.0)
```

The value 7.0 — what you were holding when the boundary was crossed — travels through the chain. A traditional compiler would propagate infinity (IEEE 754 `7.0 / 0.0 = +inf`), then `inf + bias = inf`, then `inf * w = inf` or `NaN`. The information about where the boundary was crossed and what the last good value was is destroyed.

Container preserves it. The operation that crossed the boundary, the reason, and the last value are all carried. This is the traceability that silent NaN propagation destroys.

### What it enables

**Recovery.** The origin-llvm projectile simulation showed this: when a bad sensor reading entered at step 5, the traditional path propagated NaN through 14 steps. The sort-aware path preserved the last good values and recovered. Container is what makes recovery possible — you know what you were holding.

**Diagnostics.** Instead of "the output is NaN," the system says "division by zero at operation 47, last value was 7.0, 16 downstream operations tainted." The information exists because container carried it.

**Training.** In ML training, a gradient that hits a boundary doesn't need to poison the entire batch. The container carries the last good gradient. The training loop can skip the boundary step and continue from the last known state.

---

## Pass 4: Dead Check Elimination

After passes 1-3, some runtime checks are provably unnecessary. This pass eliminates them.

### Redundant checks

If a value's sort was already checked, downstream uses don't need to check again:

```
; Before dead check elimination
%is_safe = val.is_contents %x
val.br %is_safe, label %safe, label %handle

%safe:
  %y = val.div %a, %x             ; %x is known contents here
  %check_y = val.is_contents %y   ; redundant — contents / contents = contents
  val.br %check_y, ...            ; this branch is always taken
```

The second check is dead. The sort lattice proves it. The optimizer eliminates it.

### Dominated checks

If a check dominates all uses of a value, the uses are in known-sort territory:

```
; Single check guards multiple uses
%is_safe = val.is_contents %input
val.br %is_safe, label %compute, label %handle

%compute:
  %a = val.mul %input, %w1         ; no check — dominated by %is_safe
  %b = val.add %a, %bias           ; no check — contents + contents = contents
  %c = val.mul %b, %w2             ; no check — contents * contents = contents
  ; entire compute block is check-free
```

One check at the entry. Zero checks inside. The sort narrowing from the branch propagates through every operation in the dominated block.

### Loop-invariant sorts

If a value's sort doesn't change across loop iterations, the check hoists out of the loop:

```
; Before
loop:
  %x = val.load %ptr               ; sort unknown each iteration?
  %check = val.is_contents %x
  ...

; After (if %ptr is loop-invariant and the stored value's sort is invariant)
%check = val.is_contents %initial_x   ; hoisted out of loop
loop:
  %x = val.load %ptr                   ; sort known — no check inside loop
  ...
```

---

## Pass 5: Sort-Aware Dead Code Elimination

Traditional DCE eliminates instructions whose results are unused. Sort-aware DCE goes further: it eliminates instructions whose results are *predetermined*.

### Traditional DCE

```
%x = add i32 %a, %b     ; result unused → eliminate
```

### Sort-aware DCE

```
%x = val.mul origin, %w          ; result is origin regardless of %w
%y = val.add %x, %bias           ; result is origin regardless of %bias  
%z = val.fmul %y, %w2            ; result is origin regardless of %w2
; all three are predetermined. eliminate all three.
; %w, %bias, %w2 don't need to be computed either if they have no other uses.
```

This is the sort-aware equivalent. The instructions aren't unused — they feed into the output. But their results are predetermined. A traditional optimizer can't see this because it doesn't know that NaN × anything = NaN. The sort-native optimizer knows that origin × anything = origin. The chain is dead by sort, not by use.

### The cascade

When origin-folded instructions are eliminated, their operands may become unused. Standard DCE then eliminates the operands. The cascade continues until no more eliminations are possible.

```
; Before
%w = val.load %weight_ptr          ; load weight
%x = val.mul origin, %w            ; origin × weight = origin
%y = val.add %x, %bias             ; origin + bias = origin

; After sort-aware DCE + standard DCE cascade
; %w load is eliminated — its only use was eliminated
; %x, %y eliminated — predetermined
; result: origin (one constant, no computation)
```

The weight was loaded from memory. That load is now dead. The memory access is eliminated. In a traditional compiler, the load executes, the multiply executes, the add executes — all computing a result that was predetermined the moment origin entered.

---

## Pass Pipeline

The passes run in order, each building on the previous:

```
1. Static Sort Resolution     — resolve every sort provable at compile time
2. Origin Folding              — fold subgraphs downstream of origin values
3. Container Propagation       — carry last known values through boundaries
4. Dead Check Elimination      — remove redundant runtime sort checks
5. Sort-Aware DCE              — eliminate instructions with predetermined results
6. Standard Optimization       — constant folding, CSE, loop opts, etc. (unchanged)
7. Lowering                    — sort-resolved code lowers to target IR / machine code
```

Passes 1-5 are sort-specific. Pass 6 is the existing optimization pipeline, unchanged. It runs on cleaner input because the sort passes already eliminated dead subgraphs, redundant checks, and predetermined chains.

Pass 7 lowers to the target. Sort information that was resolved statically produces clean, branch-free code. The remaining dynamic checks lower to minimal branch instructions.

---

## What the Optimizer Sees That a Traditional Optimizer Can't

| Capability | Traditional optimizer | Origin-ir optimizer |
|---|---|---|
| "This value is a known constant" | Yes (constant propagation) | Yes |
| "This branch is always taken" | Yes (branch folding) | Yes |
| "This instruction is unused" | Yes (DCE) | Yes |
| "This entire chain is predetermined" | **No** | **Yes** (origin folding) |
| "This value is safe — no check needed" | **No** | **Yes** (static sort resolution) |
| "This boundary was crossed here, carrying this value" | **No** | **Yes** (container propagation) |
| "This runtime check is redundant" | Partially (redundant null checks) | **Yes** (dead check elimination) |
| "These instructions compute a known result" | Partially (constant folding) | **Yes** (sort-aware DCE) |

The first three rows are identical. The optimizer doesn't lose anything. The last five rows are new — capabilities that exist because the sort is in the type.

---

## The Prediction

For a typical numerical program:

- **Pass 1** resolves 85-95% of values statically. Most values in a well-formed program are contents. The common path is entirely sort-resolved. Zero runtime checks emitted for these.

- **Pass 2** folds the origin path. When origin is detected (at runtime, at one of the few dynamic check points), the downstream chain folds instantly. The traditional compiler would execute every instruction in the chain.

- **Pass 3** preserves traceability on the container path. No traditional equivalent exists.

- **Passes 4-5** eliminate residual checks and predetermined instructions. The compiled output is smaller than the traditional equivalent.

- **Pass 6** runs on cleaner input. Fewer instructions, fewer branches, simpler control flow. Existing optimizations become more effective because the sort passes already removed the noise.

The net result: fewer instructions emitted, fewer branches, smaller binaries, faster execution on the origin path, identical execution on the contents path. The sort is free in the common case. It saves work in every other case.

---

## The Test

The optimizer is correct if:

1. **Soundness.** Every optimization preserves program behavior. If the traditional IR would produce value X, the origin-ir optimizer produces `contents(X)`. If the traditional IR would produce NaN, the optimizer produces `origin`. If the traditional IR would produce infinity, the optimizer produces `container`.

2. **The common case is free.** On an all-contents program, the optimizer produces identical machine code to a traditional compiler. Zero overhead. Zero extra branches. The sort wrapper is invisible.

3. **The origin case is faster.** On a program where origin enters, the optimizer produces fewer instructions than the traditional compiler. The chain folds. The traditional compiler computes the full NaN propagation chain.

4. **Monotonicity.** The sort lattice never goes backwards. Once a value is resolved as contents, it stays contents. Once a value is origin, it stays origin. The optimizer never changes a sort to a less-determined state.

---

*Level 1 defined the value. Level 2 defined the operations. Level 3 defined what the optimizer can see. Level 4 measures whether the prediction holds.*
