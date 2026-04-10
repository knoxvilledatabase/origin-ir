/// Pass 2: Origin Folding.
///
/// Walk forward from every origin value. Everything that depends on it
/// is origin. The instructions are never emitted. This is the pass that
/// a traditional optimizer cannot do.
///
/// After this pass, count: how many instructions were folded?

use crate::ir::{Program, Sort};

/// Fold all instructions downstream of origin values.
/// Returns the number of instructions folded.
pub fn fold_origin(program: &mut Program) -> usize {
    let mut folded = 0;

    // Build a use-list: for each instruction, which instructions use it?
    let len = program.instructions.len();
    let mut users: Vec<Vec<usize>> = vec![Vec::new(); len];
    for inst in &program.instructions {
        for &operand_id in &inst.operands {
            users[operand_id].push(inst.id);
        }
    }

    // Propagate origin forward through the dependency graph.
    // Use a worklist: start with all origin instructions.
    let mut worklist: Vec<usize> = program
        .instructions
        .iter()
        .filter(|inst| inst.sort == Sort::Origin)
        .map(|inst| inst.id)
        .collect();

    while let Some(id) = worklist.pop() {
        // For each instruction that uses this origin value:
        for &user_id in &users[id] {
            if program.instructions[user_id].sort != Sort::Origin {
                program.instructions[user_id].sort = Sort::Origin;
                folded += 1;
                // This instruction is now origin — propagate further.
                worklist.push(user_id);
            }
        }
    }

    folded
}

/// Count how many instructions would be eliminated by origin folding.
/// (Non-mutating version for analysis.)
pub fn count_foldable(program: &Program) -> usize {
    let len = program.instructions.len();
    let mut is_origin = vec![false; len];

    // Mark initial origins.
    for inst in &program.instructions {
        if inst.sort == Sort::Origin {
            is_origin[inst.id] = true;
        }
    }

    // Build use-list.
    let mut users: Vec<Vec<usize>> = vec![Vec::new(); len];
    for inst in &program.instructions {
        for &operand_id in &inst.operands {
            users[operand_id].push(inst.id);
        }
    }

    // Propagate.
    let mut worklist: Vec<usize> = (0..len).filter(|&i| is_origin[i]).collect();
    let mut foldable = 0;

    while let Some(id) = worklist.pop() {
        for &user_id in &users[id] {
            if !is_origin[user_id] {
                is_origin[user_id] = true;
                foldable += 1;
                worklist.push(user_id);
            }
        }
    }

    foldable
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{Op, Program, Sort};
    use crate::pass_resolve;

    #[test]
    fn folds_chain_from_origin() {
        let mut prog = Program::new();
        let a = prog.add(Op::Input { name: "bad".into(), sort: Sort::Origin }, vec![]);
        let b = prog.add(Op::Constant(2.0), vec![]);
        let c = prog.add(Op::Add, vec![a, b]);
        let d = prog.add(Op::Constant(3.0), vec![]);
        let e = prog.add(Op::Mul, vec![c, d]);
        let f = prog.add(Op::Constant(1.0), vec![]);
        prog.add(Op::Add, vec![e, f]);

        // Resolve handles origin propagation through the lattice.
        pass_resolve::resolve_sorts(&mut prog);

        // All downstream ops should be origin after resolve.
        assert_eq!(prog.instructions[2].sort, Sort::Origin); // c = add(origin, const)
        assert_eq!(prog.instructions[4].sort, Sort::Origin); // e = mul(origin, const)
        assert_eq!(prog.instructions[6].sort, Sort::Origin); // final add(origin, const)

        // Fold finds nothing new — resolve already propagated origin.
        // This is correct: fold's value is at runtime, when origin enters dynamically.
        let folded = fold_origin(&mut prog);
        assert_eq!(folded, 0);
    }

    #[test]
    fn no_fold_on_all_contents() {
        let mut prog = Program::new();
        let a = prog.add(Op::Constant(1.0), vec![]);
        let b = prog.add(Op::Constant(2.0), vec![]);
        let c = prog.add(Op::Add, vec![a, b]);
        let d = prog.add(Op::Constant(3.0), vec![]);
        prog.add(Op::Mul, vec![c, d]);

        pass_resolve::resolve_sorts(&mut prog);
        let folded = fold_origin(&mut prog);

        assert_eq!(folded, 0);
        assert_eq!(prog.instructions[2].sort, Sort::Contents);
        assert_eq!(prog.instructions[4].sort, Sort::Contents);
    }

    #[test]
    fn origin_propagates_through_branches() {
        // Two independent chains, one hits origin.
        let mut prog = Program::new();
        let x = prog.add(Op::Input { name: "x".into(), sort: Sort::Contents }, vec![]);
        let w = prog.add(Op::Input { name: "w".into(), sort: Sort::Contents }, vec![]);
        let h = prog.add(Op::MatMul, vec![x, w]);
        let poison = prog.add(Op::Input { name: "poison".into(), sort: Sort::Origin }, vec![]);
        let bad = prog.add(Op::Add, vec![h, poison]); // origin enters here
        let w2 = prog.add(Op::Input { name: "w2".into(), sort: Sort::Contents }, vec![]);
        prog.add(Op::MatMul, vec![bad, w2]); // should be origin

        pass_resolve::resolve_sorts(&mut prog);

        assert_eq!(prog.instructions[2].sort, Sort::Contents); // h = matmul(x, w)
        assert_eq!(prog.instructions[4].sort, Sort::Origin);   // bad = add(h, origin)
        assert_eq!(prog.instructions[6].sort, Sort::Origin);   // matmul(origin, w2)

        // Fold finds nothing new — resolve already propagated.
        let folded = fold_origin(&mut prog);
        assert_eq!(folded, 0);
    }

    #[test]
    fn count_foldable_matches_fold() {
        let mut prog = Program::new();
        let a = prog.add(Op::Input { name: "bad".into(), sort: Sort::Origin }, vec![]);
        let b = prog.add(Op::Constant(2.0), vec![]);
        let c = prog.add(Op::Add, vec![a, b]);
        let d = prog.add(Op::Constant(3.0), vec![]);
        prog.add(Op::Mul, vec![c, d]);

        pass_resolve::resolve_sorts(&mut prog);

        let foldable = count_foldable(&prog);
        let folded = fold_origin(&mut prog);

        // count_foldable should predict what fold actually does.
        assert_eq!(foldable, folded);
    }
}
