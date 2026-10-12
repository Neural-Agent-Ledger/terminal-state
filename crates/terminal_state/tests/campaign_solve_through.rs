//! Campaign solve-through integration test: lvlcheck + walkthrough coverage
//! for the full campaign.
//!
//! Drives the first eight campaign levels (`L1-first-pulse` .. `L8-cold-room`)
//! end-to-end, in the authored `levels/campaign.toml` manifest order:
//!
//! 1. The manifest's first eight entries are exactly `L1`..`L8`, and every one
//!    of their `.lvl` files exists and parses (`re_level::load`).
//! 2. Each level's puzzle network is solved FOR REAL — not by flipping a
//!    `solved` flag — by computing the minimal switch-toggle set with the same
//!    static analysis `re_lvlcheck` uses (`re_puzzle::solve_brute_force`) and
//!    then applying every required toggle through the actual runtime seam
//!    (`re_puzzle::puzzle_interact`) at its authored world switch cell.
//! 3. The name-shard arc (`WORD_STORY` §5 / `STORY_BIBLE` §5: seven shards, one
//!    per campaign level L1..L7, none from L8 on) accumulates correctly, and
//!    the assembled true name after L7 equals the canonical
//!    `pid://aleks_v@R·learn·A·edge·seen·named·bloom`.
//! 4. The unlock gate (level N+1 playable only once level N is solved) holds
//!    at every step.
//!
//! `terminal_state` ships a thin library target (`crates/terminal_state/src/lib.rs`)
//! alongside its `terminal-state` binary, so this test drives the REAL
//! production types directly — no local mirrors of `flow.rs`/`story.rs`:
//! [`terminal_state::flow::campaign_manifest`] (the same embedded
//! `levels/campaign.toml` parse the shipped game uses) for the manifest +
//! unlock gate ([`terminal_state::flow::CampaignManifest::is_unlocked`]), and
//! [`terminal_state::story::NameAssembly`] for the shard bookkeeping / assembled
//! true-name string.
//!
//! The puzzle-solving itself never had this gap: `re_level` and `re_puzzle`
//! are ordinary library crates `terminal_state` already depends on, and this test
//! calls their real, public solver/interaction functions — the same ones
//! `re_lvlcheck` and the shipped game use.
//!
//! # How to run
//! ```sh
//! cargo test -p terminal_state --test campaign_solve_through
//! ```

use std::path::{Path, PathBuf};

use re_level::{load, Level};
use re_puzzle::{puzzle_interact, solve_brute_force, PuzzleState, Solvability};
use terminal_state::save::Progress;
use terminal_state::story::NameAssembly;

// ── Workspace root helper (same pattern as the sibling integration tests) ───

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
}

/// The first eight campaign levels, in authored order (`PUZZLE_PROGRESSION_L1-L12`,
/// `STORY_BIBLE` §5 shard map). Cross-checked against `terminal_state::flow::campaign_manifest`
/// (the same embedded `levels/campaign.toml` the shipped game parses) in
/// `manifest_first_eight_entries_are_l1_through_l8_and_load` below.
const EXPECTED_IDS: [&str; 8] = [
    "L1-first-pulse",
    "L2-two-make-three",
    "L3-full-spectrum",
    "L4-the-angle",
    "L5-soft-footstep",
    "L6-polysemantic",
    "L7-hidden-depths",
    "L8-cold-room",
];

/// The canonical assembled true name after all seven shards (L1..L7) are
/// recovered — the fixed expected OUTCOME this test verifies against, not a
/// mirror of the shard table itself (that lives only in `story.rs`'s
/// `NAME_SHARDS`, which [`NameAssembly`] reads directly). Verbatim from
/// `docs/lore/WORD_STORY.md` (`true_name: pid://aleks_v@R·learn·A·edge·seen·named·bloom`)
/// and `docs/lore/CONTENT_PACK_L1-L12_DRAFT.md`.
const CANONICAL_TRUE_NAME: &str = "pid://aleks_v@R·learn·A·edge·seen·named·bloom";

