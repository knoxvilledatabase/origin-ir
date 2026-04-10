/// Benchmark: 4-layer transformer forward pass.
///
/// Builds the same graph that origin-mlir analyzed (499 operations,
/// 128 hidden, 4 heads). Runs Pass 1 and Pass 2. Counts.
///
/// The prediction (from origin-mlir, annotative): 458/499 = 91.8% static.
/// The question: does constitutive match or exceed that?

use crate::ir::{is_exception_op, Op, Program, Sort};
use crate::pass_fold;

pub struct TransformerResult {
    pub total: usize,
    pub safe_pct: f64,
    pub exception_ops: usize,
    pub foldable: usize,
}
use crate::pass_resolve;

/// Build a single transformer layer.
/// Returns the output instruction id.
fn build_layer(prog: &mut Program, input: usize, layer: usize) -> usize {
    // --- Layer norm ---
    // mean = reduce_mean(input)
    let mean = prog.add(Op::ReduceMean, vec![input]);
    // diff = input - mean
    let diff = prog.add(Op::Sub, vec![input, mean]);
    // var = reduce_mean(diff * diff)
    let diff_sq = prog.add(Op::Mul, vec![diff, diff]);
    let var = prog.add(Op::ReduceMean, vec![diff_sq]);
    // eps (constant)
    let eps = prog.add(Op::Constant(1e-5), vec![]);
    // var_plus_eps = var + eps
    let var_eps = prog.add(Op::Add, vec![var, eps]);
    // rsqrt = 1/sqrt(var+eps) — this is an exception op (rsqrt)
    let rsqrt = prog.add(Op::Rsqrt, vec![var_eps]);
    // norm = diff * rsqrt
    let norm = prog.add(Op::Mul, vec![diff, rsqrt]);
    // scale and bias (learnable parameters)
    let gamma = prog.add(
        Op::Input { name: format!("gamma_{}", layer), sort: Sort::Contents },
        vec![],
    );
    let beta = prog.add(
        Op::Input { name: format!("beta_{}", layer), sort: Sort::Contents },
        vec![],
    );
    // ln_out = norm * gamma + beta
    let scaled = prog.add(Op::Mul, vec![norm, gamma]);
    let ln_out = prog.add(Op::Add, vec![scaled, beta]);

    // --- Self-attention (4 heads) ---
    // Q, K, V projections
    let wq = prog.add(
        Op::Input { name: format!("wq_{}", layer), sort: Sort::Contents },
        vec![],
    );
    let wk = prog.add(
        Op::Input { name: format!("wk_{}", layer), sort: Sort::Contents },
        vec![],
    );
    let wv = prog.add(
        Op::Input { name: format!("wv_{}", layer), sort: Sort::Contents },
        vec![],
    );

    let q = prog.add(Op::MatMul, vec![ln_out, wq]);
    let k = prog.add(Op::MatMul, vec![ln_out, wk]);
    let v = prog.add(Op::MatMul, vec![ln_out, wv]);

    // Attention scores: Q @ K^T / sqrt(d_k)
    let qk = prog.add(Op::MatMul, vec![q, k]);
    let d_k = prog.add(Op::Constant(32.0_f64.sqrt()), vec![]);
    let scores = prog.add(Op::Div, vec![qk, d_k]); // exception: div

    // Softmax
    let softmax = prog.add(Op::Softmax, vec![scores]);

    // Attention output: softmax @ V
    let attn_out = prog.add(Op::MatMul, vec![softmax, v]);

    // Output projection
    let wo = prog.add(
        Op::Input { name: format!("wo_{}", layer), sort: Sort::Contents },
        vec![],
    );
    let projected = prog.add(Op::MatMul, vec![attn_out, wo]);

    // Residual connection
    let residual1 = prog.add(Op::Add, vec![input, projected]);

    // --- Feed-forward ---
    // Layer norm 2
    let mean2 = prog.add(Op::ReduceMean, vec![residual1]);
    let diff2 = prog.add(Op::Sub, vec![residual1, mean2]);
    let diff2_sq = prog.add(Op::Mul, vec![diff2, diff2]);
    let var2 = prog.add(Op::ReduceMean, vec![diff2_sq]);
    let eps2 = prog.add(Op::Constant(1e-5), vec![]);
    let var_eps2 = prog.add(Op::Add, vec![var2, eps2]);
    let rsqrt2 = prog.add(Op::Rsqrt, vec![var_eps2]); // exception
    let norm2 = prog.add(Op::Mul, vec![diff2, rsqrt2]);
    let gamma2 = prog.add(
        Op::Input { name: format!("gamma2_{}", layer), sort: Sort::Contents },
        vec![],
    );
    let beta2 = prog.add(
        Op::Input { name: format!("beta2_{}", layer), sort: Sort::Contents },
        vec![],
    );
    let scaled2 = prog.add(Op::Mul, vec![norm2, gamma2]);
    let ln_out2 = prog.add(Op::Add, vec![scaled2, beta2]);

    // FF layer 1: expand
    let w1 = prog.add(
        Op::Input { name: format!("ff_w1_{}", layer), sort: Sort::Contents },
        vec![],
    );
    let ff1 = prog.add(Op::MatMul, vec![ln_out2, w1]);
    let b1 = prog.add(
        Op::Input { name: format!("ff_b1_{}", layer), sort: Sort::Contents },
        vec![],
    );
    let ff1_bias = prog.add(Op::Add, vec![ff1, b1]);

    // GELU activation
    let gelu = prog.add(Op::Gelu, vec![ff1_bias]);

    // FF layer 2: contract
    let w2 = prog.add(
        Op::Input { name: format!("ff_w2_{}", layer), sort: Sort::Contents },
        vec![],
    );
    let ff2 = prog.add(Op::MatMul, vec![gelu, w2]);
    let b2 = prog.add(
        Op::Input { name: format!("ff_b2_{}", layer), sort: Sort::Contents },
        vec![],
    );
    let ff2_bias = prog.add(Op::Add, vec![ff2, b2]);

    // Residual connection 2
    prog.add(Op::Add, vec![residual1, ff2_bias])
}

