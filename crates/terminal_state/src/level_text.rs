//! `level_text` — bind a loaded level's TEXT to the render layer, at load time.
//!
//! The generic PLACEMENT mechanism — resolving a note's authored `pos` +
//! `facing` to a real wall face, sliding along the wall run when the exact
//! cell is claimed or blocked, and stamping the synthesized `NoteWall`
//! material — lives in [`re_runtime::notes`] (`re_imgqa` calls the same
//! function this module does, rather than hand-mirroring those constants).
//!
//! What stays here is the game's own CONTENT decisions, passed across the
//! seam as plain data:
//!
//! 1. **Which panels exist.** The embedded code panels (`crate::code_panels`,
//!    the strange loop) are registered first, so a `CodeWall { panel: N }`
//!    index keeps exactly the meaning it always had.
//! 2. **What a note's body says.** Note bodies are templated
//!    ([`crate::story::interp`]) as they are registered, so a placard can
//!    address the operator by their real handle — this is the game's Form A-7
//!    diegetic layer, not part of the generic mechanism.
//!
//! `re_runtime::notes::build_level_text` does the placement and registry
//! assembly; this module only supplies `panels` and `transform`.

use re_level::Level;
use re_render::TextSurfaceRegistry;
use re_runtime::notes::NoteBindReport;
use re_world::Map;

/// Build and publish this level's text surfaces, and stamp its note placards
/// into `map`.
///
/// Call this at EVERY level-load site, after `level.build_map()` and BEFORE
/// `re_render::registry_for_map(&map)` — the note materials are appended to
/// the map's material list, so the render registry must be built after they
/// are there.
///
/// ## Form A-7 interpolation
///
/// Note bodies are templated ([`crate::story::interp`]) as they are
/// registered, so a placard can address the operator by their real handle.
/// The pass here uses the DEFAULT profile — `lore_fourth_wall = true`,
/// matching the shipped default — which is the right answer for any caller
/// with no live [`crate::settings::Settings`] (the `--dump` path). The play
/// loader interpolates the whole `Level` up-front from the player's real
/// setting, so by the time it calls this the bodies are already resolved and
/// this pass is a no-op on them (interpolation is idempotent: a substituted
/// body has no tokens left, and a line that would drop is already gone).
pub fn bind_level_text(level: &Level, map: &mut Map) -> NoteBindReport {
    let (registry, report) = build_level_text(level, map);
    re_render::scene_text::install(registry);
    report
}

/// The PURE half of [`bind_level_text`]: build this level's registry and
/// stamp its placards, WITHOUT publishing anything process-wide.
///
/// Split out so the per-load "a fresh registry carries only THIS level's
/// panels" property is testable without touching the process-global active
/// registry — which other tests in the same binary install into
/// concurrently. (The global install/replace/clear semantics are
/// `re_render::scene_text`'s own contract and are covered by its tests.)
pub fn build_level_text(level: &Level, map: &mut Map) -> (TextSurfaceRegistry, NoteBindReport) {
    // The Tier-1 reads are cheap but not free (two small file reads), so only
    // probe when a body actually carries a brace — which, after the play
    // loader's own pass, is never.
    let profile = level
        .notes
        .iter()
        .any(|n| n.text.contains('{'))
        .then(|| crate::story::interp::OperatorProfile::for_fourth_wall(true));
    let panels: Vec<(&str, &str)> = crate::code_panels::panel_sources().collect();
    match &profile {
        // The closure only needs to live for this one call — `profile` outlives
        // it, so there is nothing to leak or box.
        Some(p) => {
            let transform = |body: &str| crate::story::interp::interpolate(body, p);
            re_runtime::notes::build_level_text(level, map, &panels, Some(&transform))
        }
        None => re_runtime::notes::build_level_text(level, map, &panels, None),
    }
}

/// Publish ONLY the embedded code panels — no notes, no pools.
///
/// For the scene that has no parsed level behind it (the hardcoded test room,
/// or a startup before any level loads): a `CodeWall` material still shows
/// real source, and there is nothing else to bind.
pub fn bind_code_panels_only() {
    let mut registry = TextSurfaceRegistry::new();
    for (label, src) in crate::code_panels::panel_sources() {
        registry.push_code_source(label, src);
    }
    re_render::scene_text::install(registry);
}

