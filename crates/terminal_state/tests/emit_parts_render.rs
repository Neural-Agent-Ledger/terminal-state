//! `[emit] parts` RENDER test — the figure must reach the FRAMEBUFFER, not just the Map.
//!
//! ## Why this test exists in this shape
//!
//! `campaign_pipes_and_emits.rs` already asserts that every `[emit]` clause "resolves to
//! at least one visible cell" — where *visible* means the cell exists in the [`Map`].
//! That is precisely the assertion shape that let SIX successive content types ship
//! invisible: the cells were in the map every time, and the 2.5D raycaster simply never
//! visited them. A map-contents assertion cannot see that.
//!
//! So this test renders. For each of the seven campaign levels that author a two-part
//! reward (a base slab in the walking slice plus the FIGURE that carries the meaning one
//! plane above it — L2..L8), it:
//!
//! 1. loads the level, builds the map and binds the `[emit]` clauses exactly as the game
//!    does ([`re_runtime::emit_parts::bind_emit_parts`], before the render registry);
//! 2. renders the spawn view UNSOLVED and asserts the figure's colour-material appears
//!    on **zero** cells (the reward is a reward — it must not be there yet);
//! 3. solves the puzzle FOR REAL through the runtime seam
//!    (`re_puzzle::solve_brute_force` + `re_puzzle::puzzle_interact` at the authored
//!    switch cells — never by setting `solved`), syncs the emit clauses
//!    ([`re_runtime::emit_parts::sync_emit_parts`]), and renders the same view again;
//! 4. asserts the figure's colour-material now covers a real run of cells in the
//!    framebuffer.
//!
//! The FIGURE may be several clauses (L2's is a deck-plane `yellow_glyph` plus a
//! `yellow_crown` one plane up), but the render assertion is judged ONLY on the figure
//! clause(s) proven geometrically to sit entirely at `pedestal.cz + 1`, i.e. the
//! `[part] offset = 0 0 1` layer — so the test cannot be satisfied by the base slab, or
//! by deck-plane figure cells, alone. That distinction is the whole defect: the base is
//! in the eye slice and always rendered; the figure was one plane up and never did.
//!
//! ## …and the WHOLE figure, not part of one (2026-08-07)
//!
//! With the renderer honest, a CONTENT defect surfaced underneath it. `bind_emit_parts`
//! correctly refuses to stamp a reward cell onto a PIPE-TAGGED cell — those belong to
//! the wiring — but the refusal is silent, so a part whose footprint an author routed a
//! pipe through simply binds fewer cells and ships with holes. Four campaign levels did:
//!
//! | level | figure | authored | bound before | eaten by |
//! |-------|--------|---------:|-------------:|----------|
//! | L1 | `red_block`    | 8 | 5 | `[pipe R->output]` |
//! | L2 | `yellow_glyph` | 5 | 2 | `[pipe mix->output]` |
//! | L4 | `edge_glyph`   | 3 | 2 | `[pipe tapered->output]` |
//! | L6 | `named_block`  | 3 | 1 | `[pipe triangle->out_named]` + `->out_watched` |
//! | L7 | `flower_head`  | 5 | 2 | `[pipe growth->bloom]` |
//!
//! So "the figure is on screen" is not enough of an assertion — L7's flower was on
//! screen the whole time, as two side petals with no middle bar. Every clause on every
//! one of these levels is now pinned to bind its FULL authored mask, against a hand-kept
//! per-level cell count, and a partial bind FAILS. The static twin of this rule lives in
//! `re_lvlcheck`'s `emit_parts` check, which names the level, the part, the lost cells
//! and the pipe that ate them.
//!
//! Run with `--nocapture` to print the glyph grid around the pedestal.

use std::path::{Path, PathBuf};

use re_core::{Framebuffer, SurfaceKind};
use re_level::{load, Level};
use re_puzzle::{puzzle_interact, solve_brute_force, PuzzleState, Solvability};
use re_render::{composite::composite_dump_once, registry_for_map, Emitter, Fov};
use re_runtime::emit_parts::{bind_emit_parts, sync_emit_parts, EmitParts};
use re_world::{CameraPose, CellKind, Map};

/// Same grid the `--dump` CLI and the golden-framebuffer test use.
const COLS: usize = 100;
const ROWS: usize = 32;