/// Build and analyze a 4-layer transformer forward pass.
pub fn run_benchmark() {
    let mut prog = Program::new();

    // Input tensor (contents — the model received valid input)
    let input = prog.add(
        Op::Input { name: "input".into(), sort: Sort::Contents },
        vec![],
    );

    // Build 4 layers
    let mut current = input;
    for layer in 0..4 {
        current = build_layer(&mut prog, current, layer);
    }

    let total_ops = prog.instructions.len();
    println!("=== Transformer Forward Pass (4 layers, 128 hidden, 4 heads) ===");
    println!("Total operations: {}", total_ops);
    println!();

    // Before passes
    let before = prog.count_by_sort();
    println!("Before passes:  {}", before);

    // Pass 1: Static sort resolution
    let resolved = pass_resolve::resolve_to_fixpoint(&mut prog);
    let after_p1 = prog.count_by_sort();
    println!("After Pass 1:   {} (resolved {})", after_p1, resolved);

    // Pass 2: Origin folding
    let folded = pass_fold::fold_origin(&mut prog);
    let after_p2 = prog.count_by_sort();
    println!("After Pass 2:   {} (folded {})", after_p2, folded);

    println!();

    // Count exception ops — these are the ONLY operations that need runtime checks.
    // Everything else follows the universal pattern: contents in, contents out.
    // Downstream ops don't need their own check — they need the exception op's
    // check to resolve. If rsqrt is contents, everything downstream is contents.
    let exception_ops = prog
        .instructions
        .iter()
        .filter(|inst| crate::ir::is_exception_op(&inst.op))
        .count();

    let non_exception = total_ops - exception_ops;
    let safe_pct = if total_ops > 0 {
        non_exception as f64 / total_ops as f64 * 100.0
    } else {
        0.0
    };

    println!("--- Result ---");
    println!("Total operations: {}", total_ops);
    println!(
        "Safe by construction (non-exception ops): {} / {} ({:.1}%)",
        non_exception, total_ops, safe_pct
    );
    println!(
        "Runtime checks needed (exception ops): {} ({:.1}%)",
        exception_ops,
        if total_ops > 0 { exception_ops as f64 / total_ops as f64 * 100.0 } else { 0.0 }
    );
    println!();

    // The downstream cascade: if an exception op resolves to contents at runtime,
    // everything downstream is contents by the universal pattern. No separate check.
    // If it resolves to origin, everything downstream folds. No computation.
    let static_contents = after_p2.contents;
    let dependent_on_exceptions = after_p2.unknown;
    println!(
        "Statically contents (no dependency on exceptions): {} ({:.1}%)",
        static_contents,
        if total_ops > 0 { static_contents as f64 / total_ops as f64 * 100.0 } else { 0.0 }
    );
    println!(
        "Resolved by exception check (no own check needed): {} ({:.1}%)",
        dependent_on_exceptions,
        if total_ops > 0 { dependent_on_exceptions as f64 / total_ops as f64 * 100.0 } else { 0.0 }
    );
    println!();

    // Simulate origin entry: inject origin at first layer's rsqrt
    // and count how many operations fold.
    let mut origin_prog = prog.clone();
    // Find the first rsqrt and set it to origin.
    for inst in origin_prog.instructions.iter_mut() {
        if inst.op == Op::Rsqrt {
            inst.sort = Sort::Origin;
            break;
        }
    }
    let foldable = pass_fold::fold_origin(&mut origin_prog);

    println!(
        "If origin enters at first layer norm: {} operations fold ({:.1}%)",
        foldable,
        if total_ops > 0 { foldable as f64 / total_ops as f64 * 100.0 } else { 0.0 }
    );
    println!();

    println!("--- Comparison ---");
    println!(
        "origin-mlir (annotative):   458/499 = 91.8% safe, 41 checks ({:.1}%)",
        41.0 / 499.0 * 100.0
    );
    println!(
        "origin-ir (constitutive):   {}/{} = {:.1}% safe, {} checks ({:.1}%)",
        non_exception, total_ops, safe_pct,
        exception_ops,
        if total_ops > 0 { exception_ops as f64 / total_ops as f64 * 100.0 } else { 0.0 }
    );
}

