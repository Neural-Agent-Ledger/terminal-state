//! Mouse-driven menu navigation: hovering a menu item moves the highlight,
//! left-clicking it commits — exactly as if the player had pressed the arrow
//! keys then Enter. Keyboard/gamepad keep working unchanged and share the
//! SAME cursor ([`crate::flow::MenuCursor`]).
//!
//! ## Reuse, not a parallel state machine
//!
//! A mouse click writes the SAME [`MenuNav::Confirm`](re_present::MenuNav::Confirm)
//! a keyboard `Enter` would, into the SAME [`MenuNav`](re_present::MenuNav)
//! message stream every per-state `*_nav` system (`menu_nav`, `settings_nav`,
//! `pause_nav`, …) already reads. No transition logic is duplicated here —
//! this module only ever does two things: move
//! [`MenuCursor::index`](crate::flow::MenuCursor::index) to the item under the
//! pointer, and write `Confirm` when that item is clicked.
//!
//! ## Single-source geometry
//!
//! Every screen's clickable rectangles come from the SAME pure functions
//! `crate::menu` uses to PAINT the highlight (`menu_rects`, `won_rects`,
//! `pause_rects`, `category_rects`, `settings_rects`, `level_select_rects`) —
//! the discipline `ts_editor`'s `button_spans()` established: paint and
//! hit-test read identical numbers, so a layout tweak can never silently move
//! where a click lands out from under where the highlight is drawn.
//!
//! ## Why `re_present` can't gate this itself
//!
//! `re_present::MenuMouseMove`/`MenuMouseClick` are emitted whenever the OS
//! cursor grab is inactive (see that crate's `emit_menu_mouse_sys` doc) —
//! `re_present` cannot `run_if(in_state(GameState::Menu))` because the engine
//! must never depend on the game's state type (dependencies point one way:
//! game -> engine). So every system below is itself
//! `run_if(in_state(..))`-gated, exactly like the keyboard `*_nav` systems.

use bevy::ecs::message::{MessageReader, MessageWriter};
use bevy::prelude::*;

use re_core::Framebuffer;
use re_present::{MenuMouseClick, MenuMouseMove, MenuNav};

use crate::flow::{BrowseCategory, GameState, LevelCatalog, MenuCursor};
use crate::menu::{hit_test, ItemRect};
use crate::save::Progress;
use crate::settings::Settings;

/// Registers the six list-menu hover/click bridges.
///
/// Added from `MenuRenderPlugin` (which already owns the per-state overlay
/// painters this module shares geometry with) rather than wired separately
/// from the binary.
pub struct MenuMousePlugin;

impl Plugin for MenuMousePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, mouse_nav_menu.run_if(in_state(GameState::Menu)))
            .add_systems(
                Update,
                mouse_nav_category_select.run_if(in_state(GameState::CategorySelect)),
            )
            .add_systems(
                Update,
                mouse_nav_level_select.run_if(in_state(GameState::LevelSelect)),
            )
            .add_systems(
                Update,
                mouse_nav_settings.run_if(in_state(GameState::Settings)),
            )
            .add_systems(Update, mouse_nav_pause.run_if(in_state(GameState::Pause)))
            .add_systems(Update, mouse_nav_won.run_if(in_state(GameState::Won)));
    }
}

/// Pure resolution of one frame's mouse hover/click events against a screen's
/// item rects.
///
/// * `hover_hit` is the LAST hover position this frame that landed on an item
///   (so, within one frame, the most recent mouse position wins — matching
///   "the pointer is here now").
/// * `click_hit` is the item a `Down(Left)` landed on, if any (there is
///   normally at most one click per frame; if more than one arrived, the
///   last one wins, same rule as hover).
///
/// Both are pure `Option<usize>` indices into whatever list the caller's
/// rects describe — no ECS, no terminal — so this is exhaustively
/// unit-testable against hand-built rects and event sequences.
pub(crate) fn resolve_mouse_nav(
    rects: &[ItemRect],
    hovers: impl Iterator<Item = (u16, u16)>,
    clicks: impl Iterator<Item = (u16, u16)>,
) -> (Option<usize>, Option<usize>) {
    let mut hover_hit = None;
    for (c, r) in hovers {
        if let Some(i) = hit_test(rects, c as usize, r as usize) {
            hover_hit = Some(i);
        }
    }
    let mut click_hit = None;
    for (c, r) in clicks {
        if let Some(i) = hit_test(rects, c as usize, r as usize) {
            click_hit = Some(i);
        }
    }
    (hover_hit, click_hit)
}