/// One `[part]` of a reward, by name, with the EXACT number of solid mask cells it
/// authors — `(part name, cells)`.
type PinnedPart = (&'static str, usize);

/// A campaign pedestal reward, split into its two semantic layers.
///
/// * `figure` — the part(s) that carry the MEANING. At least one of them must sit
///   entirely at `pedestal.cz + 1` (the `[part] offset = 0 0 1` plane): that is the layer
///   the caster historically never visited, and the one the render pass must prove is on
///   screen. A figure may also have clauses on the deck plane — L2's yellow is a
///   `yellow_glyph` on the deck with a `yellow_crown` rising one cell above it.
/// * `base` — the slab part(s), every cell at the pedestal's declared `cz`.
/// * `other` — any further clause the level authors that is neither (L3's `teal_glyph`
///   two planes up, lit by a different hidden node). Pinned exactly like the rest, but not
///   part of the solved-state lit total, because the minimal solve need not light it.
///
/// Every `[emit]` clause the level authors must be named in exactly one of these lists,
/// so a new clause cannot slip in unpinned.
struct Reward {
    level: &'static str,
    figure: &'static [PinnedPart],
    base: &'static [PinnedPart],
    other: &'static [PinnedPart],
}

impl Reward {
    fn figure_cells(&self) -> usize {
        self.figure.iter().map(|&(_, n)| n).sum()
    }
    fn base_cells(&self) -> usize {
        self.base.iter().map(|&(_, n)| n).sum()
    }
}

/// The seven campaign levels that author a two-layer reward: a base slab at the
/// pedestal's declared z and a figure that reaches one plane above it, with the EXACT
/// number of solid mask cells each `[part]` authors.
///
/// These counts are the reward's authored SILHOUETTE, hand-read from each level's
/// `[part] mask`, and the test asserts both that the masks still author them (so this
/// table cannot drift from the level) and that the binder reproduces them exactly. Two
/// ways to lose a cell, both of which this pins:
///
/// * an author routes a `[pipe]` through the footprint — the cell is reserved by the
///   wiring and the figure quietly binds short (the 2026-08-07 defect);
/// * an author edits a mask and does not update this table — then the numbers disagree
///   here, deliberately, rather than the shape changing unnoticed.
///
/// L2 and L8 were deliberately reshaped in a later authoring pass: L2's
/// figure became a two-clause glyph + crown, L8's cat a five-cell profile
/// against the north shelf row.
const TWO_PART_LEVELS: [Reward; 7] = [
    Reward {
        level: "L2-two-make-three.lvl",
        // one column — a green-to-yellow stem `... / .#. / ...` on the deck
        // under a thin yellow crown rim `... / .#. / ...` one plane up.
        figure: &[("yellow_crown", 1)],
        base: &[("yellow_base", 1)],
        other: &[],
    },
    Reward {
        level: "L3-full-spectrum.lvl",
        // `#.# / .#. / #.#` plus `### / ... / ...`; teal `.#. / #.# / .#.` at cz+2.
        figure: &[("magenta_glyph", 5)],
        base: &[("magenta_base", 3)],
        other: &[("teal_glyph", 4)],
    },
    Reward {
        level: "L4-the-angle.lvl",
        // `..# / .#. / #..` plus `### / ... / ...`
        figure: &[("edge_glyph", 3)],
        base: &[("edge_base", 3)],
        other: &[],
    },
    Reward {
        level: "L5-soft-footstep.lvl",
        // three steps along the north row — lamp `#.. / ... / ...` one plane
        // up over the tall post `#..`, then `.#.` (mid) and `..#` (low) on the deck.
        figure: &[("scene_glyph", 1)],
        base: &[
            ("scene_base", 1),
            ("scene_step_mid", 1),
            ("scene_step_low", 1),
        ],
        other: &[],
    },
    Reward {
        level: "L6-polysemantic.lvl",
        // `#.# / ... / .#.` plus `#.# / .#. / #.#`
        figure: &[("named_block", 3)],
        base: &[("watched_block", 5)],
        other: &[],
    },
    Reward {
        level: "L7-hidden-depths.lvl",
        // petals `... / ### / ...` one plane up over the stem `... / .#. / ...`
        figure: &[("flower_head", 3)],
        base: &[("flower_stem", 1)],
        other: &[],
    },
    Reward {
        level: "L8-cold-room.lvl",
        // ear rim `#.. / ... / ...` one plane up over the head `#..`, and the
        // low body `.#.`, all in the pedestal's north (y=7) row.
        figure: &[("cat_glyph", 1)],
        base: &[("cat_head", 1), ("cat_base", 1)],
        other: &[],
    },
];

/// Every campaign level that authors a reward on a pedestal — the seven above plus L1,
/// whose single-clause `red_block` does not fit the two-layer figure/base split. Used by
/// the data-level pipe-collision guard, which cares about all of them equally.
const CAMPAIGN_L1_L8: [&str; 8] = [
    "L1-first-pulse.lvl",
    "L2-two-make-three.lvl",
    "L3-full-spectrum.lvl",
    "L4-the-angle.lvl",
    "L5-soft-footstep.lvl",
    "L6-polysemantic.lvl",
    "L7-hidden-depths.lvl",
    "L8-cold-room.lvl",
];

/// L1's reward file name, its `[part]`'s authored mask cell count, and how many of those
/// can ever bind.
///
/// L1 is the odd one out and deliberately not in [`TWO_PART_LEVELS`]: `red_block` is a
/// single `[emit]` clause holding one solid 2×2×2 that spans BOTH the pedestal deck
/// (z=5) and the plane above it (z=6), so the "figure one plane up, base slab on the
/// deck" split simply does not apply to it. It still needs an exact-count pin, so it
/// gets its own.
///
/// 8 authored, 7 bindable. The eighth is `L1_NODE_CELL` — the `output` node itself,
/// which the block is authored to WRAP. A part never replaces an `Object`, and that is
/// not the defect this suite hunts: see
/// [`no_campaign_emit_part_shares_a_cell_with_a_pipe_path`] for why a node cell and a
/// pipe cell are different situations.
const L1_LEVEL: &str = "L1-first-pulse.lvl";
const L1_AUTHORED_CELLS: usize = 8;
const L1_BINDABLE_CELLS: usize = 7;
const L1_NODE_CELL: (i32, i32, i32) = (13, 4, 6);

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
}

/// Solve `level`'s puzzle through the real runtime seam: the minimal switch-toggle set
/// from `re_puzzle::solve_brute_force`, applied with `re_puzzle::puzzle_interact` at each
/// switch's authored world cell. Never sets `solved` directly.
fn solved_puzzle(level: &Level, label: &str) -> PuzzleState {
    let mut state = PuzzleState::from_level(level)
        .unwrap_or_else(|e| panic!("{label}: puzzle graph must build: {e:?}"));

    let switchable: Vec<usize> = state
        .pipes
        .iter()
        .enumerate()
        .filter(|(_, p)| p.switch_cell.is_some())
        .map(|(i, _)| i)
        .collect();

    let winning_mask = match solve_brute_force(&mut state) {
        Solvability::Solvable { winning_mask, .. } => winning_mask,
        other => panic!("{label}: must be solvable by toggling switches; solver said {other:?}"),
    };

    for (bit, &pipe_idx) in switchable.iter().enumerate() {
        let want_on = (winning_mask >> bit) & 1 == 1;
        if want_on != state.pipes[pipe_idx].is_on() {
            let (x, y, z) = state.pipes[pipe_idx].switch_coord().unwrap_or_else(|| {
                panic!("{label}: switchable pipe {pipe_idx} has no switch cell")
            });
            assert!(
                puzzle_interact(&mut state, x, y, z),
                "{label}: toggle at ({x},{y},{z}) must take"
            );
        }
    }
    assert!(
        state.solved,
        "{label}: puzzle must be SOLVED after the minimal toggle set"
    );
    state
}

