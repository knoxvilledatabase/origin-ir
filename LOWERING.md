# Origin IR: Lowering to Current Hardware

*What does a sorted value become when it hits a traditional backend?*

---

## Static-sort values: transparent

When the sort is resolved at compile time, the sort wrapper disappears. `contents<i32>` becomes `i32`. `contents<f64>` becomes `f64`. The lowered code is identical to what a traditional compiler would emit. Zero overhead. Zero extra instructions.

This is the common case — 85-95% of values in a well-formed program. The sort was useful during optimization (it enabled origin folding, dead check elimination, sort-aware DCE). At lowering, it vanishes. Its job is done.

## Origin-folded subgraphs: nothing

Instructions eliminated by origin folding are never emitted. They don't lower because they don't exist. The lowered output is smaller than the traditional equivalent by exactly the number of instructions that were folded.

## Dynamic sort checks: compare + branch

The residual 5-15% — values whose sort the compiler couldn't prove statically — lower to a comparison and conditional branch on current hardware.

```
; Origin-ir (before lowering)
%result = val.div %a, %b          ; dynamic sort — %b's value unknown

; Lowered to traditional IR (current hardware)
%b_val = extractvalue {i2, i64} %b, 1        ; get the inner value
%b_sort = extractvalue {i2, i64} %b, 0       ; get the sort tag
%is_contents = icmp eq i2 %b_sort, 1         ; compare: is it contents?
br i1 %is_contents, label %safe, label %handle

%safe:
  %result = sdiv i64 %a_val, %b_val          ; normal division
  ; continue with contents result

%handle:
  ; %b was origin or container — result sort determined by rules
  ; no division executed
```

One comparison. One branch. On the happy path (contents — the common case), the branch predictor learns quickly. The cost is one predicted branch per dynamic check point.

*The `{i2, i64}` representation is illustrative. The actual ABI — whether sort tags travel as struct fields, separate registers, or shadow state — is specified in Level 5.*

## The future: sort-aware hardware

On a sort-aware ISA (origin-isa's RISC-V extension), the dynamic check doesn't need a comparison. The 2-bit sort tag is in the register. The ALU reads it directly.

```
; Lowered to sort-aware ISA (future hardware)
DIV r1, r2 → r3              ; ALU reads r2.sort bits
                               ; if origin: r3 = origin, no division
                               ; if contents: r3 = contents(r1/r2), normal division
                               ; zero comparison, zero branch
```

The comparison and branch disappear. The sort check is a register read — part of the instruction execution, not a separate step.

## What this means for Level 4 benchmarks

| Path | Current hardware cost | Future hardware cost |
|---|---|---|
| Static contents (85-95%) | Zero — sort eliminated at compile time | Zero — same |
| Origin-folded | Negative — fewer instructions than traditional | Negative — same |
| Dynamic check, happy path | One predicted branch | Zero — register read |
| Dynamic check, origin/container | One branch + handler (no wasted computation) | Zero — register read + handler |

The Level 4 benchmarks measure current hardware costs. The origin-isa predictions measure future hardware costs. Both are stated honestly — measured vs predicted.
