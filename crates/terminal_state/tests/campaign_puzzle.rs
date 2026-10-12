//! Campaign puzzle-graph properties — assertions about the AUTHORED L11/L12
//! (and the twelve-level power-budget corpus), as opposed to the generic
//! `[sequence]` / `power_budget` / cycle-keep / inverted-target ENGINE
//! mechanics, which `re_puzzle`'s own unit tests cover on crate-local
//! fixtures. A test asserting a property OF the campaign — not merely using a
//! campaign level as a convenient sample — belongs in this crate, since the
//! campaign ships only in the game repo, not the engine.
//!
//! # How to run
//! ```sh
//! cargo test -p terminal_state --test campaign_puzzle
//! ```

use std::path::PathBuf;

use re_level::load;
use re_puzzle::{
    min_power_budget, puzzle_interact, win_reachable_under_budget, BudgetFeasibility, PuzzleState,
};

// ── Workspace root helper (same pattern as the sibling integration tests) ───

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
}

fn level_path(rel: &str) -> PathBuf {
    workspace_root().join("levels").join(rel)
}

// ═════════════════════════════════════════════════════════════════════════
// L11 — the ordered BREAK→FALL→DARK sequence-lock, on the REAL authored graph
// ═════════════════════════════════════════════════════════════════════════

/// L11 ships a budget that keeps the FULL three-step ordered drive winnable.
/// `re_puzzle`'s own unit tests already prove the `[sequence]` mechanic works
/// in the abstract (a crate-local fixture with the same graph shape); this
/// test pins that the ACTUAL authored `L11-inner-circuit.lvl` — real node
/// names, real switch cells — solves the same way.
#[test]
fn l11_sequence_lock_ordered_solve_from_authored_level() {
    let level = load(level_path("L11-inner-circuit.lvl")).expect("L11 loads");
    let mut s = PuzzleState::from_level(&level).expect("L11 builds");

    assert_eq!(
        s.sequence.patterns.len(),
        3,
        "L11 declares 3 ordered patterns"
    );
    assert_eq!(s.sequence.progress, 0, "no auto-lock at the all-OFF start");
    assert!(!s.solved);

    let idx = |n: &str| level.node_index(n).unwrap();
    let switch = |s: &PuzzleState, from: usize, to: usize| -> (i32, i32, i32) {
        s.pipes
            .iter()
            .find(|p| p.from == from && p.to == to)
            .and_then(|p| p.switch_coord())
            .unwrap()
    };
    let (s1, s2, s3, circuit, o1, o2, o3) = (
        idx("s1"),
        idx("s2"),
        idx("s3"),
        idx("circuit"),
        idx("o1"),
        idx("o2"),
        idx("o3"),
    );

    // L11: `circuit` (T2 = s1+s2) is now
    // load-bearing for both o2 and o3; the old `s2->o3` bypass is gone.
    // Pipes: A=s1->o1, B=s1->circuit, C=s2->circuit, D=s2->o2,
    // E=circuit->o2, F=circuit->o3, G=s3->o3. Budget 4.

    // P1 BREAK: +A (s1->o1) → output (1,0,0).
    let (x, y, z) = switch(&s, s1, o1);
    assert!(puzzle_interact(&mut s, x, y, z), "A on");
    assert_eq!(s.sequence.progress, 1, "P1 locks");
    assert!(!s.solved);

    // P2 FALL: −A, +B, +C, +D, +E (4/4) → circuit = s1+s2 = 2, o2 = D+E = 2.
    let (x, y, z) = switch(&s, s1, o1);
    assert!(puzzle_interact(&mut s, x, y, z), "A off"); // o1 off
    let (x, y, z) = switch(&s, s1, circuit);
    assert!(puzzle_interact(&mut s, x, y, z), "B on");
    let (x, y, z) = switch(&s, s2, circuit);
    assert!(puzzle_interact(&mut s, x, y, z), "C on");
    let (x, y, z) = switch(&s, s2, o2);
    assert!(puzzle_interact(&mut s, x, y, z), "D on");
    let (x, y, z) = switch(&s, circuit, o2);
    assert!(puzzle_interact(&mut s, x, y, z), "E on");
    assert_eq!(s.sequence.progress, 2, "P2 locks");
    assert!(!s.solved);

    // P3 DARK: −D, −E, +F, +G (4/4) → circuit stays 2 (B, C untouched),
    // o3 = F+G = 2.
    let (x, y, z) = switch(&s, s2, o2);
    assert!(puzzle_interact(&mut s, x, y, z), "D off"); // o2 off
    let (x, y, z) = switch(&s, circuit, o2);
    assert!(puzzle_interact(&mut s, x, y, z), "E off");
    let (x, y, z) = switch(&s, circuit, o3);
    assert!(puzzle_interact(&mut s, x, y, z), "F on");
    let (x, y, z) = switch(&s, s3, o3);
    assert!(puzzle_interact(&mut s, x, y, z), "G on");
    assert_eq!(s.sequence.progress, 3, "P3 locks → solved");
    assert!(
        s.solved,
        "L11 solved after the ordered BREAK→FALL→DARK drive"
    );
}