/// Render the spawn view and return the framebuffer.
fn render(map: &Map, cam: &CameraPose, level: &Level) -> Framebuffer {
    let registry = registry_for_map(map);
    let emitters: Vec<Emitter> = level.emitters.iter().map(Emitter::from_spec).collect();
    let mut fb = Framebuffer::new(COLS, ROWS);
    composite_dump_once(map, cam, &registry, &Fov::default(), &emitters, &mut fb);
    fb
}

/// How many framebuffer cells carry `color_id` through a wall surface — i.e. how much of
/// the emitted part the player can actually see.
fn part_cells_on_screen(fb: &Framebuffer, color_id: u16) -> usize {
    let mut n = 0;
    for row in 0..ROWS {
        for col in 0..COLS {
            let Some(cell) = fb.get(col, row) else {
                continue;
            };
            if cell.material == color_id
                && matches!(
                    cell.surface,
                    SurfaceKind::WallSolid | SurfaceKind::WallTrace | SurfaceKind::WallMatrix
                )
            {
                n += 1;
            }
        }
    }
    n
}

/// True when this cell was painted from the emit-part material `id`.
fn is_part_cell(fb: &Framebuffer, col: usize, row: usize, id: u16) -> bool {
    fb.get(col, row).is_some_and(|cell| {
        cell.material == id
            && matches!(
                cell.surface,
                SurfaceKind::WallSolid | SurfaceKind::WallTrace | SurfaceKind::WallMatrix
            )
    })
}

/// Print the rendered frame twice: the raw glyph grid, and a SILHOUETTE mask marking
/// which cells came from the figure layer (`F`) and which from the base slab (`b`), so a
/// human can see the shape instead of trusting a count.
fn print_grid(label: &str, fb: &Framebuffer, figure_ids: &[u16], base_ids: &[u16]) {
    let any_of =
        |col: usize, row: usize, ids: &[u16]| ids.iter().any(|&id| is_part_cell(fb, col, row, id));
    println!("---- {label} — GLYPH GRID ----");
    for row in 0..ROWS {
        let mut line = String::with_capacity(COLS);
        for col in 0..COLS {
            line.push(
                fb.get(col, row)
                    .and_then(|c| char::from_u32(c.glyph))
                    .unwrap_or(' '),
            );
        }
        let marked = (0..COLS).any(|c| any_of(c, row, figure_ids));
        println!("{}|{line}", if marked { '*' } else { ' ' });
    }
    println!("---- {label} — PART SILHOUETTE (F = figure layer, b = base slab) ----");
    for row in 0..ROWS {
        let mut line = String::with_capacity(COLS);
        for col in 0..COLS {
            line.push(if any_of(col, row, figure_ids) {
                'F'
            } else if any_of(col, row, base_ids) {
                'b'
            } else {
                '.'
            });
        }
        println!(" |{line}");
    }
}

/// A reward's clauses resolved against the bound [`EmitParts`]: indices into
/// [`EmitParts::clauses`] for the figure clauses, the subset of those that sit one plane
/// above the deck, and the base clauses.
struct RewardClauses {
    figure: Vec<usize>,
    figure_up: Vec<usize>,
    base: Vec<usize>,
}

/// How many SOLID cells a `[part]`'s mask authors. Iterated the way
/// `re_runtime::emit_parts::part_cells` iterates it (`w × h × d`, row-major), so a mask with
/// a stray trailing byte cannot inflate the count.
fn authored_mask_cells(level: &Level, part_idx: usize) -> usize {
    let Some(part) = level.parts.get(part_idx) else {
        return 0;
    };
    let (w, h, d) = (part.w.max(0), part.h.max(0), part.d.max(0));
    let mut n = 0;
    for pz in 0..d {
        for py in 0..h {
            for px in 0..w {
                let idx = px as usize + (w as usize) * (py as usize + (h as usize) * pz as usize);
                if part.mask.get(idx).copied().unwrap_or(0) != 0 {
                    n += 1;
                }
            }
        }
    }
    n
}

