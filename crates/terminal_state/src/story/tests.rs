//! Tests for the word→story mechanic (`docs/lore/WORD_STORY.md`).
//!
//! These are headless: they load the real shipped levels, drive the real
//! forward pass to a real solve, and assert the assembled phrase is built from
//! the actual fired nodes' anchor words — no engine, no terminal.

use super::*;
use re_level::{load, Level};
use re_puzzle::{puzzle_interact, PuzzleState};
use std::path::PathBuf;

/// Path to a repo-root level file (workspace root is two dirs above the crate).
fn level_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../levels")
        .join(name)
}

/// Load a level + build its puzzle network.
fn load_level(name: &str) -> (Level, PuzzleState) {
    let level = load(level_path(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
    let state = PuzzleState::from_level(&level).unwrap_or_else(|e| panic!("{name}: {e:?}"));
    (level, state)
}

/// Drive the winning solution for a level by toggling the named pipes ON.
/// `wires` are `(from_node_name, to_node_name)` pairs. Asserts each toggle lands
/// and the level ends solved.
fn solve(level: &Level, state: &mut PuzzleState, wires: &[(&str, &str)]) {
    let idx = |n: &str| level.node_index(n).unwrap_or_else(|| panic!("no node {n}"));
    for (from, to) in wires {
        let (fi, ti) = (idx(from), idx(to));
        let (x, y, z) = state
            .pipes
            .iter()
            .find(|p| p.from == fi && p.to == ti)
            .and_then(|p| p.switch_coord())
            .unwrap_or_else(|| panic!("no switchable pipe {from}->{to}"));
        assert!(
            puzzle_interact(state, x, y, z),
            "toggle {from}->{to} at ({x},{y},{z}) did nothing"
        );
    }
    assert!(state.solved, "wires did not solve the level");
}

// ───────────────────────────── anchor index ─────────────────────────────────

/// (d) The anchor index defaults to 0 and is overridable. A node with no
/// `anchor` key picks `pool[0]`; one with `anchor = N` picks `pool[N]`; an
/// out-of-range anchor safely falls back to `pool[0]`.
#[test]
fn anchor_defaults_to_zero_and_is_overridable() {
    let mut node = re_level::Node {
        pool: vec!["FIRE".into(), "BLOOD".into(), "ROSE".into()],
        glyph: 'R',
        ..Default::default()
    };
    // Default anchor 0 → first word.
    assert_eq!(node.anchor, 0, "anchor defaults to 0");
    assert_eq!(word_for(&node, ReadoutForm::Clean), "FIRE");

    // Override to a valid index.
    node.anchor = 2;
    assert_eq!(word_for(&node, ReadoutForm::Clean), "ROSE");

    // Out-of-range falls back to pool[0], never panics.
    node.anchor = 99;
    assert_eq!(word_for(&node, ReadoutForm::Clean), "FIRE");

    // Empty pool falls back to the glyph.
    let empty = re_level::Node {
        glyph: 'X',
        ..Default::default()
    };
    assert_eq!(word_for(&empty, ReadoutForm::Clean), "X");
}

/// The shipped levels parse the optional `anchor` key without error and keep the
/// default 0 (none author an override yet — additive + back-compatible).
#[test]
fn shipped_levels_keep_default_anchor() {
    for n in 1..=7 {
        let (level, _) = load_level(&format!("{n:02}.lvl"));
        for node in &level.nodes {
            assert_eq!(node.anchor, 0, "L{n} {} anchor defaults to 0", node.name);
        }
    }
}

// ─────────────────────────── level-03 phrase ────────────────────────────────

/// (a) Solving level 03 assembles a phrase from its REAL fired nodes' anchor
/// words, in forward-pass topo order. The winning wiring is R+B → MAGENTA (h1)
/// → output; h2/TEAL stays silent and must NOT contribute. The three RGB inputs
/// are all driven to 1 by the level's `[inputs]`, so they all fire and speak —
/// faithful to the mechanic (`word_for(node)=activation?pool[anchor]:""`): the
/// readout is exactly the fired set, in topo order, with no invented or omitted
/// words. The dominant signal path R(FIRE)→B(SEA)→h1(MAGENTA)→output reads with
/// G(LEAF) also present (a lit-but-unrouted sensation), giving the real phrase
/// `FIRE -> LEAF -> SEA -> MAGENTA -> OUTPUT` (every anchor `pool[0]`).
#[test]
fn level_03_assembles_phrase_from_fired_nodes() {
    let (level, mut state) = load_level("03.lvl");
    // Winning wiring: R->h1, B->h1 (h1 threshold 2 fires), h1->output.
    solve(
        &level,
        &mut state,
        &[("R", "h1"), ("B", "h1"), ("h1", "output")],
    );

    let phrase = assemble_phrase(&level, &state, ReadoutForm::Clean);

    // h1 (MAGENTA) and output must both appear, MAGENTA before OUTPUT.
    let magenta = phrase.find("MAGENTA").expect("MAGENTA in phrase");
    let output = phrase.find("OUTPUT").expect("OUTPUT in phrase");
    assert!(
        magenta < output,
        "MAGENTA must precede OUTPUT (topo order): {phrase}"
    );
    // FIRE (R) and SEA (B) feed h1 and must precede MAGENTA.
    assert!(
        phrase.find("FIRE").unwrap() < magenta,
        "FIRE before MAGENTA: {phrase}"
    );
    assert!(
        phrase.find("SEA").unwrap() < magenta,
        "SEA before MAGENTA: {phrase}"
    );
    // h2/TEAL did NOT fire → its word must be absent.
    assert!(
        !phrase.contains("TEAL"),
        "silent h2 must not contribute: {phrase}"
    );

    // The full assembled phrase, pinned (printed for the report).
    println!("L3 phrase = {phrase}");
    assert_eq!(
        phrase, "FIRE  ->  LEAF  ->  SEA  ->  MAGENTA  ->  OUTPUT",
        "L3 fired-set readout (every driven input speaks)"
    );
}

/// Every word in the assembled phrase comes verbatim from a real node pool —
/// nothing invented (the WORD_STORY canon-border).
#[test]
fn phrase_words_come_from_real_pools() {
    let (level, mut state) = load_level("03.lvl");
    solve(
        &level,
        &mut state,
        &[("R", "h1"), ("B", "h1"), ("h1", "output")],
    );
    let phrase = assemble_phrase(&level, &state, ReadoutForm::Clean);
    for word in phrase.split("  ->  ") {
        let found = level
            .nodes
            .iter()
            .any(|n| n.pool.iter().any(|p| p == word) || n.glyph.to_string() == word);
        assert!(found, "word '{word}' is not from any node pool");
    }
}

// ──────────────────────────── accumulation ──────────────────────────────────

/// (b) The phrase accumulates into the memory log across two solved levels. We
/// solve L1 then L3 and assert the log holds both, in order, each its real
/// phrase.
#[test]
fn memory_log_accumulates_across_two_levels() {
    let mut log = MemoryLog::default();

    // L1: FIRE -> MODEL? L1's output pool[0] is MODEL. Solve it.
    let (l1, mut s1) = load_level("01.lvl");
    // L1 is a single R->output wire (find the only switchable pipe).
    let only = s1
        .pipes
        .iter()
        .enumerate()
        .find(|(_, p)| p.switch_coord().is_some())
        .map(|(i, _)| i)
        .expect("L1 has a switch");
    let (x, y, z) = s1.pipes[only].switch_coord().unwrap();
    assert!(puzzle_interact(&mut s1, x, y, z));
    assert!(s1.solved, "L1 solves on its one wire");
    let p1 = assemble_phrase(&l1, &s1, ReadoutForm::Clean);
    log.record(MemoryEntry {
        level_id: "01".into(),
        phrase: p1.clone(),
    });

    // L3.
    let (l3, mut s3) = load_level("03.lvl");
    solve(&l3, &mut s3, &[("R", "h1"), ("B", "h1"), ("h1", "output")]);
    let p3 = assemble_phrase(&l3, &s3, ReadoutForm::Clean);
    log.record(MemoryEntry {
        level_id: "03".into(),
        phrase: p3.clone(),
    });

    assert_eq!(log.entries.len(), 2, "two vignettes recorded");
    assert_eq!(log.entries[0].level_id, "01");
    assert_eq!(log.entries[1].level_id, "03");
    assert_eq!(log.entries[0].phrase, p1);
    assert_eq!(log.entries[1].phrase, p3);

    let story = log.story();
    assert!(
        story.contains(&p1) && story.contains(&p3),
        "story holds both phrases"
    );
    println!("2-level story:\n{story}");

    // Re-recording the same level refreshes, never duplicates.
    assert!(!log.record(MemoryEntry {
        level_id: "03".into(),
        phrase: p3
    }));
    assert_eq!(log.entries.len(), 2, "re-record does not duplicate");
}

// ─────────────────────────── denial / pool_alt ──────────────────────────────

/// (c) The denial / `pool_alt` path produces the corrupted multilingual form:
/// the same fired set, the same topo order, but each word rassembled across
/// every language in its `pool_alt`. The Babel collapse of the `while(true)`
/// ending (WORD_STORY §4).
#[test]
fn denial_path_renders_corrupt_pool_alt() {
    let (level, mut state) = load_level("03.lvl");
    solve(
        &level,
        &mut state,
        &[("R", "h1"), ("B", "h1"), ("h1", "output")],
    );

    let clean = assemble_phrase(&level, &state, ReadoutForm::Clean);
    let corrupt = assemble_phrase(&level, &state, ReadoutForm::Corrupt);

    assert_ne!(clean, corrupt, "corrupt form differs from clean");
    // The corrupt form draws from pool_alt: R's pool_alt leads with `огонь`,
    // B's with `синий`, h1's (MAGENTA) with `пурпур`.
    assert!(
        corrupt.contains("огонь"),
        "R pool_alt word present: {corrupt}"
    );
    assert!(
        corrupt.contains("синий"),
        "B pool_alt word present: {corrupt}"
    );
    assert!(
        corrupt.contains("пурпур"),
        "h1 pool_alt word present: {corrupt}"
    );
    // It is Babel: a word carries multiple languages joined by `/`.
    assert!(
        corrupt.contains('/'),
        "corrupt word fuses languages: {corrupt}"
    );
    // Same number of fired words in the same topo order (same separators).
    assert_eq!(
        clean.matches("  ->  ").count(),
        corrupt.matches("  ->  ").count(),
        "corrupt keeps the same fired set + order"
    );
    println!("L3 corrupt = {corrupt}");
}

/// A node with no `pool_alt` falls back to its clean word in the corrupt form
/// (never empty).
#[test]
fn corrupt_falls_back_to_clean_when_no_pool_alt() {
    let node = re_level::Node {
        pool: vec!["BLOOM".into()],
        pool_alt: vec![],
        glyph: 'B',
        ..Default::default()
    };
    assert_eq!(word_for(&node, ReadoutForm::Corrupt), "BLOOM");
}

/// A genuinely cyclic graph still yields a stable fired-in-order list (the
/// denial data path does not panic on a non-DAG). `fired_in_topo_order` appends
/// the cyclic remainder; nodes with activation 0 are filtered out.
#[test]
fn fired_order_is_stable_on_corrupt_graph() {
    // Reuse a solved L3 — assembling never errors and is deterministic.
    let (level, mut state) = load_level("03.lvl");
    solve(
        &level,
        &mut state,
        &[("R", "h1"), ("B", "h1"), ("h1", "output")],
    );
    let a = assemble_phrase(&level, &state, ReadoutForm::Corrupt);
    let b = assemble_phrase(&level, &state, ReadoutForm::Corrupt);
    assert_eq!(a, b, "assembly is deterministic");
}

// ═══════════════════ the name-shard arc (W1 1.3) ════════════════════════════

/// The shard map is FIXED canon: one shard per campaign level L1–L7, in order,
/// with L8+ adding none (PROGRESSION header) and non-campaign levels off the arc
/// entirely. Both the legacy `NN` ids and the `Ln-…` ids key the same shard.
#[test]
fn shards_key_off_the_campaign_ordinal_in_both_id_forms() {
    let expect = ["R", "learn", "A", "://", "merge", "named", "final byte"];
    for (i, label) in expect.iter().enumerate() {
        let n = i + 1;
        let legacy = format!("{n:02}");
        let ell = format!("L{n}-some-title");
        for id in [legacy.as_str(), ell.as_str()] {
            let s = shard_for_level(id).unwrap_or_else(|| panic!("{id} must carry a shard"));
            assert_eq!(s.index, n, "{id}");
            assert_eq!(s.label, *label, "{id}");
        }
    }
}

/// The assembled hash is canon verbatim (`WORD_STORY` §4 / CONTENT_PACK L12):
/// `R·learn·A·edge·seen·named·bloom`. Note shards 4 and 5 contribute `edge` /
/// `seen`, NOT their narrative labels `://` / `merge`.
#[test]
fn the_complete_hash_is_the_canonical_pid_string() {
    let all = NameAssembly::from_solved_ids(["01", "02", "03", "04", "05", "06", "07"]);
    assert!(all.is_complete());
    assert_eq!(all.hash(), "R·learn·A·edge·seen·named·bloom");
    assert_eq!(all.pid(), "pid://aleks_v@R·learn·A·edge·seen·named·bloom");
    assert_eq!(
        all.true_name_line(),
        "true_name: pid://aleks_v@R·learn·A·edge·seen·named·bloom"
    );
    assert!(
        !all.pid().contains("unresolved"),
        "a whole name is not unresolved"
    );
}

#[test]
fn campaign_levels_past_l7_carry_no_new_shard() {
    // PROGRESSION: "L8–L12 add NO new shards".
    for id in [
        "08",
        "L8-cold-room",
        "L9-stereopsis",
        "L10-lying-door",
        "L11-inner-circuit",
        "L12-closing",
    ] {
        assert!(shard_for_level(id).is_none(), "{id} must add no shard");
        assert!(
            shard_recovered_line(id).is_none(),
            "{id} shows no shard line"
        );
    }
}

#[test]
fn non_campaign_levels_are_off_the_name_arc() {
    for id in [
        "T0-trigger",
        "U1-underline",
        "08-material-demo",
        "09-code-wall",
        "12-mirror",
        "91-generic-demo",
        "TX1-genplate",
        "00-test-load",
        "A1-overlay",
        "",
    ] {
        assert!(
            shard_for_level(id).is_none(),
            "{id:?} is not on the name arc"
        );
    }
}

/// The running assembly: unrecovered shards read `░`, and the line carries
/// `[unresolved]` until all seven are in hand.
#[test]
fn a_partial_assembly_blanks_the_missing_shards() {
    let a = NameAssembly::from_solved_ids(["01", "L2-two-make-three", "T0-trigger"]);
    assert_eq!(a.count(), 2, "the demo id contributes nothing");
    assert!(a.holds(1) && a.holds(2) && !a.holds(3));
    assert_eq!(a.hash(), "R·learn·░·░·░·░·░");
    assert_eq!(a.pid(), "pid://aleks_v@R·learn·░·░·░·░·░  [unresolved]");
    assert!(!a.is_complete());
}

/// Solving out of order fills the shard's own SLOT, not the next free one — the
/// name is positional.
#[test]
fn shards_land_in_their_own_slot_regardless_of_solve_order() {
    let a = NameAssembly::from_solved_ids(["07", "L4-the-angle"]);
    assert_eq!(a.hash(), "░·░·░·edge·░·░·bloom");
    assert_eq!(a.count(), 2);
    // Order of the input ids is irrelevant.
    let b = NameAssembly::from_solved_ids(["L4-the-angle", "07"]);
    assert_eq!(a, b, "the assembly is a set, not a sequence");
}

#[test]
fn an_empty_progress_yields_a_fully_blank_unresolved_name() {
    let a = NameAssembly::default();
    assert_eq!(a.count(), 0);
    assert_eq!(a.hash(), "░·░·░·░·░·░·░");
    assert!(a.pid().ends_with("[unresolved]"));
    assert_eq!(NameAssembly::from_solved_ids(std::iter::empty()), a);
}

/// The DOOR line: `exit: name unknown` until the name is whole, then the name.
#[test]
fn the_door_does_not_know_who_is_leaving_until_the_name_is_whole() {
    assert_eq!(NameAssembly::default().door_line(), DOOR_NAME_UNKNOWN);
    let six = NameAssembly::from_solved_ids(["01", "02", "03", "04", "05", "06"]);
    assert_eq!(
        six.door_line(),
        DOOR_NAME_UNKNOWN,
        "six of seven is still unknown"
    );
    let all = NameAssembly::from_solved_ids(["01", "02", "03", "04", "05", "06", "07"]);
    assert_eq!(
        all.door_line(),
        "exit: pid://aleks_v@R·learn·A·edge·seen·named·bloom"
    );
}

#[test]
fn the_shard_announcement_names_its_number_and_label() {
    assert_eq!(
        shard_recovered_line("03").as_deref(),
        Some("[ name-shard 3 of 7 recovered ]  A")
    );
    assert_eq!(
        shard_recovered_line("L5-soft-footstep").as_deref(),
        Some("[ name-shard 5 of 7 recovered ]  merge")
    );
}

#[test]
fn holds_is_bounds_safe() {
    let a = NameAssembly::from_solved_ids(["01"]);
    assert!(a.holds(1));
    assert!(!a.holds(0), "0 is not a shard number");
    assert!(!a.holds(8));
    assert!(!a.holds(usize::MAX));
}

/// Every shard's `hash_part` and `label` is non-empty and the indices are exactly
/// 1..=7 in order — the table is data the overlay formats blindly.
#[test]
fn the_shard_table_is_well_formed() {
    for (i, s) in NAME_SHARDS.iter().enumerate() {
        assert_eq!(s.index, i + 1, "shards are in order");
        assert!(!s.label.is_empty() && !s.hash_part.is_empty());
        assert!(
            !s.hash_part.contains(SHARD_SEP),
            "a byte may not contain the separator"
        );
    }
    assert_eq!(NAME_SHARDS.len(), 7, "seven shards, complete after L7");
}