/// Apply one frame's hover/click resolution to a live [`MenuCursor`] + write
/// [`MenuNav::Confirm`] on a hit click. Shared tail end of every system below.
///
/// A click's own hit index wins over a same-frame hover hit when they somehow
/// disagree (a click event always carries the position it actually landed on,
/// so it is the more authoritative of the two for what gets committed).
fn apply_hit(
    hover_hit: Option<usize>,
    click_hit: Option<usize>,
    cursor: &mut MenuCursor,
    nav: &mut MessageWriter<MenuNav>,
) {
    if let Some(i) = click_hit.or(hover_hit) {
        cursor.index = i;
    }
    if click_hit.is_some() {
        nav.write(MenuNav::Confirm);
    }
}

/// A batch of `(col, row)` cell positions drained from one frame's message
/// queue — named so [`read_events`]'s signature stays legible.
type CellPositions = Vec<(u16, u16)>;

/// Drain this frame's `MenuMouseMove`/`MenuMouseClick` into `(col, row)`
/// batches the pure [`resolve_mouse_nav`] can consume — the one place the
/// ECS message types meet the pure hit-test.
fn read_events(
    hover: &mut MessageReader<MenuMouseMove>,
    click: &mut MessageReader<MenuMouseClick>,
) -> (CellPositions, CellPositions) {
    (
        hover.read().map(|m| (m.col, m.row)).collect(),
        click.read().map(|m| (m.col, m.row)).collect(),
    )
}

fn mouse_nav_menu(
    mut hover: MessageReader<MenuMouseMove>,
    mut click: MessageReader<MenuMouseClick>,
    mut cursor: ResMut<MenuCursor>,
    mut nav: MessageWriter<MenuNav>,
    fb: Res<Framebuffer>,
    progress: Res<Progress>,
) {
    let rects = crate::menu::menu_rects(fb.cols(), fb.rows(), &progress);
    let (hovers, clicks) = read_events(&mut hover, &mut click);
    let (h, c) = resolve_mouse_nav(&rects, hovers.into_iter(), clicks.into_iter());
    apply_hit(h, c, &mut cursor, &mut nav);
}

fn mouse_nav_category_select(
    mut hover: MessageReader<MenuMouseMove>,
    mut click: MessageReader<MenuMouseClick>,
    mut cursor: ResMut<MenuCursor>,
    mut nav: MessageWriter<MenuNav>,
    fb: Res<Framebuffer>,
    catalog: Res<LevelCatalog>,
    progress: Res<Progress>,
) {
    let rects = crate::menu::category_rects(fb.cols(), &catalog, &progress);
    let (hovers, clicks) = read_events(&mut hover, &mut click);
    let (h, c) = resolve_mouse_nav(&rects, hovers.into_iter(), clicks.into_iter());
    apply_hit(h, c, &mut cursor, &mut nav);
}

