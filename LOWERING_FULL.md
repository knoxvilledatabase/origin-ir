# Origin IR: Full Lowering Specification

*Level 5: How sorted values become machine code.*

---

## The Principle

The sort did its job during optimization. Passes 1-5 resolved 85-95% of sorts statically, folded origin chains, eliminated dead checks, and removed predetermined instructions. What reaches the lowering stage is cleaner, smaller IR.

Lowering has one job: turn sorted IR into machine code without losing information and without adding cost where the sort was already resolved.

---

## The Two Cases

### Case 1: Sort resolved statically (85-95% of values)

The sort wrapper disappears. The value lowers as its inner type. `contents<i32>` becomes `i32`. `contents<f64>` becomes `f64`. `contents<tensor<4x8xf32>>` becomes `tensor<4x8xf32>`.

No struct. No tag. No extra register. No overhead. The sort existed in the IR for optimization. It served its purpose. At lowering, it's gone.

This is the same thing that happens in Val α — within contents, the wrapper is invisible. `contents(a) * contents(b) = contents(a * b)` is `rfl`. The sort doesn't show up in the arithmetic. It shows up in the type system. At the machine level, there is no type system. The sort has already done its work.

Origin-folded values don't lower at all. They were eliminated. The instructions don't exist.

### Case 2: Sort unresolved at lowering (5-15% of values)

These are the values where the compiler couldn't prove the sort statically. They need a runtime representation. This is where the ABI decision matters.

---

## The ABI Decision

Three options for representing unresolved sorts at runtime. Each has a trade-off.

### Option A: Tag in register (packed)

The sort tag shares the register with the value. Two high bits are the sort, the remaining bits are the value.

```
; 64-bit register layout
[SS][VVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVV]
 ^sort (2 bits)                    ^value (62 bits)

; Sort encoding
00 = origin
01 = contents
10 = container
11 = reserved
```

**Advantages:**
- One register per value. No register pressure increase.
- Sort check is a shift + mask + compare. Fast.
- Matches the origin-isa proposal exactly — if hardware support arrives, the representation is already correct.

**Disadvantages:**
- Steals 2 bits from the value. 62-bit integers instead of 64-bit. For most programs this doesn't matter. For programs that use the full 64-bit range, it does.
- Floating-point values can't share the register — the FPU expects IEEE 754 format in the full 64 bits. Float sort tags must go elsewhere.

**Best for:** integer values, pointers, values that don't need the full bit range.

### Option B: Tag in separate register (unpacked)

The sort tag lives in a separate register. The value register is untouched.

```
; Two registers per unresolved value
r1 = sort tag (i2, in a GPR)
r2 = value (full 64-bit, untouched)

; Sort check
cmp r1, #CONTENTS
beq .safe
```

**Advantages:**
- Value is unmodified. Full 64-bit range. IEEE 754 floats work natively.
- Sort check is one compare + one branch. Same cost as a null check.
- Clean separation — the sort register is a normal GPR, no special hardware needed.

**Disadvantages:**
- Register pressure. Every unresolved value uses two registers instead of one.
- On register-constrained architectures (x86 with 16 GPRs), this could cause spills.

**Best for:** floating-point values, values that need the full bit range, architectures with enough registers (ARM64 has 31 GPRs — the pressure is manageable).

**Register pressure reality:** the 5-15% average is the right number for a well-formed program, but register pressure is a worst-case problem, not an average problem. If a function has 8 dynamic sort values in flight simultaneously, that's 8 extra GPRs — on x86-64 with 16 general-purpose registers, that's meaningful. On RISC-V with 32 GPRs, less so.

Dynamic sort tags are allocated as GPRs by the register allocator, subject to the same spill logic as any other value. On register-constrained targets, the compiler should prefer to spill sort tags before value registers — the sort tag is cheaper to recompute than the value it describes.

This opens an optimization: if a sort tag would spill, the compiler may prefer to re-check the sort at use rather than spill and reload. A sort re-check is one compare + one branch. A spill and reload is a store, a cache line touch, and a load. Re-checking is cheaper in most cases.

### Option C: Shadow state

The sort tags live in a separate data structure — a shadow stack or a sort table — indexed by value identity.

```
; Value in register (untouched)
r1 = value

; Sort in shadow table (memory)
sort_table[value_id] = CONTENTS | CONTAINER | ORIGIN

; Sort check
load r_sort, [sort_table + value_id * 2]
cmp r_sort, #CONTENTS
beq .safe
```

**Advantages:**
- Zero register pressure. Values are untouched. Sort tags are in memory.
- Scales to any number of unresolved values.
- Matches the metadata model from origin-llvm — sort information lives alongside the IR, not inside it.

