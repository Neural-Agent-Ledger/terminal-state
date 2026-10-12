//! Lit `[emit]` figures are VISUAL FEEDBACK, not level geometry — the campaign guard.
//!
//! When a node fires, `re_runtime::emit_parts` materialises its authored figure on the
//! pedestal. `campaign_walkable.rs` floods each level on the map as BUILT (no figure
//! stamped), so nothing proved that a lit figure does not take away a route the level
//! needs — or, the other way round, that no level quietly relies on a lit figure as a
//! stepping block. This test lights EVERY clause of every campaign level (every node
//! forced to `activation = 1`, the worst case) and runs the same real-physics flood
//! fill (`move_with_collision` + `step_gravity`) on both maps:
//!
//! 1. nothing is reachable ONLY with the figures lit — no puzzle route, jump or ledge
//!    depends on a figure being solid (a figure is never a step);
//! 2. with every figure lit, every switch and the door are still reachable;
//! 3. whatever lighting takes away is confined to the figure's own footprint — a lit
//!    figure never severs a region of the room (never closes an aisle behind it).
//!
//! Levels are read, never modified.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;

use re_core::Vec3;
use re_level::load;
use re_puzzle::PuzzleState;
use re_world::{at_door, CameraPose, Map, PlayerMotion, PLAYER_RADIUS};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
}

/// The twelve campaign levels (`levels/campaign.toml` order; the manifest cross-check
/// lives in `campaign_walkable.rs`).
const CAMPAIGN_LEVELS: [&str; 12] = [
    "levels/L1-first-pulse.lvl",
    "levels/L2-two-make-three.lvl",
    "levels/L3-full-spectrum.lvl",
    "levels/L4-the-angle.lvl",
    "levels/L5-soft-footstep.lvl",
    "levels/L6-polysemantic.lvl",
    "levels/L7-hidden-depths.lvl",
    "levels/L8-cold-room.lvl",
    "levels/L9-stereopsis.lvl",
    "levels/L10-lying-door.lvl",
    "levels/L11-inner-circuit.lvl",
    "levels/L12-closing.lvl",
];

// ── the real-physics flood fill, identical to `campaign_walkable.rs` ──────────

const DT: f32 = 1.0 / 60.0;
const H_STEP: f32 = 0.05;
const GRAVITY_SUBSTEPS: usize = 3;
/// How many `(move_with_collision + gravity)` iterations one direction-probe
/// gets to cross a cell. `30 * H_STEP` = 1.5 cells of attempted travel —
/// comfortably more than the 1.0 needed, with margin for wall-slide loss.
const ADVANCE_ITERS: usize = 30;
/// Extra gravity-only iterations to let a landing settle (ramp seams, a
/// multi-frame fall onto a lower floor).
const SETTLE_ITERS: usize = 90;
/// Node-expansion budget — campaign rooms are a few hundred cells at most;
/// this is a generous multiple, purely a runaway guard.
const EXPANSION_BUDGET: usize = 40_000;

const DIRS_8: [(f32, f32); 8] = [
    (1.0, 0.0),
    (-1.0, 0.0),
    (0.0, 1.0),
    (0.0, -1.0),
    (
        std::f32::consts::FRAC_1_SQRT_2,
        std::f32::consts::FRAC_1_SQRT_2,
    ),
    (
        std::f32::consts::FRAC_1_SQRT_2,
        -std::f32::consts::FRAC_1_SQRT_2,
    ),
    (
        -std::f32::consts::FRAC_1_SQRT_2,
        std::f32::consts::FRAC_1_SQRT_2,
    ),
    (
        -std::f32::consts::FRAC_1_SQRT_2,
        -std::f32::consts::FRAC_1_SQRT_2,
    ),
];

/// Run gravity-only settling at the current pose until grounded or the
/// budget runs out. Returns the final `PlayerMotion` (read `.grounded`).
fn settle(cam: &mut CameraPose, map: &Map) -> PlayerMotion {
    let mut motion = PlayerMotion::default();
    for _ in 0..SETTLE_ITERS {
        cam.step_gravity(&mut motion, DT, map);
        if motion.grounded {
            break;
        }
    }
    motion
}