/// `sequence_progress` is `None` for an ordinary single-target level (so the HUD
/// keeps its node-count readout), and tracks the lock for an L11 level: it starts
/// `(0, 3)` and reaches `(3, 3)` once the ordered BREAK→FALL→DARK drive solves it.
/// Lives here rather than in `flow`'s own tests because it asserts a property
/// OF the campaign's authored L11 graph, not the generic `sequence_progress`
/// mechanism.
#[test]
fn sequence_progress_tracks_l11_lock() {
    // Single-target level → None.
    let l3 = load(level_path("03.lvl")).expect("L3 loads");
    let s3 = PuzzleState::from_level(&l3).expect("L3 builds");
    assert_eq!(
        re_runtime::hud_probe::sequence_progress(&s3),
        None,
        "single-target level has no lock"
    );

    // L11 sequence-lock level → Some, starting at (0, 3).
    let l11 = load(level_path("L11-inner-circuit.lvl")).expect("L11 loads");
    let mut s = PuzzleState::from_level(&l11).expect("L11 builds");
    assert_eq!(
        re_runtime::hud_probe::sequence_progress(&s),
        Some((0, 3)),
        "lock starts unlocked"
    );

    let idx = |n: &str| l11.node_index(n).unwrap();
    let sw = |s: &PuzzleState, from: &str, to: &str| -> (i32, i32, i32) {
        s.pipes
            .iter()
            .find(|p| p.from == idx(from) && p.to == idx(to))
            .and_then(|p| p.switch_coord())
            .unwrap()
    };
    // Drive P1→P2→P3 in order (matches the level's documented solve).
    // L11: `circuit` (T2 = s1+s2) is now
    // load-bearing for both o2 and o3; the old `s2->o3` bypass is gone.
    for (from, to) in [
        ("s1", "o1"),      // +A → P1
        ("s1", "o1"),      // -A
        ("s1", "circuit"), // +B
        ("s2", "circuit"), // +C
        ("s2", "o2"),      // +D
        ("circuit", "o2"), // +E → P2
        ("s2", "o2"),      // -D
        ("circuit", "o2"), // -E
        ("circuit", "o3"), // +F
        ("s3", "o3"),      // +G → P3
    ] {
        let (x, y, z) = sw(&s, from, to);
        puzzle_interact(&mut s, x, y, z);
    }
    assert_eq!(
        re_runtime::hud_probe::sequence_progress(&s),
        Some((3, 3)),
        "all three steps locked"
    );
    assert!(s.solved, "L11 solved after the ordered drive");
}

// ---------------------------------------------------------------------------
// L12-closing.lvl — BOTH endings reachable from the authored level file
// ---------------------------------------------------------------------------

/// Resolve a pipe's switch world-cell by node-pair, on a level-built state.
fn switch_by_pair(s: &PuzzleState, from: usize, to: usize) -> (i32, i32, i32) {
    s.pipes
        .iter()
        .find(|p| p.from == from && p.to == to)
        .and_then(|p| p.switch_coord())
        .unwrap_or_else(|| panic!("no switch for pipe {from}->{to}"))
}