// ── Puzzle solving — the REAL thing, via `re_puzzle`'s public solver + the
// real interaction seam. No game-side involvement needed or possible here. ─

/// Load `level_path`, build its puzzle graph, and drive it to a genuine WIN by
/// applying the minimal switch-toggle set `re_puzzle::solve_brute_force`
/// reports (the same static analysis `re_lvlcheck`'s solvability gate uses),
/// each toggle applied through `re_puzzle::puzzle_interact` at the switch's
/// authored world cell — never by setting `state.solved` directly.
///
/// Panics (a genuine test failure, not a swallowed case) if the level is not
/// solvable by toggling switches alone, or if the applied toggle count
/// diverges from the solver's reported minimum. Returns the loaded level, the
/// now-solved puzzle state, and the number of toggles applied.
fn solve_minimally(level_path: &Path) -> (Level, PuzzleState, u32) {
    let lv = load(level_path).unwrap_or_else(|e| panic!("load {}: {e}", level_path.display()));
    let mut state = PuzzleState::from_level(&lv).unwrap_or_else(|e| {
        panic!(
            "{}: puzzle graph must build (DAG): {e:?}",
            level_path.display()
        )
    });

    // Same filter + order `solve_brute_force` uses internally for its
    // `winning_mask` bit assignment, so bit `i` here means exactly what it
    // means there.
    let switchable: Vec<usize> = state
        .pipes
        .iter()
        .enumerate()
        .filter(|(_, p)| p.switch_cell.is_some())
        .map(|(i, _)| i)
        .collect();

    let (min_toggles, winning_mask) = match solve_brute_force(&mut state) {
        Solvability::Solvable {
            min_toggles,
            winning_mask,
            ..
        } => (min_toggles, winning_mask),
        other => panic!(
            "{}: must be solvable by toggling switches with min_toggles > 0; solver said {other:?}",
            level_path.display()
        ),
    };

    let mut applied = 0u32;
    for (bit, &pipe_idx) in switchable.iter().enumerate() {
        let want_on = (winning_mask >> bit) & 1 == 1;
        let is_on = state.pipes[pipe_idx].is_on();
        if want_on != is_on {
            let (x, y, z) = state.pipes[pipe_idx].switch_coord().unwrap_or_else(|| {
                panic!(
                    "{}: switchable pipe {pipe_idx} has no switch coordinate",
                    level_path.display()
                )
            });
            assert!(
                puzzle_interact(&mut state, x, y, z),
                "{}: toggle at switch cell ({x},{y},{z}) for pipe {pipe_idx} must take",
                level_path.display()
            );
            applied += 1;
        }
    }

    assert_eq!(
        applied,
        min_toggles,
        "{}: applied {applied} toggle(s) but the static solver reported \
         min_toggles={min_toggles} — the minimal-toggle set diverged",
        level_path.display()
    );
    assert!(
        state.solved,
        "{}: puzzle must report SOLVED after applying the minimal toggle set",
        level_path.display()
    );
    (lv, state, applied)
}

// ── Test 1: manifest order + load ───────────────────────────────────────────

#[test]
fn manifest_first_eight_entries_are_l1_through_l8_and_load() {
    let manifest = terminal_state::flow::campaign_manifest();
    let entries = manifest.levels();
    assert!(
        entries.len() >= EXPECTED_IDS.len(),
        "campaign.toml must list at least {} levels; got {}",
        EXPECTED_IDS.len(),
        entries.len()
    );

    let root = workspace_root();
    for (i, expected_id) in EXPECTED_IDS.iter().enumerate() {
        let entry = &entries[i];
        assert_eq!(
            &entry.id, expected_id,
            "manifest position {i} must be {expected_id}; got {}",
            entry.id
        );

        let path = root.join(&entry.path);
        assert!(path.exists(), "level file missing: {}", path.display());
        load(&path).unwrap_or_else(|e| panic!("load {}: {e}", path.display()));
    }
}