/// The integer cell key of a world position, using the SAME `floor()`
/// convention `re_world::at_door` and `terminal_state::flow::apply_zone_inputs` use
/// — so keys compare directly against door/zone coordinates without any
/// eye-height adjustment.
const fn cell_key(p: Vec3) -> (i32, i32, i32) {
    (p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32)
}

/// Attempt to physically walk from `from` one cell in direction `dir`, using
/// the real `move_with_collision` (radius-probed wall-slide) + `step_gravity`
/// (ramp-following, floor-snapping) code. Returns the settled landing
/// position iff the player made real progress (crossed into a different
/// integer cell) and ended up GROUNDED there — a mid-air / falling landing is
/// not a standable destination and is not reported as reachable.
fn try_step(map: &Map, from: Vec3, dir: (f32, f32)) -> Option<Vec3> {
    let mut cam = CameraPose {
        pos: from,
        yaw: 0.0,
        pitch: 0.0,
    };
    let mut motion = PlayerMotion::default();
    for _ in 0..ADVANCE_ITERS {
        cam.move_with_collision(dir.0 * H_STEP, dir.1 * H_STEP, map, PLAYER_RADIUS);
        for _ in 0..GRAVITY_SUBSTEPS {
            cam.step_gravity(&mut motion, DT, map);
        }
    }
    // Let a ramp seam / short fall resolve before judging groundedness.
    for _ in 0..SETTLE_ITERS {
        if motion.grounded {
            break;
        }
        cam.step_gravity(&mut motion, DT, map);
    }
    if !motion.grounded {
        return None;
    }
    let moved = (cam.pos.x - from.x).abs() > 0.5 || (cam.pos.y - from.y).abs() > 0.5;
    if !moved {
        return None;
    }
    Some(cam.pos)
}

/// Physics-based flood fill of every cell reachable from `spawn` (which must
/// already be a settled, grounded position). Returns one representative
/// grounded world position per reachable integer cell key.
fn flood_fill_reachable(map: &Map, spawn: Vec3) -> HashMap<(i32, i32, i32), Vec3> {
    let mut reachable: HashMap<(i32, i32, i32), Vec3> = HashMap::new();
    let mut queue: VecDeque<Vec3> = VecDeque::new();
    reachable.insert(cell_key(spawn), spawn);
    queue.push_back(spawn);

    let mut expansions = 0usize;
    while let Some(pos) = queue.pop_front() {
        if expansions >= EXPANSION_BUDGET {
            break;
        }
        expansions += 1;
        for &dir in &DIRS_8 {
            let Some(landed) = try_step(map, pos, dir) else {
                continue;
            };
            let key = cell_key(landed);
            if reachable.contains_key(&key) {
                continue;
            }
            // Re-anchor to the cell's horizontal centre for a clean next
            // probe (the raw landing may be off-centre from a wall-slide),
            // keeping whatever z the physics produced. Falls back to the raw
            // landed position if the centre itself doesn't settle grounded
            // (e.g. a shaped/cut corner cell).
            let anchor = Vec3::new(key.0 as f32 + 0.5, key.1 as f32 + 0.5, landed.z);
            let mut acam = CameraPose {
                pos: anchor,
                yaw: 0.0,
                pitch: 0.0,
            };
            let am = settle(&mut acam, map);
            let canonical = if am.grounded { acam.pos } else { landed };
            reachable.insert(key, canonical);
            queue.push_back(canonical);
        }
    }
    reachable
}

/// Peak height (in cells) a jump from a standing start reaches:
/// `h = v² / (2·g)`, from the engine's own constants. Several campaign
/// switches sit 2-3 cells above the standing floor and are reached by
/// jumping toward them and interacting mid-air — `re_lvlcheck`'s
/// `switch_physically_reachable` already accounts for this; this mirrors its
/// exact `reach_lo`/`reach_hi` formula rather than re-deriving a new one.
fn max_jump_height() -> f32 {
    (re_world::JUMP_IMPULSE * re_world::JUMP_IMPULSE) / (2.0 * re_world::GRAVITY)
}

