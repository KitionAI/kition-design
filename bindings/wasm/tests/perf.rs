//! The performance mode of kition-convergence for Kition Design (Kition `docs/kits.md` section 7,
//! "Collaboration budgets"): `cargo test --release -p kition-design-kit --test perf -- --ignored
//! --nocapture`.

mod common;

use common::DesignKit;
use kition_convergence::CountingAlloc;
use kition_convergence::perf::{self, PerfOptions};

#[global_allocator]
static ALLOC: CountingAlloc = CountingAlloc::new();

#[test]
#[ignore = "a measurement, run in release builds"]
fn budgets() {
    let report = perf::measure(&DesignKit, &PerfOptions { allocator: Some(&ALLOC), ..PerfOptions::default() }).unwrap();
    println!("{}", report.render());
}
