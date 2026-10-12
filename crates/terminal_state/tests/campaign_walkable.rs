//! Campaign WALKABILITY test — proves a player can PHYSICALLY finish each of
//! the twelve campaign levels, not merely that the puzzle graph is solvable.
//!
//! # Why this exists
//!
//! `levels/L5-soft-footstep.lvl` shipped UNWINNABLE and every existing test
//! passed: `re_puzzle::solve_brute_force` proved the network solvable,
//! `crates/terminal_state/tests/campaign_solve_through.rs` drove L1–L8 to a real
//! `state.solved == true` by calling `re_puzzle::puzzle_interact` at every
//! authored switch cell, and `re_lvlcheck` proved every switch had *some*
//! standable column nearby. None of that proves a player can reach the DOOR
//! after solving, and none of it proves the win condition is *simultaneously*
//! satisfiable when a `[zone_input]` un-solves the network the instant the
//! player leaves the floor-zone (`watch_win` in `src/main.rs` fires only when
//! `solved && at_door` hold in the SAME frame). L5's warm zone sat
//! nine cells from the door — solvable, reachable-by-column, and completely
//! uncompletable, all three green.
//!
//! This test closes that gap with a REAL physics flood fill from spawn: it
//! drives `re_world::CameraPose::move_with_collision` + `step_gravity` — the
//! exact collision/gravity code the shipped game runs every frame, including
//! ramp/staircase climbing (`levels/L1-first-pulse.lvl`'s two-storey puzzle)
//! — outward from the spawn point until no new cell is discovered. It then
//! checks the real win predicates against the reachable set:
//!
//! 1. every switchable pipe's switch cell is within interact range of some
//!    reachable, grounded cell — horizontally, `terminal_state::flow::faced_switch`'s
//!    reach rule (Chebyshev distance 1 in x/y); vertically, standing height
//!    PLUS a jump (`re_lvlcheck::switch_physically_reachable`'s formula —
//!    several campaign switches sit 2-3 cells up and are meant to be hit
//!    mid-jump);
//! 2. the door is within `re_world::DOOR_ENTRY_RADIUS` of some reachable cell
//!    (`re_world::at_door`, the exact win predicate);
//! 3. — the rule that would have caught L5 — for EVERY level with a
//!    `[zone_input]` whose input the win genuinely depends on, there exists a
//!    reachable cell that is BOTH inside that zone's box AND within
//!    `DOOR_ENTRY_RADIUS` of the door, i.e. `solved && at_door` CAN hold in
//!    the same frame. This is a general rule over `level.zone_inputs`, not an
//!    L5 special case — it runs (and passes trivially, vacuously) on every
//!    other campaign level, none of which authors a zone.
//!
//! No level file under `levels/` is modified by this test or by fixing a
//! failure it finds — a FAIL here is reported, not patched.
//!
//! # How to run
//! ```sh
//! cargo test -p terminal_state --test campaign_walkable
//! ```

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;

use re_core::Vec3;
use re_level::{load, Level};
use re_puzzle::{puzzle_interact, set_input_value, solve_brute_force, PuzzleState, Solvability};
use re_world::{at_door, CameraPose, Map, PlayerMotion, DOOR_ENTRY_RADIUS, PLAYER_RADIUS};

// ── Workspace root helper (same pattern as the sibling integration tests) ───

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
}

/// The twelve campaign levels, in authored `levels/campaign.toml` order.
/// Cross-checked against `terminal_state::flow::campaign_manifest` (the same
/// embedded file the shipped game parses) in
/// `campaign_manifest_lists_exactly_these_twelve_levels_in_order` below, so
/// this constant cannot silently drift from the real manifest.
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

// ═════════════════════════════════════════════════════════════════════════
// Physics-based flood fill — the REAL collision + gravity code, not a
// hand-rolled discrete pathfinder. This is what makes the test prove
// PHYSICAL reachability (ramps, radius-probed wall-slide, gravity settling)
// rather than a graph abstraction that could miss exactly the class of bug
// that shipped L5.
// ═════════════════════════════════════════════════════════════════════════

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