/// Drop the active text surfaces — call at level teardown so a menu/idle
/// frame carries no stale level text.
pub fn clear_level_text() {
    re_render::scene_text::clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    use re_level::{Level, LevelNote, NoteKind};
    use re_world::{CellKind, Map};

    /// The active registry `re_render::scene_text` publishes is
    /// PROCESS-GLOBAL, and `cargo test` runs a crate's tests in parallel
    /// threads. Every test here that installs/reads the active registry
    /// takes this lock first so they cannot race each other.
    fn registry_test_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        LOCK.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn note(kind: NoteKind, x: i32, y: i32, z: i32, facing: i32, text: &str) -> LevelNote {
        LevelNote {
            kind,
            x,
            y,
            z,
            facing,
            text: text.to_string(),
        }
    }

    /// An 8×8×3 room: solid perimeter, air inside.
    fn room() -> Map {
        let mut m = Map::new(8, 8, 3);
        for x in 0..8 {
            for y in 0..8 {
                for z in 0..3 {
                    let perim = x == 0 || y == 0 || x == 7 || y == 7;
                    if perim {
                        m.set(x, y, z, CellKind::WallSolid);
                    } else if z == 0 {
                        m.set(x, y, z, CellKind::Floor);
                    }
                }
            }
        }
        m
    }

    /// The seam is wired correctly: the game's real code panels (not an empty
    /// list) reach `re_runtime::notes` through [`build_level_text`], so the
    /// registry's code-panel count matches `crate::code_panels::panel_sources`.
    #[test]
    fn build_level_text_registers_the_games_real_code_panels() {
        let mut map = room();
        let level = Level {
            notes: vec![note(NoteKind::Researcher, 3, 3, 1, 360, "first")],
            ..Default::default()
        };
        let (registry, report) = build_level_text(&level, &mut map);
        assert_eq!(report.placed, 1);
        assert_eq!(
            registry.code_len(),
            crate::code_panels::panel_sources().count(),
            "the game's own panel list must reach the generic builder"
        );
        assert_eq!(registry.note_len(), 1);
    }

    /// The publishing wrapper must be behaviourally identical to the pure
    /// builder: same stamping, same report. (That `install` then publishes it
    /// is `re_render::scene_text`'s contract; the live install path is
    /// exercised end-to-end by the flow tests, which load real levels through
    /// `enter_playing`.)
    #[test]
    fn the_publishing_wrapper_reports_exactly_what_the_builder_does() {
        let level = Level {
            notes: vec![
                note(NoteKind::Researcher, 3, 3, 1, 360, "first"),
                // Aims at the same cell as the first ⇒ slides one along the run.
                note(NoteKind::SystemLog, 5, 3, 1, 360, "second"),
            ],
            ..Default::default()
        };
        let mut built_map = room();
        let (_, built) = build_level_text(&level, &mut built_map);
        let mut bound_map = room();
        let bound = bind_level_text(&level, &mut bound_map);
        assert_eq!(
            bound, built,
            "the wrapper changes nothing about the binding"
        );
        assert_eq!(bound.placed, 2);
        assert!(bound.unplaced.is_empty());
        assert_eq!(
            bound_map.materials().len(),
            built_map.materials().len(),
            "both paths stamp the map identically"
        );
        clear_level_text();
    }

    /// A note body with an unresolved `{token}` runs through
    /// [`crate::story::interp`] before it reaches the registry — the game's
    /// own content decision, not the generic mechanism. With no live
    /// `Settings`, the default (`lore_fourth_wall = true`) profile resolves
    /// `{username}` to a real value or its A-7 §8 fallback, never leaving the
    /// literal brace in the published text.
    #[test]
    fn a_brace_carrying_note_body_is_interpolated_before_publishing() {
        let mut map = room();
        let level = Level {
            notes: vec![note(
                NoteKind::SystemLog,
                3,
                3,
                1,
                360,
                "operator session: {username}",
            )],
            ..Default::default()
        };
        let (registry, report) = build_level_text(&level, &mut map);
        assert_eq!(report.placed, 1);

        let _serial = registry_test_lock();
        let _scope = re_render::SceneTextScope::new(registry);
        let mut saw_brace = false;
        for steps in 0..=100u32 {
            let u = f32::from(steps as u16) / 100.0;
            let g = re_render::scene_text::sample_note_panel(0, u, 0.5, 0.0, 0.0);
            if char::from_u32(g.glyph.glyph) == Some('{') {
                saw_brace = true;
            }
        }
        assert!(
            !saw_brace,
            "an unresolved template brace must never reach the published panel"
        );
    }
}
