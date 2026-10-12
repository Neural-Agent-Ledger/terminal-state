//! Campaign RENDER SANITY — proves every one of the twelve campaign levels, as
//! authored, casts to a real, non-degenerate 3D frame from its own spawn point.
//!
//! # Why this exists
//!
//! `crates/re_render`'s own unit-test suite proves the ENGINE's raycaster
//! against synthetic and non-campaign fixtures (`re_world::test_room`,
//! `sky_open_room`, `08-material-demo.lvl`, …) — those tests are convenient
//! samples of an engine feature and must not depend on this repo's campaign
//! `.lvl` content, since the engine ships without it.
//!
//! What none of those prove is that the REAL, authored campaign geometry — the
//! actual colonnades, shelving, shafts and multi-storey rooms `levels/L1..L12`
//! ship — still casts to a sane frame end to end. Real shipped content has
//! repeatedly found renderer bugs synthetic fixtures missed: a sky-leak fix
//! that used to live here as `campaign_enclosed_rooms_drop_to_near_zero_sky`
//! / `campaign_open_top_finale_room_keeps_its_sky` (now re_render-side,
//! re-pointed to non-campaign content) was itself found via a real `--dump`
//! audit of the shipped campaign, not a synthetic-room test.
//!
//! This is a property OF THE CAMPAIGN (every level renders sane real geometry
//! from its own spawn), so it belongs in the game repo, not `re_render`.
//!
//! # How to run
//! ```sh
//! cargo test -p terminal_state --test campaign_render
//! ```

use std::path::PathBuf;

use re_core::Framebuffer;
use re_level::load;
use re_render::{composite::composite_dump_once, registry_for_map, Emitter, Fov};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
}

/// The twelve campaign levels, in authored `levels/campaign.toml` order.
/// Duplicated locally — same rationale as the sibling `campaign_walkable.rs`
/// and `campaign_level_parse.rs`: this is a separate test binary, and there is
/// no shared lib surface for test-only fixtures.
const CAMPAIGN_LEVELS: [(&str, &str); 12] = [
    ("L1-first-pulse", "levels/L1-first-pulse.lvl"),
    ("L2-two-make-three", "levels/L2-two-make-three.lvl"),
    ("L3-full-spectrum", "levels/L3-full-spectrum.lvl"),
    ("L4-the-angle", "levels/L4-the-angle.lvl"),
    ("L5-soft-footstep", "levels/L5-soft-footstep.lvl"),
    ("L6-polysemantic", "levels/L6-polysemantic.lvl"),
    ("L7-hidden-depths", "levels/L7-hidden-depths.lvl"),
    ("L8-cold-room", "levels/L8-cold-room.lvl"),
    ("L9-stereopsis", "levels/L9-stereopsis.lvl"),
    ("L10-lying-door", "levels/L10-lying-door.lvl"),
    ("L11-inner-circuit", "levels/L11-inner-circuit.lvl"),
    ("L12-closing", "levels/L12-closing.lvl"),
];

const COLS: usize = 100;
const ROWS: usize = 40;

