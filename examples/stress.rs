//! Stress-test for multi-spinner clear(), success(), fail(), and add() interactions.
//!
//! Run with: cargo run --example stress

use nanospinner::MultiSpinner;
use std::thread;
use std::time::Duration;

fn pause(secs: u64) {
    thread::sleep(Duration::from_secs(secs));
}

fn main() {
    // ── Phase 1: Sequential clears ──────────────────────────────────────
    // Start with 6 lines, clear them one at a time across frames.
    // Each clear should shrink the block by one row with no ghost lines.
    println!("Phase 1: Sequential clears");
    {
        let h = MultiSpinner::new().start();
        let a = h.add("Compiling core...");
        let b = h.add("Compiling utils...");
        let c = h.add("Compiling api...");
        let d = h.add("Compiling cli...");
        let e = h.add("Compiling tests...");
        let f = h.add("Compiling bench...");

        pause(1);
        f.clear(); // 6 → 5
        pause(1);
        d.clear(); // 5 → 4 (middle line)
        pause(1);
        b.clear(); // 4 → 3
        pause(1);
        a.success();
        c.fail_with("api had warnings");
        e.success_with("tests green!");
        h.stop();
    }

    pause(1);

    // ── Phase 2: Simultaneous clears ────────────────────────────────────
    // Clear multiple lines in the same frame (before the next 80ms tick).
    println!("\nPhase 2: Simultaneous clears");
    {
        let h = MultiSpinner::new().start();
        let a = h.add("Downloading dep A...");
        let b = h.add("Downloading dep B...");
        let c = h.add("Downloading dep C...");
        let d = h.add("Downloading dep D...");
        let e = h.add("Downloading dep E...");

        pause(2);
        // Clear 3 at once — block should jump from 5 to 2 cleanly.
        b.clear();
        c.clear();
        d.clear();
        pause(2);
        a.success();
        e.success_with("All deps fetched.");
        h.stop();
    }

    pause(1);

    // ── Phase 3: Clear everything, then stop ────────────────────────────
    // All lines cleared — stop() should leave zero ghost lines.
    println!("\nPhase 3: Clear all then stop");
    {
        let h = MultiSpinner::new().start();
        let a = h.add("Temp task 1...");
        let b = h.add("Temp task 2...");
        let c = h.add("Temp task 3...");

        pause(2);
        a.clear();
        b.clear();
        c.clear();
        pause(1);
        h.stop();
    }

    pause(1);

    // ── Phase 4: Add lines after clears ─────────────────────────────────
    // Clear some lines, then add new ones — the block should grow back.
    println!("\nPhase 4: Add after clear");
    {
        let h = MultiSpinner::new().start();
        let a = h.add("Step 1: Init...");
        let b = h.add("Step 2: Validate...");
        let c = h.add("Step 3: Transform...");

        pause(1);
        b.clear(); // 3 → 2
        pause(1);
        let d = h.add("Step 4: Upload...");
        let e = h.add("Step 5: Notify...");
        // Now 4 visible lines (a, c, d, e)
        pause(2);
        a.success();
        c.success();
        d.fail_with("Upload timed out.");
        e.success_with("Notified anyway.");
        h.stop();
    }

    pause(1);

    // ── Phase 5: Mixed finalization frenzy ──────────────────────────────
    // Interleave clears, successes, fails, updates, and adds rapidly.
    println!("\nPhase 5: Mixed finalization frenzy");
    {
        let h = MultiSpinner::new().start();
        let a = h.add("Lint...");
        let b = h.add("Typecheck...");
        let c = h.add("Unit tests...");
        let d = h.add("Integration tests...");
        let e = h.add("E2E tests...");
        let f = h.add("Coverage...");
        let g = h.add("Security scan...");
        let i = h.add("License check...");

        pause(1);
        a.clear(); // dismiss lint
        c.success(); // unit tests pass
        pause(1);
        let j = h.add("Deploy preview...");
        e.clear(); // dismiss e2e
        f.fail_with("Coverage below threshold.");
        pause(1);
        b.success_with("Types OK.");
        d.fail_with("3 integration tests failed.");
        g.success();
        pause(1);
        i.clear(); // dismiss license check
        j.success_with("Preview deployed!");
        h.stop();
    }

    println!("\nAll phases complete. If you see no ghost lines above, the fix works.");
}