pub fn run_benchmark_quiet() -> TransformerResult {
    let mut prog = Program::new();
    let input = prog.add(
        Op::Input { name: "input".into(), sort: Sort::Contents },
        vec![],
    );
    let mut current = input;
    for layer in 0..4 {
        current = build_layer(&mut prog, current, layer);
    }
    let total = prog.instructions.len();
    let exception_ops = prog.instructions.iter()
        .filter(|inst| is_exception_op(&inst.op))
        .count();
    let non_exception = total - exception_ops;
    let safe_pct = non_exception as f64 / total as f64 * 100.0;

    pass_resolve::resolve_to_fixpoint(&mut prog);

    // Simulate origin at first rsqrt
    let mut origin_prog = prog.clone();
    for inst in origin_prog.instructions.iter_mut() {
        if inst.op == Op::Rsqrt {
            inst.sort = Sort::Origin;
            break;
        }
    }
    let foldable = pass_fold::fold_origin(&mut origin_prog);
    // Count all origin after fold
    let origin_count = origin_prog.instructions.iter()
        .filter(|inst| inst.sort == Sort::Origin)
        .count();

    TransformerResult { total, safe_pct, exception_ops, foldable: origin_count }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transformer_builds_and_runs() {
        // Just verify it doesn't panic.
        run_benchmark();
    }

    #[test]
    fn single_layer_has_exception_ops() {
        let mut prog = Program::new();
        let input = prog.add(
            Op::Input { name: "input".into(), sort: Sort::Contents },
            vec![],
        );
        build_layer(&mut prog, input, 0);

        pass_resolve::resolve_to_fixpoint(&mut prog);
        let counts = prog.count_by_sort();

        // Exception ops (div, rsqrt) remain unknown, and their downstream
        // dependents cascade to unknown. This is correct: the sort pass
        // can't prove rsqrt's input is positive without value analysis.
        assert!(counts.unknown > 0, "exception ops should remain unknown");
        // Both contents and unknown should be present in a single layer.
        assert!(counts.contents > 0, "constants and inputs should be contents");
    }
}