/// EVERY authored cell of EVERY clause must bind — the "no silent partial bind" rule —
/// and every clause must be one the hand-kept [`TWO_PART_LEVELS`] row names, at the
/// count it pins, on the plane its layer requires.
///
/// The binder drops a cell that is out of bounds, already solid, or PIPE-TAGGED, and says
/// nothing about it. This compares, clause by clause, what the `[part]` mask authors
/// against what the binding actually claimed. The table's counts are checked against the
/// masks as well, so the table cannot drift from the level unnoticed.
///
/// Layers are checked geometrically, not trusted from the table: every base clause sits
/// entirely at `pedestal.cz`; every figure clause sits entirely at `cz` or `cz + 1`; and
/// at least one figure clause sits entirely at `cz + 1` — so the figure can never be
/// satisfied by deck-plane content alone. That distinction is the whole defect: the deck
/// is in the eye slice and always rendered; the plane above it never was.
fn resolve_and_pin_every_clause(
    parts: &EmitParts,
    level: &Level,
    label: &str,
    reward: &Reward,
) -> RewardClauses {
    // The clause list is built in `[emit]` order, one clause per clause that bound at
    // least one cell — so a length match is what makes `clauses[i] ↔ emits[i]` sound.
    assert_eq!(
        parts.clauses.len(),
        level.emits.len(),
        "{label}: an [emit] clause bound NOTHING at all and was dropped"
    );
    let part_name_of = |i: usize| {
        level
            .parts
            .get(level.emits[i].part)
            .map_or("<?>", |p| p.name.as_str())
    };

    for (i, clause) in parts.clauses.iter().enumerate() {
        let part_idx = level.emits[i].part;
        let part_name = part_name_of(i);
        let authored = authored_mask_cells(level, part_idx);
        assert_eq!(
            clause.cells.len(),
            authored,
            "{label}: [part {part_name}] authors {authored} solid mask cell(s) but the \
             binding claimed only {}. The missing cells are refused by \
             `bind_emit_parts` — almost always because a [pipe] path runs through the \
             part's footprint and pipe-tagged cells are reserved for the wiring. Run \
             `cargo run -p re_lvlcheck --bin lvlcheck -- levels/{label}` for the exact \
             cells and the pipe that ate them, then MOVE THE PIPE.",
            clause.cells.len()
        );
    }

    // Every clause is named in the table exactly once, and every table entry is a clause.
    let pinned: Vec<(&str, usize, &str)> = reward
        .figure
        .iter()
        .map(|&(n, c)| (n, c, "FIGURE"))
        .chain(reward.base.iter().map(|&(n, c)| (n, c, "BASE")))
        .chain(reward.other.iter().map(|&(n, c)| (n, c, "OTHER")))
        .collect();
    for i in 0..parts.clauses.len() {
        let name = part_name_of(i);
        let hits = pinned.iter().filter(|(n, _, _)| *n == name).count();
        assert_eq!(
            hits, 1,
            "{label}: [emit] clause [part {name}] must be pinned exactly once in \
             TWO_PART_LEVELS (figure / base / other) — found {hits}"
        );
    }

    let base_z = level.pedestal.cz as usize;
    let figure_z = base_z + 1;
    let all_at = |i: usize, z: usize| parts.clauses[i].cells.iter().all(|&(_, _, cz, _)| cz == z);

    let resolve = |&(name, count, layer): &(&str, usize, &str)| -> usize {
        let i = (0..parts.clauses.len())
            .find(|&i| part_name_of(i) == name)
            .unwrap_or_else(|| {
                panic!(
                    "{label}: the {layer} [part {name}] is in TWO_PART_LEVELS but no \
                     [emit] clause binds it"
                )
            });
        let authored = authored_mask_cells(level, level.emits[i].part);
        assert_eq!(
            authored, count,
            "{label}: the {layer} [part {name}] mask authors {authored} solid cell(s) but \
             TWO_PART_LEVELS pins {count} — the mask changed; update the table (and its \
             comment) deliberately"
        );
        assert_eq!(
            parts.clauses[i].cells.len(),
            count,
            "{label}: the {layer} [part {name}] must bind exactly {count} cell(s)"
        );
        i
    };

    let figure: Vec<usize> = pinned
        .iter()
        .filter(|p| p.2 == "FIGURE")
        .map(resolve)
        .collect();
    let base: Vec<usize> = pinned
        .iter()
        .filter(|p| p.2 == "BASE")
        .map(resolve)
        .collect();
    for p in pinned.iter().filter(|p| p.2 == "OTHER") {
        resolve(p);
    }

    for &i in &base {
        assert!(
            all_at(i, base_z),
            "{label}: BASE [part {}] must sit entirely on the deck plane z={base_z}",
            part_name_of(i)
        );
    }
    for &i in &figure {
        assert!(
            all_at(i, base_z) || all_at(i, figure_z),
            "{label}: FIGURE [part {}] must sit entirely at z={base_z} or z={figure_z}",
            part_name_of(i)
        );
    }
    let figure_up: Vec<usize> = figure
        .iter()
        .copied()
        .filter(|&i| all_at(i, figure_z))
        .collect();
    assert!(
        !figure_up.is_empty(),
        "{label}: no FIGURE clause binds at the figure plane z={figure_z} (pedestal \
         cz={base_z}) — the level's two-layer reward has nothing one plane up"
    );

    RewardClauses {
        figure,
        figure_up,
        base,
    }
}

/// The SCRIPTED WALK: a starting eye pose, then one eye position per cell forward along
/// its facing, stopping in front of the first solid eye-slice cell (which is the
/// reward's own base slab). This is literally the player walking up to the pedestal.
///
/// The spawn view alone is not enough to judge by: every one of these levels spawns the
/// player at the far `-x` door with the puzzle's own nodes and pipe runs authored at
/// `z = 2` in between — L8's `output` node at `(16,8,2)` plus its pipe run down `x = 16`
/// is a solid screen of off-slice content across the whole forward FOV — so from spawn
/// the figure is honestly OCCLUDED by nearer content. Occlusion at range is the renderer
/// working; invisibility from three cells away is the renderer broken.
///
/// Takes the starting pose as a parameter rather than deriving it from `level` directly
/// — see [`reward_vantage`] for why.
fn approach_walk(start: CameraPose, eye_plane: i32, map: &Map) -> Vec<CameraPose> {
    let (dx, dy) = start.dir();
    let mut out = vec![start];
    let (mut px, mut py) = (start.pos.x, start.pos.y);
    for _ in 0..12 {
        let (nx, ny) = (px + dx, py + dy);
        if map
            .cell_at(nx.floor() as i32, ny.floor() as i32, eye_plane)
            .is_solid()
        {
            break;
        }
        px = nx;
        py = ny;
        let mut cam = start;
        cam.pos.x = px;
        cam.pos.y = py;
        out.push(cam);
    }
    out
}

