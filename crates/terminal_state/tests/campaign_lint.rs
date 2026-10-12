//! Campaign-specific `re_lvlcheck` lint sweep — the twelve `L1..L12` levels
//! only. The generic, engine-repo sweep over the archive `levels/01..07.lvl`
//! corpus lives in `re_lvlcheck/tests/example_levels.rs`; this file holds the
//! campaign half plus the "twelve campaign levels declare a budget" assertion
//! — both assert a property OF the campaign content, not a generic engine
//! feature, so they live with the game, not the engine.
//!
//! # How to run
//! ```sh
//! cargo test -p terminal_state --test campaign_lint
//! ```

use std::path::PathBuf;

use re_lvlcheck::{
    check_level, check_path, solve_is_winnable, Check, LevelReport, Severity, SolveVerdict,
};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
}

fn level_path(name: &str) -> PathBuf {
    workspace_root().join("levels").join(name)
}

/// The FULL L1..L12 campaign — each verified COHERENT/SOLVABLE by the linter
/// CLI when authored; this sweep keeps them that way. (The campaign's new win
/// modes — L11 sequence-lock, L12 inverted-target + cycle-keep — keep an
/// acyclic authored graph, so the static `dag`/structural checks still apply;
/// their ordered/cyclic *runtime* nuances are covered by `re_puzzle`/
/// `campaign_puzzle.rs` unit tests, not the static linter.)
const CAMPAIGN: &[&str] = &[
    "L1-first-pulse.lvl",
    "L2-two-make-three.lvl",
    "L3-full-spectrum.lvl",
    "L4-the-angle.lvl",
    "L5-soft-footstep.lvl",
    "L6-polysemantic.lvl",
    "L7-hidden-depths.lvl",
    "L8-cold-room.lvl",
    "L9-stereopsis.lvl",
    "L10-lying-door.lvl",
    "L11-inner-circuit.lvl",
    "L12-closing.lvl",
];

/// Find a named [`Check`] in a report, panicking (with the report named) if the
/// rule never ran — a missing check is a linter defect, not a pass.
fn check_named<'a>(r: &'a LevelReport, name: &str) -> &'a Check {
    r.checks
        .iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| panic!("[{}] missing check {name}", r.level_name))
}

#[test]
fn campaign_levels_all_solve() {
    for name in CAMPAIGN {
        let r = check_path(level_path(name));
        let solve = r
            .solve
            .unwrap_or_else(|| panic!("[{name}] no solve verdict"));
        assert!(
            solve_is_winnable(solve),
            "[{name}] is not winnable: {:?}",
            solve
        );
        assert!(
            !matches!(solve, SolveVerdict::Unsolvable | SolveVerdict::NetworkError),
            "[{name}] solve verdict is a hard failure: {:?}",
            solve
        );
    }
}

/// Reachability is REPORTED, not gated: what MUST hold is that the level is
/// still solvable (asserted above) — so here we only surface the reachability
/// state and assert it is computed for every level (no panic, complete
/// report), printing any buried switches for the log.
#[test]
fn campaign_levels_reachability_reported() {
    for name in CAMPAIGN {
        let r = check_path(level_path(name));
        assert!(
            r.reachable.is_some(),
            "[{name}] reachability must be evaluated"
        );
        if r.reachable == Some(false) {
            let detail = r
                .checks
                .iter()
                .find(|c| c.name == "reachable")
                .map(|c| c.detail.as_str())
                .unwrap_or("?");
            eprintln!("[{name}] NOTE: buried/unreachable switch(es) — {detail}");
        }
    }
}

/// Every campaign level's `notes` and `thresholds` checks must not FAIL: a
/// notes FAIL is authored text the binder cannot place (parsed, never seen);
/// a thresholds FAIL is a nonsensical or win-blocking threshold. WARNs are
/// tolerated (gaze-search/slide placement, parser-default thresholds).
#[test]
fn campaign_levels_notes_and_thresholds_do_not_fail() {
    for name in CAMPAIGN {
        let r = check_path(level_path(name));
        for cname in ["notes", "thresholds"] {
            let c = check_named(&r, cname);
            assert_ne!(
                c.severity,
                Severity::Fail,
                "[{name}] {cname} check FAILED: {}",
                c.detail
            );
        }
    }
}

#[test]
fn campaign_levels_structurally_valid() {
    for name in CAMPAIGN {
        let r = check_path(level_path(name));
        // Every campaign level must have a valid DAG, valid edges, and door
        // targets that resolve.
        for cname in ["edges", "door-targets", "inputs", "dag"] {
            let c = check_named(&r, cname);
            assert_ne!(
                c.severity,
                Severity::Fail,
                "[{name}] structural check {cname} FAILED: {}",
                c.detail
            );
        }
    }
}

/// The twelve shipped campaign levels all pass the budget rule — no FAILs.
/// Moved from `re_lvlcheck/src/tests.rs` (`every_shipped_campaign_level_passes_
/// the_power_budget_rule`): it asserts "all twelve campaign levels declare a
/// budget", a fact about the campaign, not a generic engine feature.
#[test]
fn every_shipped_campaign_level_passes_the_power_budget_rule() {
    let dir = workspace_root().join("levels");
    let mut seen = 0usize;
    for entry in std::fs::read_dir(&dir).expect("levels/").flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("lvl") {
            continue;
        }
        let Ok(level) = re_level::load(&path) else {
            continue;
        };
        if level.power_budget == 0 {
            continue;
        }
        seen += 1;
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let r = check_level(&level, &name);
        let c = check_named(&r, "power_budget");
        assert_ne!(
            c.severity,
            Severity::Fail,
            "{name}: power_budget check FAILED: {}",
            c.detail
        );
        println!("{name}: {}", c.detail);
    }
    assert_eq!(seen, 12, "all twelve campaign levels must declare a budget");
}

/// Every one of the twelve shipped campaign levels declares a budget; every
/// OTHER level this repo ships stays UNLIMITED. This asserts a fact about the
/// CAMPAIGN's own budget authoring choices (which levels use the mechanic at
/// all), not a generic engine feature, and its premise — "scan `levels/`,
/// find exactly these twelve" — only holds where the campaign files actually
/// live, so it belongs with the game content, not the engine. This complements
/// `every_shipped_campaign_level_passes_the_power_budget_rule` above (which
/// only checks the POSITIVE half, that the twelve pass the rule) with the
/// NEGATIVE half: nothing this repo ships besides the twelve sets a budget.
#[test]
fn campaign_levels_declare_a_budget_and_nothing_else_does() {
    let dir = workspace_root().join("levels");
    let mut budgeted = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("levels/").flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("lvl") {
            continue;
        }
        let Ok(lv) = re_level::load(&path) else {
            continue;
        };
        if lv.power_budget > 0 {
            let stem = path.file_stem().unwrap().to_string_lossy().to_string();
            budgeted.push((stem, lv.power_budget));
        }
    }
    budgeted.sort();
    let names: Vec<&str> = budgeted.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(
        names,
        vec![
            "L1-first-pulse",
            "L10-lying-door",
            "L11-inner-circuit",
            "L12-closing",
            "L2-two-make-three",
            "L3-full-spectrum",
            "L4-the-angle",
            "L5-soft-footstep",
            "L6-polysemantic",
            "L7-hidden-depths",
            "L8-cold-room",
            "L9-stereopsis",
        ],
        "exactly the twelve campaign rooms carry a power_budget"
    );
}
