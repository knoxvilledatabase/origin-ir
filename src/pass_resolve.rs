/// Pass 1: Static Sort Resolution.
///
/// Walk the IR and resolve every sort provable at compile time.
/// Constants are contents. Operations on known-contents operands produce contents.
/// Origin absorbs. Container propagates. The lattice does the work.
///
/// After this pass, count: how many instructions are statically resolved
/// vs how many remain Unknown (needing runtime checks)?

use crate::ir::{is_exception_op, Op, Program, Sort};

/// Resolve sorts for all instructions in the program.
/// Returns the number of sorts that were resolved in this pass.
pub fn resolve_sorts(program: &mut Program) -> usize {
    let mut resolved = 0;

    // Single forward pass over SSA — each instruction's operands are
    // defined before it, so we can resolve in order.
    for i in 0..program.instructions.len() {
        let inst = &program.instructions[i];

        // Skip already-resolved instructions.
        if inst.sort != Sort::Unknown {
            continue;
        }

        // Input sorts are declared, not computed. Don't override them.
        if matches!(inst.op, Op::Input { .. }) {
            continue;
        }

        let operand_sorts: Vec<Sort> = inst
            .operands
            .iter()
            .map(|&id| program.instructions[id].sort)
            .collect();

        let new_sort = resolve_instruction_sort(&inst.op, &operand_sorts);

        if new_sort != Sort::Unknown {
            program.instructions[i].sort = new_sort;
            resolved += 1;
        }
    }

    resolved
}

/// Resolve the sort of a single instruction given its operand sorts.
fn resolve_instruction_sort(op: &Op, operand_sorts: &[Sort]) -> Sort {
    // If any operand is unknown, the result is unknown (can't resolve statically).
    // Exception: if another operand is origin, origin wins regardless.
    let has_unknown = operand_sorts.iter().any(|s| *s == Sort::Unknown);
    let has_origin = operand_sorts.iter().any(|s| *s == Sort::Origin);
    let has_container = operand_sorts.iter().any(|s| *s == Sort::Container);

    // Origin absorbs — even if other operands are unknown.
    if has_origin {
        return Sort::Origin;
    }

    // If any operand is unknown and none is origin, we can't fully resolve.
    // But if one operand is container and none is origin, container propagates.
    if has_unknown {
        if has_container {
            // Container beats unknown (container propagates, unknown doesn't absorb).
            // Actually: we don't know the unknown operand. It could be origin.
            // Conservative: can't resolve.
            return Sort::Unknown;
        }
        return Sort::Unknown;
    }

    // All operands are resolved (contents or container, no unknown, no origin).

    if has_container {
        return Sort::Container;
    }

    // All operands are contents. Apply the operation rules.
    if is_exception_op(op) {
        // Exception operations might produce a sort transition.
        // At the static level, we can only resolve if we know the values.
        // For now: if all operands are contents, the *common case* is contents.
        // The runtime check handles the edge cases (div by zero, sqrt of negative).
        //
        // This is the honest tradeoff: exception ops with all-contents operands
        // are *probably* contents but we can't prove it without knowing values.
        // Mark as Unknown — the runtime check decides.
        Sort::Unknown
    } else {
        // Non-exception operation. All operands contents → result is contents.
        // The universal pattern. No check needed.
        Sort::Contents
    }
}

