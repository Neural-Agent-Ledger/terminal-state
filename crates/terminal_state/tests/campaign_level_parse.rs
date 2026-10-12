//! Campaign LEVEL-CONTENT parsing checks — properties OF specific campaign
//! `.lvl` files (as opposed to `re_level`'s own unit suite, which tests the
//! `.lvl` GRAMMAR against non-campaign fixtures).
//!
//! Lives in the game repo rather than the engine's `re_level` suite because
//! the assertion below is not a convenient sample of a `[sequence]` parser
//! feature (that generic case is covered by `re_level`'s own inline
//! `SEQ_BASE`-fixture tests) — it pins a fact about the shipped campaign's
//! `levels/L11-inner-circuit.lvl` content itself, so it belongs with the game
//! content, not the engine.
//!
//! # How to run
//! ```sh
//! cargo test -p terminal_state --test campaign_level_parse
//! ```

use std::path::PathBuf;

use re_level::load;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
}

/// `levels/L11-inner-circuit.lvl` ("Inner Circuit") declares its `[sequence]`
/// win mode with three ordered steps, all distinct — the parser enforces
/// distinctness generically (`re_level`'s `sequence_duplicate_patterns_error`),
/// this pins that the SHIPPED level actually exercises it with three real,
/// pairwise-distinct patterns.
#[test]
fn l11_level_file_declares_three_distinct_patterns() {
    let path = workspace_root().join("levels/L11-inner-circuit.lvl");
    assert!(path.exists(), "level file missing: {}", path.display());
    let lv = load(&path).expect("L11 loads");
    assert_eq!(lv.sequence.patterns.len(), 3, "L11 has three ordered steps");
    // All three must be distinct (the parser enforces it, assert it holds).
    assert_ne!(lv.sequence.patterns[0], lv.sequence.patterns[1]);
    assert_ne!(lv.sequence.patterns[1], lv.sequence.patterns[2]);
    assert_ne!(lv.sequence.patterns[0], lv.sequence.patterns[2]);
}