/// The eye pose the walk starts from — `level.spawn_camera()` for every level EXCEPT
/// L3, and the reason for the exception is the caster's own single-slice model
/// (`re_render::gameplay_eye_ref_z` / `crates/re_render/src/offslice.rs`): the primary
/// ray walks exactly ONE horizontal grid slice, `floor(eye_z)`, and the off-slice scan
/// only ever looks a few planes ABOVE that slice, never below. Every OTHER two-part
/// campaign level spawns the player on the SAME storey as its `[pedestal]` (z=1), so
/// `level.spawn_camera()` already puts the walk on the right slice — that coincidence is
/// what `approach_walk`'s doc comment describes.
///
/// L3's 2026-09-23 redesign is the first
/// two-part level built across two storeys: `[player] spawn = 9 1 6` puts the player on
/// the MEZZANINE for a real narrative beat ("frame 1 looks down over the whole room...
/// before the player takes the stair down to wire the floor"), five z-planes above the
/// `[pedestal]` at z=1. A straight-line walk from there renders the mezzanine's own
/// slice for all 12 steps and can never see the ground-floor reward — not a level
/// defect (the mezzanine is real, reachable, and exactly as authored) but this shared
/// helper's baked-in assumption ("spawn's storey == the pedestal's storey") going false
/// for the first time. Re-pointing the WHOLE test at the mezzanine spawn would just make
/// it assert nothing, which is the failure mode `campaign_pipes_and_emits.rs` already
/// demonstrated once — so instead this returns the real, in-bounds, reachable
/// ground-floor pose any player is standing at once they DO walk down (one cell inside
/// the door at `(17,8,1)`, facing `-x` down the `y=8` row toward the pedestal — the same
/// "one cell inside the door, facing back into the room" convention every sibling
/// two-part level's own `[player] spawn` already uses, e.g. L2's `16 6 1 facing -x`
/// beside its own `12 6 1` pedestal). The authored `[player] spawn` on the mezzanine is
/// untouched; only this render-verification vantage differs from it.
///
/// L6's 2026-09-23 redesign is the same
/// single-storey shape as L2/L4/L5/L8 (spawn and `[pedestal]` both at z=1) but the
/// authored `[player] spawn = 10 19 1 facing -y` deliberately faces the north
/// `watching_lattice` decal across the room, not the pedestal at `(14,10,1)` — the
/// spawn's own comment says so ("the spawn faces it directly across the room"). A
/// straight walk north from there stays on the `x=10` column and never crosses the
/// pedestal's `x=13..15`. Same fix, same reasoning: the level's authored spawn is a
/// real, deliberate framing choice, not a defect, so it stays; this returns the door-
/// side pose instead (one cell inside the open east-wall notch at `(20,10,1)`, facing
/// `-x` back down the SAME `y=10` row the pedestal sits on).
///
/// L7's 2026-09-23 redesign is the same
/// shape again: `[player] spawn = 12 1 1 facing +y` is the approved "thesis" panorama
/// (gold godray shafts, the river, the phantom on the far bank), authored on the
/// `water`/`light`/`growth` centreline `x=12`, while the `[pedestal]` sits at
/// `x=15..17` per the growth→bloom pipe detour that keeps the flower's silhouette
/// clear of the wiring (`[pipe growth->bloom]`'s own header comment). A straight walk
/// south from `x=12` never reaches that column. Same fix: the door-side pose, one cell
/// inside the door at `(23,6,1)`, facing `-x` back down the pedestal's own `y=6` row.
fn reward_vantage(level: &Level, label: &str) -> CameraPose {
    match label {
        "L3-full-spectrum.lvl" => CameraPose::spawn_at(16, 8, 1, 360), // "-x"
        "L6-polysemantic.lvl" => CameraPose::spawn_at(19, 10, 1, 360), // "-x"
        "L7-hidden-depths.lvl" => CameraPose::spawn_at(22, 6, 1, 360), // "-x"
        _ => level.spawn_camera(),
    }
}

/// Sum of [`part_cells_on_screen`] over several clause colour ids.
fn clause_cells_on_screen(fb: &Framebuffer, parts: &EmitParts, clauses: &[usize]) -> usize {
    clauses
        .iter()
        .map(|&i| part_cells_on_screen(fb, parts.clauses[i].color_id))
        .sum()
}

/// One level, end to end: solved ⇒ the figure's up-plane layer is on screen somewhere
/// along the walk up to it; unsolved ⇒ no figure cell is on screen anywhere. `print`
/// dumps the clearest view's glyph grid for the levels the report quotes.
fn assert_figure_renders_when_solved(path: &Path, print: bool, reward: &Reward) {
    let label = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let level = load(path).unwrap_or_else(|e| panic!("load {}: {e}", path.display()));

    let mut map = level.build_map();
    // Bind BEFORE the render registry is derived — the same order the game uses.
    let mut parts = bind_emit_parts(&level, &mut map);
    assert!(
        !parts.is_empty(),
        "{label}: the level authors [emit] clauses; binding must produce at least one"
    );

    // ── 0. The WHOLE authored silhouette bound, not a subset of it, every clause pinned. ──
    let clauses = resolve_and_pin_every_clause(&parts, &level, &label, reward);
    let figure_ids: Vec<u16> = clauses
        .figure
        .iter()
        .map(|&i| parts.clauses[i].color_id)
        .collect();
    let base_ids: Vec<u16> = clauses
        .base
        .iter()
        .map(|&i| parts.clauses[i].color_id)
        .collect();
    let up_ids: Vec<u16> = clauses
        .figure_up
        .iter()
        .map(|&i| parts.clauses[i].color_id)
        .collect();

    // ── 1. Solve for real, then materialise the parts. ──
    let puzzle = solved_puzzle(&level, &label);
    assert!(
        sync_emit_parts(&mut parts, &puzzle, &mut map),
        "{label}: solving must materialise at least one part cell"
    );
    // …and STAMPED it: binding resolves the cells, lighting writes them. Every one of
    // them, or the reward on screen is not the reward that was authored.
    let (fig_n, base_n) = (reward.figure_cells(), reward.base_cells());
    assert_eq!(
        parts.lit_cell_count(),
        fig_n + base_n,
        "{label}: solving must materialise every authored reward cell \
         ({fig_n} figure + {base_n} base)"
    );
    for &i in clauses.figure.iter().chain(&clauses.base) {
        assert!(
            parts.clauses[i].is_lit(),
            "{label}: reward clause {i} must be lit when solved"
        );
    }

    // ── 2. SOLVED: walk up to the reward; the figure's UP-PLANE layer must appear. ──
    // Judged on the `cz + 1` clauses alone, never on deck-plane figure clauses, so a
    // figure that is only visible where it sits in the eye slice does not pass.
    let walk = approach_walk(reward_vantage(&level, &label), level.pedestal.cz, &map);
    let mut best: Option<(usize, usize, Framebuffer)> = None;
    let mut counts = Vec::with_capacity(walk.len());
    let mut whole = Vec::with_capacity(walk.len());
    for (i, cam) in walk.iter().enumerate() {
        let fb = render(&map, cam, &level);
        let n = clause_cells_on_screen(&fb, &parts, &clauses.figure_up);
        counts.push(n);
        whole.push(clause_cells_on_screen(&fb, &parts, &clauses.figure));
        if best.as_ref().is_none_or(|(_, bn, _)| n > *bn) {
            best = Some((i, n, fb));
        }
    }
    let (best_i, after, best_fb) = best.expect("the walk always has at least the spawn pose");
    if print {
        print_grid(
            &format!(
                "{label} — solved, eye at ({:.1},{:.1}) [step {best_i} of the walk]",
                walk[best_i].pos.x, walk[best_i].pos.y
            ),
            &best_fb,
            &figure_ids,
            &base_ids,
        );
    }
    println!(
        "[emit-render] {label}: up-plane figure cells per walk step {counts:?} (best \
         {after}); whole figure {whole:?}"
    );
    assert!(
        after >= 4,
        "{label}: the emitted FIGURE's up-plane layer (the [part] clause(s) one plane above \
         the pedestal deck, material id(s) {up_ids:?}) must be visible in the rendered frame \
         once the puzzle is solved — got {counts:?} cell(s) across the whole approach walk. \
         This is the defect the map-contents assertion in campaign_pipes_and_emits.rs cannot \
         see: the cells are in the Map, but the 2.5D caster never visited them."
    );

    // ── 3. UNSOLVED: at the very same eye, no part of the figure is there yet. ──
    let cold = PuzzleState::from_level(&level).expect("puzzle graph rebuilds");
    sync_emit_parts(&mut parts, &cold, &mut map);
    let fb_before = render(&map, &walk[best_i], &level);
    assert_eq!(
        clause_cells_on_screen(&fb_before, &parts, &clauses.figure),
        0,
        "{label}: at the very eye where the solved figure's up-plane covers {after} cells, \
         an UNSOLVED puzzle must show none of the figure"
    );
}