/// L12 `exit 0`: the real level boots with `gather->face->REC` already ON
/// (REC lit, "confidence 100%"). Prune `face->REC`, then build `unname`
/// (T2 = gather + face: you can only un-name what you have looked at) and
/// route it to EXIT. EXIT fires, REC reads 0 ⇒ the inverted win fires.
#[test]
fn l12_closing_exit0_branch_wins() {
    let level = load(level_path("L12-closing.lvl")).expect("L12 loads");
    let mut s = PuzzleState::from_level(&level).expect("L12 builds");

    // The level declared both win modes.
    assert!(s.cycle_keep, "cycle_keep armed for the while(true) branch");
    assert!(
        !s.target_off.is_empty(),
        "target_off armed for the exit 0 branch"
    );

    let idx = |n: &str| level.node_index(n).unwrap();
    let (gather, face, unname, rec, exit) = (
        idx("gather"),
        idx("face"),
        idx("unname"),
        idx("REC"),
        idx("EXIT"),
    );

    // The load state: gather->face->REC ON, REC lit, nothing won yet (EXIT
    // is dark and target_off is not yet satisfied).
    assert_eq!(
        s.nodes[rec].activation, 1,
        "REC starts lit — confidence 100%"
    );
    assert!(!s.solved, "a lit REC alone is not a win");

    // Prune the self-feed, then build unname and route it to EXIT.
    let (x, y, z) = switch_by_pair(&s, face, rec);
    assert!(puzzle_interact(&mut s, x, y, z), "prune face->REC");
    for (from, to) in [(gather, unname), (face, unname), (unname, exit)] {
        let (x, y, z) = switch_by_pair(&s, from, to);
        assert!(
            puzzle_interact(&mut s, x, y, z),
            "{from}->{to} must take under the budget"
        );
    }

    assert_eq!(s.nodes[exit].activation, 1, "EXIT fires");
    assert_eq!(s.nodes[rec].activation, 0, "REC reads 0 (pruned)");
    assert!(!s.cycle_held, "no loop closed on the exit 0 branch");
    assert!(s.solved, "EXIT on + REC off ⇒ exit 0 win");
}

/// L12 `while(true)`: the real level boots with REC already lit (no feed
/// needed). Close the REC->MEM->REC loop on top of the load draw; under
/// cycle_keep the ring stands ⇒ the kept-cycle win.
#[test]
fn l12_closing_while_true_branch_wins() {
    let level = load(level_path("L12-closing.lvl")).expect("L12 loads");
    let mut s = PuzzleState::from_level(&level).expect("L12 builds");

    let idx = |n: &str| level.node_index(n).unwrap();
    let (rec, mem) = (idx("REC"), idx("MEM"));

    assert_eq!(
        s.nodes[rec].activation, 1,
        "REC already lit at boot — no feed needed"
    );
    assert!(!s.cycle_held, "still acyclic before the ring closes");

    let (x, y, z) = switch_by_pair(&s, rec, mem);
    assert!(puzzle_interact(&mut s, x, y, z), "REC->MEM ON");
    assert!(!s.cycle_held, "still acyclic — ring not yet closed");

    let (x, y, z) = switch_by_pair(&s, mem, rec);
    assert!(
        puzzle_interact(&mut s, x, y, z),
        "MEM->REC ON closes the ring"
    );

    assert!(s.cycle_held, "the kept loop stands");
    assert!(s.solved, "a kept cycle ⇒ while(true) win");
    assert_eq!(
        s.power_used(),
        4,
        "the load draw (2) plus the closed ring (2) is 4/4"
    );
    // The forward pass settled (the test returning at all proves termination).
    assert_eq!(
        s.nodes[rec].activation, 1,
        "REC latches ON in the kept loop"
    );
    assert_eq!(
        s.nodes[mem].activation, 1,
        "MEM latches ON in the kept loop"
    );
}

