/// The IR graph representation.
///
/// A program is a list of instructions in SSA form. Each instruction
/// has an opcode, operand references, and a sort. The sort starts as
/// Unknown and is resolved by the passes.

/// The sort of a value at the IR level.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Sort {
    /// Safe territory. Arithmetic lives here.
    Contents,
    /// Boundary crossed. Last known value preserved.
    Container,
    /// Nothing to retrieve. Everything downstream folds.
    Origin,
    /// Not yet resolved. The passes will determine this.
    Unknown,
}

/// The operations the IR supports.
#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    /// A known constant value. Always contents.
    Constant(f64),
    /// An external input. Sort depends on declaration.
    Input { name: String, sort: Sort },
    /// Non-exception operations. Follow the universal pattern.
    Add,
    Sub,
    Mul,
    /// Exception operations. Sort transition depends on inner values.
    Div,
    Sqrt,
    Log,
    Exp,
    /// Tensor operations (sort follows universal pattern on tensors).
    MatMul,
    /// Layer norm components.
    ReduceSum,
    ReduceMean,
    /// Activation functions.
    Relu,
    Gelu,
    Softmax,
    /// Element-wise.
    Rsqrt,
}

/// A single instruction in the IR.
#[derive(Debug, Clone)]
pub struct Instruction {
    /// Unique identifier.
    pub id: usize,
    /// The operation.
    pub op: Op,
    /// References to operand instructions (by id).
    pub operands: Vec<usize>,
    /// The resolved sort of this instruction's result.
    pub sort: Sort,
}

/// A program: a list of instructions in SSA form.
#[derive(Debug, Clone)]
pub struct Program {
    pub instructions: Vec<Instruction>,
}

impl Program {
    pub fn new() -> Self {
        Program {
            instructions: Vec::new(),
        }
    }

    /// Add an instruction and return its id.
    pub fn add(&mut self, op: Op, operands: Vec<usize>) -> usize {
        let id = self.instructions.len();
        let sort = match &op {
            Op::Constant(_) => Sort::Contents, // literals are always contents
            Op::Input { sort, .. } => *sort,
            _ => Sort::Unknown,
        };
        self.instructions.push(Instruction {
            id,
            op,
            operands,
            sort,
        });
        id
    }

    /// Count instructions by sort.
    pub fn count_by_sort(&self) -> SortCounts {
        let mut counts = SortCounts::default();
        for inst in &self.instructions {
            match inst.sort {
                Sort::Contents => counts.contents += 1,
                Sort::Container => counts.container += 1,
                Sort::Origin => counts.origin += 1,
                Sort::Unknown => counts.unknown += 1,
            }
        }
        counts.total = self.instructions.len();
        counts
    }
}

#[derive(Debug, Default)]
pub struct SortCounts {
    pub contents: usize,
    pub container: usize,
    pub origin: usize,
    pub unknown: usize,
    pub total: usize,
}

impl std::fmt::Display for SortCounts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Total: {} | Contents: {} ({:.1}%) | Origin: {} | Container: {} | Unknown: {} ({:.1}%)",
            self.total,
            self.contents,
            if self.total > 0 { self.contents as f64 / self.total as f64 * 100.0 } else { 0.0 },
            self.origin,
            self.container,
            self.unknown,
            if self.total > 0 { self.unknown as f64 / self.total as f64 * 100.0 } else { 0.0 },
        )
    }
}

/// Determine if an operation is an exception (sort transition depends on inner values).
pub fn is_exception_op(op: &Op) -> bool {
    matches!(op, Op::Div | Op::Sqrt | Op::Log | Op::Rsqrt)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_is_contents() {
        let mut prog = Program::new();
        prog.add(Op::Constant(42.0), vec![]);
        assert_eq!(prog.instructions[0].sort, Sort::Contents);
    }

    #[test]
    fn input_carries_declared_sort() {
        let mut prog = Program::new();
        prog.add(Op::Input { name: "x".into(), sort: Sort::Contents }, vec![]);
        prog.add(Op::Input { name: "y".into(), sort: Sort::Unknown }, vec![]);
        assert_eq!(prog.instructions[0].sort, Sort::Contents);
        assert_eq!(prog.instructions[1].sort, Sort::Unknown);
    }

    #[test]
    fn operation_starts_unknown() {
        let mut prog = Program::new();
        let a = prog.add(Op::Constant(1.0), vec![]);
        let b = prog.add(Op::Constant(2.0), vec![]);
        prog.add(Op::Add, vec![a, b]);
        assert_eq!(prog.instructions[2].sort, Sort::Unknown);
    }
}