/// Nearest Chebyshev (cell) distance from any reachable cell to `target` —
/// used only to make a FAIL message diagnostic (how close did we get?).
fn nearest_chebyshev(reachable: &HashMap<(i32, i32, i32), Vec3>, target: (i32, i32, i32)) -> i32 {
    reachable
        .keys()
        .map(|&(x, y, z)| {
            (x - target.0)
                .abs()
                .max((y - target.1).abs())
                .max((z - target.2).abs())
        })
        .min()
        .unwrap_or(i32::MAX)
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

// ═════════════════════════════════════════════════════════════════════════
// Puzzle-side helpers — building a genuinely SOLVED `PuzzleState` the same
// way `campaign_solve_through.rs` does (real `puzzle_interact` calls at real
// switch cells, never `state.solved = true` directly), so the zone/door
// simultaneity check below tests the real win, not an assumed one.
// ═════════════════════════════════════════════════════════════════════════

/// Build a puzzle state that reports `solved == true` by applying the
/// minimal switch-toggle set `solve_brute_force` reports, through the real
/// `puzzle_interact` seam at each pipe's authored switch cell — exactly
/// `campaign_solve_through.rs::solve_minimally`, duplicated locally because
/// that file is a separate test binary (no shared lib surface for test-only
/// helpers). `Err` names why a winning configuration could not be built.
fn winning_state(level: &Level) -> Result<PuzzleState, String> {
    let mut probe = PuzzleState::from_level(level)
        .map_err(|e| format!("puzzle graph must build (DAG): {e:?}"))?;
    let switchable: Vec<usize> = probe
        .pipes
        .iter()
        .enumerate()
        .filter(|(_, p)| p.switch_cell.is_some())
        .map(|(i, _)| i)
        .collect();

    let verdict = solve_brute_force(&mut probe);
    let mut fresh = PuzzleState::from_level(level)
        .map_err(|e| format!("puzzle graph must build (DAG): {e:?}"))?;
    match verdict {
        Solvability::TriviallySolvedAtLoad => {
            // The authored initial configuration already wins.
        }
        Solvability::Solvable { winning_mask, .. } => {
            for (bit, &pipe_idx) in switchable.iter().enumerate() {
                let want_on = (winning_mask >> bit) & 1 == 1;
                let is_on = fresh.pipes[pipe_idx].is_on();
                if want_on != is_on {
                    let (x, y, z) = fresh.pipes[pipe_idx].switch_coord().ok_or_else(|| {
                        format!("switchable pipe {pipe_idx} has no switch coordinate")
                    })?;
                    if !puzzle_interact(&mut fresh, x, y, z) {
                        return Err(format!(
                            "toggle at switch cell ({x},{y},{z}) for pipe {pipe_idx} did not take"
                        ));
                    }
                }
            }
        }
        other => {
            return Err(format!(
                "not solvable by toggling switches alone: {other:?}"
            ))
        }
    }
    if !fresh.solved {
        return Err(
            "applied the solver's winning toggle set but the resulting state did not report solved"
                .into(),
        );
    }
    Ok(fresh)
}

/// Which of `level.zone_inputs` (by index) are GENUINELY required for the win
/// — forcing that zone's input to 0 while the rest of `winning` stays at its
/// solved configuration breaks `solved`. A decorative zone whose value the
/// win does not depend on is not required, and the simultaneity check is not
/// applied to it (there is nothing to prove: the player can be anywhere for
/// an input the win ignores).
fn zones_required_for_win(level: &Level, winning: &PuzzleState) -> Vec<usize> {
    let mut required = Vec::new();
    for (zi, zone) in level.zone_inputs.iter().enumerate() {
        let mut probe = winning.clone();
        set_input_value(&mut probe, zone.node_index, 0);
        if !probe.solved {
            required.push(zi);
        }
    }
    required
}

// ═════════════════════════════════════════════════════════════════════════
// The per-level check.
// ═════════════════════════════════════════════════════════════════════════

/// Load `id` from `rel_path`, physics-flood-fill from spawn, and assert:
/// every switchable pipe's switch is reachable, the door is reachable, and
/// (for levels with `[zone_input]`) the required zone(s) and the door are
/// SIMULTANEOUSLY reachable — the general rule that would have caught L5.
fn assert_campaign_level_walkable(id: &str, rel_path: &str) {
    let path = workspace_root().join(rel_path);
    assert!(
        path.exists(),
        "{id}: level file missing: {}",
        path.display()
    );
    let lv = load(&path).unwrap_or_else(|e| panic!("{id}: load {}: {e}", path.display()));

    let map = lv.build_map();
    let mut cam = lv.spawn_camera();
    let spawn_motion = settle(&mut cam, &map);
    assert!(
        spawn_motion.grounded,
        "{id}: spawn point {:?} never settled onto solid ground — the player cannot even start",
        (lv.player.x, lv.player.y, lv.player.z)
    );

    let reachable = flood_fill_reachable(&map, cam.pos);
    assert!(
        reachable.len() > 1,
        "{id}: physics flood fill from spawn discovered ONLY the spawn cell ({} reachable cell(s)) \
         — the player is sealed in",
        reachable.len()
    );

    // ── 1. Every switchable pipe's switch cell is within interact range of
    // some reachable cell. ────────────────────────────────────────────────
    for (pi, pipe) in lv.pipes.iter().enumerate() {
        let Some(sc) = pipe.switch_cell else { continue };
        let Some(&cell) = pipe.path.get(sc) else {
            panic!(
                "{id}: pipe {pi} ({}->{}) switch_cell index {sc} is off the end of its \
                 {}-cell path — this is a level-authoring defect independent of physics",
                pipe.from,
                pipe.to,
                pipe.path.len()
            );
        };
        assert!(
            switch_reachable(&reachable, cell),
            "{id}: the switch for pipe {pi} ({}->{}) at cell {cell:?} is NOT within interact \
             range of any spawn-reachable cell (nearest reachable cell is {} away, Chebyshev) \
             — {} distinct cells were reachable from spawn {:?}",
            pipe.from,
            pipe.to,
            nearest_chebyshev(&reachable, cell),
            reachable.len(),
            (lv.player.x, lv.player.y, lv.player.z),
        );
    }

    // ── 2. The door itself is reachable. ──────────────────────────────────
    let door = (lv.door.x, lv.door.y, lv.door.z);
    let door_reachable = reachable
        .values()
        .any(|&p| at_door(p, door.0, door.1, door.2));
    assert!(
        door_reachable,
        "{id}: the door at {door:?} is NOT within DOOR_ENTRY_RADIUS ({DOOR_ENTRY_RADIUS}) of any \
         spawn-reachable cell (nearest reachable cell is {} away, Chebyshev) — {} distinct cells \
         were reachable from spawn {:?}",
        nearest_chebyshev(&reachable, door),
        reachable.len(),
        (lv.player.x, lv.player.y, lv.player.z),
    );

    // ── 3. The general zone+door simultaneity rule — this is what would
    // have caught L5. Runs (and passes vacuously) on every level with no
    // `[zone_input]`. ──────────────────────────────────────────────────────
    if !lv.zone_inputs.is_empty() {
        let winning = winning_state(&lv).unwrap_or_else(|e| {
            panic!("{id}: could not construct a winning puzzle state to test zone/door simultaneity: {e}")
        });
        let required = zones_required_for_win(&lv, &winning);
        for &zi in &required {
            let zone = &lv.zone_inputs[zi];
            let simultaneous = reachable.values().any(|&p| {
                let (cx, cy, cz) = cell_key(p);
                zone.contains(cx, cy, cz) && at_door(p, door.0, door.1, door.2)
            });
            let best = reachable
                .values()
                .filter(|&&p| {
                    let (cx, cy, cz) = cell_key(p);
                    zone.contains(cx, cy, cz)
                })
                .map(|&p| nearest_chebyshev(&HashMap::from([(cell_key(p), p)]), door))
                .min();
            assert!(
                simultaneous,
                "{id}: zone_input '{}' (region ({},{},{})-({},{},{})) is REQUIRED for the win \
                 (forcing it to 0 un-solves the puzzle) but no spawn-reachable cell is BOTH \
                 inside the zone AND within DOOR_ENTRY_RADIUS ({DOOR_ENTRY_RADIUS}) of the door \
                 at {door:?} — `solved && at_door` (the exact `watch_win` predicate in \
                 src/main.rs) can NEVER hold in the same frame. Closest reachable \
                 in-zone cell is {} away from the door (Chebyshev). This is exactly the L5 \
                 unwinnable-shipped bug class, checked generally over every authored zone.",
                zone.name,
                zone.x0,
                zone.y0,
                zone.z0,
                zone.x1,
                zone.y1,
                zone.z1,
                best.map_or_else(
                    || "no reachable in-zone cell at all".to_string(),
                    |d| d.to_string()
                ),
            );
        }
    }
}

// ═════════════════════════════════════════════════════════════════════════
// One #[test] per level — named so `cargo test -p terminal_state --test
// campaign_walkable` lists exactly the twelve levels this suite covers.
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn l1_first_pulse_is_walkable() {
    assert_campaign_level_walkable(CAMPAIGN_LEVELS[0].0, CAMPAIGN_LEVELS[0].1);
}

#[test]
fn l2_two_make_three_is_walkable() {
    assert_campaign_level_walkable(CAMPAIGN_LEVELS[1].0, CAMPAIGN_LEVELS[1].1);
}

#[test]
fn l3_full_spectrum_is_walkable() {
    assert_campaign_level_walkable(CAMPAIGN_LEVELS[2].0, CAMPAIGN_LEVELS[2].1);
}

#[test]
fn l4_the_angle_is_walkable() {
    assert_campaign_level_walkable(CAMPAIGN_LEVELS[3].0, CAMPAIGN_LEVELS[3].1);
}

/// The regression test: L5 shipped unwinnable (zone nine cells from the
/// door). Would have FAILED before the fix that moved the zone to the
/// threshold `(1,5,1)-(2,7,1)`; must PASS now.
#[test]
fn l5_soft_footstep_is_walkable() {
    assert_campaign_level_walkable(CAMPAIGN_LEVELS[4].0, CAMPAIGN_LEVELS[4].1);
}

#[test]
fn l6_polysemantic_is_walkable() {
    assert_campaign_level_walkable(CAMPAIGN_LEVELS[5].0, CAMPAIGN_LEVELS[5].1);
}

#[test]
fn l7_hidden_depths_is_walkable() {
    assert_campaign_level_walkable(CAMPAIGN_LEVELS[6].0, CAMPAIGN_LEVELS[6].1);
}

#[test]
fn l8_cold_room_is_walkable() {
    assert_campaign_level_walkable(CAMPAIGN_LEVELS[7].0, CAMPAIGN_LEVELS[7].1);
}

#[test]
fn l9_stereopsis_is_walkable() {
    assert_campaign_level_walkable(CAMPAIGN_LEVELS[8].0, CAMPAIGN_LEVELS[8].1);
}

#[test]
fn l10_lying_door_is_walkable() {
    assert_campaign_level_walkable(CAMPAIGN_LEVELS[9].0, CAMPAIGN_LEVELS[9].1);
}

#[test]
fn l11_inner_circuit_is_walkable() {
    assert_campaign_level_walkable(CAMPAIGN_LEVELS[10].0, CAMPAIGN_LEVELS[10].1);
}

#[test]
fn l12_closing_is_walkable() {
    assert_campaign_level_walkable(CAMPAIGN_LEVELS[11].0, CAMPAIGN_LEVELS[11].1);
}

// ═════════════════════════════════════════════════════════════════════════
// Manifest cross-check — proves `CAMPAIGN_LEVELS` above cannot silently
// drift from the real, embedded `levels/campaign.toml` the shipped game
// parses.
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn campaign_manifest_lists_exactly_these_twelve_levels_in_order() {
    let manifest = terminal_state::flow::campaign_manifest();
    let entries = manifest.levels();
    assert_eq!(
        entries.len(),
        CAMPAIGN_LEVELS.len(),
        "campaign.toml must list exactly {} levels (this walkability suite covers every one); \
         got {}",
        CAMPAIGN_LEVELS.len(),
        entries.len()
    );
    for (i, (expected_id, expected_path)) in CAMPAIGN_LEVELS.iter().enumerate() {
        let entry = &entries[i];
        assert_eq!(
            entry.id.as_str(),
            *expected_id,
            "manifest position {i} must be {expected_id}; got {}",
            entry.id
        );
        assert_eq!(
            entry.path.as_str(),
            *expected_path,
            "manifest path for {expected_id} must be {expected_path}; got {}",
            entry.path
        );
        println!("campaign_walkable covers: {expected_id} ({expected_path})");
    }
}

/// Guard against a future `try_step`/`flood_fill_reachable` refactor silently
/// becoming a no-op (e.g. `switch_reachable` always returning `true`) by
/// proving the physics flood fill can also report NON-reachability on a
/// synthetic sealed room — a cell entirely surrounded by solid walls is not
/// discovered even though it exists in the map.
#[test]
fn flood_fill_does_not_claim_a_sealed_cell_is_reachable() {
    use re_world::{CellKind, Map};

    // A standard single-storey room: z=0 floor slab, z=1 the walkable air
    // layer (edges WallSolid, interior Empty), z=2 ceiling slab — the same
    // shape every campaign level's `[cells]` grid uses. A fully sealed 1x1x1
    // vault at (5,5,1), walled on all 4 horizontal sides with no opening,
    // must NOT be found reachable from spawn (1,1,1).
    let mut map = Map::new(8, 8, 3);
    for y in 0..8usize {
        for x in 0..8usize {
            map.set_cell(x, y, 0, CellKind::Floor, 0);
            map.set_cell(x, y, 2, CellKind::WallSolid, 0);
            let edge = x == 0 || y == 0 || x == 7 || y == 7;
            map.set_cell(
                x,
                y,
                1,
                if edge {
                    CellKind::WallSolid
                } else {
                    CellKind::Empty
                },
                0,
            );
        }
    }
    // Seal the vault: (5,5,1) surrounded by WallSolid on all 4 horizontal
    // sides (its neighbors), with air only inside itself.
    for &(nx, ny) in &[(4usize, 5usize), (6, 5), (5, 4), (5, 6)] {
        map.set_cell(nx, ny, 1, CellKind::WallSolid, 0);
    }
    map.set_cell(5, 5, 1, CellKind::Empty, 0);

    let spawn = Vec3::new(1.5, 1.5, re_world::STANDING_EYE_Z);
    let reachable = flood_fill_reachable(&map, spawn);

    assert!(
        !reachable.contains_key(&(5, 5, 1)),
        "the sealed vault at (5,5,1) must NOT be reported reachable — the flood fill would be \
         a no-op if it were"
    );
    // Sanity: the open room (a normal interior cell) IS found.
    assert!(
        reachable.contains_key(&(2, 2, 1)),
        "an ordinary open interior cell must be reachable — the flood fill found nothing at all"
    );
}