/// Run resolution to fixpoint (for programs with loops / back-edges).
/// For straight-line SSA, one pass is sufficient.
pub fn resolve_to_fixpoint(program: &mut Program) -> usize {
    let mut total_resolved = 0;
    loop {
        let resolved = resolve_sorts(program);
        if resolved == 0 {
            break;
        }
        total_resolved += resolved;
    }
    total_resolved
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{Op, Program, Sort};

    #[test]
    fn resolves_add_of_constants() {
        let mut prog = Program::new();
        let a = prog.add(Op::Constant(1.0), vec![]);
        let b = prog.add(Op::Constant(2.0), vec![]);
        prog.add(Op::Add, vec![a, b]);

        resolve_sorts(&mut prog);
        assert_eq!(prog.instructions[2].sort, Sort::Contents);
    }

    #[test]
    fn resolves_chain_of_contents() {
        let mut prog = Program::new();
        let a = prog.add(Op::Constant(1.0), vec![]);
        let b = prog.add(Op::Constant(2.0), vec![]);
        let c = prog.add(Op::Add, vec![a, b]);
        let d = prog.add(Op::Constant(3.0), vec![]);
        prog.add(Op::Mul, vec![c, d]);

        resolve_sorts(&mut prog);
        assert_eq!(prog.instructions[2].sort, Sort::Contents); // add
        assert_eq!(prog.instructions[4].sort, Sort::Contents); // mul
    }

    #[test]
    fn origin_absorbs_through_chain() {
        let mut prog = Program::new();
        let a = prog.add(Op::Input { name: "bad".into(), sort: Sort::Origin }, vec![]);
        let b = prog.add(Op::Constant(2.0), vec![]);
        let c = prog.add(Op::Add, vec![a, b]);
        let d = prog.add(Op::Constant(3.0), vec![]);
        prog.add(Op::Mul, vec![c, d]);

        resolve_sorts(&mut prog);
        assert_eq!(prog.instructions[2].sort, Sort::Origin); // add
        assert_eq!(prog.instructions[4].sort, Sort::Origin); // mul — origin folded
    }

    #[test]
    fn div_stays_unknown_even_with_contents_operands() {
        // Division is an exception — can't prove the divisor is non-zero statically.
        let mut prog = Program::new();
        let a = prog.add(Op::Input { name: "x".into(), sort: Sort::Contents }, vec![]);
        let b = prog.add(Op::Input { name: "y".into(), sort: Sort::Contents }, vec![]);
        prog.add(Op::Div, vec![a, b]);

        resolve_sorts(&mut prog);
        assert_eq!(prog.instructions[2].sort, Sort::Unknown); // can't prove y ≠ 0
    }

    #[test]
    fn unknown_input_blocks_resolution() {
        let mut prog = Program::new();
        let a = prog.add(Op::Constant(1.0), vec![]);
        let b = prog.add(Op::Input { name: "ext".into(), sort: Sort::Unknown }, vec![]);
        prog.add(Op::Add, vec![a, b]);

        resolve_sorts(&mut prog);
        assert_eq!(prog.instructions[2].sort, Sort::Unknown);
    }

    #[test]
    fn container_propagates() {
        let mut prog = Program::new();
        let a = prog.add(Op::Input { name: "edge".into(), sort: Sort::Container }, vec![]);
        let b = prog.add(Op::Constant(5.0), vec![]);
        prog.add(Op::Add, vec![a, b]);

        resolve_sorts(&mut prog);
        assert_eq!(prog.instructions[2].sort, Sort::Container);
    }

    #[test]
    fn origin_beats_container() {
        let mut prog = Program::new();
        let a = prog.add(Op::Input { name: "dead".into(), sort: Sort::Origin }, vec![]);
        let b = prog.add(Op::Input { name: "edge".into(), sort: Sort::Container }, vec![]);
        prog.add(Op::Add, vec![a, b]);

        resolve_sorts(&mut prog);
        assert_eq!(prog.instructions[2].sort, Sort::Origin);
    }

    #[test]
    fn matmul_contents_contents_is_contents() {
        let mut prog = Program::new();
        let a = prog.add(Op::Input { name: "x".into(), sort: Sort::Contents }, vec![]);
        let b = prog.add(Op::Input { name: "w".into(), sort: Sort::Contents }, vec![]);
        prog.add(Op::MatMul, vec![a, b]);

        resolve_sorts(&mut prog);
        assert_eq!(prog.instructions[2].sort, Sort::Contents);
    }

    #[test]
    fn relu_contents_is_contents() {
        let mut prog = Program::new();
        let a = prog.add(Op::Input { name: "x".into(), sort: Sort::Contents }, vec![]);
        prog.add(Op::Relu, vec![a]);

        resolve_sorts(&mut prog);
        assert_eq!(prog.instructions[1].sort, Sort::Contents);
    }
}