**Disadvantages:**
- Memory access for every sort check. Even with caching, this is slower than a register read.
- The sort table must be maintained across function calls, which adds ABI complexity.

**Best for:** programs with many unresolved values, debugging/diagnostic builds where traceability matters more than speed.

---

## The Recommendation

**Option B (separate register) as the default.** Option A as an optimization for integer-only code paths. Option C for diagnostic builds.

The reasoning:

1. **Option B preserves the value.** The inner value is untouched. IEEE 754 floats, full-range integers, pointers — all work natively. This is the least surprising choice for backend authors and the easiest to verify.

2. **The register cost is bounded.** Only 5-15% of values are unresolved. The rest lowered without a tag (Case 1). On a 4-layer transformer with 499 operations, at most 41 values carry a runtime tag. Register pressure from 41 extra GPRs across the entire function is manageable, especially with the optimizer having already eliminated the origin-folded chains.

3. **Option A is a valid optimization.** For integer-heavy code paths where the full 64-bit range isn't needed, the packed representation saves a register per unresolved value. The lowering pass can choose per-value based on type and usage analysis.

4. **Option C is the diagnostic mode.** When traceability matters more than speed — debugging, testing, safety-critical verification — the shadow table carries full sort + reason + last-value information for every value, not just the unresolved ones. This is the mode that catches the stb_image gamma bug with full provenance.

---

## Lowering by Sort State

### contents (statically resolved)

```
; Origin-ir
%x = contents<i32> 42
%y = val.add %x, %z          ; %z also statically contents

; Lowered (identical to traditional)
mov r1, #42
add r1, r1, r2                ; no tag, no check, no overhead
```

### origin (statically resolved)

```
; Origin-ir
%x = origin
%y = val.mul %x, %w          ; origin × anything = origin (folded by Pass 2)
%z = val.add %y, %bias       ; origin + anything = origin (folded by Pass 2)

; Lowered: nothing
; %y and %z were eliminated. %x is a constant origin.
; If the result is used, it's a constant load of the origin sentinel.
```

### contents (dynamically resolved, after a runtime check)

```
; Origin-ir
%is_safe = val.is_contents %input
val.br %is_safe, label %compute, label %handle

%compute:
  %a = val.mul %input, %w1
  %b = val.add %a, %bias

; Lowered (Option B)
cmp r_sort_input, #CONTENTS
bne .handle

.compute:
  mul r1, r_input, r_w1           ; no tag — sort was checked at entry
  add r1, r1, r_bias              ; no tag — contents propagated
```

Inside the guarded block, no tags. No checks. The sort was checked once at the entry. Everything inside is bare arithmetic. The same code a traditional compiler would emit.

### Dynamic sort check at division

```
; Origin-ir
%result = val.div %a, %b     ; %b is dynamically sorted

; Lowered (Option B)
cmp r_sort_b, #CONTENTS
bne .handle_sort

; Check for zero divisor (within contents)
cmp r_b, #0
beq .div_by_zero

; Normal division
sdiv r_result, r_a, r_b
mov r_sort_result, #CONTENTS
b .continue

.div_by_zero:
cmp r_a, #0
beq .origin                    ; 0/0 → origin
mov r_sort_result, #CONTAINER  ; n/0 → container, last value preserved
mov r_result, r_a
b .continue

.origin:
mov r_sort_result, #ORIGIN
b .continue

.handle_sort:
; %b was origin or container — apply lattice rules
...

.continue:
```

This is the full cost of a dynamic sort check at a division. On the happy path (both contents, non-zero divisor): two compares, two predicted branches, one division. The traditional compiler does one division and either traps or produces UB. The origin-ir version costs two extra branches on the happy path but handles every edge case with named sorts instead of traps, UB, or silent NaN.

---

## Lowering to LLVM IR

The primary lowering target. Origin-ir lowers to standard LLVM IR, then LLVM handles register allocation, instruction selection, and machine code emission.

### Statically resolved values

```
; Origin-ir
%x = contents<i32> 42
%y = val.add %x, %z

; Lowered to LLVM IR
%x = i32 42
%y = add i32 %x, %z
```

Identical. No sort representation at all.

### Dynamically resolved values (Option B)

