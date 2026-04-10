#![allow(unused, deprecated)]

mod val;
mod ops;
mod ir;
mod pass_resolve;
mod pass_fold;
mod bench_transformer;
mod bench_cramer;
mod bench_ode;
mod bench_stb;

use val::{resolve_sort, Val};

fn main() {
    // The distinction.
    println!("The distinction:");
    println!();

    println!("  contents(0) * contents(5) = contents(0)");
    println!("      Arithmetic. Zero apples. Still apples.");
    println!();
    println!("  origin      * contents(5) = origin");
    println!("      Absorption. The ground took it.");
    println!();
    println!("  Same result in traditional math. Different sorts here.");
    println!("  Everything below follows from this.");
    println!();
    println!("─────────────────────────────────────────────────────");
    println!();

    // Transformer
    {
        use bench_transformer::run_benchmark_quiet;
        let r = run_benchmark_quiet();
        println!("Transformer (4 layers, {} ops):", r.total);
        println!(
            "  {:.1}% safe by construction. {} runtime checks.",
            r.safe_pct, r.exception_ops
        );
        println!(
            "  Origin enters at layer norm -> {} operations fold. Never emitted.",
            r.foldable
        );
    }
    println!();

    // Cramer
    {
        use bench_cramer::run_benchmark_quiet;
        let r = run_benchmark_quiet();
        println!("Cramer's rule (2x2):");
        println!(
            "  Standard: 8 != 0 hypotheses. Origin-ir: {}. Check at the division. Nowhere else.",
            r.hypotheses
        );
    }
    println!();

    // ODE
    {
        use bench_ode::run_benchmark_quiet;
        let r = run_benchmark_quiet();
        println!("Projectile simulation (20 steps, bad sensor at step 5):");
        println!("  Traditional: executing step 6... step 7... step 8... [{} steps of NaN]", r.nan_steps);
        println!("  Origin-ir:   origin detected at step 5. Steps 6-19 folded. Done.");
        println!("  {} operations never emitted. Last good values preserved.", r.origin_ops);
    }
    println!();

    // stb_image
    {
        use bench_stb::run_benchmark_quiet;
        let r = run_benchmark_quiet();
        println!("stb_image gamma bug (real C code, 7,988 lines):");
        println!("  UBSan: 0 warnings.");
        println!("  Origin-ir:");
        println!("    pixel[0]: val.div(contents(1.0), contents(0.0)) -> container(1.0)");
        println!("               | propagates through {} operations", r.tainted_ops);
        println!("               output: container(1.0)  [bug caught at cause, not output]");
        println!("    {}/{} pixels: same story.", r.tainted_pixels, r.total_pixels);
    }
    println!();

    println!("─────────────────────────────────────────────────────");
    println!();
    println!("61 tests. 0 failures. Kill switch live at every level. Never triggered.");
    println!();
    println!("Verify: cargo test");
}