/// The two L12 endings are MUTUALLY EXCLUSIVE by budget, not by precedence:
/// `exit 0` spends the entire 4-unit cap (P0, P2, P3, P4 ON; P1 pruned), so
/// there is no unit left to also close the REC<->MEM ring. `exit 0` needs 4
/// units with P5 and P6 OFF, and the ring needs 2 more. 6 > 4, so no
/// reachable state satisfies both.
#[test]
fn l12_closing_endings_are_exclusive() {
    let level = load(level_path("L12-closing.lvl")).expect("L12 loads");
    let mut s = PuzzleState::from_level(&level).expect("L12 builds");
    let idx = |n: &str| level.node_index(n).unwrap();
    let (gather, face, unname, rec, mem, exit) = (
        idx("gather"),
        idx("face"),
        idx("unname"),
        idx("REC"),
        idx("MEM"),
        idx("EXIT"),
    );

    // exit 0 first: prune face->REC, build unname, route to EXIT — 4/4 budget.
    let (x, y, z) = switch_by_pair(&s, face, rec);
    assert!(puzzle_interact(&mut s, x, y, z), "prune face->REC");
    for (from, to) in [(gather, unname), (face, unname), (unname, exit)] {
        let (x, y, z) = switch_by_pair(&s, from, to);
        assert!(
            puzzle_interact(&mut s, x, y, z),
            "{from}->{to} must take under the budget"
        );
    }
    assert_eq!(s.power_used(), 4, "exit 0 spends the entire budget");
    assert!(s.solved && !s.cycle_held, "exit 0 win, no kept cycle");

    // Now try to ALSO close the loop, on top of the still-wired exit. Both
    // ring edges must be REFUSED — there is no spare unit at 4/4.
    let (x, y, z) = switch_by_pair(&s, rec, mem);
    assert!(
        !puzzle_interact(&mut s, x, y, z),
        "REC->MEM refused: budget is full"
    );
    let (x, y, z) = switch_by_pair(&s, mem, rec);
    assert!(
        !puzzle_interact(&mut s, x, y, z),
        "MEM->REC refused: budget is full"
    );

    // The refusal leaves the exit 0 win completely undisturbed.
    assert_eq!(
        s.power_used(),
        4,
        "the refused toggles left the budget untouched"
    );
    assert!(
        s.solved && !s.cycle_held,
        "exit 0 win still stands — no branch flip"
    );
}

// ═════════════════════════════════════════════════════════════════════════
// L12 power budget — both authored endings reachable
// ═════════════════════════════════════════════════════════════════════════

/// L12 ships a budget that keeps BOTH authored endings reachable.
/// L12: the level boots with `gather->face`
/// and `face->REC` already ON (recognition = "confidence 100%"). `exit 0`
/// means pruning `face->REC`, then building `unname` (T2 = gather + face) and
/// routing it to EXIT — 4 units, the only exit-0 configuration. `while(true)`
/// means closing the `REC<->MEM` ring on top of the load draw — 2 more units,
/// 4/4 total. 6 > 4 keeps the two endings mutually exclusive.
#[test]
fn l12_budget_keeps_both_finale_endings_reachable() {
    let level = load(level_path("L12-closing.lvl")).expect("L12 loads");
    let base = PuzzleState::from_level(&level).expect("L12 builds");
    assert_eq!(base.power_budget, 4, "L12 ships with power_budget = 4");
    assert!(base.cycle_keep);

    let idx = |n: &str| level.node_index(n).unwrap();
    let switch = |s: &PuzzleState, from: &str, to: &str| -> (i32, i32, i32) {
        let (f, t) = (idx(from), idx(to));
        s.pipes
            .iter()
            .find(|p| p.from == f && p.to == t)
            .and_then(|p| p.switch_coord())
            .unwrap()
    };

    // The level boots with the recognition pair lit: gather->face, face->REC.
    assert_eq!(
        base.power_used(),
        2,
        "L12 loads with the recognition pair ON"
    );

    // ENDING 1 — `exit 0`: prune face->REC, then build unname (needs BOTH
    // gather and face) and route it to EXIT. 4 units, the only exit-0 config.
    let mut a = base.clone();
    let (x, y, z) = switch(&a, "face", "REC");
    assert!(puzzle_interact(&mut a, x, y, z), "prune face->REC");
    for (f, t) in [("gather", "unname"), ("face", "unname"), ("unname", "EXIT")] {
        let (x, y, z) = switch(&a, f, t);
        assert!(
            puzzle_interact(&mut a, x, y, z),
            "{f}->{t} must take under the budget"
        );
    }
    assert_eq!(a.power_used(), 4, "exit 0 draws exactly the budget");
    assert!(
        a.solved && !a.cycle_held,
        "EXIT fires and REC reads 0 — the release ending"
    );

    // ENDING 2 — `while(true)`: close the REC<->MEM ring on top of the
    // authored load draw. 2 more toggles, 4/4 total.
    let mut b = base;
    for (f, t) in [("REC", "MEM"), ("MEM", "REC")] {
        let (x, y, z) = switch(&b, f, t);
        assert!(
            puzzle_interact(&mut b, x, y, z),
            "{f}->{t} must take under the budget"
        );
    }
    assert_eq!(b.power_used(), 4, "the cage draws exactly the budget");
    assert!(
        b.cycle_held && b.solved,
        "the ring stands — the cage ending"
    );
}

// ═════════════════════════════════════════════════════════════════════════
// The safety net — every campaign level's budget is set at (or above) the
// true winnable minimum.
// ═════════════════════════════════════════════════════════════════════════

