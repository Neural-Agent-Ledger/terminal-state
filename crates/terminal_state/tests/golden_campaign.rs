//! The CAMPAIGN half of the golden-framebuffer corpus.
//!
//! This crate does not own the render harness itself — that lives in
//! `re_golden`, an engine-side dev-dep, since it uses only engine crates. This
//! file owns just the 7 STORY keys that are genuinely campaign content: `L1`,
//! `L5`, `L8`–`L12`. The other 23 keys (engine/example/archive levels) live in
//! the sibling corpus at `re_golden/tests/engine_corpus.rs`, checked against
//! `tests/golden/engine_fb_hashes.txt`.
//!
//! All the render/hash/classify/report logic lives in [`re_golden::check_corpus`];
//! this file supplies only the corpus (which levels, which store) — see that
//! function's doc for the fail-closed accounting rules (MISMATCH / UNREADABLE /
//! NO-GOLDEN / ORPHAN-GOLDEN, and why blessing never clears the last two).
//!
//! # Refresh procedure
//!
//! ```sh
//! TS_BLESS_GOLDEN=1 cargo test -p terminal_state --test golden_campaign
//! ```
//! Commit the updated `tests/golden/campaign_fb_hashes.txt` together with the
//! render change and a dated note in `tests/golden/README.md`. Never bless to
//! silence an unexpected failure — investigate first.

use std::path::PathBuf;

/// Return the workspace root: `CARGO_MANIFEST_DIR` (which points to
/// `crates/terminal_state`) stepped up two levels to the repo root.
fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
}

/// The 7 campaign story keys curated for regression coverage.
///
/// * `L1-first-pulse` — the two-storey furnished opening level (switch below,
///   door above).
/// * `L5-soft-footstep`, `L9-stereopsis`, `L10-lying-door`, `L11-inner-circuit`,
///   `L12-closing` — story-arc anchors through to the final level.
/// * `L8-cold-room` — the dedicated snow-particle anchor, proven by the
///   shipped `re_render::cold_levels_render_visible_snow_in_a_dump_frame` unit
///   test to place >=20 deterministic flake cells in this exact
///   `composite_dump_once` frame.
fn curated_levels() -> Vec<(&'static str, PathBuf)> {
    let root = workspace_root();

    macro_rules! lvl {
        ($key:literal, $file:literal) => {
            ($key, root.join("levels").join($file))
        };
    }

    vec![
        lvl!("L1-first-pulse", "L1-first-pulse.lvl"),
        lvl!("L5-soft-footstep", "L5-soft-footstep.lvl"),
        lvl!("L8-cold-room", "L8-cold-room.lvl"),
        lvl!("L9-stereopsis", "L9-stereopsis.lvl"),
        lvl!("L10-lying-door", "L10-lying-door.lvl"),
        lvl!("L11-inner-circuit", "L11-inner-circuit.lvl"),
        lvl!("L12-closing", "L12-closing.lvl"),
    ]
}

#[test]
fn campaign_golden_framebuffer_hashes() {
    let store = workspace_root().join("tests/golden/campaign_fb_hashes.txt");
    re_golden::check_corpus(&store, &curated_levels());
}