// ── Test 2: solve L1→L8 in order; shards + unlock gate ──────────────────────

#[test]
fn l1_through_l8_solve_in_manifest_order_shards_and_gating_hold() {
    let manifest = terminal_state::flow::campaign_manifest();
    let root = workspace_root();

    // The REAL persisted-progress type (`terminal_state::save::Progress`), built and
    // mutated in memory only — `Progress::default()` touches no disk, and
    // `CampaignManifest::is_unlocked` is the exact gate the shipped menu calls.
    let mut progress = Progress::default();
    // Ids solved so far, in solve order — feeds `NameAssembly::from_solved_ids`
    // (the exact function the win/menu flow uses to compute the shard arc).
    let mut solved_ids: Vec<String> = Vec::new();

    for (i, expected_id) in EXPECTED_IDS.iter().enumerate() {
        let ordinal = i + 1; // 1..=8, matches the shard-map's campaign ordinal

        let entry = manifest
            .get(expected_id)
            .unwrap_or_else(|| panic!("manifest missing entry for {expected_id}"));

        // ── Gate BEFORE solving: this level is unlocked (its own
        // predecessor was solved in the prior loop iteration, or it's L1),
        // and the NEXT level is still LOCKED. ──────────────────────────────
        assert!(
            manifest.is_unlocked(expected_id, &progress),
            "{expected_id} must be unlocked (predecessor already solved, or it is L1)"
        );
        if let Some(next_id) = EXPECTED_IDS.get(ordinal) {
            assert!(
                !manifest.is_unlocked(next_id, &progress),
                "{next_id} must be LOCKED before {expected_id} is solved"
            );
        }

        // ── Solve this level for real ────────────────────────────────────
        let path = root.join(&entry.path);
        let (_level, state, applied) = solve_minimally(&path);
        assert!(
            applied > 0,
            "{expected_id}: min-toggle count must be strictly > 0 (non-trivial puzzle); \
             got {applied}"
        );
        assert!(
            state.solved,
            "{expected_id}: must report SOLVED after the minimal toggle set"
        );
        progress.mark_solved(expected_id);
        solved_ids.push(expected_id.to_string());

        // ── Gate AFTER solving: the NEXT level unlocks. ──────────────────
        if let Some(next_id) = EXPECTED_IDS.get(ordinal) {
            assert!(
                manifest.is_unlocked(next_id, &progress),
                "{next_id} must UNLOCK once {expected_id} is solved"
            );
        }

        // ── Shard bookkeeping via the REAL `NameAssembly` (the same
        // derivation the won-screen readout uses): L1..L7 each add exactly
        // one shard in order; L8 adds none. ──────────────────────────────
        let assembly = NameAssembly::from_solved_ids(solved_ids.iter().map(String::as_str));
        let expected_shard_count = ordinal.min(7);
        assert_eq!(
            assembly.count(),
            expected_shard_count,
            "running shard count must be {expected_shard_count} after solving {expected_id} \
             (L8 must add none)"
        );
    }

    // After L8: still exactly 7 shards — L8 confirmed to add none.
    let assembly = NameAssembly::from_solved_ids(solved_ids.iter().map(String::as_str));
    assert_eq!(
        assembly.count(),
        7,
        "L8 must not have added an eighth shard — the name completes at L7"
    );
    assert!(
        assembly.is_complete(),
        "all seven shards recovered ⇒ the name assembly must report complete"
    );

    // ── The assembled true name after L7 (all seven shards recovered),
    // via the REAL `NameAssembly::pid()` — the same string the door/won
    // screen renders. ───────────────────────────────────────────────────
    assert_eq!(
        assembly.pid(),
        CANONICAL_TRUE_NAME,
        "the assembled true name after L7 must equal the canonical pid string"
    );
}