fn mouse_nav_level_select(
    mut hover: MessageReader<MenuMouseMove>,
    mut click: MessageReader<MenuMouseClick>,
    mut cursor: ResMut<MenuCursor>,
    mut nav: MessageWriter<MenuNav>,
    fb: Res<Framebuffer>,
    catalog: Res<LevelCatalog>,
    browse: Res<BrowseCategory>,
    progress: Res<Progress>,
) {
    let indices = catalog.indices_in(browse.0);
    if indices.is_empty() {
        hover.clear();
        click.clear();
        return;
    }
    let sel = cursor.index.min(indices.len() - 1);
    let rows = crate::menu::level_select_rows(&catalog, browse.0, &progress, sel);
    let rects = crate::menu::level_select_rects(fb.cols(), &rows);
    let (hovers, clicks) = read_events(&mut hover, &mut click);
    let (h, c) = resolve_mouse_nav(&rects, hovers.into_iter(), clicks.into_iter());
    apply_hit(h, c, &mut cursor, &mut nav);
}

fn mouse_nav_settings(
    mut hover: MessageReader<MenuMouseMove>,
    mut click: MessageReader<MenuMouseClick>,
    mut cursor: ResMut<MenuCursor>,
    mut nav: MessageWriter<MenuNav>,
    fb: Res<Framebuffer>,
    settings: Res<Settings>,
) {
    // The window scrolls with the cursor (`crate::menu::settings_window`), so
    // hit-testing needs the same `(sel, total_rows)` the painter used this
    // frame — a scrolled-off row is never hoverable/clickable.
    let sel = cursor.index;
    let rects = crate::menu::settings_rects(fb.cols(), sel, fb.rows(), &settings);
    let (hovers, clicks) = read_events(&mut hover, &mut click);
    let (h, c) = resolve_mouse_nav(&rects, hovers.into_iter(), clicks.into_iter());
    apply_hit(h, c, &mut cursor, &mut nav);
}

fn mouse_nav_pause(
    mut hover: MessageReader<MenuMouseMove>,
    mut click: MessageReader<MenuMouseClick>,
    mut cursor: ResMut<MenuCursor>,
    mut nav: MessageWriter<MenuNav>,
    fb: Res<Framebuffer>,
) {
    let rects = crate::menu::pause_rects(fb.cols(), fb.rows());
    let (hovers, clicks) = read_events(&mut hover, &mut click);
    let (h, c) = resolve_mouse_nav(&rects, hovers.into_iter(), clicks.into_iter());
    apply_hit(h, c, &mut cursor, &mut nav);
}