/// Composite (the SAME pipeline `--dump` and the live game use, not the bare
/// raycast) a real dump-sized frame at `id`'s own `spawn_camera()`, and assert:
/// some WALL geometry is hit, some FLOOR geometry is hit (not an all-void
/// frame), and every cell's depth is finite and non-negative — the class of
/// defect (NaN/negative depth reaching the frame) this project has hit before,
/// checked here against the real, complex authored geometry rather than a
/// synthetic room.
fn assert_campaign_level_renders(id: &str, rel_path: &str) {
    let path = workspace_root().join(rel_path);
    assert!(
        path.exists(),
        "{id}: level file missing: {}",
        path.display()
    );
    let lv = load(&path).unwrap_or_else(|e| panic!("{id}: load {}: {e}", path.display()));

    let map = lv.build_map();
    let cam = lv.spawn_camera();
    let registry = registry_for_map(&map);
    let fov = Fov::default();
    let emitters: Vec<Emitter> = lv.emitters.iter().map(Emitter::from_spec).collect();

    let mut fb = Framebuffer::new(COLS, ROWS);
    composite_dump_once(&map, &cam, &registry, &fov, &emitters, &mut fb);

    let mut walls = 0usize;
    let mut floors = 0usize;
    for row in 0..fb.rows() {
        for col in 0..fb.cols() {
            let c = fb.get(col, row).expect("in-bounds cell");
            assert!(
                c.depth.is_finite() && c.depth >= 0.0,
                "{id}: cell ({col},{row}) has a non-finite or negative depth ({}) — real \
                 campaign geometry must never reach the frame with a broken depth",
                c.depth
            );
            if c.surface.is_wall() {
                walls += 1;
            }
            if c.surface == re_core::SurfaceKind::Floor {
                floors += 1;
            }
        }
    }
    assert!(
        walls > 0,
        "{id}: spawn-camera frame shows NO wall geometry at all — degenerate cast"
    );
    assert!(
        floors > 0,
        "{id}: spawn-camera frame shows NO floor geometry at all — degenerate cast"
    );
}

// ═════════════════════════════════════════════════════════════════════════
// One #[test] per level — named so `cargo test -p terminal_state --test
// campaign_render` lists exactly the twelve levels this suite covers.
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn l1_first_pulse_renders() {
    assert_campaign_level_renders(CAMPAIGN_LEVELS[0].0, CAMPAIGN_LEVELS[0].1);
}

#[test]
fn l2_two_make_three_renders() {
    assert_campaign_level_renders(CAMPAIGN_LEVELS[1].0, CAMPAIGN_LEVELS[1].1);
}

#[test]
fn l3_full_spectrum_renders() {
    assert_campaign_level_renders(CAMPAIGN_LEVELS[2].0, CAMPAIGN_LEVELS[2].1);
}

#[test]
fn l4_the_angle_renders() {
    assert_campaign_level_renders(CAMPAIGN_LEVELS[3].0, CAMPAIGN_LEVELS[3].1);
}

#[test]
fn l5_soft_footstep_renders() {
    assert_campaign_level_renders(CAMPAIGN_LEVELS[4].0, CAMPAIGN_LEVELS[4].1);
}

#[test]
fn l6_polysemantic_renders() {
    assert_campaign_level_renders(CAMPAIGN_LEVELS[5].0, CAMPAIGN_LEVELS[5].1);
}

/// The enclosed-room regression's original real-data case (see the module doc
/// above): a fully sealed campaign room must still render real wall/floor
/// geometry (this test doesn't measure the sky fraction itself — that generic
/// engine property now lives in `re_render`'s own suite against non-campaign
/// fixtures — but it keeps L7 in the campaign-wide render-sanity sweep).
#[test]
fn l7_hidden_depths_renders() {
    assert_campaign_level_renders(CAMPAIGN_LEVELS[6].0, CAMPAIGN_LEVELS[6].1);
}

#[test]
fn l8_cold_room_renders() {
    assert_campaign_level_renders(CAMPAIGN_LEVELS[7].0, CAMPAIGN_LEVELS[7].1);
}

#[test]
fn l9_stereopsis_renders() {
    assert_campaign_level_renders(CAMPAIGN_LEVELS[8].0, CAMPAIGN_LEVELS[8].1);
}

#[test]
fn l10_lying_door_renders() {
    assert_campaign_level_renders(CAMPAIGN_LEVELS[9].0, CAMPAIGN_LEVELS[9].1);
}

#[test]
fn l11_inner_circuit_renders() {
    assert_campaign_level_renders(CAMPAIGN_LEVELS[10].0, CAMPAIGN_LEVELS[10].1);
}

/// The open-top-finale regression's original real-data case (see the module
/// doc above): L12's deliberately roofless vault must still render real
/// wall/floor geometry from spawn (the sky-panorama-dominance property itself
/// now lives in `re_render`'s own suite against the `sky_open_room` fixture).
#[test]
fn l12_closing_renders() {
    assert_campaign_level_renders(CAMPAIGN_LEVELS[11].0, CAMPAIGN_LEVELS[11].1);
}