#[test]
fn every_two_part_campaign_reward_renders_its_figure_when_solved() {
    let root = workspace_root();
    for reward in &TWO_PART_LEVELS {
        assert_figure_renders_when_solved(&root.join("levels").join(reward.level), false, reward);
    }
}

/// The two levels the review quotes by eye: L7's flower must have a head, L8's cat must
/// be a cat. Prints both grids under `--nocapture`.
#[test]
fn l7_flower_and_l8_cat_render_with_their_figure_layer() {
    let root = workspace_root();
    for name in ["L7-hidden-depths.lvl", "L8-cold-room.lvl"] {
        let reward = TWO_PART_LEVELS
            .iter()
            .find(|r| r.level == name)
            .expect("both levels are in the two-part table");
        assert_figure_renders_when_solved(&root.join("levels").join(name), true, reward);
    }
}

/// The regression this test guards against, stated as one assertion an author cannot
/// misread: **no campaign `[emit]` part may share a cell with a `[pipe]` path.**
///
/// Independent of the render pass AND of the binder — it intersects the authored
/// `[part]` footprint with the authored `[pipe] path` lists straight out of the parsed
/// level, so it fires on the `.lvl` even if `bind_emit_parts` ever changes its refusal
/// rule. It is the same rule `re_lvlcheck`'s `emit_parts` check runs across every level
/// (there for the level author and the editor's save-time validation drawer; here so it
/// also gates `cargo test -p terminal_state`).
///
/// NODE CELLS ARE EXEMPT, and the exemption is the interesting part. A pipe path BEGINS
/// and ENDS on its two nodes' cells by construction, so any reward authored to wrap its
/// own source node — L1's `red_block` is a solid 2×2×2 built around `output` at
/// (13,4,6) — necessarily shares that one cell with the pipe. That cell is an `Object`,
/// it never carries a pipe tag, and a part could never claim it whatever the wiring did:
/// the reward is embracing the thing it celebrates, not fighting the wiring for a cell.
/// The defect this test exists for is the OTHER thing — a wire routed through the
/// figure's body — and conflating the two would train an author to ignore the finding.
#[test]
fn no_campaign_emit_part_shares_a_cell_with_a_pipe_path() {
    let root = workspace_root();
    for name in CAMPAIGN_L1_L8 {
        let path = root.join("levels").join(name);
        let level = load(&path).unwrap_or_else(|e| panic!("load {}: {e}", path.display()));
        let node_cells: Vec<(i32, i32, i32)> =
            level.nodes.iter().map(|n| (n.x, n.y, n.z)).collect();

        let mut collisions: Vec<String> = Vec::new();
        for emit in &level.emits {
            let part_name = level
                .parts
                .get(emit.part)
                .map_or("<?>", |p| p.name.as_str());
            for cell in authored_part_cells(&level, emit.part) {
                if node_cells.contains(&cell) {
                    continue; // see the doc comment: a node anchor is not wiring.
                }
                for pipe in &level.pipes {
                    if pipe.path.contains(&cell) {
                        collisions.push(format!(
                            "[part {part_name}] cell {cell:?} lies on [pipe {}->{}]",
                            pipe.from, pipe.to
                        ));
                    }
                }
            }
        }
        assert!(
            collisions.is_empty(),
            "{name}: {} reward cell(s) sit on a pipe path, so the [emit] figure will bind \
             short and ship with holes in its silhouette. A pipe's route between two fixed \
             endpoints is free; the reward's shape is authored intent — MOVE THE PIPE.\n  {}",
            collisions.len(),
            collisions.join("\n  ")
        );
    }
}