```
; Origin-ir
%result = val.div %a, %b

; Lowered to LLVM IR
%sort_b = load i2, ptr %sort_ptr_b
%is_contents = icmp eq i2 %sort_b, 1
br i1 %is_contents, label %contents_path, label %sort_handler

%contents_path:
  %b_val = ... ; extract inner value
  %is_zero = icmp eq i64 %b_val, 0
  br i1 %is_zero, label %div_zero, label %normal_div

%normal_div:
  %result = sdiv i64 %a_val, %b_val
  store i2 1, ptr %sort_ptr_result        ; contents
  br label %continue

%div_zero:
  %a_is_zero = icmp eq i64 %a_val, 0
  br i1 %a_is_zero, label %origin, label %container
  ...
```

### With origin-llvm metadata

When lowering to LLVM IR that will be processed by the origin-llvm pass, sort information attaches as metadata:

```
%result = sdiv i64 %a, %b, !sort !{!"contents"}
```

The origin-llvm verifier reads the metadata and confirms the sort is consistent. The proof chain from origin-ir through LLVM is preserved.

---

## Lowering Tensors

A sorted tensor is one bucket. The sort applies to the whole tensor.

### Statically contents tensor

```
; Origin-ir
%t = contents<tensor<4x8xf32>>
%result = val.matmul %t, %w

; Lowered: standard matmul, no sort overhead
%result = linalg.matmul ins(%t, %w) outs(%init)
```

### Dynamically sorted tensor

The sort tag is per-tensor, not per-element. One check for the entire tensor.

```
; Origin-ir
%t = val<tensor<4x8xf32>>        ; sort unknown
%result = val.matmul %t, %w

; Lowered: one check, then standard matmul
%sort_t = load i2, ptr %sort_ptr_t
%is_contents = icmp eq i2 %sort_t, 1
br i1 %is_contents, label %compute, label %handle

%compute:
  %result = linalg.matmul ins(%t, %w) outs(%init)    ; standard, no per-element checks
```

One check for a 4×8 tensor. Not 32 checks. The sort is on the bucket.

---

## Function Call ABI

### Callee with declared sorts

```
; Origin-ir
declare contents<i32> @pure_function(contents<i32>, contents<i32>)

; Lowered: standard calling convention, no sort tags passed
declare i32 @pure_function(i32, i32)
```

The sort declaration was a compile-time contract. At the call site, the caller already knows the argument sorts are contents (or checked them). The callee already knows it returns contents. No tags cross the function boundary.

### Callee with dynamic sorts

```
; Origin-ir
declare val<i32> @external_function(val<i32>)

; Lowered (Option B): sort tag in a separate argument register
; Platform-specific: on ARM64, sort tag in x8, value in x0
; On x86-64, sort tag in a predetermined register (e.g., r11)
declare {i2, i32} @external_function({i2, i32})
```

For functions that cross sort boundaries (external code, callbacks, FFI), the sort tag is passed alongside the value. The calling convention specifies which register carries the tag.

### The FFI boundary

When calling into code that doesn't know about sorts (libc, external libraries):

```
; Origin-ir calls traditional function
%input_val = val.to_inner %input          ; extract inner value, assert contents
%result = call i32 @traditional_function(i32 %input_val)
%sorted_result = val.from_contents %result ; wrap result as contents
```

The sort is checked before crossing the FFI boundary (val.to_inner asserts contents). The return value is wrapped. The traditional function sees and returns bare values. The sort system is invisible to external code.

---

## The Cost Summary

| What | Cost on current hardware |
|---|---|
| Statically resolved value (85-95%) | Zero. Lowers identically to traditional. |
| Origin-folded chain | Negative. Fewer instructions than traditional. |
| Dynamic sort check (happy path) | One compare + one predicted branch per check point. |
| Dynamic sort check (origin detected) | One compare + one branch + chain folds (net positive). |
| Sort tag register (Option B) | One GPR per unresolved value. Bounded by 5-15% of total. |
| Function call with declared sorts | Zero. Tags don't cross the boundary. |
| Function call with dynamic sorts | One extra register per dynamic argument. |
| FFI boundary | One assertion + one wrap per crossing. |

---

## The Connection to origin-isa

On current hardware, the dynamic sort check costs one compare + one branch. On sort-aware hardware (origin-isa), it costs zero — the ALU reads the sort bits from the register.

Option A (packed tag in register) is the ABI that maps directly to the origin-isa register format. If the ISA extension is adopted, Option A code runs natively. No re-lowering needed. The sort bits that origin-ir packed into the high bits are the sort bits that the ALU reads.

This is the upgrade path: origin-ir ships today with Option B (separate register) for correctness and compatibility. When sort-aware hardware exists, origin-ir switches to Option A and the residual branch cost drops to zero.

---

*Level 1 defined the value. Level 2 defined the operations. Level 3 defined the optimizer. Level 4 defined the benchmarks. Level 5 defined how sorted values become machine code. Level 6 connects language frontends.*
