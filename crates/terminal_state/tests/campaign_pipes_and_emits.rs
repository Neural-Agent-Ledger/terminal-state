//! Campaign content-reachability tests: the authored puzzle NETWORK and the authored
//! per-level `[emit]` REWARD must actually exist in the world the game builds.
//!
//! Both were authored-but-invisible before this suite:
//!
//! * `[room] render_pipes` defaulted OFF and not one of `levels/L1..L12` set it, so the
//!   pipes — the only thing the player manipulates — were never stamped as geometry.
//!   With them off, the pipe flow scroll, the powered-hue lift, the streaming node
//!   word-pool, and both event pulses (switch-toggle / cycle-reject, which write to
//!   `encode_pipe_side_data(pipe)` tags) had no surface to appear on.
//! * `Level::emits` was parsed and name-resolved and then consumed by nothing.
//!
//! These tests pin the level data (`render_pipes` really is on, and it really does
//! produce tagged `WallTrace` cells) and the runtime rule (a source node firing stamps
//! its part; un-firing removes it; a reset clears it).

use re_level::Level;
use re_puzzle::PuzzleState;
use re_runtime::emit_parts::{bind_emit_parts, sync_emit_parts, EmitParts};
use re_world::{CellKind, Map};

/// The 12 shipped campaign levels, in campaign order.
const CAMPAIGN: &[&str] = &[
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

/// Integration tests run with the CRATE root as CWD; the levels live at the workspace
/// root, one directory up from `crates/terminal_state`.
fn level_path(rel: &str) -> String {
    format!("../../{rel}")
}

fn load(rel: &str) -> Level {
    re_level::load(level_path(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

/// Count the cells carrying a live PIPE tag (`side_data` bit 15), whatever kind they
/// ended up as — this is the tag the render state bus and the event pulses key on.
fn pipe_tagged_cells(map: &Map) -> Vec<(i32, i32, i32, CellKind)> {
    let (sx, sy, sz) = map.size();
    let mut out = Vec::new();
    for z in 0..sz as i32 {
        for y in 0..sy as i32 {
            for x in 0..sx as i32 {
                if re_core::decode_pipe_tag(map.side_data_at(x, y, z)).is_some() {
                    out.push((x, y, z, map.cell_at(x, y, z)));
                }
            }
        }
    }
    out
}

/// EVERY campaign level opts into pipe rendering. This is level DATA, so it is pinned
/// here rather than left to a reviewer's grep.
#[test]
fn every_campaign_level_renders_its_pipes() {
    for rel in CAMPAIGN {
        let level = load(rel);
        assert!(
            level.render_pipes,
            "{rel}: [room] render_pipes must be on — otherwise the puzzle network the \
             player manipulates is never drawn"
        );
        assert!(
            !level.pipes.is_empty(),
            "{rel}: a campaign level must author pipes"
        );
    }
}

/// `render_pipes` must actually reach the built map: every campaign level's map carries
/// pipe-tagged `WallTrace` cells (the surface the flow material, the powered-hue lift,
/// the pool words, and the two event pulses all render on).
#[test]
fn campaign_levels_build_tagged_walltrace_pipe_cells() {
    for rel in CAMPAIGN {
        let level = load(rel);
        let map = level.build_map();
        let tagged = pipe_tagged_cells(&map);
        let traces = tagged
            .iter()
            .filter(|&&(_, _, _, k)| k == CellKind::WallTrace)
            .count();
        assert!(
            traces > 0,
            "{rel}: render_pipes is on but the built map has NO tagged WallTrace cells \
             ({} tagged cells total) — the pipes are still invisible",
            tagged.len()
        );
    }
}

/// A pipe trace never replaces a puzzle ANCHOR or a room surface.
///
/// `re_level::stamp_pipes` writes into `CellKind::Empty` only, so a trace can never
/// clobber a node, the door, a wall, a floor or a ceiling. The one thing that CAN
/// outrank a stamped trace afterwards is the level's own `[cells]` grid, which is
/// authoritative for geometry and stamps last while PRESERVING `side_data` — so on an
/// editor-format level a path cell that the author painted as a ramp tread or a floor
/// slab legitimately ends up solid while still carrying the tag. That case is allowed
/// here (and only for the surface kinds an author paints); a tagged `Object` would mean
/// a real clobber and is not.
#[test]
fn pipe_traces_never_clobber_a_puzzle_anchor() {
    for rel in CAMPAIGN {
        let level = load(rel);
        let map = level.build_map();
        for (x, y, z, kind) in pipe_tagged_cells(&map) {
            assert_ne!(
                kind,
                CellKind::Object,
                "{rel}: node/door cell ({x},{y},{z}) was overwritten by a pipe trace"
            );
            assert!(
                matches!(
                    kind,
                    CellKind::WallTrace
                        | CellKind::Empty
                        | CellKind::WallSolid
                        | CellKind::Floor
                        | CellKind::Ceiling
                ),
                "{rel}: ({x},{y},{z}) carries a pipe tag on an unexpected {kind:?}"
            );
        }
    }
}

/// KNOWN LIMITATION, pinned so it cannot regress silently: `[cells]` is authoritative
/// for geometry and stamps AFTER `stamp_pipes`, so on a `[cells]`-authored level any
/// pipe path cell the author painted as air is demoted from `WallTrace` back to
/// `Empty` — the tag survives, the visible trace does not.
///
/// L1 is the only campaign level in that format. Its ground-floor R→switch leg is
/// therefore painted explicitly as `/` in the `[cells]` grid (see the level's own
/// comment); the rest of its path — the staircase corridor and the two floor-2 cells
/// beside the exit door — is deliberately left as air because `WallTrace` is
/// COLLIDABLE and those cells sit in a walking layer.
///
/// This test states exactly how many of L1's path cells survive, so a future `re_level`
/// ordering fix (re-stamping traces after `[cells]`, mirroring the node-anchor
/// re-stamp) shows up here as a deliberate change rather than a surprise.
#[test]
fn l1_cells_grid_demotes_the_unpainted_part_of_its_pipe_path() {
    let level = load("levels/L1-first-pulse.lvl");
    let map = level.build_map();
    let path = &level.pipes[0].path;
    let visible = path
        .iter()
        .filter(|&&(x, y, z)| map.cell_at(x, y, z) == CellKind::WallTrace)
        .count();
    let demoted_to_air = path
        .iter()
        .filter(|&&(x, y, z)| {
            map.cell_at(x, y, z) == CellKind::Empty
                && re_core::decode_pipe_tag(map.side_data_at(x, y, z)).is_some()
        })
        .count();
    assert_eq!(
        visible, 4,
        "the authored `/` run is the visible part of L1's pipe"
    );
    assert!(
        demoted_to_air > 0,
        "L1 still has path cells `[cells]` demoted back to air — if this hits 0 the \
         re_level ordering was fixed and this test's premise should be revisited"
    );
    // And the surviving traces are all ABOVE the ground walking layer, so they are
    // drawn by the off-slice pass and never block the player.
    for &(x, y, z) in path {
        if map.cell_at(x, y, z) == CellKind::WallTrace {
            assert!(z >= 2, "L1 trace ({x},{y},{z}) is in the walking layer");
        }
    }
}

/// The whole point of the tag: it is DISJOINT from the R1 colour-material id space, so
/// a pipe trace can never be mistaken for a coloured geometry cell (and vice versa).
#[test]
fn pipe_tags_stay_in_their_own_namespace() {
    for rel in CAMPAIGN {
        let level = load(rel);
        let map = level.build_map();
        for (x, y, z, _) in pipe_tagged_cells(&map) {
            let sd = map.side_data_at(x, y, z);
            assert!(
                sd >= re_core::PIPE_TAG_FLAG,
                "{rel}: ({x},{y},{z}) tag {sd:#x} is not in the pipe namespace"
            );
            let pipe = re_core::decode_pipe_tag(sd).expect("decodes");
            assert!(
                (pipe as usize) < level.pipes.len(),
                "{rel}: ({x},{y},{z}) decodes to pipe {pipe} but the level has {} pipes",
                level.pipes.len()
            );
        }
    }
}

// ───────────────────────────── `[emit]` parts ──────────────────────────────

/// Every campaign level authors an `[emit]` reward, and every clause binds to at least
/// one real, visible world cell.
#[test]
fn every_campaign_level_binds_its_emit_reward() {
    for rel in CAMPAIGN {
        let level = load(rel);
        assert!(
            !level.emits.is_empty(),
            "{rel}: a campaign level must author an [emit] reward"
        );
        let mut map = level.build_map();
        let bound = bind_emit_parts(&level, &mut map);
        assert_eq!(
            bound.clauses.len(),
            level.emits.len(),
            "{rel}: every [emit] clause must resolve to at least one visible cell"
        );
        assert_eq!(bound.lit_cell_count(), 0, "{rel}: a fresh bind is unlit");
    }
}

/// The RUNTIME rule, driven through a real campaign level: firing an `[emit]` source
/// node stamps its part into the world; un-firing removes it and restores the cells.
#[test]
fn firing_an_emit_source_stamps_its_part_and_unfiring_removes_it() {
    let rel = "levels/L1-first-pulse.lvl";
    let level = load(rel);
    let mut map = level.build_map();
    let mut parts = bind_emit_parts(&level, &mut map);
    let mut puzzle = PuzzleState::from_level(&level).expect("L1 network builds");

    // L1's single pipe is authored OFF, so `output` has not fired at load.
    let src = parts.clauses[0].source_node;
    assert_eq!(puzzle.nodes[src].activation, 0, "L1 starts unsolved");
    let cells = parts.clauses[0].cells.clone();
    assert!(!cells.is_empty(), "the red block resolves to real cells");
    let before: Vec<CellKind> = cells
        .iter()
        .map(|&(x, y, z, _)| map.cell_at(x as i32, y as i32, z as i32))
        .collect();
    assert!(
        before.iter().all(|&k| k == CellKind::Empty),
        "the block's cells are empty air before it fires"
    );

    // Nothing to do while the source is cold.
    assert!(!sync_emit_parts(&mut parts, &puzzle, &mut map));
    assert_eq!(parts.lit_cell_count(), 0);

    // FIRE.
    puzzle.nodes[src].activation = 1;
    assert!(
        sync_emit_parts(&mut parts, &puzzle, &mut map),
        "firing the source changes the world"
    );
    assert_eq!(
        parts.lit_cell_count(),
        cells.len(),
        "the whole mask materialised"
    );
    let color_id = parts.clauses[0].color_id;
    for &(x, y, z, _) in &cells {
        assert_eq!(
            map.cell_at(x as i32, y as i32, z as i32),
            CellKind::WallSolid,
            "({x},{y},{z}) materialised"
        );
        assert_eq!(
            map.side_data_at(x as i32, y as i32, z as i32),
            color_id,
            "({x},{y},{z}) shades from the part's own colour material"
        );
    }

    // Idle: no further world change (the map dirty-flag stays quiet).
    assert!(!sync_emit_parts(&mut parts, &puzzle, &mut map));

    // UN-FIRE.
    puzzle.nodes[src].activation = 0;
    assert!(
        sync_emit_parts(&mut parts, &puzzle, &mut map),
        "un-firing the source changes the world"
    );
    assert_eq!(parts.lit_cell_count(), 0);
    for (i, &(x, y, z, sd)) in cells.iter().enumerate() {
        assert_eq!(
            map.cell_at(x as i32, y as i32, z as i32),
            before[i],
            "({x},{y},{z}) restored"
        );
        assert_eq!(map.side_data_at(x as i32, y as i32, z as i32), sd);
    }
}

/// RESET: a teardown drops the bindings and a reload re-binds from a fresh map, so the
/// reward starts unlit again and no stale binding survives into the next level.
#[test]
fn a_reset_clears_the_stamped_parts() {
    let rel = "levels/L2-two-make-three.lvl";
    let level = load(rel);
    let mut map = level.build_map();
    let mut parts = bind_emit_parts(&level, &mut map);
    let mut puzzle = PuzzleState::from_level(&level).expect("L2 network builds");

    // Force every emit source on, so something is definitely stamped.
    for c in &parts.clauses {
        puzzle.nodes[c.source_node].activation = 1;
    }
    sync_emit_parts(&mut parts, &puzzle, &mut map);
    assert!(parts.lit_cell_count() > 0, "the reward materialised");

    // TEARDOWN (`flow::teardown_level`): the bindings are dropped.
    parts = EmitParts::default();
    assert!(parts.is_empty(), "teardown clears every binding");

    // RELOAD: a fresh map + a fresh bind is unlit and carries no stamped cell.
    let level2 = load(rel);
    let mut map2 = level2.build_map();
    let parts2 = bind_emit_parts(&level2, &mut map2);
    assert_eq!(parts2.lit_cell_count(), 0, "a reloaded level starts unlit");
    for c in &parts2.clauses {
        for &(x, y, z, _) in &c.cells {
            assert_eq!(
                map2.cell_at(x as i32, y as i32, z as i32),
                CellKind::Empty,
                "({x},{y},{z}) is empty air again after the reset"
            );
        }
    }
}

/// An `[emit]` part never covers a pipe trace, a node, a door, or a wall: the two
/// stamping passes coexist on the same map without either clobbering the other.
#[test]
fn emit_parts_never_claim_a_pipe_trace_or_any_other_geometry() {
    for rel in CAMPAIGN {
        let level = load(rel);
        let mut map = level.build_map();
        let bound = bind_emit_parts(&level, &mut map);
        for c in &bound.clauses {
            for &(x, y, z, sd) in &c.cells {
                assert_eq!(
                    map.cell_at(x as i32, y as i32, z as i32),
                    CellKind::Empty,
                    "{rel}: emit part claimed non-air at ({x},{y},{z})"
                );
                assert!(
                    re_core::decode_pipe_tag(sd).is_none(),
                    "{rel}: emit part claimed a pipe-tagged cell at ({x},{y},{z})"
                );
            }
        }
    }
}

/// SAFETY INVARIANT — a materialised `[emit]` reward must never wall the player in.
///
/// An emit part is stamped as a SOLID (`CellKind::WallSolid`) cell, and `WallSolid` is
/// collidable. The parts anchor on the `[pedestal]`, which sits in the same room the
/// player walks through, so a badly-placed part could in principle appear on solve and
/// cut the exit off — which would make the level unwinnable at exactly the moment it is
/// supposed to be won.
///
/// This pins that it does not happen on any shipped campaign level: within the DOOR's
/// own z-layer, the set of cells connected to the door must be the same before and after
/// every emit clause is lit (minus the cells the parts now occupy, which must themselves
/// not have been load-bearing). Layer-local and level-agnostic — no per-level fixture.
#[test]
fn a_lit_emit_reward_never_disconnects_the_door() {
    for rel in CAMPAIGN {
        let level = load(rel);
        let mut map = level.build_map();
        let mut parts = bind_emit_parts(&level, &mut map);
        let dz = level.door.z;

        let before = reachable_in_layer(&map, (level.door.x, level.door.y), dz);
        assert!(
            before.len() > 1,
            "{rel}: the door at ({},{},{dz}) is already isolated before anything lights",
            level.door.x,
            level.door.y
        );

        // Light EVERY clause (the worst case — all rewards materialised at once).
        let mut puzzle = PuzzleState::from_level(&level).expect("network builds");
        for c in &parts.clauses {
            if let Some(n) = puzzle.nodes.get_mut(c.source_node) {
                n.activation = 1;
            }
        }
        sync_emit_parts(&mut parts, &puzzle, &mut map);

        let after = reachable_in_layer(&map, (level.door.x, level.door.y), dz);
        let occupied: Vec<(i32, i32)> = parts
            .clauses
            .iter()
            .flat_map(|c| c.cells.iter())
            .filter(|&&(_, _, z, _)| z as i32 == dz)
            .map(|&(x, y, _, _)| (x as i32, y as i32))
            .collect();
        for cell in &before {
            if occupied.contains(cell) {
                continue; // the part stands here now — that is the point of it.
            }
            assert!(
                after.contains(cell),
                "{rel}: lighting the [emit] reward cut ({},{}) off from the door at                  ({},{}) in layer z={dz} — the level would become unwinnable on solve",
                cell.0,
                cell.1,
                level.door.x,
                level.door.y
            );
        }
    }
}

/// Every cell 4-connected to `start` within one z-layer that the player could stand in
/// (not collidable, in bounds). The door anchor itself is an `Object` cell (collidable),
/// so the seed is included unconditionally and the walk starts from its neighbours.
fn reachable_in_layer(map: &Map, start: (i32, i32), z: i32) -> Vec<(i32, i32)> {
    let (sx, sy, _) = map.size();
    let mut seen = vec![start];
    let mut stack = vec![start];
    while let Some((x, y)) = stack.pop() {
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let (nx, ny) = (x + dx, y + dy);
            if nx < 0 || ny < 0 || nx >= sx as i32 || ny >= sy as i32 {
                continue;
            }
            if seen.contains(&(nx, ny)) {
                continue;
            }
            if map.cell_at(nx, ny, z).is_collidable() {
                continue;
            }
            seen.push((nx, ny));
            stack.push((nx, ny));
        }
    }
    seen
}
