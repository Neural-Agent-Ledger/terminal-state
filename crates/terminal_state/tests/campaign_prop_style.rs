//! Cross-crate agreement test between the engine and the game content.
//!
//! `re_level` used to derive a level's default prop pack from
//! `include_str!("../../../levels/campaign.toml")` at load time — an edge that
//! cannot survive the engine/game repo split (the engine repo has no
//! `levels/campaign.toml`). Levels are now self-describing: each campaign
//! `.lvl` authors its own `[meta] prop_style`, and `re_level` no longer reads
//! the manifest at all.
//!
//! That split creates exactly one new way for the two sources to drift apart:
//! nothing stops a future edit to `levels/L*.lvl` or to `campaign.toml`'s
//! `movement` field from disagreeing with the other. This test is that guard —
//! it has a dev-only path to BOTH `re_level` (the parsed `.lvl`) and the
//! game's own `campaign_manifest()` (the parsed `campaign.toml`), which no
//! shipped code path combines, and additionally exercises `re_voximport`'s
//! writer (a save/reload cycle) to confirm the agreement survives an editor
//! round trip, not just a fresh load.

use re_level::props::PropStyle;
use terminal_state::flow::{campaign_manifest, Movement};

/// Integration tests run with the crate root as CWD; the levels live at the
/// workspace root, one directory up from `crates/terminal_state` (same convention as
/// `campaign_pipes_and_emits.rs`).
fn level_path(rel: &str) -> String {
    format!("../../{rel}")
}

/// `campaign.toml`'s own `"P1".."P4"` tag for a parsed [`Movement`] — the
/// inverse of `Movement::from_tag`, recovered here because
/// [`PropStyle::for_movement`] takes the manifest's string convention
/// directly and `Movement` does not otherwise expose it.
const fn movement_tag(m: Movement) -> &'static str {
    match m {
        Movement::P1 => "P1",
        Movement::P2 => "P2",
        Movement::P3 => "P3",
        Movement::P4 => "P4",
    }
}

/// Every campaign level's `[meta] prop_style` (parsed by `re_level`) equals
/// `PropStyle::for_movement` of its `campaign.toml` `movement` (parsed by
/// `terminal_state::flow`). If a future edit changes one without the other, this is
/// the only thing that would notice.
#[test]
fn campaign_prop_style_agrees_with_campaign_toml_movement() {
    let mut checked = 0;
    for entry in campaign_manifest().levels() {
        let lvl = re_level::load(level_path(&entry.path))
            .unwrap_or_else(|e| panic!("{}: {e}", entry.path));
        assert!(
            lvl.prop_style_authored,
            "{}: every campaign level must author [meta] prop_style (A1)",
            entry.path
        );
        let want = PropStyle::for_movement(movement_tag(entry.movement)).unwrap_or_else(|| {
            panic!(
                "{}: unrecognized movement tag {:?}",
                entry.path, entry.movement
            )
        });
        assert_eq!(
            lvl.prop_default_style,
            want,
            "{}: [meta] prop_style must equal the movement's pack ({:?} -> {})",
            entry.path,
            entry.movement,
            want.as_str()
        );

        // Save/reload round trip: the agreement must survive an editor save,
        // not just the level's own authored text (re_voximport is the only
        // writer that ever touches this key — see `ts_editor`'s save path).
        let saved = re_voximport::to_lvl_text(&lvl);
        let reloaded = re_level::load_string(&saved, &entry.path)
            .unwrap_or_else(|e| panic!("{}: re-parse after save failed: {e}", entry.path));
        assert_eq!(
            reloaded.prop_default_style, want,
            "{}: prop_style must survive a re_voximport save/reload cycle",
            entry.path
        );
        assert!(
            reloaded.prop_style_authored,
            "{}: stays authored after a save/reload",
            entry.path
        );

        checked += 1;
    }
    assert_eq!(
        checked, 12,
        "expected all twelve campaign levels to be checked"
    );
}

/// A cheap sanity floor: the loop above can't silently pass by finding zero
/// disagreements because it iterated zero movements or a degenerate manifest.
#[test]
fn campaign_has_all_four_movements_and_twelve_levels() {
    let levels = campaign_manifest().levels();
    assert_eq!(
        levels.len(),
        12,
        "the campaign manifest must list all twelve levels"
    );
    let seen: std::collections::HashSet<Movement> = levels.iter().map(|e| e.movement).collect();
    assert_eq!(
        seen.len(),
        4,
        "expected all four movements P1..P4 in the manifest"
    );
}