/// L1's `red_block`, pinned to its full bindable count and rendered in the alcove it
/// lives in.
///
/// Its pipe used to climb into the alcove at x=13 — `(13,3,5)` then `(13,4,5)` — putting
/// two path cells inside the pedestal box, so the block bound 5 of 8 and materialised as
/// a broken shell around the node. The tail now approaches `output` from `+x` along the
/// far wall and the block binds all 7 of its claimable cells.
///
/// L1 cannot use [`approach_walk`]: the player spawns on the GROUND floor and the reward
/// is on the second storey up a staircase, so the eye is placed directly in the upper
/// hall and walked east into the alcove instead. Run with `--nocapture` to see it.
#[test]
fn l1_red_block_binds_every_claimable_cell_and_renders_in_its_alcove() {
    let path = workspace_root().join("levels").join(L1_LEVEL);
    let level = load(&path).unwrap_or_else(|e| panic!("load {}: {e}", path.display()));
    let mut map = level.build_map();
    let mut parts = bind_emit_parts(&level, &mut map);

    assert_eq!(
        parts.clauses.len(),
        1,
        "L1 authors exactly one [emit] clause"
    );
    let part_idx = level.emits[0].part;
    assert_eq!(
        authored_mask_cells(&level, part_idx),
        L1_AUTHORED_CELLS,
        "L1's red_block is a solid 2x2x2"
    );

    let bound: Vec<(i32, i32, i32)> = parts.clauses[0]
        .cells
        .iter()
        .map(|&(x, y, z, _)| (x as i32, y as i32, z as i32))
        .collect();
    assert_eq!(
        bound.len(),
        L1_BINDABLE_CELLS,
        "L1: red_block must bind every cell it can claim — got {bound:?}. A short bind \
         means a [pipe] path is back inside the pedestal box; run \
         `cargo run -p re_lvlcheck --bin lvlcheck -- levels/{L1_LEVEL}` for the culprit."
    );

    // The ONE unclaimed cell, and the reason it is unclaimed: it is the `output` NODE,
    // not wiring. Asserted rather than assumed, so a future edit that moves the node (or
    // re-routes a pipe back through the block) cannot pass this off as the same thing.
    let missing: Vec<(i32, i32, i32)> = authored_part_cells(&level, part_idx)
        .into_iter()
        .filter(|c| !bound.contains(c))
        .collect();
    assert_eq!(
        missing,
        vec![L1_NODE_CELL],
        "exactly the node cell is unclaimed"
    );
    let (nx, ny, nz) = L1_NODE_CELL;
    assert_eq!(
        map.cell_at(nx, ny, nz),
        CellKind::Object,
        "({nx},{ny},{nz}) is an Object — the puzzle anchor the block wraps"
    );
    assert!(
        level
            .nodes
            .iter()
            .any(|n| (n.x, n.y, n.z) == L1_NODE_CELL && n.is_output),
        "and that Object is the `output` node itself, not a door or a stray"
    );

    // ── Render it. Eye in the upper hall at (9,4,5) looking +x through the doorway
    // gap into the alcove, then one step per cell until the alcove's far wall. ──
    let color_id = parts.clauses[0].color_id;
    let puzzle = solved_puzzle(&level, L1_LEVEL);
    assert!(
        sync_emit_parts(&mut parts, &puzzle, &mut map),
        "solving stamps the block"
    );
    assert_eq!(
        parts.lit_cell_count(),
        L1_BINDABLE_CELLS,
        "every claimable cell materialised"
    );

    let walk = alcove_walk(&map, 9, 4, 5, 120);
    let mut best: Option<(usize, usize, Framebuffer)> = None;
    let mut counts = Vec::with_capacity(walk.len());
    for (i, cam) in walk.iter().enumerate() {
        let fb = render(&map, cam, &level);
        let n = part_cells_on_screen(&fb, color_id);
        counts.push(n);
        if best.as_ref().is_none_or(|(_, bn, _)| n > *bn) {
            best = Some((i, n, fb));
        }
    }
    let (best_i, after, _best_fb) = best.expect("the walk always has its first pose");
    // Print the APPROACH view, not the best one: the walk ends with the eye pressed
    // against the block, where it fills the grid and shows nothing. Step 0 stands back
    // in the hall and is the frame a human can actually judge.
    print_grid(
        &format!(
            "{L1_LEVEL} — solved, eye at ({:.1},{:.1},{:.1}) [step 0 of {}]",
            walk[0].pos.x,
            walk[0].pos.y,
            walk[0].pos.z,
            walk.len()
        ),
        &render(&map, &walk[0], &level),
        &[color_id],
        &[],
    );
    println!("[emit-render] {L1_LEVEL}: block cells per walk step {counts:?} (best {after})");
    assert!(
        after >= 4,
        "{L1_LEVEL}: the materialised red_block must be visible from inside its own \
         alcove once the puzzle is solved — got {counts:?}"
    );

    // UNSOLVED, same eye: the alcove is empty. The reward is a reward.
    let cold = PuzzleState::from_level(&level).expect("puzzle graph rebuilds");
    sync_emit_parts(&mut parts, &cold, &mut map);
    assert_eq!(
        part_cells_on_screen(&render(&map, &walk[best_i], &level), color_id),
        0,
        "{L1_LEVEL}: an UNSOLVED puzzle shows none of the block"
    );
}

/// A short scripted walk for a level whose reward is not reachable from spawn: start at
/// the given cell + `facing` yaw column and step one cell at a time along the facing,
/// stopping in front of the first solid cell in the eye plane.
fn alcove_walk(map: &Map, x: i32, y: i32, z: i32, facing_yaw_col: i32) -> Vec<CameraPose> {
    let start = CameraPose::spawn_at(x, y, z, facing_yaw_col);
    let (dx, dy) = start.dir();
    let mut out = vec![start];
    let (mut px, mut py) = (start.pos.x, start.pos.y);
    for _ in 0..8 {
        let (nx, ny) = (px + dx, py + dy);
        if map
            .cell_at(nx.floor() as i32, ny.floor() as i32, z)
            .is_solid()
        {
            break;
        }
        px = nx;
        py = ny;
        let mut cam = start;
        cam.pos.x = px;
        cam.pos.y = py;
        out.push(cam);
    }
    out
}

/// Resolve a `[part]`'s solid mask into absolute world cells, anchored on the level's
/// `[pedestal]` — a hand-kept mirror of `re_runtime::emit_parts::part_cells` (private) and
/// of `re_lvlcheck`'s copy. X/Y take the pedestal bbox's MIN corner; Z takes the
/// pedestal's DECLARED `cz`.
fn authored_part_cells(level: &Level, part_idx: usize) -> Vec<(i32, i32, i32)> {
    let Some(part) = level.parts.get(part_idx) else {
        return Vec::new();
    };
    let (w, h, d) = (part.w.max(0), part.h.max(0), part.d.max(0));
    let ped = &level.pedestal;
    let (base_x, base_y, base_z) = (
        ped.cx - ped.w / 2 + part.off_x,
        ped.cy - ped.h / 2 + part.off_y,
        ped.cz + part.off_z,
    );
    let mut out = Vec::new();
    for pz in 0..d {
        for py in 0..h {
            for px in 0..w {
                let idx = px as usize + (w as usize) * (py as usize + (h as usize) * pz as usize);
                if part.mask.get(idx).copied().unwrap_or(0) != 0 {
                    out.push((base_x + px, base_y + py, base_z + pz));
                }
            }
        }
    }
    out
}