fn mouse_nav_won(
    mut hover: MessageReader<MenuMouseMove>,
    mut click: MessageReader<MenuMouseClick>,
    mut cursor: ResMut<MenuCursor>,
    mut nav: MessageWriter<MenuNav>,
    fb: Res<Framebuffer>,
) {
    let rects = crate::menu::won_rects(fb.cols(), fb.rows());
    let (hovers, clicks) = read_events(&mut hover, &mut click);
    let (h, c) = resolve_mouse_nav(&rects, hovers.into_iter(), clicks.into_iter());
    apply_hit(h, c, &mut cursor, &mut nav);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::ItemRect;

    fn rects() -> Vec<ItemRect> {
        vec![
            ItemRect {
                index: 0,
                row: 4,
                col_start: 10,
                col_end: 20,
            },
            ItemRect {
                index: 1,
                row: 6,
                col_start: 10,
                col_end: 20,
            },
            ItemRect {
                index: 2,
                row: 8,
                col_start: 10,
                col_end: 20,
            },
        ]
    }

    #[test]
    fn hover_inside_a_rect_returns_its_index() {
        let (h, c) = resolve_mouse_nav(&rects(), std::iter::once((15, 6)), std::iter::empty());
        assert_eq!(h, Some(1));
        assert_eq!(c, None);
    }

    #[test]
    fn hover_outside_every_rect_returns_none() {
        let (h, _) = resolve_mouse_nav(&rects(), std::iter::once((15, 5)), std::iter::empty());
        assert_eq!(h, None, "row 5 is between items — no rect covers it");
    }

    #[test]
    fn click_inside_a_rect_returns_its_index() {
        let (_, c) = resolve_mouse_nav(&rects(), std::iter::empty(), std::iter::once((12, 8)));
        assert_eq!(c, Some(2));
    }

    #[test]
    fn column_boundaries_are_start_inclusive_end_exclusive() {
        let r = rects();
        // col_start=10 is inside; col_end=20 is NOT (one past the last painted cell).
        let (h1, _) = resolve_mouse_nav(&r, std::iter::once((10, 4)), std::iter::empty());
        assert_eq!(h1, Some(0));
        let (h2, _) = resolve_mouse_nav(&r, std::iter::once((20, 4)), std::iter::empty());
        assert_eq!(h2, None, "col_end is exclusive");
        let (h3, _) = resolve_mouse_nav(&r, std::iter::once((19, 4)), std::iter::empty());
        assert_eq!(h3, Some(0));
    }

    #[test]
    fn last_hover_this_frame_wins() {
        let (h, _) = resolve_mouse_nav(
            &rects(),
            vec![(15, 4), (15, 8)].into_iter(),
            std::iter::empty(),
        );
        assert_eq!(h, Some(2), "the most recent position in the frame wins");
    }

    #[test]
    fn a_miss_after_a_hit_this_frame_keeps_the_earlier_hit() {
        // The pointer moves off every item within the same frame — the caller
        // (`apply_hit`) must NOT reset the cursor to nothing; the LAST HIT
        // (not the last event) is what `resolve_mouse_nav` reports.
        let (h, _) = resolve_mouse_nav(
            &rects(),
            vec![(15, 4), (15, 5) /* miss */].into_iter(),
            std::iter::empty(),
        );
        assert_eq!(h, Some(0), "a later miss must not erase an earlier hit");
    }

    #[test]
    fn click_wins_over_a_disagreeing_hover_in_apply_hit() {
        let mut cursor = MenuCursor {
            index: 0,
            len: 3,
            pending_commit: false,
        };
        let mut app = App::new();
        app.add_message::<MenuNav>();
        let mut nav_writer_state: bevy::ecs::system::SystemState<MessageWriter<MenuNav>> =
            bevy::ecs::system::SystemState::new(app.world_mut());
        {
            let mut nav = nav_writer_state.get_mut(app.world_mut()).unwrap();
            apply_hit(Some(0), Some(2), &mut cursor, &mut nav);
        }
        assert_eq!(
            cursor.index, 2,
            "click index wins over a differing hover index"
        );
    }

    #[test]
    fn hover_only_moves_cursor_without_writing_confirm() {
        let mut cursor = MenuCursor {
            index: 0,
            len: 3,
            pending_commit: false,
        };
        let mut app = App::new();
        app.add_message::<MenuNav>();
        let mut nav_writer_state: bevy::ecs::system::SystemState<MessageWriter<MenuNav>> =
            bevy::ecs::system::SystemState::new(app.world_mut());
        {
            let mut nav = nav_writer_state.get_mut(app.world_mut()).unwrap();
            apply_hit(Some(1), None, &mut cursor, &mut nav);
        }
        assert_eq!(cursor.index, 1);
        // No MenuNav::Confirm should have been written.
        let messages = app.world().resource::<Messages<MenuNav>>();
        assert_eq!(
            messages.len(),
            0,
            "a hover with no click must not write Confirm"
        );
    }

    #[test]
    fn click_hit_writes_confirm() {
        let mut cursor = MenuCursor {
            index: 0,
            len: 3,
            pending_commit: false,
        };
        let mut app = App::new();
        app.add_message::<MenuNav>();
        let mut nav_writer_state: bevy::ecs::system::SystemState<MessageWriter<MenuNav>> =
            bevy::ecs::system::SystemState::new(app.world_mut());
        {
            let mut nav = nav_writer_state.get_mut(app.world_mut()).unwrap();
            apply_hit(None, Some(2), &mut cursor, &mut nav);
        }
        assert_eq!(cursor.index, 2);
        let messages = app.world().resource::<Messages<MenuNav>>();
        assert_eq!(
            messages.len(),
            1,
            "a click hit must write exactly one Confirm"
        );
    }
}