/// **The catastrophic-regression gate.** For EVERY campaign level that declares
/// a `power_budget`, a winning state must still be REACHABLE under that budget.
/// This test brute-forces the constrained state space with the real interaction
/// rules ([`win_reachable_under_budget`]), so it also covers L11's stateful
/// sequence lock and L12's cycle-keep endings, which a stateless sweep cannot.
///
/// It FAILS if any budget is ever set one too low, and it also pins each level's
/// computed true minimum so a graph edit that moves the floor is caught rather
/// than silently absorbed.
#[test]
fn every_budgeted_campaign_level_is_still_winnable() {
    // (file stem, shipped budget, computed true minimum).
    // Shipped > minimum on exactly two levels, both deliberate and both
    // explained in the level file's own header comment.
    const CAMPAIGN: [(&str, u32, u32); 12] = [
        ("L1-first-pulse", 1, 1),
        ("L2-two-make-three", 3, 3),
        ("L3-full-spectrum", 3, 3),
        // L4: `tapered` T 1->2 and budget
        // 2->3 — both stacked inputs (`is_round` AND `is_pointed`) are now
        // required, closing the old off-axis single-source solve.
        ("L4-the-angle", 3, 3),
        ("L5-soft-footstep", 3, 3),
        ("L6-polysemantic", 5, 5),
        ("L7-hidden-depths", 5, 5),
        ("L8-cold-room", 3, 3),
        ("L9-stereopsis", 6, 6),
        // L10: `J` T 1->2 and budget
        // 2->3 — the junction genuinely weighs both inputs before either branch.
        ("L10-lying-door", 3, 3),
        // L11: the `s2->o3` bypass around
        // `circuit` is REMOVED — `circuit` (now T2 = s1+s2) is load-bearing for
        // both o2 and o3, so the stateless minimum is the true 4 ({B,C,F,G}),
        // and budget 4 is exactly that minimum (no slack).
        ("L11-inner-circuit", 4, 4),
        // L12: rebuilt around `face`/
        // `unname`. The cheaper ending is now while(true) (+P5,+P6 = 2 toggles
        // from the authored load draw); exit 0 needs the full 4 (build `unname`
        // + prune `face->REC`). Budget 4 is the tightest cap that keeps BOTH
        // endings reachable; the computed true minimum is 2 (the ring).
        ("L12-closing", 4, 2),
    ];

    let mut report = Vec::new();
    for (stem, declared, expected_min) in CAMPAIGN {
        let level = load(level_path(&format!("{stem}.lvl")))
            .unwrap_or_else(|e| panic!("{stem}: load: {e}"));
        assert_eq!(
            level.power_budget, declared,
            "{stem}: the .lvl must declare power_budget = {declared}"
        );
        let state =
            PuzzleState::from_level(&level).unwrap_or_else(|e| panic!("{stem}: build: {e:?}"));
        assert_eq!(
            state.power_budget, declared,
            "{stem}: the budget reaches re_puzzle"
        );

        // 1. WINNABLE under the shipped budget — the gate that must never fail.
        assert_eq!(
            win_reachable_under_budget(&state, declared),
            BudgetFeasibility::Winnable,
            "{stem}: power_budget = {declared} makes the level UNWINNABLE. A budget \
             below the level's true minimum ships an unsolvable level."
        );

        // 2. The authored initial draw must itself fit the cap, or the player
        //    starts in a state the rule says is illegal.
        assert!(
            state.power_used() <= declared,
            "{stem}: the authored start already draws {} of {declared}",
            state.power_used()
        );

        // 3. The true minimum is where we think it is.
        let min =
            min_power_budget(&state).unwrap_or_else(|| panic!("{stem}: no budget wins at all"));
        assert_eq!(
            min, expected_min,
            "{stem}: the computed minimum budget moved (table says {expected_min}, \
             search says {min}) — re-derive the shipped power_budget before editing \
             this table"
        );
        assert!(
            declared >= min,
            "{stem}: declared {declared} < true minimum {min}"
        );

        let one_lower = if declared > 1 {
            format!("{:?}", win_reachable_under_budget(&state, declared - 1))
        } else {
            "n/a".to_string()
        };
        report.push(format!(
            "  {stem:<20} budget {declared} of {:>1} pipes   true-min {min}   at budget-1: {one_lower}",
            state.pipes.len()
        ));
    }
    println!("power_budget campaign table:\n{}", report.join("\n"));
}