/// Pixel-free colour split: does this rgb read as the part's own red (`#FF4040`), or as
/// the near-white top tone the banded variant asks for?
const fn is_whitish(rgb: u32) -> bool {
    let (r, g) = ((rgb >> 16) & 0xFF, (rgb >> 8) & 0xFF);
    r > 0 && g * 2 > r
}

/// Load L1 with `extra` `[part]` keys spliced into `red_block`, bind + solve it, and
/// render the step-0 alcove approach. Returns the frame, the part's colour id, and the
/// emit material's registered name.
fn l1_with_part_keys(extra: &str) -> (Framebuffer, u16, String) {
    let path = workspace_root().join("levels").join(L1_LEVEL);
    let text = std::fs::read_to_string(&path).expect("read L1");
    // Splice `extra` right after the `[part red_block]` header LINE, whatever its line
    // ending is. `.lvl` files checked out under this repo's `core.autocrlf = true`
    // (the Windows default here) materialise with CRLF, while `re_level`'s own parser
    // (`str::lines()`) already treats CRLF and LF as equivalent — so this splice must
    // too, rather than hardcoding an LF-only anchor that stops matching the moment the
    // file is checked out on Windows (the same class of defect that once hit
    // `re_level`'s own `[room]` replace-line test).
    let header = "[part red_block]";
    let header_at = text.find(header).expect("L1's red_block header moved");
    let after_header = header_at + header.len();
    let eol_len = if text[after_header..].starts_with("\r\n") {
        2
    } else if text[after_header..].starts_with('\n') {
        1
    } else {
        panic!("L1's red_block header line has no recognised line ending");
    };
    let splice_at = after_header + eol_len;
    let mut text = text;
    text.insert_str(splice_at, extra);
    let level = re_level::load_string(&text, L1_LEVEL).expect("patched L1 parses");
    let mut map = level.build_map();
    let mut parts = bind_emit_parts(&level, &mut map);
    let color_id = parts.clauses[0].color_id;
    let name = map
        .materials()
        .iter()
        .find(|m| {
            m.name
                .starts_with(re_runtime::emit_parts::EMIT_MATERIAL_PREFIX)
        })
        .map(|m| m.name.clone())
        .expect("the emit material is registered");
    let puzzle = solved_puzzle(&level, L1_LEVEL);
    assert!(
        sync_emit_parts(&mut parts, &puzzle, &mut map),
        "solving stamps the block"
    );
    let walk = alcove_walk(&map, 9, 4, 5, 120);
    (render(&map, &walk[0], &level), color_id, name)
}

/// `[part] height = half` + `top = #RRGGBB`: the figure is still a figure (it renders
/// when solved), it paints only where the full-height figure would paint (never above
/// it — the band is a SUBSET of the full silhouette, per column), it paints strictly
/// fewer cells, and both colours are on screen. The default (no keys) keeps the exact
/// pre-band material name, so the golden frames of every existing level are untouched.
#[test]
fn a_half_height_two_tone_part_renders_inside_the_full_silhouette() {
    let (full, id_full, name_full) = l1_with_part_keys("");
    assert!(
        !name_full.contains("~z"),
        "a part with no height/top keys keeps its pre-band material name: {name_full:?}"
    );
    let (band, id_band, name_band) =
        l1_with_part_keys("height       = half\ntop          = #FFFFFF\ntop_from     = 0.5\n");
    let parsed = re_render::zband::parse(&name_band)
        .unwrap_or_else(|| panic!("banded name parses: {name_band:?}"));
    assert!(
        (parsed.hi - 0.5).abs() < 1e-6 && parsed.lo == 0.0,
        "{parsed:?}"
    );
    assert_eq!(parsed.top, Some((0xFF_FFFF, 0.5)));

    print_grid(
        "L1 — red_block height=half top=#FFFFFF",
        &band,
        &[id_band],
        &[],
    );
    let n_full = part_cells_on_screen(&full, id_full);
    let n_band = part_cells_on_screen(&band, id_band);
    assert!(
        n_band >= 4,
        "the banded figure must still render when solved: {n_band}"
    );
    assert!(
        n_band < n_full,
        "a half-height band paints fewer cells: {n_band} vs {n_full}"
    );

    let mut outside = Vec::new();
    let (mut tone, mut base) = (0, 0);
    for row in 0..ROWS {
        for col in 0..COLS {
            if !is_part_cell(&band, col, row, id_band) {
                continue;
            }
            if !is_part_cell(&full, col, row, id_full) {
                outside.push((col, row));
            }
            let fg = band.get(col, row).map_or(0, |c| c.fg);
            if is_whitish(fg) {
                tone += 1;
            } else {
                base += 1;
            }
        }
    }
    assert!(
        outside.is_empty(),
        "the band painted outside the full figure's silhouette at {outside:?}"
    );
    assert!(
        tone > 0 && base > 0,
        "both tones on screen: top {tone}, base {base}"
    );
}

/// The z-band name suffix is written in two crates: `re_render::zband::suffix` (used
/// here by `emit_parts`) and `re_level::props::zband_suffix` (the prop kit's solid
/// furniture — `re_level` cannot depend on the renderer). They must agree byte for
/// byte, or a banded prop would silently render as a full-height block.
#[test]
fn the_zband_suffix_mirror_in_re_level_matches_the_renderer() {
    for (lo, hi, top) in [
        (0.0, 0.5, None),
        (0.0, 0.2, Some((0xCD_E4FF, 0.7))),
        (0.0, 0.42, Some((0x00_0001, 0.0))),
        (0.1, 1.0, Some((0xFF_FFFF, 1.0))),
        (0.0, 0.333, None),
    ] {
        let a = re_render::zband::suffix(lo, hi, top);
        assert_eq!(
            a,
            re_level::props::zband_suffix(lo, hi, top),
            "({lo},{hi},{top:?})"
        );
        assert!(
            re_render::zband::parse(&format!("m{a}")).is_some(),
            "{a} parses"
        );
    }
    let m = re_level::props::solid_furniture_material(0x6C_7888, "table");
    let band = re_render::zband::parse(&m.name).expect("a solid furniture material is a band");
    assert!((band.hi - re_level::props::solid_furniture_band("table")).abs() < 0.006);
    assert!(band.top.is_some(), "with its bright top");
}