/// Is `switch` within interact range of some reachable, GROUNDED cell?
/// Mirrors `terminal_state::flow::faced_switch`'s real reach rule for the
/// horizontal axes: the player stands in a cell such that `floor(pos + dir)`
/// (for SOME achievable continuous yaw) lands on the switch cell — some
/// reachable cell is within Chebyshev distance 1 in x/y (any of the 8
/// neighbors, or the switch's own column — a player facing straight along
/// one axis can target the column they stand in).
///
/// The VERTICAL axis is more generous than a flat-footed `faced_switch`
/// probe: the player can JUMP toward a switch above the standing floor and
/// interact at the apex, so the reachable z-band from a grounded cell at
/// z-floor `z` is `[z - 0.5, z + max_jump_height() + 1.5]` — exactly
/// `re_lvlcheck::switch_physically_reachable`'s formula (the `+1.5` is the
/// probe's `dz=+1` tolerance plus a head/arm allowance), now checked against
/// cells actually proven SPAWN-CONNECTED by the physics flood fill, which is
/// the gap `re_lvlcheck` alone does not close.
fn switch_reachable(reachable: &HashMap<(i32, i32, i32), Vec3>, switch: (i32, i32, i32)) -> bool {
    let jump_reach = max_jump_height();
    reachable.keys().any(|&(x, y, z)| {
        (x - switch.0).abs() <= 1
            && (y - switch.1).abs() <= 1
            && (switch.2 as f32) >= z as f32 - 0.5
            && (switch.2 as f32) <= z as f32 + jump_reach + 1.5
    })
}

#[test]
fn lit_emit_figures_never_carry_or_sever_a_campaign_route() {
    for rel in CAMPAIGN_LEVELS {
        let lv = load(workspace_root().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"));

        // Both maps carry the emit bindings (so the palette is identical); only one is lit.
        let mut unlit = lv.build_map();
        let _ = re_runtime::emit_parts::bind_emit_parts(&lv, &mut unlit);
        let mut lit = lv.build_map();
        let mut parts = re_runtime::emit_parts::bind_emit_parts(&lv, &mut lit);
        let mut all_fired = PuzzleState::from_level(&lv).unwrap_or_else(|e| panic!("{rel}: {e:?}"));
        for n in &mut all_fired.nodes {
            n.activation = 1;
        }
        re_runtime::emit_parts::sync_emit_parts(&mut parts, &all_fired, &mut lit);
        assert!(
            parts.lit_cell_count() > 0,
            "{rel}: every campaign level authors an emit figure"
        );

        let flood = |map: &Map| {
            let mut cam = lv.spawn_camera();
            settle(&mut cam, map);
            flood_fill_reachable(map, cam.pos)
        };
        let r_unlit = flood(&unlit);
        let r_lit = flood(&lit);

        // 1. No cell is reachable only BECAUSE a figure is lit.
        let gained: Vec<_> = r_lit.keys().filter(|k| !r_unlit.contains_key(k)).collect();
        assert!(
            gained.is_empty(),
            "{rel}: cells {gained:?} are reachable only with the emit figures lit — a figure              is being used as level geometry (a step), which it must not be"
        );

        // 2. Every switch and the door stay reachable with every figure lit.
        for (pi, pipe) in lv.pipes.iter().enumerate() {
            let Some(cell) = pipe.switch_cell.and_then(|sc| pipe.path.get(sc).copied()) else {
                continue;
            };
            assert!(
                switch_reachable(&r_lit, cell),
                "{rel}: with the emit figures lit, the switch for pipe {pi} at {cell:?} is unreachable"
            );
        }
        let door = (lv.door.x, lv.door.y, lv.door.z);
        assert!(
            r_lit.values().any(|&p| at_door(p, door.0, door.1, door.2)),
            "{rel}: with the emit figures lit, the door at {door:?} is unreachable"
        );

        // 3. Anything lighting takes away is the figure's own footprint, nothing behind it.
        let footprint: HashSet<(i32, i32)> = parts
            .clauses
            .iter()
            .flat_map(|c| c.cells.iter().map(|&(x, y, _, _)| (x as i32, y as i32)))
            .collect();
        let severed: Vec<_> = r_unlit
            .keys()
            .filter(|k| !r_lit.contains_key(k) && !footprint.contains(&(k.0, k.1)))
            .collect();
        assert!(
            severed.is_empty(),
            "{rel}: lighting the emit figures cuts off {severed:?}, which are not under a figure"
        );
    }
}
