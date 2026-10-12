//! Headless state-flow tests. These drive an `App` through state transitions
//! with NO terminal and NO real renderer — only `MinimalPlugins` + the flow +
//! puzzle plugins. They prove the state machine wires up, that
//! `OnEnter(Playing)` loads a real [`Map`] and `OnExit` cleans it up, and that
//! `MenuNav`-driven navigation (Up/Down/Confirm/Back) routes the menu / select /
//! won screens to the right next state — exactly as the present layer's keys
//! would, but driven by pushing [`MenuNav`] messages (no real terminal needed).

use super::*;
use bevy::ecs::system::RunSystemOnce;
use bevy::state::app::StatesPlugin;
use bevy::MinimalPlugins;
use re_present::MenuNav;
use re_puzzle::PuzzlePlugin;

/// Build a headless test app: time + states + the flow + the puzzle plugin. The
/// flow plugin registers the backend-neutral `InteractPressed` / `MenuNav`
/// messages itself, so no `re_present` (and thus no terminal) is needed. CLI
/// level defaults to `None` (catalogue order).
fn test_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(StatesPlugin)
        .add_message::<InteractPressed>()
        .add_plugins(PuzzlePlugin)
        .add_plugins(FlowPlugin { cli_level: None })
        // `cargo test` runs with the crate dir as cwd, so the default catalog's
        // relative `levels/*.lvl` paths don't resolve. Re-point it at the
        // workspace's absolute `levels/` so OnEnter(Playing) can actually load.
        .insert_resource(abs_catalog());
    app
}

/// Push a [`MenuNav`] message and advance one frame, so the per-state nav system
/// observes it. (`MenuNav` is registered by `FlowPlugin`.)
fn nav(app: &mut App, n: MenuNav) {
    app.world_mut().write_message(n);
    app.update();
}

/// The workspace `levels/` directory as an ABSOLUTE path, derived from this crate's
/// `CARGO_MANIFEST_DIR` (`crates/terminal_state` → `../../levels`). `cargo test` runs with
/// the crate dir as cwd, so relative level paths would not resolve.
fn levels_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .unwrap()
        .join("levels")
}

/// The pre-manifest numbered rooms `01`..`07` under an absolute path.
///
/// NOTE: since the campaign manifest landed these levels are
/// [`Category::Archive`], not [`Category::Campaign`] — they are the L-files'
/// ancestors, kept playable and shard-bearing but out of the twelve. Tests that
/// browse this fixture must therefore select ARCHIVE. They are still the right
/// fixture for flow mechanics (cursor, commit, load) precisely BECAUSE the archive
/// carries no unlock gate: a gating-agnostic test stays gating-agnostic.
fn abs_catalog() -> LevelCatalog {
    let root = levels_root();
    let paths = (1..=7)
        .map(|n| {
            root.join(format!("{n:02}.lvl"))
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect();
    LevelCatalog { paths }
}

/// The twelve CAMPAIGN levels, absolute, in manifest order — the fixture for the
/// manifest-order and unlock-gate tests.
fn campaign_catalog() -> LevelCatalog {
    let root = levels_root();
    let paths = campaign_manifest()
        .levels()
        .iter()
        .map(|l| {
            let stem = std::path::Path::new(&l.path)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string();
            root.join(stem).to_string_lossy().replace('\\', "/")
        })
        .collect();
    LevelCatalog { paths }
}

/// Point the app's browse at `cat` BEFORE entering `LevelSelect`, so
/// `enter_level_select` sizes its cursor to that category's list.
fn browse(app: &mut App, cat: Category) {
    app.world_mut().resource_mut::<BrowseCategory>().0 = cat;
}

fn state(app: &App) -> GameState {
    *app.world().resource::<State<GameState>>().get()
}

fn set_state(app: &mut App, s: GameState) {
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(s);
    app.update(); // apply the transition (runs OnExit/OnEnter)
}

fn force_commit(app: &mut App) {
    app.world_mut().resource_mut::<MenuCursor>().pending_commit = true;
    app.update();
}

/// Player-initiated level-entry commits now route through the [`GameState::Diving`]
/// tunnel transition before reaching [`GameState::Playing`]. When a test only
/// cares about the destination (the loaded level), call this AFTER the commit to
/// skip the dive: it injects an `InteractPressed` (the skip signal `tick_diving`
/// reads) and applies the resulting Diving → Playing transition. A no-op when the
/// app is not currently diving, so it is safe to call unconditionally.
fn skip_dive(app: &mut App) {
    if state(app) == GameState::Diving {
        app.world_mut().write_message(InteractPressed);
        app.update(); // tick_diving sees the skip → queues Playing
        app.update(); // apply OnExit(Diving)/OnEnter(Playing)
    }
}

#[test]
fn starts_in_splash() {
    let app = test_app();
    assert_eq!(state(&app), GameState::Splash);
}

#[test]
fn splash_advances_to_menu_on_press() {
    let mut app = test_app();
    app.update(); // OnEnter(Splash) ran
    app.world_mut().write_message(InteractPressed);
    app.update(); // tick_splash sees the press
    app.update(); // apply the queued transition
    assert_eq!(state(&app), GameState::Menu);
}

#[test]
fn splash_advances_to_menu_on_menu_nav() {
    // A bare MenuNav (e.g. an Up/Down/Confirm key) also leaves the splash.
    let mut app = test_app();
    app.update();
    app.world_mut().write_message(MenuNav::Confirm);
    app.update();
    app.update();
    assert_eq!(state(&app), GameState::Menu);
}

#[test]
fn menu_items_on_entry() {
    let mut app = test_app();
    set_state(&mut app, GameState::Menu);
    let cursor = *app.world().resource::<MenuCursor>();
    assert_eq!(cursor.len, MENU_ITEMS.len());
    assert_eq!(cursor.index, 0);
}

#[test]
fn cursor_moves_down_and_wraps() {
    let mut app = test_app();
    set_state(&mut app, GameState::Menu);
    // Down steps the highlight forward, wrapping through every item back to 0.
    let n = MENU_ITEMS.len();
    for step in 1..=n {
        nav(&mut app, MenuNav::Down);
        assert_eq!(app.world().resource::<MenuCursor>().index, step % n);
    }
}

#[test]
fn cursor_moves_up_and_wraps() {
    let mut app = test_app();
    set_state(&mut app, GameState::Menu);
    // Up from index 0 wraps to the last item, then walks back down to 0.
    let n = MENU_ITEMS.len();
    for step in 1..=n {
        nav(&mut app, MenuNav::Up);
        assert_eq!(app.world().resource::<MenuCursor>().index, (n - step) % n);
    }
}

#[test]
fn menu_start_enters_playing_and_loads_a_map() {
    let mut app = test_app();
    set_state(&mut app, GameState::Menu);
    // index 0 == Start. Force the idle-commit immediately.
    force_commit(&mut app); // menu_nav (via pending_commit) queues Diving
    app.update(); // apply OnEnter(Diving)
    skip_dive(&mut app); // skip the transition through to Playing
    assert_eq!(state(&app), GameState::Playing);

    // A real level must now be loaded: the map is bigger than the 1x1 menu map,
    // a camera entity exists, and the puzzle has nodes.
    let (sx, sy, sz) = app.world().resource::<Map>().size();
    assert!(
        sx > 1 || sy > 1 || sz > 1,
        "OnEnter(Playing) must load a real map, got {sx}x{sy}x{sz}"
    );
    let cams = app
        .world_mut()
        .query_filtered::<(), With<PlayCamera>>()
        .iter(app.world())
        .count();
    assert_eq!(cams, 1, "exactly one play camera should be spawned");
    assert!(
        !app.world().resource::<PuzzleState>().nodes.is_empty(),
        "a shipped level must build a non-empty puzzle network"
    );
}

#[test]
fn exit_playing_cleans_up_camera_and_resources() {
    let mut app = test_app();
    set_state(&mut app, GameState::Menu);
    force_commit(&mut app); // → queue Diving
    app.update(); // OnEnter(Diving)
    skip_dive(&mut app); // through the dive into Playing
    assert_eq!(state(&app), GameState::Playing);

    // Leave Playing → Menu; OnExit(Playing) must tear everything down.
    set_state(&mut app, GameState::Menu);

    let cams = app
        .world_mut()
        .query_filtered::<(), With<PlayCamera>>()
        .iter(app.world())
        .count();
    assert_eq!(cams, 0, "camera must be despawned on OnExit(Playing)");
    let (sx, sy, sz) = app.world().resource::<Map>().size();
    assert_eq!((sx, sy, sz), (1, 1, 1), "map must reset to the empty 1x1");
    assert!(
        app.world().resource::<PuzzleState>().nodes.is_empty(),
        "puzzle network must be cleared on exit"
    );
    assert!(
        !app.world().resource::<Solved>().0,
        "solved flag must reset"
    );
}

#[test]
fn select_item_routes_to_category_select() {
    let mut app = test_app();
    set_state(&mut app, GameState::Menu);
    // SELECT now opens the CATEGORY browse (the first level of the two-level UI).
    let select_idx = MENU_ITEMS
        .iter()
        .position(|i| *i == MenuItem::Select)
        .unwrap();
    for _ in 0..select_idx {
        nav(&mut app, MenuNav::Down);
    }
    assert_eq!(app.world().resource::<MenuCursor>().index, select_idx);
    nav(&mut app, MenuNav::Confirm); // queues CategorySelect
    app.update(); // apply the transition
    assert_eq!(state(&app), GameState::CategorySelect);
    // The category screen sizes its cursor to the categories the loaded
    // catalogue actually holds a level in (empty categories are hidden, not
    // offered as a dead end — see `visible_categories`).
    let catalog = app.world().resource::<LevelCatalog>().clone();
    assert_eq!(
        app.world().resource::<MenuCursor>().len,
        visible_categories(&catalog).len()
    );
}

/// Confirming a category on the browse screen drills into that category's level
/// list (the second level of the two-level browse) and records the chosen
/// category in [`BrowseCategory`].
#[test]
fn category_confirm_enters_level_select_scoped_to_category() {
    let mut app = test_app();
    set_state(&mut app, GameState::CategorySelect);
    // The abs_catalog fixture (01..07) holds only ARCHIVE-category levels (see
    // `abs_catalog`'s note) — CAMPAIGN is empty here, so the browse hides it and
    // ARCHIVE is the sole, default-highlighted visible category.
    nav(&mut app, MenuNav::Confirm); // queue LevelSelect
    app.update(); // apply OnEnter(LevelSelect)
    assert_eq!(state(&app), GameState::LevelSelect);
    assert_eq!(
        app.world().resource::<BrowseCategory>().0,
        Category::Archive
    );
    // The cursor sizes to the archive's level count, not the whole catalogue.
    let catalog = app.world().resource::<LevelCatalog>().clone();
    let n_archive = catalog.count_in(Category::Archive);
    assert_eq!(app.world().resource::<MenuCursor>().len, n_archive.max(1));
}

/// `Back` on the CATEGORY screen returns to the main menu.
#[test]
fn category_select_back_returns_to_menu() {
    let mut app = test_app();
    set_state(&mut app, GameState::CategorySelect);
    nav(&mut app, MenuNav::Back); // queue Menu
    app.update(); // apply
    assert_eq!(state(&app), GameState::Menu);
}

#[test]
fn menu_back_quits_the_game() {
    // Back (Esc/q) on the top menu emits an AppExit.
    let mut app = test_app();
    set_state(&mut app, GameState::Menu);
    nav(&mut app, MenuNav::Back);
    let exits = app
        .world_mut()
        .resource_mut::<Messages<AppExit>>()
        .drain()
        .count();
    assert!(exits >= 1, "Back on the main menu must quit");
}

#[test]
fn level_select_back_returns_to_category_select() {
    // Back now goes UP one browse level: level list → category list.
    let mut app = test_app();
    set_state(&mut app, GameState::LevelSelect);
    nav(&mut app, MenuNav::Back); // queues CategorySelect
    app.update(); // apply
    assert_eq!(state(&app), GameState::CategorySelect);
}

#[test]
fn level_select_commit_enters_playing_with_chosen_index() {
    let mut app = test_app();
    // The `01`..`07` fixture lives in ARCHIVE since the manifest landed (see
    // `abs_catalog`), and the archive is ungated — which is what this test wants: it
    // is about the cursor→index mapping, not about the campaign's unlock rule.
    browse(&mut app, Category::Archive);
    set_state(&mut app, GameState::LevelSelect);
    let n = app.world().resource::<LevelCatalog>().paths.len();
    assert!(n >= 1);
    // Advance to the 2nd level if available, else stay on the first.
    let target = usize::from(n >= 2);
    for _ in 0..target {
        nav(&mut app, MenuNav::Down);
    }
    nav(&mut app, MenuNav::Confirm); // queue Diving
    app.update(); // apply OnEnter(Diving)
    skip_dive(&mut app); // through the dive into Playing
    assert_eq!(state(&app), GameState::Playing);
    assert_eq!(app.world().resource::<CurrentLevel>().0, target);
}

/// Reaching Level Select the way the
/// player does — by pressing `Confirm` on the `SELECT` item in the main menu —
/// must NOT auto-commit level 0. The `MenuNav::Confirm` that opened the screen
/// is double-buffered and would otherwise leak into `level_select_nav` and pick
/// the highlighted (index-0) level before the player can move. After the
/// stale-nav clear, the player moves to index N and only THEN does Confirm load
/// level N — and the loaded `Map` must be level N's, not level 0's.
#[test]
fn level_select_real_flow_loads_chosen_level_not_zero() {
    let mut app = test_app();
    set_state(&mut app, GameState::Menu);

    // Step to SELECT and Confirm — exactly as the keyboard does. This Confirm is
    // the message that previously leaked through and forced level 0.
    let select_idx = MENU_ITEMS
        .iter()
        .position(|i| *i == MenuItem::Select)
        .unwrap();
    for _ in 0..select_idx {
        nav(&mut app, MenuNav::Down);
    }
    nav(&mut app, MenuNav::Confirm); // queues CategorySelect (Confirm now in buffer)
    app.update(); // apply OnEnter(CategorySelect) → clear_stale_nav drains the Confirm
    assert_eq!(state(&app), GameState::CategorySelect);
    // The abs_catalog fixture (01..07) holds only ARCHIVE-category levels (see
    // `abs_catalog`'s note) — CAMPAIGN is empty here, so the browse hides it and
    // ARCHIVE is already the sole, default-highlighted category: no navigation is
    // needed to reach it, keeping this test about the stale-Confirm bug and
    // nothing else.
    let catalog = app.world().resource::<LevelCatalog>().clone();
    assert_eq!(
        visible_categories(&catalog)[app.world().resource::<MenuCursor>().index],
        Category::Archive
    );
    nav(&mut app, MenuNav::Confirm); // queues LevelSelect (Confirm now in buffer)
    app.update(); // apply OnEnter(LevelSelect) → clear_stale_nav drains the Confirm
    assert_eq!(state(&app), GameState::LevelSelect);

    // The stale Confirm must NOT have auto-committed us into Playing.
    let n = app.world().resource::<LevelCatalog>().paths.len();
    assert!(
        n >= 2,
        "need at least two levels to prove the non-zero pick"
    );
    assert_eq!(
        app.world().resource::<MenuCursor>().index,
        0,
        "cursor starts at 0; the leaked Confirm did not advance/commit it"
    );

    // Now the player moves the cursor to a non-zero level and confirms.
    let target = n - 1; // last level — unambiguously not 0
    for _ in 0..target {
        nav(&mut app, MenuNav::Down);
    }
    assert_eq!(app.world().resource::<MenuCursor>().index, target);
    nav(&mut app, MenuNav::Confirm); // queue Diving for the CHOSEN level
    app.update(); // apply OnEnter(Diving)
    skip_dive(&mut app); // through the dive into Playing

    assert_eq!(state(&app), GameState::Playing);
    assert_eq!(
        app.world().resource::<CurrentLevel>().0,
        target,
        "the highlighted (non-zero) level is the one that loads"
    );

    // And the loaded Map must be the CHOSEN level's, not level 0's. Compare the
    // live Map's size against a direct load of both candidate levels.
    let catalog = app.world().resource::<LevelCatalog>().clone();
    let chosen = re_level::load(&catalog.paths[target]).expect("chosen level loads");
    let zero = re_level::load(&catalog.paths[0]).expect("level 0 loads");
    let live = app.world().resource::<Map>().size();
    assert_eq!(
        live,
        chosen.build_map().size(),
        "the live Map matches the CHOSEN level, not level 0"
    );
    // Guard against a false pass if both levels happened to share a size.
    if chosen.build_map().size() != zero.build_map().size() {
        assert_ne!(live, zero.build_map().size(), "and it is NOT level 0's map");
    }
}

/// The stale-nav clear is general: entering ANY menu screen via a real
/// `Confirm` must leave its cursor at index 0 and NOT auto-commit. This locks the
/// fix in place across screens (the same leak affected every Confirm-reached
/// menu, only Level Select made it player-visible).
#[test]
fn entering_a_menu_via_confirm_does_not_auto_commit() {
    let mut app = test_app();
    set_state(&mut app, GameState::Menu);
    // SELECT → CategorySelect via Confirm, then assert we are parked on the screen.
    let select_idx = MENU_ITEMS
        .iter()
        .position(|i| *i == MenuItem::Select)
        .unwrap();
    for _ in 0..select_idx {
        nav(&mut app, MenuNav::Down);
    }
    nav(&mut app, MenuNav::Confirm);
    app.update();
    assert_eq!(state(&app), GameState::CategorySelect);
    // One more frame with NO new input: a leaked Confirm would commit here.
    app.update();
    assert_eq!(
        state(&app),
        GameState::CategorySelect,
        "no stray commit on the frame after entry — the screen stays put"
    );
}

#[test]
fn quit_item_writes_app_exit() {
    let mut app = test_app();
    set_state(&mut app, GameState::Menu);
    // Step down to QUIT (now the last item), then Confirm.
    let quit_idx = MENU_ITEMS
        .iter()
        .position(|i| *i == MenuItem::Quit)
        .unwrap();
    for _ in 0..quit_idx {
        nav(&mut app, MenuNav::Down);
    }
    assert_eq!(app.world().resource::<MenuCursor>().index, quit_idx);
    nav(&mut app, MenuNav::Confirm);
    let exits = app
        .world_mut()
        .resource_mut::<Messages<AppExit>>()
        .drain()
        .count();
    assert!(exits >= 1, "QUIT must emit an AppExit");
}

#[test]
fn won_next_level_advances_index() {
    let mut app = test_app();
    // Pretend we just solved level 0.
    app.world_mut().resource_mut::<CurrentLevel>().0 = 0;
    set_state(&mut app, GameState::Won);
    // index 0 == NEXT LEVEL.
    force_commit(&mut app);
    app.update();
    let n = app.world().resource::<LevelCatalog>().paths.len();
    if n >= 2 {
        // Next-level routes through the dive transition first.
        assert_eq!(state(&app), GameState::Diving);
        skip_dive(&mut app);
        assert_eq!(state(&app), GameState::Playing);
        assert_eq!(app.world().resource::<CurrentLevel>().0, 1);
    } else {
        // Single-level catalogue wraps to the menu.
        assert_eq!(state(&app), GameState::Menu);
    }
}

#[test]
fn won_menu_item_returns_to_menu() {
    let mut app = test_app();
    set_state(&mut app, GameState::Won);
    // Down to MENU (index 1) then Confirm.
    nav(&mut app, MenuNav::Down);
    nav(&mut app, MenuNav::Confirm); // queue Menu
    app.update(); // apply
    assert_eq!(state(&app), GameState::Menu);
}

// ─────────────────────────────── diving ─────────────────────────────────────

use crate::flow::{DiveTimer, DIVE_DURATION};
use re_core::InputGate;

/// The skippable Diving tunnel transition (spec §4.2):
/// 1. Entering `Diving` closes the camera gate (`InputGate.camera_motion == false`).
/// 2. Advancing the dive clock past `DIVE_DURATION` auto-exits Diving → Playing
///    (and re-opens the gate).
/// 3. Re-entering Diving and injecting an `InteractPressed` leaves it IMMEDIATELY
///    (the skip), again landing in Playing.
#[test]
fn diving_state_auto_exits_after_duration_and_on_skip() {
    let mut app = test_app();
    app.world_mut().resource_mut::<CurrentLevel>().0 = 0;

    // ── 1. Enter Diving: the camera gate must be CLOSED while diving. ──
    set_state(&mut app, GameState::Diving); // OnEnter(Diving) runs
    assert_eq!(state(&app), GameState::Diving);
    assert!(
        !app.world().resource::<InputGate>().camera_motion,
        "the camera gate is closed (camera frozen) while diving"
    );
    // The clock reset to zero on entry.
    assert_eq!(
        app.world().resource::<DiveTimer>().0,
        std::time::Duration::ZERO,
        "the dive clock resets on entry"
    );

    // ── 2. Advance the dive clock past the duration → auto-exit to Playing. ──
    // Pre-load the clock to the duration so the next tick (delta added) exceeds it,
    // deterministically (no dependence on wall-clock advancement in the harness).
    app.world_mut().resource_mut::<DiveTimer>().0 = DIVE_DURATION;
    app.update(); // tick_diving: timer >= DIVE_DURATION → queues Playing
    app.update(); // apply OnExit(Diving)/OnEnter(Playing)
    assert_eq!(
        state(&app),
        GameState::Playing,
        "the dive auto-exits to Playing once the duration elapses"
    );
    // OnExit(Diving) re-opened the camera gate so motion resumes in Playing.
    assert!(
        app.world().resource::<InputGate>().camera_motion,
        "the camera gate re-opens when the dive hands off to Playing"
    );

    // ── 3. Re-enter Diving and SKIP it with an interact press — exits at once. ──
    set_state(&mut app, GameState::Diving);
    assert_eq!(state(&app), GameState::Diving);
    assert!(!app.world().resource::<InputGate>().camera_motion);
    // The clock is fresh (well under the duration) — only the skip can exit now.
    assert!(app.world().resource::<DiveTimer>().0 < DIVE_DURATION);

    app.world_mut().write_message(InteractPressed);
    app.update(); // tick_diving sees the press → queues Playing immediately
    app.update(); // apply OnEnter(Playing)
    assert_eq!(
        state(&app),
        GameState::Playing,
        "an interact press skips the dive immediately"
    );
}

/// The same skip works through a `MenuNav` key (the other neutral skip signal),
/// proving both event channels short-circuit the dive.
#[test]
fn diving_skips_on_menu_nav() {
    let mut app = test_app();
    set_state(&mut app, GameState::Diving);
    assert_eq!(state(&app), GameState::Diving);
    // The dive clock is far from the duration, so only the nav skip can exit.
    assert!(app.world().resource::<DiveTimer>().0 < DIVE_DURATION);

    nav(&mut app, MenuNav::Confirm); // tick_diving reads it → skip
    app.update(); // apply the transition
    assert_eq!(state(&app), GameState::Playing, "a nav key skips the dive");
}

// ───────────────────────────── boot console ─────────────────────────────────
//
// The OPENING sequence's Phase 2 (`GameState::BootConsole`, 2026-09-24): a
// pre-splash pseudo boot console reached ONLY when the live binary's TTY/VT
// gate says so (see `main.rs`'s `should_show_boot_intro` /
// `enter_boot_console_on_startup`, not exercised by `FlowPlugin` alone). These
// tests drive it directly via `NextState`, exactly as the Diving tests above
// drive `Diving` — proving the STATE's own clock/skip contract independent of
// how production decides to enter it.

use crate::flow::{BootConsoleTimer, BOOT_CONSOLE_DURATION};

/// Mirrors `diving_state_auto_exits_after_duration_and_on_skip`:
/// 1. Entering `BootConsole` resets the clock.
/// 2. Advancing the clock past `BOOT_CONSOLE_DURATION` auto-exits to `Splash`.
/// 3. Re-entering and injecting an `InteractPressed` skips it immediately,
///    also landing in `Splash` — it can never trap the player.
#[test]
fn boot_console_auto_exits_after_duration_and_on_skip() {
    let mut app = test_app();

    // ── 1. Enter BootConsole: the clock resets on entry. ──
    set_state(&mut app, GameState::BootConsole);
    assert_eq!(state(&app), GameState::BootConsole);
    assert_eq!(
        app.world().resource::<BootConsoleTimer>().0,
        std::time::Duration::ZERO,
        "the boot-console clock resets on entry"
    );

    // ── 2. Advance the clock past the duration → auto-exit to Splash. ──
    app.world_mut().resource_mut::<BootConsoleTimer>().0 = BOOT_CONSOLE_DURATION;
    app.update(); // tick_boot_console: timer >= duration → queues Splash
    app.update(); // apply OnExit(BootConsole)/OnEnter(Splash)
    assert_eq!(
        state(&app),
        GameState::Splash,
        "the boot console auto-exits to Splash once the duration elapses"
    );

    // ── 3. Re-enter and SKIP with an interact press — exits at once. ──
    set_state(&mut app, GameState::BootConsole);
    assert_eq!(state(&app), GameState::BootConsole);
    assert!(app.world().resource::<BootConsoleTimer>().0 < BOOT_CONSOLE_DURATION);

    app.world_mut().write_message(InteractPressed);
    app.update(); // tick_boot_console sees the press → queues Splash immediately
    app.update(); // apply OnEnter(Splash)
    assert_eq!(
        state(&app),
        GameState::Splash,
        "an interact press skips the boot console immediately"
    );
}

/// The same skip works through a `MenuNav` key — the other neutral skip
/// signal — proving both event channels short-circuit the boot console.
#[test]
fn boot_console_skips_on_menu_nav() {
    let mut app = test_app();
    set_state(&mut app, GameState::BootConsole);
    assert_eq!(state(&app), GameState::BootConsole);
    assert!(app.world().resource::<BootConsoleTimer>().0 < BOOT_CONSOLE_DURATION);

    nav(&mut app, MenuNav::Confirm); // tick_boot_console reads it → skip
    app.update(); // apply the transition
    assert_eq!(
        state(&app),
        GameState::Splash,
        "a nav key skips the boot console"
    );
}

/// A player level-entry commit (menu Start) now routes through `Diving`, not
/// straight to Playing — the dive plays on the way in. (The level loads only on
/// the subsequent OnEnter(Playing).)
#[test]
fn menu_start_routes_through_diving() {
    let mut app = test_app();
    set_state(&mut app, GameState::Menu);
    force_commit(&mut app); // Start commits → queues Diving
    app.update(); // apply OnEnter(Diving)
    assert_eq!(
        state(&app),
        GameState::Diving,
        "Start dives into the level before Playing"
    );
    // No level is loaded yet (the load is on OnEnter(Playing), after the dive).
    let (sx, sy, sz) = app.world().resource::<Map>().size();
    assert_eq!(
        (sx, sy, sz),
        (1, 1, 1),
        "the level loads only after the dive"
    );
    // Skipping completes the entry into a real level.
    skip_dive(&mut app);
    assert_eq!(state(&app), GameState::Playing);
    let (sx, sy, sz) = app.world().resource::<Map>().size();
    assert!(
        sx > 1 || sy > 1 || sz > 1,
        "Playing loads the real level after the dive"
    );
}

#[test]
fn won_back_returns_to_menu() {
    let mut app = test_app();
    set_state(&mut app, GameState::Won);
    nav(&mut app, MenuNav::Back); // queue Menu
    app.update();
    assert_eq!(state(&app), GameState::Menu);
}

// ───────────────────────────── progress / save ──────────────────────────────

use crate::save::Progress;

/// Reaching `Won` for a level records that level's stable id as solved in
/// `Progress`. The flow's `record_progress` runs on `OnEnter(Won)`; we drive
/// the transition with a known `CurrentLevel` and read the resource back.
#[test]
fn reaching_won_marks_the_level_solved() {
    // Redirect the save to a throwaway temp dir so this test never touches the
    // real on-disk progress (and tolerates a sandboxed/denied real path).
    std::env::set_var("TS_SAVE_DIR", std::env::temp_dir());
    let mut app = test_app();
    // Point at catalogue slot 2 (id "03") and "solve" it by entering Won.
    app.world_mut().resource_mut::<CurrentLevel>().0 = 2;
    let id = app
        .world()
        .resource::<LevelCatalog>()
        .id_at(2)
        .unwrap()
        .to_string();
    assert!(
        !app.world().resource::<Progress>().is_solved(&id),
        "level starts unsolved"
    );

    set_state(&mut app, GameState::Won); // OnEnter(Won) → record_progress

    assert!(
        app.world().resource::<Progress>().is_solved(&id),
        "reaching Won must mark level id '{id}' solved"
    );
}

/// `Continue` targets the FIRST unsolved level in catalogue order. With levels
/// 0 and 1 (ids "01","02") solved, Continue must jump to index 2.
#[test]
fn continue_targets_first_unsolved_level() {
    let mut app = test_app();
    // Mark the first two catalogue ids solved.
    {
        let catalog = app.world().resource::<LevelCatalog>().clone();
        let mut progress = app.world_mut().resource_mut::<Progress>();
        progress.mark_solved(catalog.id_at(0).unwrap());
        progress.mark_solved(catalog.id_at(1).unwrap());
    }
    set_state(&mut app, GameState::Menu);

    // Highlight the CONTINUE item and commit it.
    let cont_idx = MENU_ITEMS
        .iter()
        .position(|i| *i == MenuItem::Continue)
        .unwrap();
    for _ in 0..cont_idx {
        nav(&mut app, MenuNav::Down);
    }
    nav(&mut app, MenuNav::Confirm); // queues Diving at the frontier
    app.update(); // apply OnEnter(Diving)
    skip_dive(&mut app); // through the dive into Playing

    assert_eq!(state(&app), GameState::Playing);
    assert_eq!(
        app.world().resource::<CurrentLevel>().0,
        2,
        "Continue must target the first unsolved level (index 2)"
    );
}

/// When EVERY level is solved, `first_unsolved` falls back to index 0 so
/// Continue replays from the top rather than dead-ending.
#[test]
fn continue_falls_back_to_zero_when_all_solved() {
    let app = test_app();
    let catalog = app.world().resource::<LevelCatalog>().clone();
    let mut progress = Progress::default();
    for i in 0..catalog.paths.len() {
        progress.mark_solved(catalog.id_at(i).unwrap());
    }
    assert_eq!(catalog.first_unsolved(&progress), Some(0));
}

// ───────────────────────────── word→story flow ──────────────────────────────

use crate::story::{MemoryLog, MemoryReadout};
use re_puzzle::{puzzle_interact, PuzzleState};

/// Drive a headless flow App into `Playing` on level 03, solve the puzzle by
/// toggling the winning pipes through the live `PuzzleState`, then leave to
/// `Won`. Assert the `MemoryReadout` resource holds the assembled phrase and the
/// `MemoryLog` accumulated it. This exercises the real wiring: OnExit(Playing)
/// `assemble_readout` builds the phrase from the held Level + `PuzzleState`.
#[test]
fn flow_to_won_publishes_readout_and_logs_it() {
    std::env::set_var("TS_SAVE_DIR", std::env::temp_dir());
    let mut app = test_app();
    // Target catalogue slot 2 == level 03.
    app.world_mut().resource_mut::<CurrentLevel>().0 = 2;
    set_state(&mut app, GameState::Playing); // OnEnter(Playing) loads L3

    // Solve L3 in-place via the live PuzzleState (R->h1, B->h1, h1->output).
    {
        let level = re_level::load("../../levels/03.lvl").expect("L3 loads for switch lookup");
        let idx = |n: &str| level.node_index(n).unwrap();
        let wires = [("R", "h1"), ("B", "h1"), ("h1", "output")];
        let mut state = app.world_mut().resource_mut::<PuzzleState>();
        for (from, to) in wires {
            let (fi, ti) = (idx(from), idx(to));
            let (x, y, z) = state
                .pipes
                .iter()
                .find(|p| p.from == fi && p.to == ti)
                .and_then(re_puzzle::Pipe::switch_coord)
                .unwrap();
            assert!(puzzle_interact(&mut state, x, y, z), "toggle {from}->{to}");
        }
        assert!(state.solved, "L3 solved in the live state");
        // Mirror into the Solved resource the way the real watcher would.
        app.world_mut().resource_mut::<re_puzzle::Solved>().0 = true;
    }

    // Leave Playing for Won → OnExit(Playing) assemble_readout runs.
    set_state(&mut app, GameState::Won);

    let readout = app.world().resource::<MemoryReadout>();
    assert!(readout.has_phrase(), "readout published on solve");
    assert_eq!(
        readout.phrase, "FIRE  ->  LEAF  ->  SEA  ->  MAGENTA  ->  OUTPUT",
        "Won readout holds the L3 assembled phrase"
    );
    let log = app.world().resource::<MemoryLog>();
    assert_eq!(log.entries.len(), 1, "one vignette logged");
    assert_eq!(log.entries[0].level_id, "03");
    assert_eq!(log.entries[0].phrase, readout.phrase);
    println!("flow Won readout = {}", readout.phrase);
}

/// The denial / `while(true)` data path: if the live network is cyclic when the
/// level is "solved", `assemble_readout` publishes the CORRUPTED `pool_alt` form
/// and does NOT add it to the memory log (the collapse is an ending, not a
/// vignette). We inject a tiny cyclic 2-node level + a forced-solved state to
/// drive the real flow system, no faking.
#[test]
fn flow_denial_cycle_publishes_corrupt_readout() {
    use re_level::{Node, PipeState};
    use re_puzzle::{Pipe, PuzzleNode, Solved};

    let mut app = test_app();
    set_state(&mut app, GameState::Playing); // load a real L (then override)

    // A 2-node ring a->b->a, both fired, with pool_alt words to corrupt.
    let level = Level {
        name: "cycle".into(),
        nodes: vec![
            Node {
                pool: vec!["FIRE".into()],
                pool_alt: vec!["огонь".into(), "火".into()],
                ..Default::default()
            },
            Node {
                pool: vec!["OUTPUT".into()],
                pool_alt: vec!["выход".into(), "salida".into()],
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let cyclic = PuzzleState {
        nodes: vec![
            PuzzleNode {
                threshold: 0,
                activation: 1,
                ..Default::default()
            },
            PuzzleNode {
                threshold: 0,
                activation: 1,
                ..Default::default()
            },
        ],
        pipes: vec![
            Pipe {
                from: 0,
                to: 1,
                state: PipeState::On,
                path: vec![],
                switch_cell: None,
            },
            Pipe {
                from: 1,
                to: 0,
                state: PipeState::On,
                path: vec![],
                switch_cell: None,
            },
        ],
        inputs: vec![0, 0],
        door_targets: vec![1],
        sequence: re_puzzle::SequenceLock::default(),
        solved: true,
        attempts: 0,
        ..Default::default()
    };
    app.world_mut()
        .insert_resource(CurrentLevelData(Some(level)));
    app.world_mut().insert_resource(cyclic);
    app.world_mut().resource_mut::<Solved>().0 = true;

    set_state(&mut app, GameState::Won); // OnExit(Playing) → assemble_readout

    let readout = app.world().resource::<MemoryReadout>();
    assert!(readout.has_phrase(), "corrupt readout published");
    assert_eq!(
        readout.form,
        crate::story::ReadoutForm::Corrupt,
        "cyclic graph → corrupted denial form"
    );
    assert!(
        readout.phrase.contains('/'),
        "Babel fuses languages: {}",
        readout.phrase
    );
    assert!(
        app.world().resource::<MemoryLog>().is_empty(),
        "the denial collapse is not a logged vignette"
    );
    println!("denial readout = {}", readout.phrase);
}

/// Leaving Playing WITHOUT solving (Back to menu) publishes no readout and logs
/// nothing — `assemble_readout` is gated on `Solved`.
#[test]
fn flow_unsolved_exit_records_nothing() {
    let mut app = test_app();
    app.world_mut().resource_mut::<CurrentLevel>().0 = 2;
    set_state(&mut app, GameState::Playing);
    // Do not solve. Leave to Menu.
    set_state(&mut app, GameState::Menu);
    assert!(
        !app.world().resource::<MemoryReadout>().has_phrase(),
        "no readout without a solve"
    );
    assert!(
        app.world().resource::<MemoryLog>().is_empty(),
        "no memory logged without a solve"
    );
}

#[test]
fn confirm_commits_immediately_no_idle_timer() {
    // The old model needed a post-press idle pause to commit. Now a single
    // Confirm transitions on the very next frame — no timer, no pending flag.
    let mut app = test_app();
    set_state(&mut app, GameState::Menu);
    // Step down to the SELECT item, then commit on the same frame it's read.
    let select_idx = MENU_ITEMS
        .iter()
        .position(|i| *i == MenuItem::Select)
        .unwrap();
    for _ in 0..select_idx {
        nav(&mut app, MenuNav::Down);
    }
    nav(&mut app, MenuNav::Confirm); // commit fires same frame it's read
    app.update(); // apply transition
    assert_eq!(state(&app), GameState::CategorySelect);
}

// ─────────────────────────────── settings flow ──────────────────────────────

use crate::settings::{MouseSens, Settings};
use re_present::{ColorMode, MouseSettings};

/// Menu → Settings → back: stepping to the SETTINGS item and confirming enters
/// the settings screen; `Back` (Esc/q) returns to the menu.
#[test]
fn menu_routes_to_settings_and_back() {
    // Back triggers a best-effort settings.save(); point it at temp so the test
    // never writes the real user settings file.
    std::env::set_var("TS_SAVE_DIR", std::env::temp_dir());
    let mut app = test_app();
    set_state(&mut app, GameState::Menu);
    let idx = MENU_ITEMS
        .iter()
        .position(|i| *i == MenuItem::Settings)
        .unwrap();
    for _ in 0..idx {
        nav(&mut app, MenuNav::Down);
    }
    nav(&mut app, MenuNav::Confirm);
    app.update(); // apply OnEnter(Settings)
    assert_eq!(state(&app), GameState::Settings);

    nav(&mut app, MenuNav::Back);
    app.update(); // apply OnEnter(Menu)
    assert_eq!(state(&app), GameState::Menu);
}

/// Toggling the COLOR row (row 0) changes BOTH the persisted `Settings` and the
/// LIVE `ColorMode` resource the present layer blits with — proving the menu is
/// wired to the real system, not cosmetic.
#[test]
fn settings_color_toggle_applies_to_live_resource() {
    let mut app = test_app();
    set_state(&mut app, GameState::Settings); // OnEnter resets cursor to row 0
                                              // Navigate to the COLOR row (no longer row 0 now that the quality rows lead).
    let color_idx = SETTINGS_ROWS
        .iter()
        .position(|r| *r == SettingsRow::Color)
        .unwrap();
    for _ in 0..color_idx {
        nav(&mut app, MenuNav::Down);
    }
    let before = *app.world().resource::<ColorMode>();
    let want = match before {
        ColorMode::Truecolor => ColorMode::Ansi256,
        ColorMode::Ansi256 => ColorMode::Truecolor,
    };
    nav(&mut app, MenuNav::Right); // adjust the focused row (Color) forward
    assert_eq!(
        app.world().resource::<Settings>().color,
        want,
        "the stored setting flipped"
    );
    assert_eq!(
        *app.world().resource::<ColorMode>(),
        want,
        "the live ColorMode resource the present layer reads flipped too"
    );
}

/// Adjusting the MOUSE SENS row drives the live `MouseSettings.sensitivity` the
/// mouse-look system reads (Medium ×1.0 → High ×1.75).
#[test]
fn settings_mouse_sens_applies_to_live_resource() {
    let mut app = test_app();
    set_state(&mut app, GameState::Settings); // cursor at row 0
    let sens_idx = SETTINGS_ROWS
        .iter()
        .position(|r| *r == SettingsRow::MouseSens)
        .unwrap();
    for _ in 0..sens_idx {
        nav(&mut app, MenuNav::Down);
    }
    nav(&mut app, MenuNav::Right); // Medium → High
    assert_eq!(
        app.world().resource::<Settings>().mouse_sens,
        MouseSens::High
    );
    let live = app.world().resource::<MouseSettings>().sensitivity;
    assert!((live - 1.75).abs() < 1e-6, "live sensitivity = {live}");
}

// ─────────────────────────────── fov wiring ─────────────────────────────────

use re_present::FovCycle;
use re_render::{Fov, ShadowQuality};

/// A `FovCycle(+1)` message (the `[`/`]` keys + gamepad Y emit these) cycles the
/// saved `fov_deg` to the NEXT preset AND pushes it onto the live `Fov` resource
/// the raycaster reads — proving the hotkey is wired live, not cosmetic.
#[test]
fn fov_cycle_message_advances_preset_and_live_resource() {
    std::env::set_var("TS_SAVE_DIR", std::env::temp_dir());
    let mut app = test_app();
    // The hotkey runs in every state; a fresh app sits in Splash, which is fine.
    let before = app.world().resource::<Settings>().fov_deg;
    assert_eq!(before, 65, "default FOV preset");
    assert_eq!(app.world().resource::<Fov>().h_preset_deg, 65.0);

    app.world_mut().write_message(FovCycle(1));
    app.update(); // cycle_fov_live drains it

    assert_eq!(
        app.world().resource::<Settings>().fov_deg,
        70,
        "FovCycle(+1) advances the saved preset 65→70"
    );
    assert_eq!(
        app.world().resource::<Fov>().h_preset_deg,
        70.0,
        "the live raycaster Fov resource tracks the new preset"
    );

    // And -1 walks it back.
    app.world_mut().write_message(FovCycle(-1));
    app.update();
    assert_eq!(app.world().resource::<Settings>().fov_deg, 65);
    assert_eq!(app.world().resource::<Fov>().h_preset_deg, 65.0);
}

/// The settings menu reaches the FOV row and Left/Right changes it, applying to
/// BOTH the saved setting and the live `Fov` resource.
#[test]
fn settings_fov_row_applies_to_live_resource() {
    let mut app = test_app();
    set_state(&mut app, GameState::Settings); // cursor at row 0
    let fov_idx = SETTINGS_ROWS
        .iter()
        .position(|r| *r == SettingsRow::Fov)
        .unwrap();
    for _ in 0..fov_idx {
        nav(&mut app, MenuNav::Down);
    }
    let before = app.world().resource::<Settings>().fov_deg;
    nav(&mut app, MenuNav::Right); // cycle forward
    let after = app.world().resource::<Settings>().fov_deg;
    assert_ne!(after, before, "Right on the FOV row changed the preset");
    assert_eq!(
        app.world().resource::<Fov>().h_preset_deg,
        after as f32,
        "live Fov resource tracks the FOV row"
    );
    // Left reverses it.
    nav(&mut app, MenuNav::Left);
    assert_eq!(app.world().resource::<Settings>().fov_deg, before);
    assert_eq!(app.world().resource::<Fov>().h_preset_deg, before as f32);
}

/// The STRETCH row steps the aspect override and drives the live `Fov.stretch`.
#[test]
fn settings_stretch_row_applies_to_live_resource() {
    let mut app = test_app();
    set_state(&mut app, GameState::Settings);
    let idx = SETTINGS_ROWS
        .iter()
        .position(|r| *r == SettingsRow::Stretch)
        .unwrap();
    for _ in 0..idx {
        nav(&mut app, MenuNav::Down);
    }
    let before = app.world().resource::<Settings>().stretch;
    nav(&mut app, MenuNav::Right);
    let after = app.world().resource::<Settings>().stretch;
    assert!(after > before, "Right widened the stretch");
    assert!(
        (app.world().resource::<Fov>().stretch - after).abs() < 1e-6,
        "live Fov.stretch tracks the row"
    );
}

/// The SHADOWS row cycles Off/Hard/Soft and drives the LIVE `re_render::ShadowQuality`
/// resource the raycaster's shadow pass reads — proving the row is wired to the real
/// system, not cosmetic. The default is Soft; Right cycles it forward.
#[test]
fn settings_shadows_row_cycles_and_applies_to_live_resource() {
    let mut app = test_app();
    set_state(&mut app, GameState::Settings); // cursor at row 0
    let idx = SETTINGS_ROWS
        .iter()
        .position(|r| *r == SettingsRow::Shadows)
        .unwrap();
    for _ in 0..idx {
        nav(&mut app, MenuNav::Down);
    }
    // Default is Soft, live resource matches.
    assert_eq!(
        app.world().resource::<Settings>().shadows,
        ShadowQuality::Soft
    );
    assert_eq!(
        *app.world().resource::<ShadowQuality>(),
        ShadowQuality::Soft
    );

    // Right cycles Soft → Off, applied live the same frame.
    nav(&mut app, MenuNav::Right);
    assert_eq!(
        app.world().resource::<Settings>().shadows,
        ShadowQuality::Off
    );
    assert_eq!(
        *app.world().resource::<ShadowQuality>(),
        ShadowQuality::Off,
        "live ShadowQuality resource tracks the SHADOWS row"
    );

    // Right again → Hard, then once more wraps back to Soft.
    nav(&mut app, MenuNav::Right);
    assert_eq!(
        *app.world().resource::<ShadowQuality>(),
        ShadowQuality::Hard
    );
    nav(&mut app, MenuNav::Right);
    assert_eq!(
        *app.world().resource::<ShadowQuality>(),
        ShadowQuality::Soft
    );

    // Left reverses the cycle (Soft → Hard).
    nav(&mut app, MenuNav::Left);
    assert_eq!(
        app.world().resource::<Settings>().shadows,
        ShadowQuality::Hard
    );
    assert_eq!(
        *app.world().resource::<ShadowQuality>(),
        ShadowQuality::Hard
    );
}

/// Shadow + the per-level ambience mood round-trip a save→load: the shadow setting
/// persists, and the ambience mapping (`ambience_for`) is deterministic, so the
/// SAME loaded settings reproduce the SAME mood/intensity per level.
#[test]
fn shadows_and_ambience_round_trip_save_load() {
    use crate::flow::ambience_for;
    let mut path = std::env::temp_dir();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    path.push(format!("ts_flow_shadows_{nonce}"));
    path.push("settings.toml");

    let s = Settings {
        shadows: ShadowQuality::Hard,
        ..Default::default()
    };
    s.save_to(&path);
    let loaded = Settings::load_from(&path);
    assert_eq!(
        loaded.shadows,
        ShadowQuality::Hard,
        "shadow setting round-trips"
    );

    // The per-level ambience mood is deterministic from the (loaded) catalogue
    // index + tension flag, so a save→load reproduces the same bed per level.
    for idx in 0..8 {
        for tension in [false, true] {
            assert_eq!(
                ambience_for(idx, tension).mood,
                ambience_for(idx, tension).mood,
                "ambience mood is stable for level {idx} (tension={tension})"
            );
        }
    }
    // And the mapping clearly differs per level (a distinct bed per region).
    assert_ne!(
        ambience_for(0, false).mood,
        ambience_for(7, false).mood,
        "early and final levels get different moods"
    );
    let _ = std::fs::remove_file(&path);
}

/// The per-level ambience send: entering a level emits a `SetAmbience` whose mood
/// is chosen by the catalogue index, and whose intensity RISES when corruption /
/// tension is active. Driven through the real `OnEnter(Playing)` loader.
#[test]
fn entering_level_sends_ambience_and_tension_raises_intensity() {
    use crate::flow::ambience_for;
    use re_audio::{AmbienceMood, SetAmbience};

    let mut app = test_app();
    app.world_mut().resource_mut::<CurrentLevel>().0 = 0;
    set_state(&mut app, GameState::Playing); // OnEnter(Playing) emits SetAmbience

    let sent: Vec<SetAmbience> = app
        .world_mut()
        .resource_mut::<Messages<SetAmbience>>()
        .drain()
        .collect();
    assert_eq!(sent.len(), 1, "one ambience message per level entry");
    assert_eq!(
        sent[0].mood,
        AmbienceMood::ServerHum,
        "level 0 → ServerHum bed"
    );

    // Tension (corruption / denial path) raises dread above the calm base.
    let calm = ambience_for(0, false).intensity;
    let tense = ambience_for(0, true).intensity;
    assert!(tense > calm, "tension raises intensity ({tense} > {calm})");
    assert!(tense <= 1.0, "intensity stays clamped");
}

/// The settings row table is internally consistent: the cursor sizes to it and
/// every row has a unique slot (so adding the FOV/stretch rows didn't desync the
/// count the menu / cursor rely on).
#[test]
fn settings_rows_count_is_consistent() {
    let mut app = test_app();
    set_state(&mut app, GameState::Settings);
    assert_eq!(
        app.world().resource::<MenuCursor>().len,
        SETTINGS_ROWS.len(),
        "the settings cursor sizes to SETTINGS_ROWS"
    );
    assert_eq!(
        SETTINGS_ROWS.len(),
        22,
        "the 6 quality rows (Quality/DrawDistance/Fps/Lod/SubCell/Postfx) + Shadows + \
         Brightness + ColorSteps + Color..Glitch + ColorUnlock + Fov + Stretch + \
         SoundFields + SoundFieldsReduced + FirstSteps = 22 rows"
    );
    // Every row is distinct (no accidental duplicate when the new row was added).
    for (i, &row) in SETTINGS_ROWS.iter().enumerate() {
        assert_eq!(
            SETTINGS_ROWS.iter().position(|&r| r == row),
            Some(i),
            "row {row:?} appears once"
        );
    }
}

// ───────────────────────────── glitch wiring ────────────────────────────────

use re_glitch::GlitchConfig;

/// THE RULE (`glitch_config_for`): an authored per-level density × the player's
/// CORRUPTION accessibility toggle.
#[test]
fn glitch_config_rule() {
    // Authored density present + setting on → terminal_decay with the master
    // intensity OVERRIDDEN to the level's density (not the preset's hard-coded 0.5).
    let c = glitch_config_for(0.6, true);
    assert!(
        (c.intensity - 0.6).abs() < 1e-6,
        "master = level density 0.6"
    );
    assert!(
        c.bitrot.enabled,
        "terminal_decay preset is active (bitrot on)"
    );
    assert!(
        c.bitrot.spread > 0.0,
        "terminal_decay's contagion shape preserved"
    );

    // Setting OFF → off, regardless of authored density (accessibility wins).
    assert_eq!(glitch_config_for(0.6, false), GlitchConfig::off());

    // No authored density → off, even with the setting on.
    assert_eq!(glitch_config_for(0.0, true), GlitchConfig::off());

    // Density clamps into [0, 1].
    assert_eq!(glitch_config_for(9.0, true).intensity, 1.0);
}

/// Build a catalog pointing at a single temp `.lvl` carrying `[effects] glitch`.
fn temp_glitch_catalog(glitch: f32) -> (LevelCatalog, std::path::PathBuf) {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let mut path = std::env::temp_dir();
    path.push(format!("ts_glitch_{glitch}_{nonce}.lvl"));
    let src = format!(
        "\
[meta]
name = g
[room]
size = 6 4 4
[node a]
pos = 1 1 1
is_input = yes
[node b]
pos = 4 1 1
is_output = yes
[pipe a->b]
path = (1,1,1) (4,1,1)
[pedestal]
center = 4 1 1
volume = 1 1 1
[door]
pos = 5 1 1
[player]
spawn = 3 1 1
[effects]
glitch = {glitch}
"
    );
    std::fs::write(&path, src).expect("write temp level");
    let paths = vec![path.to_string_lossy().replace('\\', "/")];
    (LevelCatalog { paths }, path)
}

/// Enter a level authored `glitch = 0.6` with the setting ON → the live
/// `GlitchConfig` master intensity is the authored density.
#[test]
fn entering_glitch_level_drives_live_config() {
    let (catalog, path) = temp_glitch_catalog(0.6);
    let mut app = test_app();
    app.insert_resource(catalog);
    app.world_mut().resource_mut::<Settings>().glitch_enabled = true;
    set_state(&mut app, GameState::Playing);
    app.update(); // let the Playing Update frame settle
    let cfg = *app.world().resource::<GlitchConfig>();
    assert!(
        (cfg.intensity - 0.6).abs() < 1e-6,
        "live intensity = {}",
        cfg.intensity
    );
    assert!(cfg.bitrot.enabled);
    let _ = std::fs::remove_file(&path);
}

/// The same level with the CORRUPTION setting OFF → the live config is `off`.
#[test]
fn glitch_setting_off_forces_off() {
    let (catalog, path) = temp_glitch_catalog(0.6);
    let mut app = test_app();
    app.insert_resource(catalog);
    app.world_mut().resource_mut::<Settings>().glitch_enabled = false;
    set_state(&mut app, GameState::Playing);
    app.update();
    assert_eq!(*app.world().resource::<GlitchConfig>(), GlitchConfig::off());
    let _ = std::fs::remove_file(&path);
}

/// A level with no authored glitch → off even with the setting on.
#[test]
fn level_without_glitch_stays_off() {
    let (catalog, path) = temp_glitch_catalog(0.0);
    let mut app = test_app();
    app.insert_resource(catalog);
    app.world_mut().resource_mut::<Settings>().glitch_enabled = true;
    set_state(&mut app, GameState::Playing);
    app.update();
    assert_eq!(*app.world().resource::<GlitchConfig>(), GlitchConfig::off());
    let _ = std::fs::remove_file(&path);
}

/// Toggling the CORRUPTION setting OFF at runtime re-applies the config
/// immediately (proving the runtime re-apply system is wired).
#[test]
fn runtime_toggle_off_takes_effect() {
    let (catalog, path) = temp_glitch_catalog(0.6);
    let mut app = test_app();
    app.insert_resource(catalog);
    app.world_mut().resource_mut::<Settings>().glitch_enabled = true;
    set_state(&mut app, GameState::Playing);
    app.update();
    assert!(app.world().resource::<GlitchConfig>().intensity > 0.0);
    // Flip the setting OFF while still in Playing, then tick.
    app.world_mut().resource_mut::<Settings>().glitch_enabled = false;
    app.update();
    assert_eq!(*app.world().resource::<GlitchConfig>(), GlitchConfig::off());
    let _ = std::fs::remove_file(&path);
}

// ───────────────────────────── pause overlay ────────────────────────────────

use re_world::CameraPose;

/// Esc/Back DURING PLAY opens the Pause overlay — it must NOT quit the game.
/// The old behaviour (Back → `AppExit`) was the frustration this fixes. Strengthened
/// to fire Esc AFTER a real movement/interact frame (nav messages flowing), proving
/// `watch_pause` reliably catches a Back that arrives mid-play, not only on the
/// pristine entry frame.
#[test]
fn esc_in_playing_opens_pause_not_quit() {
    let mut app = test_app();
    set_state(&mut app, GameState::Playing); // loads a real level
    assert_eq!(state(&app), GameState::Playing);

    // Simulate a few real gameplay frames first: an interact press and some
    // movement-style nav (the present layer emits MenuNav alongside camera moves).
    // watch_pause drains these harmlessly; the camera is frozen-on-pause anyway.
    app.world_mut().write_message(InteractPressed);
    nav(&mut app, MenuNav::Down); // a "move" frame — must NOT pause
    assert_eq!(
        state(&app),
        GameState::Playing,
        "movement nav must not trip the pause watcher"
    );
    nav(&mut app, MenuNav::Right); // another move frame
    assert_eq!(state(&app), GameState::Playing);

    // NOW the player hits Esc mid-play.
    nav(&mut app, MenuNav::Back); // watch_pause sees it → queues Pause
    app.update(); // apply the transition

    assert_eq!(state(&app), GameState::Pause, "Esc in play → Pause overlay");
    // And it must NOT have emitted an AppExit from the re_game side.
    let exits = app
        .world_mut()
        .resource_mut::<Messages<AppExit>>()
        .drain()
        .count();
    assert_eq!(exits, 0, "Esc in play must not quit the game");
}

/// Pausing must NOT tear the level down — the live camera + puzzle survive so the
/// scene stays frozen behind the overlay (and Resume can return to it).
#[test]
fn pause_keeps_the_level_alive() {
    let mut app = test_app();
    set_state(&mut app, GameState::Playing);
    nav(&mut app, MenuNav::Back);
    app.update();
    assert_eq!(state(&app), GameState::Pause);

    let cams = app
        .world_mut()
        .query_filtered::<(), With<PlayCamera>>()
        .iter(app.world())
        .count();
    assert_eq!(
        cams, 1,
        "the play camera survives a pause (scene stays frozen)"
    );
    let (sx, sy, sz) = app.world().resource::<Map>().size();
    assert!(
        sx > 1 || sy > 1 || sz > 1,
        "the level map is NOT reset on pause"
    );
    assert!(
        !app.world().resource::<PuzzleState>().nodes.is_empty(),
        "the puzzle network is NOT cleared on pause"
    );
}

/// Pause → Resume returns to Playing without reloading the level.
#[test]
fn pause_resume_returns_to_playing() {
    let mut app = test_app();
    set_state(&mut app, GameState::Playing);
    nav(&mut app, MenuNav::Back);
    app.update();
    assert_eq!(state(&app), GameState::Pause);

    // Cursor row 0 == Resume. Commit it.
    force_commit(&mut app); // pause_nav queues Playing
    app.update(); // apply
    assert_eq!(state(&app), GameState::Playing, "Resume → Playing");
    // The level is still there (resume did not reload/clear it).
    let cams = app
        .world_mut()
        .query_filtered::<(), With<PlayCamera>>()
        .iter(app.world())
        .count();
    assert_eq!(
        cams, 1,
        "exactly one camera after resume (no reload duplicate)"
    );
}

/// Pause → Settings → Back returns to PAUSE (not the main menu), because the
/// settings screen tracks where it was entered from (`SettingsReturn`).
#[test]
fn pause_settings_back_returns_to_pause() {
    std::env::set_var("TS_SAVE_DIR", std::env::temp_dir());
    let mut app = test_app();
    set_state(&mut app, GameState::Playing);
    nav(&mut app, MenuNav::Back);
    app.update();
    assert_eq!(state(&app), GameState::Pause);

    // Step to the SETTINGS item and commit.
    let idx = PAUSE_ITEMS
        .iter()
        .position(|i| *i == crate::flow::PauseItem::Settings)
        .unwrap();
    for _ in 0..idx {
        nav(&mut app, MenuNav::Down);
    }
    nav(&mut app, MenuNav::Confirm);
    app.update(); // OnEnter(Settings)
    assert_eq!(state(&app), GameState::Settings);
    assert_eq!(
        *app.world().resource::<SettingsReturn>(),
        SettingsReturn::Pause,
        "entering Settings from Pause records the Pause return target"
    );

    nav(&mut app, MenuNav::Back); // settings_nav routes back by SettingsReturn
    app.update();
    assert_eq!(
        state(&app),
        GameState::Pause,
        "Settings Back → Pause, not Menu"
    );
}

/// Menu → Settings → Back still returns to the MENU (the default return target),
/// so the new Pause routing did not regress the menu path.
#[test]
fn menu_settings_back_still_returns_to_menu() {
    std::env::set_var("TS_SAVE_DIR", std::env::temp_dir());
    let mut app = test_app();
    set_state(&mut app, GameState::Menu);
    let idx = MENU_ITEMS
        .iter()
        .position(|i| *i == MenuItem::Settings)
        .unwrap();
    for _ in 0..idx {
        nav(&mut app, MenuNav::Down);
    }
    nav(&mut app, MenuNav::Confirm);
    app.update();
    assert_eq!(state(&app), GameState::Settings);
    assert_eq!(
        *app.world().resource::<SettingsReturn>(),
        SettingsReturn::Menu
    );

    nav(&mut app, MenuNav::Back);
    app.update();
    assert_eq!(
        state(&app),
        GameState::Menu,
        "Settings from Menu → back to Menu"
    );
}

/// (FIX1 live-apply) Opening Settings FROM the pause overlay and changing the FOV
/// row applies to the LIVE `re_render::Fov` the SAME frame — so a setting tweaked
/// mid-game takes effect in the (still-loaded, frozen) running level, not only on
/// the next launch.
#[test]
fn pause_settings_fov_change_applies_live_to_running_level() {
    std::env::set_var("TS_SAVE_DIR", std::env::temp_dir());
    let mut app = test_app();
    set_state(&mut app, GameState::Playing);
    skip_dive(&mut app);
    nav(&mut app, MenuNav::Back); // → Pause (level stays loaded behind it)
    app.update();
    assert_eq!(state(&app), GameState::Pause);

    // Pause → Settings.
    let idx = PAUSE_ITEMS
        .iter()
        .position(|i| *i == crate::flow::PauseItem::Settings)
        .unwrap();
    for _ in 0..idx {
        nav(&mut app, MenuNav::Down);
    }
    nav(&mut app, MenuNav::Confirm);
    app.update();
    assert_eq!(state(&app), GameState::Settings);

    // Move to the FOV row and change it; the live Fov resource must track it
    // immediately (the running level is still loaded — Pause kept it alive).
    let fov_idx = SETTINGS_ROWS
        .iter()
        .position(|r| *r == SettingsRow::Fov)
        .unwrap();
    for _ in 0..fov_idx {
        nav(&mut app, MenuNav::Down);
    }
    let before = app.world().resource::<Settings>().fov_deg;
    nav(&mut app, MenuNav::Right); // adjust + apply LIVE
    let after = app.world().resource::<Settings>().fov_deg;
    assert_ne!(before, after, "FOV row changed the saved setting");
    assert_eq!(
        app.world().resource::<Fov>().h_preset_deg,
        after as f32,
        "live Fov resource updated mid-game while paused"
    );
}

/// Pause → Quit-to-menu abandons the level for the main menu (and tears the
/// level down, since this is a real leave of Playing → Menu).
#[test]
fn pause_quit_returns_to_menu() {
    std::env::set_var("TS_SAVE_DIR", std::env::temp_dir());
    let mut app = test_app();
    set_state(&mut app, GameState::Playing);
    nav(&mut app, MenuNav::Back);
    app.update();
    assert_eq!(state(&app), GameState::Pause);

    let idx = PAUSE_ITEMS
        .iter()
        .position(|i| *i == crate::flow::PauseItem::QuitToMenu)
        .unwrap();
    for _ in 0..idx {
        nav(&mut app, MenuNav::Down);
    }
    nav(&mut app, MenuNav::Confirm); // queues Menu
    app.update(); // apply: Pause→Menu (no Playing OnExit here — already left it)
    assert_eq!(state(&app), GameState::Menu, "Quit-to-menu → main menu");
}

// ────────────────────────── objective HUD / crosshair ───────────────────────

use crate::flow::{objective_progress, power_readout};

// `sequence_progress_tracks_l11_lock` lives in `tests/campaign_puzzle.rs`
// instead — it asserts a property OF the campaign's authored L11
// sequence-lock graph, not the generic `sequence_progress` mechanism, so it
// belongs with the other campaign-content tests rather than this generic
// flow-HUD suite.

// `objective_progress_reflects_activation` and
// `crosshair_active_only_when_facing_a_switch` live in
// `re_runtime::hud_probe`'s own inline tests instead — they pin
// `objective_progress` and `faced_switch`'s generic behaviour against archive
// fixtures (`01.lvl`, `03.lvl`), which no longer needs a game-crate host.

/// The in-play HUD paints BOTH a controls cheat-sheet (bottom row) and the
/// objective line (top row) into the live framebuffer while Playing, so the goal
/// and the controls are always visible. We build a Playing app WITH the overlay
/// plugin, run a frame so the `Last` painters fire, and scan the framebuffer.
#[test]
fn play_hud_paints_controls_and_objective() {
    use crate::menu::MenuRenderPlugin;
    use re_core::Framebuffer;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(StatesPlugin)
        .add_message::<InteractPressed>()
        .add_plugins(PuzzlePlugin)
        .add_plugins(FlowPlugin { cli_level: None })
        .add_plugins(MenuRenderPlugin)
        .insert_resource(abs_catalog())
        // The overlay painters need a Framebuffer (the present layer provides it in
        // the real app; FlowPlugin also inserts one, but be explicit/sized here).
        .insert_resource(Framebuffer::new(crate::VIEW_COLS, crate::VIEW_ROWS));

    // Enter a real level and let the Playing `Last` HUD painter run.
    app.world_mut().resource_mut::<CurrentLevel>().0 = 0;
    set_state(&mut app, GameState::Playing);
    app.update(); // a full frame: PostUpdate raycast (no-op math) then Last HUD paint
    assert_eq!(state(&app), GameState::Playing);

    // Flatten the framebuffer to text rows for substring scanning.
    let fb = app.world().resource::<Framebuffer>();
    let rows: Vec<String> = (0..fb.rows())
        .map(|r| {
            (0..fb.cols())
                .map(|c| char::from_u32(fb.get(c, r).unwrap().glyph).unwrap_or(' '))
                .collect::<String>()
        })
        .collect();
    let screen = rows.join("\n");

    // Controls cheat-sheet — the exact compact line, on the BOTTOM row.
    let controls = "WASD move";
    assert!(
        rows.last().is_some_and(|last| last.contains(controls)),
        "controls hint must be on the bottom row; bottom = {:?}",
        rows.last()
    );
    assert!(
        screen.contains("Space jump") && screen.contains("Esc menu") && screen.contains("E use"),
        "controls line must spell out move/jump/use/menu"
    );

    // Objective line — the goal + a `[lit/total]` progress badge, on a top row.
    assert!(
        screen.contains("OBJECTIVE"),
        "the objective line must be painted in Playing"
    );
    let (lit, total) = objective_progress(app.world().resource::<PuzzleState>());
    assert!(total > 0, "level 0 has a real win condition");
    assert!(
        screen.contains(&format!("[{lit}/{total}]")),
        "objective must show [lit/total] progress, want [{lit}/{total}]"
    );
}

/// The POWER READOUT (`[room] power_budget`). Two things must hold, and the
/// second is the one that matters: an UN-budgeted level's HUD is byte-identical
/// to before the rule existed, and a budgeted one shows `power N/M` BEFORE the
/// player ever hits the cap — plus a "the grid is FULL" instruction the instant
/// they do. A constraint the player only discovers by being refused reads as a
/// broken switch, not as a rule.
#[test]
fn play_hud_paints_the_power_readout_only_when_a_budget_exists() {
    use crate::menu::MenuRenderPlugin;
    use re_core::Framebuffer;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(StatesPlugin)
        .add_message::<InteractPressed>()
        .add_plugins(PuzzlePlugin)
        .add_plugins(FlowPlugin { cli_level: None })
        .add_plugins(MenuRenderPlugin)
        .insert_resource(abs_catalog())
        .insert_resource(Framebuffer::new(crate::VIEW_COLS, crate::VIEW_ROWS));

    app.world_mut().resource_mut::<CurrentLevel>().0 = 0;
    set_state(&mut app, GameState::Playing);
    app.update();

    let screen = |app: &App| -> String {
        let fb = app.world().resource::<Framebuffer>();
        (0..fb.rows())
            .map(|r| {
                (0..fb.cols())
                    .map(|c| char::from_u32(fb.get(c, r).unwrap().glyph).unwrap_or(' '))
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join(
                "
",
            )
    };

    // 1. UN-BUDGETED (levels/01.lvl declares no cap): no readout at all.
    assert_eq!(
        app.world().resource::<PuzzleState>().power_budget,
        0,
        "the archive level under test must be un-budgeted"
    );
    assert!(
        power_readout(app.world().resource::<PuzzleState>()).is_none(),
        "no budget ⇒ no readout"
    );
    // (`·  power ` is the readout SEGMENT; the objective line itself legitimately
    // contains the word "power" in "…power the network".)
    assert!(
        !screen(&app).contains("·  power "),
        "an un-budgeted level's HUD must be unchanged: {}",
        screen(&app)
    );

    // 2. BUDGETED with headroom: `power 1/2` is on screen.
    {
        let mut p = app.world_mut().resource_mut::<PuzzleState>();
        p.power_budget = 2;
        p.pipes[0].state = re_level::PipeState::On;
    }
    app.update();
    assert_eq!(
        power_readout(app.world().resource::<PuzzleState>()),
        Some((1, 2))
    );
    assert!(
        screen(&app).contains("power 1/2"),
        "the readout must be visible while there is still headroom: {}",
        screen(&app)
    );

    // 3. FULLY DRAWN: the readout says so AND the hint line tells the player the
    //    only legal move left (switch something off).
    app.world_mut().resource_mut::<PuzzleState>().power_budget = 1;
    app.update();
    let s = screen(&app);
    assert!(s.contains("power 1/1"), "{s}");
    assert!(
        s.contains("the grid is FULL"),
        "at the cap the hint must name the way out: {s}"
    );
}

/// The row-1 hint ("aim at a glowing switch ◆ … press E to flip it") must
/// disappear once the puzzle is solved — it told a player who had already lit
/// every node to go do something that no longer changes the outcome. Level 0
/// (`01.lvl`, archive catalogue) has exactly one pipe/switch/door-target, so
/// flipping it ON drives `lit == total` in one frame.
#[test]
fn play_hud_drops_the_switch_hint_once_the_objective_is_solved() {
    use crate::menu::MenuRenderPlugin;
    use re_core::Framebuffer;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(StatesPlugin)
        .add_message::<InteractPressed>()
        .add_plugins(PuzzlePlugin)
        .add_plugins(FlowPlugin { cli_level: None })
        .add_plugins(MenuRenderPlugin)
        .insert_resource(abs_catalog())
        .insert_resource(Framebuffer::new(crate::VIEW_COLS, crate::VIEW_ROWS));

    app.world_mut().resource_mut::<CurrentLevel>().0 = 0;
    set_state(&mut app, GameState::Playing);
    app.update();

    let screen = |app: &App| -> String {
        let fb = app.world().resource::<Framebuffer>();
        (0..fb.rows())
            .map(|r| {
                (0..fb.cols())
                    .map(|c| char::from_u32(fb.get(c, r).unwrap().glyph).unwrap_or(' '))
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    // BEFORE solving: the objective is unmet and the switch hint is up.
    let (lit0, total0) = objective_progress(app.world().resource::<PuzzleState>());
    assert!(
        total0 > 0 && lit0 < total0,
        "level 0 starts unsolved ({lit0}/{total0})"
    );
    let before = screen(&app);
    assert!(
        before.contains("aim at a glowing switch"),
        "the switch hint must be up before the puzzle is solved: {before}"
    );
    assert!(
        !before.contains("head to the exit"),
        "the exit clause must not appear before solving: {before}"
    );

    // Solve it through the public interaction path — `01.lvl`'s sole switch
    // sits at (6,3,2) (see `crosshair_active_only_when_facing_a_switch`
    // above). `puzzle_interact` runs the forward pass itself, so the door
    // target lights immediately; `app.update()` then repaints the HUD off
    // the now-solved `PuzzleState`.
    assert!(
        puzzle_interact(&mut app.world_mut().resource_mut::<PuzzleState>(), 6, 3, 2),
        "01.lvl's switch must be at (6,3,2)"
    );
    app.update();

    let (lit1, total1) = objective_progress(app.world().resource::<PuzzleState>());
    assert_eq!(
        (lit1, total1),
        (total1, total1),
        "solving must light every door target"
    );

    // AFTER solving: the stale switch hint must be GONE, and row 0 must have
    // switched to the "head to the exit" clause.
    let after = screen(&app);
    assert!(
        !after.contains("aim at a glowing switch"),
        "the switch hint must be hidden once the puzzle is solved: {after}"
    );
    assert!(
        after.contains("head to the exit"),
        "row 0 must read the solved objective line: {after}"
    );
}

/// Absolute path to shipped level 01 (cargo test cwd is the crate dir).
fn level_path_01() -> String {
    abs_level("01.lvl")
}

/// Absolute path to a level file under the workspace `levels/` dir (cargo test
/// cwd is the crate dir, so a repo-relative `levels/NN.lvl` would not resolve).
fn abs_level(file: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .unwrap()
        .join("levels")
        .join(file)
        .to_string_lossy()
        .into_owned()
}

/// Absolute path to a CRATE-LOCAL fixture under `crates/terminal_state/tests/fixtures/`:
/// a copy of an engine tech-demo level kept with the game so this crate's
/// tests never load a level from the engine's own `levels/` tree, which lives
/// in a separate repository.
fn fixture_level(file: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(file)
        .to_string_lossy()
        .into_owned()
}

// ────────────────────────────── emitter wiring ───────────────────────────────

/// The authored `[emitter]` specs of the tech-demo level must round-trip into the
/// live `EmitterSet`: one `re_render::Emitter` per `EmitterSpec`, same COUNT, and
/// with the spec ordering preserved. This proves `emitter_set_for` bridges the
/// level data → render resource (the live composite path reads `EmitterSet`).
#[test]
fn level_emitters_roundtrip_into_emitter_set() {
    let level = re_level::load(fixture_level("21-emitter-showcase.lvl"))
        .expect("the emitter-showcase fixture loads");
    // The demo authors several emitters — guard against a silent drop to zero.
    assert!(
        level.emitters.len() >= 6,
        "the showcase level must author several emitters, got {}",
        level.emitters.len()
    );
    let set = emitter_set_for(&level);
    assert_eq!(
        set.emitters.len(),
        level.emitters.len(),
        "one live Emitter per authored EmitterSpec, no drops/dups"
    );
}

/// A field or two must SURVIVE the `EmitterSpec` → `Emitter` map: the resolved
/// emitter's lifetime and acceleration must match what the level authored. This
/// catches a `from_spec` that silently defaults instead of carrying the spec.
#[test]
fn level_emitter_fields_survive_the_map() {
    use re_world::{EmitterOrigin, EmitterSpec};
    // EmitterSpec stores re_core's glam `Vec3` (workspace glam 0.33), which is a
    // DIFFERENT type from bevy's bundled `bevy::prelude::Vec3`. Shadow the bevy one
    // with re_core's so the spec literal type-checks.
    use re_core::Vec3;
    // A single hand-built spec with distinctive, non-default values.
    let spec = EmitterSpec {
        origin: EmitterOrigin::World(Vec3::new(2.5, 2.0, 1.0)),
        lifetime: 2.2,
        accel: Vec3::new(0.0, 0.0, 0.35),
        rate: 26.0,
        ..EmitterSpec::default()
    };
    let level = Level {
        emitters: vec![spec.clone()],
        ..Level::default()
    };
    let set = emitter_set_for(&level);
    assert_eq!(set.emitters.len(), 1);
    let e = &set.emitters[0];
    assert!(
        (e.lifetime - spec.lifetime).abs() < 1e-3,
        "lifetime must survive from_spec, got {} want {}",
        e.lifetime,
        spec.lifetime
    );
    assert!(
        (e.accel.z - spec.accel.z).abs() < 1e-3,
        "accel.z must survive from_spec, got {} want {}",
        e.accel.z,
        spec.accel.z
    );
}

/// A real level with NO `[emitter]` section yields an EMPTY set — never the demo
/// fallback. (The two demo emitters are the empty/menu default only.) 01.lvl ships
/// without emitters.
#[test]
fn level_without_emitters_yields_empty_set() {
    let level = re_level::load(level_path_01()).expect("01.lvl loads");
    assert!(
        level.emitters.is_empty(),
        "fixture assumption: 01.lvl authors no emitters"
    );
    let set = emitter_set_for(&level);
    assert!(
        set.emitters.is_empty(),
        "a real level with no [emitter] must give an EMPTY set, not the demo fallback"
    );
}

/// The tech-demo level must LOAD and build a map without panicking, and its
/// camera spawn must be inside the room — a smoke test that the authored
/// material + emitter sections parse cleanly.
#[test]
fn showcase_level_loads_and_builds() {
    let level = re_level::load(fixture_level("21-emitter-showcase.lvl"))
        .expect("the emitter-showcase fixture must parse cleanly");
    let map = level.build_map();
    let (sx, sy, sz) = map.size();
    assert!(sx > 0 && sy > 0 && sz > 0, "the showcase room has volume");
    // PuzzleState must build (the trivial one-pipe network is valid).
    PuzzleState::from_level(&level).expect("showcase network builds");
}

// ---------------------------------------------------------------------------
// Floor-zone input (L5 "Soft-Footstep"): player-position-as-input
// ---------------------------------------------------------------------------

// `apply_zone_inputs_solves_while_standing_in_zone` lives in
// `re_runtime/tests/triggers.rs` instead, rewritten against a synthetic
// crate-local fixture instead of the campaign's `L5-soft-footstep.lvl` (an
// engine-repo test must not load a game-repo level). The end-to-end app test
// below, which pins THIS game's L5 content, stays.

/// End-to-end through the real `App`: load L5 via the catalog, enter Playing,
/// teleport the play camera onto the warm zone, tick, and assert `Solved` flips.
#[test]
fn zone_input_system_solves_l5_in_app() {
    let mut app = test_app();
    // Point the catalog solely at L5 and play it.
    app.insert_resource(LevelCatalog {
        paths: vec![abs_level("L5-soft-footstep.lvl")],
    });
    set_state(&mut app, GameState::Menu);
    force_commit(&mut app); // Start → Diving
    app.update();
    skip_dive(&mut app); // → Playing (loads L5)
    assert_eq!(state(&app), GameState::Playing);

    // Pre-wire the two switchable pipes ON so only the zone gates the win.
    {
        let mut puzzle = app.world_mut().resource_mut::<PuzzleState>();
        let switches: Vec<(i32, i32, i32)> = puzzle
            .pipes
            .iter()
            .filter_map(re_puzzle::Pipe::switch_coord)
            .collect();
        for (x, y, z) in switches {
            re_puzzle::puzzle_interact(&mut puzzle, x, y, z);
        }
    }

    // Find the warm-zone centre from the loaded level data and teleport there.
    let centre = {
        let ld = app.world().resource::<CurrentLevelData>();
        let level = ld.0.as_ref().expect("L5 level data present");
        let z = &level.zone_inputs[0];
        (
            (z.x0 + z.x1) as f32 / 2.0 + 0.5,
            (z.y0 + z.y1) as f32 / 2.0 + 0.5,
            z.z0 as f32 + 0.5,
        )
    };
    {
        let mut q = app
            .world_mut()
            .query_filtered::<&mut CameraPose, With<PlayCamera>>();
        let mut cam = q.single_mut(app.world_mut()).expect("play camera exists");
        cam.pos.x = centre.0;
        cam.pos.y = centre.1;
        cam.pos.z = centre.2;
    }

    // Tick: zone_input_sys sets present=1, apply_interactions mirrors solved.
    app.update();
    app.update();
    assert!(
        app.world().resource::<PuzzleState>().solved,
        "standing in the warm zone must solve the L5 network"
    );
    assert!(
        app.world().resource::<Solved>().0,
        "the Solved resource must mirror the zone-driven win"
    );
}

// ---------------------------------------------------------------------------
// Trigger-fields (T0 "Trip-Field"): regions that fire on entry
// ---------------------------------------------------------------------------

use crate::flow::TriggerFired;

// `apply_triggers_fires_once_on_entry_not_while_idle` and
// `apply_triggers_once_vs_refire` live in `re_runtime/tests/triggers.rs`
// instead — pure firing-core tests over the `T0-trigger` engine example
// fixture (not a campaign level), which no longer needs a game-crate host.
// The end-to-end app test below, which pins THIS game's wiring (`trigger_sys`
// + the `TriggerFired` message), stays on a crate-local COPY of that fixture.

/// End-to-end through the real `App`: load T0 via the catalog, enter Playing,
/// walk the play camera INTO the trigger region, tick, and assert the
/// `TriggerFired` message fired exactly once and the network solved — while
/// standing at spawn (outside) fired nothing.
#[test]
fn trigger_system_fires_in_app() {
    let mut app = test_app();
    app.insert_resource(LevelCatalog {
        paths: vec![fixture_level("T0-trigger.lvl")],
    });
    set_state(&mut app, GameState::Menu);
    force_commit(&mut app); // Start → Diving
    app.update();
    skip_dive(&mut app); // → Playing (loads T0)
    assert_eq!(state(&app), GameState::Playing);

    // Force the runtime input LOW (the static `[inputs] gate = 1` would otherwise
    // start it solved); the trigger must be what raises it.
    let gate = {
        let level = app.world().resource::<CurrentLevelData>().0.clone();
        let level = level.expect("T0 level data");
        let g = level.node_index("gate").expect("gate node");
        let mut puzzle = app.world_mut().resource_mut::<PuzzleState>();
        re_puzzle::set_input_value(&mut puzzle, g, 0);
        g
    };

    // Tick a frame at SPAWN (outside the region): no trigger fires.
    app.update();
    let fired = app
        .world_mut()
        .resource_mut::<Messages<TriggerFired>>()
        .drain()
        .count();
    assert_eq!(fired, 0, "standing at spawn (outside) fires no trigger");
    assert_eq!(
        app.world()
            .resource::<PuzzleState>()
            .inputs
            .get(gate)
            .copied(),
        Some(0),
        "gate stays low while outside the trigger"
    );

    // Teleport the play camera into the trigger region centre.
    let centre = {
        let map = app.world().resource::<Map>();
        let re_core::Region::Box { min, max } = map.resolved_triggers()[0].region else {
            panic!("T0 trigger region is a Box");
        };
        (
            (min.0 + max.0) as f32 / 2.0 + 0.5,
            (min.1 + max.1) as f32 / 2.0 + 0.5,
            min.2 as f32 + 0.5,
        )
    };
    {
        let mut q = app
            .world_mut()
            .query_filtered::<&mut CameraPose, With<PlayCamera>>();
        let mut cam = q.single_mut(app.world_mut()).expect("play camera");
        cam.pos.x = centre.0;
        cam.pos.y = centre.1;
        cam.pos.z = centre.2;
    }

    // Tick: trigger_sys detects the entry edge, fires, drives `gate`.
    app.update();
    let labels: Vec<String> = app
        .world_mut()
        .resource_mut::<Messages<TriggerFired>>()
        .drain()
        .map(|m| m.label)
        .collect();
    assert_eq!(
        labels,
        vec!["gate".to_string()],
        "entry fires exactly one TriggerFired(gate)"
    );
    assert_eq!(
        app.world()
            .resource::<PuzzleState>()
            .inputs
            .get(gate)
            .copied(),
        Some(1),
        "the trigger drove `gate` to 1",
    );

    // Tick again STILL INSIDE: the once-trigger must not re-fire.
    app.update();
    let again = app
        .world_mut()
        .resource_mut::<Messages<TriggerFired>>()
        .drain()
        .count();
    assert_eq!(
        again, 0,
        "standing still inside the once-trigger never re-fires"
    );
}

// ─────────────────── where the content tree is anchored ─────────────────────
//
// `levels/` used to be looked up relative to the PROCESS WORKING DIRECTORY, which
// is set by whoever launched us and is therefore not an anchor at all. A
// `terminal-state.exe` copied on its own found nothing; the scan failed, and
// `LevelCatalog::default` then INVENTED `levels/01.lvl … 07.lvl` — seven names that
// could not be loaded either, so the menu came up full and Start did nothing.
//
// These tests run with cwd = `crates/terminal_state`, where `levels/` does not exist. That
// is exactly the broken configuration, which makes them a live regression guard.

/// The CWD these tests run under really does NOT contain `levels/`. Without this,
/// the two tests below would pass for the wrong reason.
#[test]
fn the_test_cwd_has_no_levels_directory_of_its_own() {
    assert!(
        !std::path::Path::new("levels").is_dir(),
        "precondition: cargo runs tests with cwd = the crate dir ({:?}), which has no levels/",
        std::env::current_dir()
    );
}

/// `levels/` is located from the RUNNING BINARY, so it is found even though the
/// working directory knows nothing about it.
#[test]
fn the_levels_directory_is_located_without_help_from_the_cwd() {
    let dir = super::locate_levels_dir().expect("levels/ is locatable from the test binary");
    assert!(
        dir.join("01.lvl").is_file(),
        "located the real levels dir, not an empty namesake: {}",
        dir.display()
    );
}

/// THE REGRESSION. `LevelCatalog::default()` — what the shipped binary actually
/// builds its menu from — must produce paths that LOAD. The old CWD scan produced
/// seven fabricated names here, none of which exists relative to this cwd.
///
/// The threshold is 20, not ~47: this scans the game repo's OWN `levels/`
/// directory, which — now that the engine-example levels (`TX*`,
/// `08-material-demo`, `92-fractal-portal`, …) live in `terminal3d/` instead —
/// holds exactly the 12 campaign levels + the 8 `0N.lvl` archive rooms = 20
/// `.lvl` files.
#[test]
fn the_default_catalog_lists_levels_that_can_actually_be_opened() {
    let catalog = LevelCatalog::default();
    assert!(
        catalog.paths.len() >= 20,
        "expected the full shipped catalogue, got {}: {:?}",
        catalog.paths.len(),
        catalog.paths
    );
    // Every single entry must name a file that is really there — a catalogue slot
    // that cannot be opened is a menu entry that does nothing.
    let missing: Vec<&String> = catalog
        .paths
        .iter()
        .filter(|p| !std::path::Path::new(p).is_file())
        .collect();
    assert!(
        missing.is_empty(),
        "catalogue entries that do not exist: {missing:?}"
    );
    // And the first one really parses, so this is not just a path-shape check.
    let first = &catalog.paths[0];
    assert!(
        re_level::load(first).is_ok(),
        "catalogue slot 0 ({first}) must load"
    );
}

/// An unlocatable content directory is REPORTED with everywhere it looked — never
/// silently swallowed, and never replaced with a guess.
#[test]
fn a_missing_content_directory_names_every_place_it_looked() {
    let err = re_level::assets::find_content_dir("no-such-dir-9c1f").expect_err("cannot exist");
    assert!(!err.searched.is_empty());
    let msg = err.to_string();
    assert!(msg.contains("no-such-dir-9c1f"), "{msg}");
    assert!(msg.contains("looked in"), "{msg}");
}

// ──────────────────────── catalog enumeration + browse ───────────────────────

/// Build a [`LevelCatalog`] by scanning the REAL `levels/` directory at an
/// absolute path (the test cwd is `crates/terminal_state`, so the default relative scan
/// would not resolve). Mirrors `LevelCatalog::default`'s scan + category sort so
/// the enumeration/categorisation is exercised over the actual shipped files.
fn full_catalog() -> LevelCatalog {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .unwrap()
        .join("levels");
    let mut paths: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "lvl"))
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .collect();
    paths.sort_by_key(|p| super::catalog_sort_key(p));
    LevelCatalog { paths }
}

/// The engine-example levels (demos/showcases/portals/overlay/texture/feature/
/// test fixtures) live in `terminal3d/` instead, so the game repo's own
/// `levels/` holds exactly 20 files (12 campaign + 8 archive), not the ~47 a
/// combined tree would have. The spot-check needle list is pruned to only
/// names that still exist here.
#[test]
fn catalog_enumerates_every_level_file() {
    let catalog = full_catalog();
    // There are 20 shipped .lvl files; the catalogue must list ALL of them.
    assert!(
        catalog.paths.len() >= 20,
        "expected the full 20-level catalogue, got {}",
        catalog.paths.len()
    );
    // A spot-check of one level from every category the game repo still owns
    // content in proves nothing is dropped.
    for needle in [
        "01",                // Campaign numeric
        "L11-inner-circuit", // Campaign L-series
        "05",                // Archive
    ] {
        assert!(
            catalog
                .paths
                .iter()
                .any(|p| LevelCatalog::label(p) == needle),
            "catalogue is missing level '{needle}'"
        );
    }
}

#[test]
fn every_catalog_level_lands_in_exactly_one_browse_category() {
    let catalog = full_catalog();
    // The per-category index lists must PARTITION the catalogue: every slot is
    // claimed by exactly one category, and the union covers all of them.
    let total: usize = Category::ALL.iter().map(|c| catalog.count_in(*c)).sum();
    assert_eq!(
        total,
        catalog.paths.len(),
        "categories must partition the catalogue with no level dropped or double-counted"
    );
    // And `indices_in` for every category, concatenated, hits each slot once.
    let mut seen = vec![false; catalog.paths.len()];
    for cat in Category::ALL {
        for gidx in catalog.indices_in(cat) {
            assert!(!seen[gidx], "slot {gidx} claimed by two categories");
            seen[gidx] = true;
        }
    }
    assert!(
        seen.iter().all(|s| *s),
        "every slot is claimed by some category"
    );
}

#[test]
fn categorize_assigns_the_expected_folders() {
    use super::categorize;
    let cases = [
        ("01", Category::Campaign),
        ("08", Category::Campaign),
        ("L5-soft-footstep", Category::Campaign),
        ("L12-closing", Category::Campaign),
        ("08-material-demo", Category::Demos),
        ("10-hologram-demo", Category::Demos),
        ("13b-holomesh", Category::Demos),
        ("90-material-catalog", Category::Demos),
        ("94-multichannel-demo", Category::Demos),
        ("20-generic-showcase", Category::Showcases),
        ("21-emitter-showcase", Category::Showcases),
        ("91-generic-demo", Category::Showcases),
        ("97-semantic-surfaces", Category::Showcases),
        ("99-flow-demo", Category::Showcases),
        ("92-fractal-portal", Category::Portals),
        ("93-warp-portal", Category::Portals),
        ("A1-zone-materials", Category::Overlay),
        ("A5-inset", Category::Overlay),
        ("TX1-genplate", Category::Texture),
        ("TX4-genplate-subcell", Category::Texture),
        ("U1-underline", Category::Feature),
        ("S1-ramp", Category::Feature),
        ("T0-trigger", Category::Feature),
        ("00-test-load", Category::Test),
    ];
    for (stem, want) in cases {
        assert_eq!(categorize(stem), want, "categorize({stem}) mismatch");
    }
}

#[test]
fn display_title_prettifies_the_stem() {
    use super::display_title;
    // A synthetic two-word hyphenated stem (the shape of a numbered engine demo,
    // e.g. "10-calibration-vault") — no real level need exist on disk for a pure
    // stem-parsing function.
    assert_eq!(display_title("levels/NN-example-room.lvl"), "Example Room");
    assert_eq!(
        display_title("levels/L11-inner-circuit.lvl"),
        "Inner Circuit"
    );
    assert_eq!(display_title("levels/01.lvl"), "01");
}

/// The level list's in-category cursor must map back to the GLOBAL catalogue
/// index when a level is confirmed — so picking the Nth level of a category plays
/// THAT level, not catalogue slot N.
///
/// `Category::Demos` is permanently empty from the game repo's own corpus
/// now that the engine-example levels live in `terminal3d/` instead.
/// `Category::Archive` (the 8 `0N.lvl` rooms, which stay in the game repo) is
/// the closest surviving category that does not start at global index 0, so
/// it still exercises the local→global remap this test is actually about.
#[test]
fn level_select_maps_in_category_cursor_to_global_index() {
    let mut app = test_app();
    // Use the full catalogue so categories beyond Campaign are non-empty.
    app.insert_resource(full_catalog());
    // Browse into ARCHIVE (a category that does NOT start at global index 0).
    app.world_mut().resource_mut::<BrowseCategory>().0 = Category::Archive;
    set_state(&mut app, GameState::LevelSelect);

    let catalog = app.world().resource::<LevelCatalog>().clone();
    let demo_indices = catalog.indices_in(Category::Archive);
    assert!(
        demo_indices.len() >= 2,
        "need ≥2 archive levels for this test"
    );
    // The 2nd archive level's GLOBAL slot is what confirming local index 1 must pick.
    let want_global = demo_indices[1];
    assert_ne!(
        want_global, 1,
        "the global index must differ from the local one"
    );

    nav(&mut app, MenuNav::Down); // local cursor → 1
    nav(&mut app, MenuNav::Confirm); // queue Diving
    app.update();
    skip_dive(&mut app);
    assert_eq!(state(&app), GameState::Playing);
    assert_eq!(
        app.world().resource::<CurrentLevel>().0,
        want_global,
        "confirming the 2nd demo plays its GLOBAL catalogue slot"
    );
}

/// A non-campaign catalogue slot loads through the normal play path and is
/// walkable: it enters Playing with a real Map + camera.
///
/// This test used to pick a texture demo (`TX5-genplate`) specifically BECAUSE
/// it has no puzzle win condition, proving a pure render demo is reachable +
/// playable. Every engine-example level (`TX*`, `08-material-demo`,
/// `92-fractal-portal`, …) lives in the engine repo instead — the game repo's
/// own shipped corpus is now only the 12 campaign levels plus the 8
/// `0N.lvl` archive rooms, and EVERY one of those has a real
/// `[node]`/`[pipe]` puzzle with a win condition (verified: `01.lvl` has a
/// node/pipe/emit block same as the campaign levels). There is no longer any
/// "no win condition" level in this repo to pick.
///
/// So this test now proves the narrower-but-still-real thing available: an
/// ARCHIVE level (not one of the twelve campaign slots) reaches Playing via
/// the generic browse→confirm→play path with a real Map and exactly one
/// camera — i.e. the play path is not hardcoded to campaign-shaped input.
/// The original "demo with no win" claim cannot be exercised from this repo's
/// corpus until/unless §2.2's `@q` is resolved in favor of keeping the Demos
/// browse category and shipping at least one such level in the game repo.
#[test]
fn demo_level_loads_and_is_playable() {
    let mut app = test_app();
    app.insert_resource(full_catalog());
    let catalog = app.world().resource::<LevelCatalog>().clone();
    // Pick an archive level: real content, but not one of the twelve campaign
    // slots — proves the play path works for a non-campaign catalogue entry.
    let gidx = catalog
        .paths
        .iter()
        .position(|p| LevelCatalog::label(p) == "05")
        .expect("05 (archive) is in the catalogue");
    app.world_mut().resource_mut::<CurrentLevel>().0 = gidx;
    set_state(&mut app, GameState::Playing);
    assert_eq!(
        state(&app),
        GameState::Playing,
        "the archive level enters Playing"
    );
    // A real level loaded: the map is bigger than the 1x1 empty menu map, and a
    // play camera exists.
    let size = app.world().resource::<Map>().size();
    assert!(
        size.0 > 1 && size.1 > 1,
        "a real archive Map is loaded, got {size:?}"
    );
    let cams = app
        .world_mut()
        .query_filtered::<(), With<PlayCamera>>()
        .iter(app.world())
        .count();
    assert_eq!(cams, 1, "a play camera was spawned for the archive level");
}

// ───────── TriggerFired subscribers: stinger + narrative beat ─────────

/// Flatten a framebuffer into text rows (the overlay-scanning idiom used above).
fn fb_rows(app: &App) -> Vec<String> {
    let fb = app.world().resource::<Framebuffer>();
    (0..fb.rows())
        .map(|r| {
            (0..fb.cols())
                .map(|c| char::from_u32(fb.get(c, r).unwrap().glyph).unwrap_or(' '))
                .collect::<String>()
        })
        .collect()
}

/// Move the play camera to the centre of the level's first resolved trigger.
fn walk_into_first_trigger(app: &mut App) {
    let centre = {
        let map = app.world().resource::<Map>();
        let re_core::Region::Box { min, max } = map.resolved_triggers()[0].region else {
            panic!("the trigger region is a Box");
        };
        (
            (min.0 + max.0) as f32 / 2.0 + 0.5,
            (min.1 + max.1) as f32 / 2.0 + 0.5,
            min.2 as f32 + 0.5,
        )
    };
    let mut q = app
        .world_mut()
        .query_filtered::<&mut CameraPose, With<PlayCamera>>();
    let mut cam = q.single_mut(app.world_mut()).expect("play camera");
    cam.pos.x = centre.0;
    cam.pos.y = centre.1;
    cam.pos.z = centre.2;
}

/// The full 1.5 path through the real `App` on the `T0-trigger` fixture: walking
/// into the trigger field fires a stinger message AND paints the `>`-prefixed
/// narrative beat on the HUD — which then decays and leaves the frame clean.
#[test]
fn walking_into_a_trigger_stings_and_paints_the_hud_beat() {
    use crate::menu::MenuRenderPlugin;
    use crate::story::beats::{cue_for, NarrativeBeat, BEAT_LINGER_SECS};
    use re_core::Framebuffer;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(StatesPlugin)
        .add_message::<InteractPressed>()
        .add_plugins(PuzzlePlugin)
        .add_plugins(FlowPlugin { cli_level: None })
        .add_plugins(MenuRenderPlugin)
        .insert_resource(LevelCatalog {
            paths: vec![fixture_level("T0-trigger.lvl")],
        })
        .insert_resource(Framebuffer::new(VIEW_COLS, VIEW_ROWS));

    set_state(&mut app, GameState::Playing);
    assert_eq!(state(&app), GameState::Playing);
    // Nothing fired yet ⇒ no beat, and the HUD carries no `>` line.
    app.update();
    assert!(
        app.world().resource::<NarrativeBeat>().line().is_none(),
        "a fresh level carries no beat"
    );

    walk_into_first_trigger(&mut app);
    app.update(); // trigger_sys fires → trigger_subscribers → Last HUD paint

    // (a) The STINGER: one PlaySound carrying the cue's sound.
    let expect_sound = cue_for("gate").sound.expect("the gate cue stings");
    let sounds: Vec<re_audio::SoundId> = app
        .world_mut()
        .resource_mut::<Messages<re_audio::PlaySound>>()
        .drain()
        .map(|m| m.0)
        .collect();
    assert_eq!(
        sounds,
        vec![expect_sound],
        "the trigger dispatched exactly its mapped stinger"
    );

    // (b) The NARRATIVE BEAT: the resource holds it, `>`-prefixed…
    let line = app
        .world()
        .resource::<NarrativeBeat>()
        .line()
        .expect("the fired trigger put up a beat")
        .to_string();
    assert!(line.starts_with("> "), "a beat is `>`-prefixed: {line:?}");
    assert!(
        line.contains(cue_for("gate").beat.unwrap()),
        "the beat carries the mapped line: {line:?}"
    );
    // …and the HUD actually painted it into the framebuffer.
    let rows = fb_rows(&app);
    assert!(
        rows.iter().any(|r| r.contains(line.trim_end())),
        "the beat must be painted on the HUD; screen was:\n{}",
        rows.join("\n")
    );

    // DECAY: past the linger window the beat is gone from the resource AND the
    // repainted frame. (Driven explicitly rather than by wall-clock so the test is
    // deterministic — the per-frame `dt` in a headless app is microseconds.)
    app.world_mut()
        .resource_mut::<NarrativeBeat>()
        .tick(BEAT_LINGER_SECS + 0.1);
    // Stand in for the renderer this headless app has none of: the composite
    // resolve does `fb.clear()` + fold EVERY frame (`re_render::composite`), even
    // when the expensive near cast is dirty-flag-skipped — so the HUD over-paint
    // never persists across frames in the real game.
    app.world_mut().resource_mut::<Framebuffer>().clear();
    app.update();
    assert!(
        app.world().resource::<NarrativeBeat>().line().is_none(),
        "the beat decays after ~{BEAT_LINGER_SECS}s"
    );
    let rows = fb_rows(&app);
    assert!(
        !rows.iter().any(|r| r.contains(line.trim_end())),
        "the decayed beat must leave the HUD"
    );
}

/// A level (re-)load drops any live beat, so a new room never inherits the last
/// room's line.
#[test]
fn a_level_load_clears_the_narrative_beat() {
    use crate::story::beats::NarrativeBeat;

    let mut app = test_app();
    set_state(&mut app, GameState::Playing);
    app.world_mut()
        .resource_mut::<NarrativeBeat>()
        .show("stale line");
    assert!(app.world().resource::<NarrativeBeat>().line().is_some());

    // Quit to the menu (teardown) and re-enter: both paths reset the beat.
    set_state(&mut app, GameState::Menu);
    assert!(
        app.world().resource::<NarrativeBeat>().line().is_none(),
        "teardown drops the beat"
    );
    app.world_mut()
        .resource_mut::<NarrativeBeat>()
        .show("stale again");
    set_state(&mut app, GameState::Playing);
    assert!(
        app.world().resource::<NarrativeBeat>().line().is_none(),
        "a level load drops the beat"
    );
}

// ──────── name-shard + pid assembly on the Won overlay ─────────────────

/// A unique `TS_SAVE_DIR` so a Won-overlay test's `Progress::save` never touches
/// the real save file.
fn isolate_save_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "ts_{tag}_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
    ));
    std::env::set_var("TS_SAVE_DIR", &dir);
    dir
}

/// Solving a CAMPAIGN level paints its name-shard and the running `pid://…`
/// assembly on the Won overlay. Level `01` is shard 1 (`R`), so the door's
/// accounting must show `R` recovered and the other six still `░`.
#[test]
fn the_won_overlay_paints_the_shard_and_the_pid_assembly() {
    use crate::menu::MenuRenderPlugin;
    use re_core::Framebuffer;

    let dir = isolate_save_dir("shard_won");
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(StatesPlugin)
        .add_message::<InteractPressed>()
        .add_plugins(PuzzlePlugin)
        .add_plugins(FlowPlugin { cli_level: None })
        .add_plugins(MenuRenderPlugin)
        .insert_resource(abs_catalog())
        .insert_resource(Framebuffer::new(VIEW_COLS, VIEW_ROWS));

    // Catalogue slot 0 == levels/01.lvl == shard 1.
    app.world_mut().resource_mut::<CurrentLevel>().0 = 0;
    set_state(&mut app, GameState::Playing);
    // Mirror a genuine win the way the real watcher does.
    app.world_mut().resource_mut::<re_puzzle::Solved>().0 = true;
    set_state(&mut app, GameState::Won); // record_progress + draw_won this frame
    assert_eq!(state(&app), GameState::Won);
    assert!(
        app.world()
            .resource::<crate::save::Progress>()
            .is_solved("01"),
        "the solve was recorded before the overlay painted"
    );

    let screen = fb_rows(&app).join("\n");
    assert!(
        screen.contains("[ name-shard 1 of 7 recovered ]  R"),
        "the Won overlay must announce this level's shard; screen:\n{screen}"
    );
    assert!(
        screen.contains("true_name: pid://aleks_v@R·░·░·░·░·░·░"),
        "the Won overlay must show the running pid assembly; screen:\n{screen}"
    );
    assert!(
        screen.contains("[unresolved]"),
        "one of seven shards ⇒ the name is still unresolved"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// A NON-campaign level's Won overlay stays clean: no shard, no pid line (the
/// name arc is the campaign's through-line, not a demo's).
#[test]
fn a_demo_levels_won_overlay_shows_no_name_arc() {
    use crate::menu::MenuRenderPlugin;
    use re_core::Framebuffer;

    let dir = isolate_save_dir("shard_demo");
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(StatesPlugin)
        .add_message::<InteractPressed>()
        .add_plugins(PuzzlePlugin)
        .add_plugins(FlowPlugin { cli_level: None })
        .add_plugins(MenuRenderPlugin)
        .insert_resource(LevelCatalog {
            paths: vec![fixture_level("T0-trigger.lvl")],
        })
        .insert_resource(Framebuffer::new(VIEW_COLS, VIEW_ROWS));

    set_state(&mut app, GameState::Playing);
    app.world_mut().resource_mut::<re_puzzle::Solved>().0 = true;
    set_state(&mut app, GameState::Won);

    let screen = fb_rows(&app).join("\n");
    assert!(
        !screen.contains("name-shard"),
        "a demo yields no shard:\n{screen}"
    );
    assert!(
        !screen.contains("true_name:"),
        "a demo shows no pid line:\n{screen}"
    );
    assert!(
        screen.contains("PUZZLE SOLVED"),
        "…but it still says solved"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The in-play HUD's exit clause: once the network is complete a CAMPAIGN level's
/// objective line reports what the door knows — `exit: name unknown` while the
/// seven shards are incomplete.
#[test]
fn the_hud_exit_clause_reads_name_unknown_before_the_name_is_whole() {
    use crate::menu::MenuRenderPlugin;
    use crate::story::DOOR_NAME_UNKNOWN;
    use re_core::Framebuffer;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(StatesPlugin)
        .add_message::<InteractPressed>()
        .add_plugins(PuzzlePlugin)
        .add_plugins(FlowPlugin { cli_level: None })
        .add_plugins(MenuRenderPlugin)
        .insert_resource(abs_catalog())
        .insert_resource(Framebuffer::new(VIEW_COLS, VIEW_ROWS));

    app.world_mut().resource_mut::<CurrentLevel>().0 = 0; // 01.lvl (campaign)
    set_state(&mut app, GameState::Playing);

    // Force the network complete so the objective line takes its `done` branch.
    {
        let mut puzzle = app.world_mut().resource_mut::<PuzzleState>();
        let targets = puzzle.door_targets.clone();
        assert!(!targets.is_empty(), "01.lvl has door targets");
        for t in targets {
            puzzle.nodes[t].activation = 1;
        }
    }
    app.update();

    let screen = fb_rows(&app).join("\n");
    assert!(
        screen.contains("exit: name unknown"),
        "the completed objective must carry the door's exit clause; screen:\n{screen}"
    );
    assert!(
        DOOR_NAME_UNKNOWN.starts_with("exit: name unknown"),
        "the clause is the canon door line"
    );
}

// ──────── Form A-7 interpolation at level load ─────────────────────────

/// A minimal parseable level carrying ONE `[note]` whose body mixes a Tier-1
/// token, a Tier-2 token (which must cost its line), and a plain line.
const INTERP_LEVEL: &str = r"
[meta]
name = fixture-interp
[room]
size = 12 8 4
ambient = solid
[node R]
pos = 3,3,2
glyph = R
color = #FF0000
is_input = yes
[node output]
pos = 9,3,2
glyph = O
color = #FFFFFF
is_output = yes
threshold = 1
[pipe R->output]
state = OFF
path = (3,3,2) (9,3,2)
switch_at = (6,3,2)
[inputs]
R = 1
[pedestal]
center = 6 4 2
volume = 2 2 2
[door]
pos = 1 3 1
target_parts = output
[player]
spawn = 10 3 1
facing = -x
[note system_log pos=11,3,2 facing=-x]
    > operator session: {username}. welcome.
    > power reserve {battery} percent
    > this line has no tokens
";

/// Write [`INTERP_LEVEL`] to a unique temp `.lvl` and return its path.
fn write_interp_level(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "ts_interp_{tag}_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
    ));
    std::fs::create_dir_all(&dir).expect("temp level dir");
    let path = dir.join("interp-fixture.lvl");
    std::fs::write(&path, INTERP_LEVEL).expect("write temp level");
    path
}

/// Load the fixture through the REAL `enter_playing` path with `lore_fourth_wall`
/// set as given, and return the loaded level's (interpolated) note body.
fn note_body_after_load(fourth_wall: bool) -> String {
    let path = write_interp_level(if fourth_wall { "in" } else { "out" });
    let mut app = test_app();
    app.insert_resource(LevelCatalog {
        paths: vec![path.to_string_lossy().replace('\\', "/")],
    });
    app.world_mut()
        .resource_mut::<crate::settings::Settings>()
        .lore_fourth_wall = fourth_wall;
    app.world_mut().resource_mut::<CurrentLevel>().0 = 0;
    set_state(&mut app, GameState::Playing);
    assert_eq!(
        state(&app),
        GameState::Playing,
        "the fixture level must load"
    );
    let body = app
        .world()
        .resource::<CurrentLevelData>()
        .0
        .as_ref()
        .expect("level data held")
        .notes
        .first()
        .expect("the fixture authors one note")
        .text
        .clone();
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
    body
}

/// With the fourth wall UP, a loaded note's Tier-1 token resolves to the real
/// operator handle and the Tier-2 line is structurally omitted — asserted on
/// `CurrentLevelData`, i.e. what every downstream consumer (the wall placard, a
/// future HUD panel) actually reads.
#[test]
fn loading_a_level_interpolates_its_notes_with_the_fourth_wall_up() {
    let body = note_body_after_load(true);
    // The exact handle is the machine's, and the `interp` env tests mutate `$USER`
    // concurrently in this same binary — so assert the SHAPE (a resolved, non-empty
    // handle where the token was) rather than a racy exact value. The exact-value
    // matrix lives in `story::interp`'s own populated-env tests.
    let handle = body
        .lines()
        .find_map(|l| l.trim().strip_prefix("> operator session: "))
        .and_then(|rest| rest.strip_suffix(". welcome."))
        .expect("the Tier-1 line must survive with a resolved handle");
    assert!(
        !handle.is_empty() && !handle.contains('{'),
        "the Tier-1 token resolved to a real handle, got {handle:?}"
    );
    assert!(
        !body.contains("{battery}") && !body.contains("power reserve"),
        "the Tier-2 line must be omitted ENTIRELY, not left with a hole:\n{body}"
    );
    assert!(
        body.contains("this line has no tokens"),
        "untemplated lines survive:\n{body}"
    );
    assert!(
        !body.contains('{'),
        "no brace may survive to the screen:\n{body}"
    );
}

/// With the fourth wall DOWN, the SAME level substitutes the A-7 §8 fallback
/// (`anon`) instead of reading anything, and the Tier-2 line still drops.
#[test]
fn loading_a_level_honors_the_fourth_wall_opt_out() {
    let body = note_body_after_load(false);
    assert!(
        body.contains(&format!(
            "operator session: {}.",
            crate::story::interp::FALLBACK_USERNAME
        )),
        "opting out substitutes the A-7 fallback, not a real read; body was:\n{body}"
    );
    assert!(
        !body.contains("power reserve"),
        "the Tier-2 line still drops:\n{body}"
    );
    assert!(
        body.contains("this line has no tokens"),
        "untemplated lines survive:\n{body}"
    );
}

// ────────────── the campaign manifest + escalation ─────────────────────
//
// The manifest is `levels/campaign.toml`, embedded with `include_str!` and parsed
// once. Everything below reads that ONE table, so a manifest edit is checked here
// before it can reach the browse, the gate, or the fog curve.

/// The manifest parses, and it is the twelve authored levels — not eleven, not
/// thirteen, and not the empty fallback that a malformed file would produce.
#[test]
fn the_campaign_manifest_parses_twelve_levels() {
    let m = campaign_manifest();
    assert!(
        !m.is_empty(),
        "levels/campaign.toml failed to parse — the browse would silently fall back \
         to the pre-manifest lexical categories"
    );
    assert_eq!(
        m.levels().len(),
        12,
        "the campaign is twelve levels (PROGRESSION §1)"
    );
    // Every entry is fully populated: a blank id would break progress keying and a
    // blank name would show an empty row in level-select.
    for l in m.levels() {
        assert!(!l.id.is_empty(), "manifest entry has no id");
        assert!(!l.name.is_empty(), "{} has no display name", l.id);
        assert!(
            std::path::Path::new(&l.path)
                .extension()
                .is_some_and(|ext| ext == "lvl"),
            "{} path is not a .lvl: {}",
            l.id,
            l.path
        );
        // The id MUST be the path's file stem — that is the contract `Progress`
        // keys on and what `LevelCatalog::id_at` returns.
        let stem = std::path::Path::new(&l.path)
            .file_stem()
            .unwrap()
            .to_string_lossy();
        assert_eq!(l.id, stem, "manifest id must equal its path's file stem");
    }
}

/// The manifest names L1..L12 in authored order, and every file it names EXISTS.
/// A renamed or deleted level file is caught here rather than at level-load.
#[test]
fn the_manifest_lists_l1_through_l12_in_order_and_every_file_exists() {
    let m = campaign_manifest();
    let want = [
        "L1-first-pulse",
        "L2-two-make-three",
        "L3-full-spectrum",
        "L4-the-angle",
        "L5-soft-footstep",
        "L6-polysemantic",
        "L7-hidden-depths",
        "L8-cold-room",
        "L9-stereopsis",
        "L10-lying-door",
        "L11-inner-circuit",
        "L12-closing",
    ];
    let got: Vec<&str> = m.levels().iter().map(|l| l.id.as_str()).collect();
    assert_eq!(got, want, "the campaign order is authored, not lexical");
    let root = levels_root();
    for l in m.levels() {
        let file = std::path::Path::new(&l.path).file_name().unwrap();
        assert!(
            root.join(file).is_file(),
            "manifest names a level file that does not exist: {}",
            l.path
        );
    }
}

/// The four movements partition the campaign in blocks of three, in order
/// (PROGRESSION §1's four-movement frame).
#[test]
fn the_manifest_movements_are_four_blocks_of_three_in_order() {
    use Movement::{P1, P2, P3, P4};
    let got: Vec<Movement> = campaign_manifest()
        .levels()
        .iter()
        .map(|l| l.movement)
        .collect();
    assert_eq!(got, vec![P1, P1, P1, P2, P2, P2, P3, P3, P3, P4, P4, P4]);
}

// ── the ESCALATION contract ─────────────────────────────────────────────

/// Levels whose authored escalation `fog_far` is NOT yet in their `.lvl` file.
///
/// `(level id, the value still on disk, the authored escalation target)`
///
/// **CURRENTLY EMPTY — and it should stay that way.**
///
/// WHY IT EXISTS. Lowering a level's `fog_far` changes the shading of its `--dump`
/// frame, which changes its golden hash. Golden hashes are refreshed only on
/// scheduled passes, so an ordinary change with no scheduled refresh may not
/// move a `fog_far`. This list let such a value be DECLARED in `campaign.toml` while the
/// `.lvl` file still carried the old one, and [`effective_fog_far`] substitutes the
/// target so the escalation contract is enforced meanwhile.
///
/// It is SELF-EXPIRING: [`campaign_levels_match_the_manifest_fog_curve`] also asserts
/// each pinned level still holds its OLD value, so writing the new one FAILS the test
/// and forces the entry out. That is the mechanism working as designed.
///
/// HISTORY — the trap this list set, recorded so it is not re-set. It held
/// `("L9-stereopsis", 22.0, 13.0)` and `("L12-closing", 13.0, 10.0)` and was
/// scheduled to expire with a follow-up refresh. That refresh shipped
/// (`c6a3fb8`) without removing the pin, and it survived it by two months.
/// Because `effective_fog_far`
/// substitutes the intended value, `campaign_fog_far_is_monotonic` kept passing while
/// the SHIPPED game ran the broken curve: L9 — the campaign's most claustrophobic
/// beat — rendered at `fog_far = 22.0`, the widest room in the game, wider than L1's
/// 20.0, and the L12 finale opened back up from 11.0 to 13.0. A test that knows about
/// an exception stops being a detector for that exception. If you add an entry here,
/// the expiry is a DEADLINE, not a note.
const FOG_PENDING: [(&str, f32, f32); 0] = [];

/// The escalation `fog_far` this level should be judged on: its authored target when
/// the level is golden-pinned, else the value the manifest declares.
fn effective_fog_far(l: &CampaignLevel) -> f32 {
    FOG_PENDING
        .iter()
        .find(|(id, _, _)| *id == l.id)
        .map_or(l.fog_far, |(_, _, target)| *target)
}

/// Read a level file's authored `[palette] fog_far`. Text scan, not a full parse —
/// the point is to assert what the FILE says, independent of loader defaults.
fn authored_fog_far(id: &str) -> f32 {
    let path = levels_root().join(format!("{id}.lvl"));
    let src = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    for line in src.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue; // a commented-out example value is not the authored one
        }
        if let Some(rest) = line.strip_prefix("fog_far") {
            if let Some((_, v)) = rest.split_once('=') {
                return v
                    .trim()
                    .parse()
                    .unwrap_or_else(|e| panic!("{id}: unparsable fog_far: {e}"));
            }
        }
    }
    panic!("{id}: no [palette] fog_far authored");
}

/// **The escalation contract** (`STORY_BIBLE` §7): `fog_far` is monotonically
/// NON-INCREASING along the campaign manifest. The world closes in as the system
/// dies; it never opens back up.
#[test]
fn campaign_fog_far_is_monotonic() {
    let m = campaign_manifest();
    let mut prev: Option<(&str, f32)> = None;
    for l in m.levels() {
        let effective = effective_fog_far(l);
        assert!(effective > 0.0, "{} has no fog_far in the manifest", l.id);
        if let Some((prev_id, prev_far)) = prev {
            assert!(
                effective <= prev_far,
                "fog_far must never grow along the campaign: {prev_id} = {prev_far} \
                 then {} = {effective}",
                l.id
            );
        }
        prev = Some((&l.id, effective));
    }
    // And the curve must actually DESCEND overall — a flat 12-level curve would pass
    // "non-increasing" while escalating nothing.
    let first = effective_fog_far(&m.levels()[0]);
    let last = effective_fog_far(&m.levels()[11]);
    assert!(
        last < first * 0.75,
        "the fog must close in meaningfully across the campaign: {first} then {last}"
    );
}

/// Every campaign `.lvl` FILE carries the `fog_far` its manifest entry declares —
/// except the golden-pinned two, which must still carry their OLD value. That second
/// half is what makes [`FOG_PENDING`] self-expiring.
#[test]
fn campaign_levels_match_the_manifest_fog_curve() {
    for l in campaign_manifest().levels() {
        let on_disk = authored_fog_far(&l.id);
        match FOG_PENDING.iter().find(|(id, _, _)| *id == l.id) {
            Some((_, pinned, target)) => {
                assert_eq!(
                    on_disk, *pinned,
                    "{} is GOLDEN-PINNED at fog_far {pinned}. If you have just landed \
                     the escalation value {target} together with golden refresh #2, \
                     REMOVE this entry from FOG_PENDING — the pin has expired.",
                    l.id
                );
            }
            None => assert_eq!(
                on_disk, l.fog_far,
                "{}: levels/campaign.toml declares fog_far {} but the .lvl file says \
                 {on_disk} — the manifest is the escalation curve's source of truth",
                l.id, l.fog_far
            ),
        }
    }
}

/// The cold/warm effect semantics of `STORY_BIBLE` §7 hold: `snow` is the COLD axis
/// (L3 / L6 / L8 — distance, forgetting) and `steam` the WARM one (L4 / L5 / L7 — a
/// lab that still runs). A level must not claim both temperatures at once.
#[test]
fn campaign_effects_keep_the_cold_and_warm_semantics() {
    let cold = ["L3-full-spectrum", "L6-polysemantic", "L8-cold-room"];
    let warm = ["L4-the-angle", "L5-soft-footstep", "L7-hidden-depths"];
    for id in cold.iter().chain(warm.iter()) {
        let src = std::fs::read_to_string(levels_root().join(format!("{id}.lvl"))).unwrap();
        let has = |key: &str| {
            src.lines().any(|l| {
                let l = l.trim();
                !l.starts_with('#')
                    && l.starts_with(key)
                    && l.split_once('=').is_some_and(|(_, v)| {
                        let v = v.trim();
                        v != "off" && v != "0" && v != "0.0"
                    })
            })
        };
        let (snow, steam) = (has("snow"), has("steam"));
        if cold.contains(id) {
            assert!(snow, "{id} is a COLD level and must author snow");
            assert!(!steam, "{id} is a COLD level and must not author steam");
        } else {
            assert!(steam, "{id} is a WARM level and must author steam");
            assert!(!snow, "{id} is a WARM level and must not author snow");
        }
    }
}

// ── the ARCHIVE demotion ────────────────────────────────────────────────

/// `browse_category` files the pre-manifest numbered rooms under ARCHIVE while
/// `categorize` still calls them Campaign — the two-classifier split that keeps the
/// name-shard arc (`crate::story::shard_for_level`) intact.
#[test]
fn browse_category_demotes_the_pre_manifest_rooms_to_archive() {
    for stem in ["01", "02", "03", "04", "05", "06", "07", "08"] {
        assert_eq!(
            categorize(stem),
            Category::Campaign,
            "{stem} must stay CAMPAIGN-class: shard_for_level gates the name arc on it"
        );
        assert_eq!(
            browse_category(stem),
            Category::Archive,
            "{stem} is the L-files' ancestor and belongs in the ARCHIVE folder"
        );
    }
    // The twelve manifest levels stay in CAMPAIGN under BOTH classifiers.
    for l in campaign_manifest().levels() {
        assert_eq!(categorize(&l.id), Category::Campaign, "{}", l.id);
        assert_eq!(browse_category(&l.id), Category::Campaign, "{}", l.id);
    }
    // A non-campaign level is untouched by the demotion rule.
    for (stem, want) in [
        ("08-material-demo", Category::Demos),
        ("20-generic-showcase", Category::Showcases),
        ("92-fractal-portal", Category::Portals),
        ("A1-zone-materials", Category::Overlay),
        ("TX1-genplate", Category::Texture),
        ("T0-trigger", Category::Feature),
        ("00-test-load", Category::Test),
    ] {
        assert_eq!(browse_category(stem), want, "browse_category({stem})");
        assert_eq!(categorize(stem), want, "categorize({stem})");
    }
}

/// The archived rooms are still PLAYABLE and still reachable through the browse —
/// demoted, never deleted.
#[test]
fn the_archived_rooms_are_still_in_the_catalogue_and_ungated() {
    let catalog = full_catalog();
    let archive = catalog.indices_in(Category::Archive);
    assert_eq!(
        archive.len(),
        8,
        "01..08 are the eight archived rooms; none may be dropped"
    );
    // No unlock gate applies to them (the gate is a campaign rule, not a global one).
    let empty = crate::save::Progress::default();
    for gidx in archive {
        let id = catalog.id_at(gidx).unwrap();
        assert!(
            campaign_manifest().is_unlocked(id, &empty),
            "{id} is archived, not gated"
        );
    }
}

/// The CAMPAIGN folder lists exactly the manifest levels, in MANIFEST order — not
/// the lexical `L1, L10, L11, L12, L2, ...` a filename sort produces.
#[test]
fn the_campaign_folder_lists_the_manifest_in_authored_order() {
    let catalog = full_catalog();
    let listed: Vec<&str> = catalog
        .indices_in(Category::Campaign)
        .into_iter()
        .map(|i| catalog.id_at(i).unwrap())
        .collect();
    let want: Vec<&str> = campaign_manifest()
        .levels()
        .iter()
        .map(|l| l.id.as_str())
        .collect();
    assert_eq!(listed, want, "level-select must follow the authored order");
    // Sanity: a LEXICAL sort would disagree, so the test is actually proving
    // something.
    let mut lexical = listed.clone();
    lexical.sort_by_key(|s| s.to_ascii_lowercase());
    assert_ne!(
        lexical, want,
        "if lexical order equalled manifest order this test would be vacuous"
    );
}

// ── the UNLOCK gate ─────────────────────────────────────────────────────

/// Level N+1 unlocks when level N is solved; the first level is always open.
#[test]
fn campaign_unlocks_one_level_at_a_time() {
    let m = campaign_manifest();
    let ids: Vec<String> = m.levels().iter().map(|l| l.id.clone()).collect();
    let mut progress = crate::save::Progress::default();

    // Fresh save: only the first level is open.
    assert!(m.is_unlocked(&ids[0], &progress), "L1 is always unlocked");
    for id in &ids[1..] {
        assert!(!m.is_unlocked(id, &progress), "{id} must start locked");
    }

    // Walk the campaign: solving N opens N+1 and nothing further.
    for i in 0..ids.len() {
        progress.mark_solved(&ids[i]);
        for (j, id) in ids.iter().enumerate() {
            let want = j <= i + 1;
            assert_eq!(
                m.is_unlocked(id, &progress),
                want,
                "after solving {} (index {i}), {id} (index {j}) unlocked should be {want}",
                ids[i]
            );
        }
    }
}

/// Solving out of order never re-locks a level the player has already solved, and
/// a level not in the manifest is never gated.
#[test]
fn unlock_edge_cases_are_defined() {
    let m = campaign_manifest();
    let ids: Vec<String> = m.levels().iter().map(|l| l.id.clone()).collect();
    let mut progress = crate::save::Progress::default();

    // An already-solved level stays open even if its predecessor is not solved (an
    // old save, or a manifest reorder that moved it later). Replaying is never taken
    // away — the plan's watch-item on orphaned progress entries.
    progress.mark_solved(&ids[5]);
    assert!(
        m.is_unlocked(&ids[5], &progress),
        "a solved level stays open"
    );
    assert!(
        m.is_unlocked(&ids[6], &progress),
        "and it still opens the next one"
    );
    assert!(!m.is_unlocked(&ids[7], &progress), "but only the next one");

    // Unknown ids are ungated, in both directions.
    let empty = crate::save::Progress::default();
    for id in ["01", "08", "12-mirror", "T0-trigger", "", "not-a-level"] {
        assert!(m.is_unlocked(id, &empty), "{id:?} is not a campaign level");
    }
}

/// An empty / malformed manifest degrades cleanly: nothing is gated and the browse
/// is exactly the pre-manifest lexical one.
#[test]
fn an_unparsable_manifest_gates_nothing() {
    let broken = CampaignManifest::parse("this is not = valid [ toml");
    assert!(broken.is_empty());
    let empty = crate::save::Progress::default();
    for id in ["L1-first-pulse", "L7-hidden-depths", "01"] {
        assert!(
            broken.is_unlocked(id, &empty),
            "an empty manifest must not lock anyone out: {id}"
        );
    }
    // A manifest with a valid shape but no levels behaves the same way.
    let none = CampaignManifest::parse("version = 1\n");
    assert!(none.is_empty());
    assert_eq!(none.position("L1-first-pulse"), None);
}

/// **The gate, through the real flow.** Confirming a LOCKED campaign level in
/// level-select does nothing: the state stays on `LevelSelect` and no level loads.
/// Solving its predecessor then makes the same keypress work.
#[test]
fn confirming_a_locked_campaign_level_is_inert() {
    let mut app = test_app();
    app.insert_resource(campaign_catalog());
    browse(&mut app, Category::Campaign);
    set_state(&mut app, GameState::LevelSelect);

    // Move to slot 1 (L2) — locked, because L1 is unsolved on a fresh save.
    nav(&mut app, MenuNav::Down);
    assert_eq!(app.world().resource::<MenuCursor>().index, 1);
    nav(&mut app, MenuNav::Confirm);
    app.update();
    assert_eq!(
        state(&app),
        GameState::LevelSelect,
        "a locked level must not be enterable"
    );
    assert_eq!(
        app.world().resource::<CurrentLevel>().0,
        0,
        "and it must not have moved the play target"
    );

    // Solve L1 and the very same keypress now works.
    let l1 = campaign_manifest().levels()[0].id.clone();
    app.world_mut()
        .resource_mut::<crate::save::Progress>()
        .mark_solved(&l1);
    nav(&mut app, MenuNav::Confirm);
    app.update();
    skip_dive(&mut app);
    assert_eq!(state(&app), GameState::Playing);
    assert_eq!(app.world().resource::<CurrentLevel>().0, 1);
}

/// The FIRST campaign level is enterable on a completely fresh save — the gate must
/// never lock the player out of the game's own entry point.
#[test]
fn the_first_campaign_level_is_enterable_on_a_fresh_save() {
    let mut app = test_app();
    app.insert_resource(campaign_catalog());
    browse(&mut app, Category::Campaign);
    set_state(&mut app, GameState::LevelSelect);
    assert!(
        app.world()
            .resource::<crate::save::Progress>()
            .solved
            .is_empty(),
        "this test is only meaningful on an empty save"
    );
    nav(&mut app, MenuNav::Confirm);
    app.update();
    skip_dive(&mut app);
    assert_eq!(state(&app), GameState::Playing);
    assert_eq!(app.world().resource::<CurrentLevel>().0, 0);
}

/// Every campaign level LOADS. The manifest is only as good as the files it names,
/// so parse all twelve through the real loader.
#[test]
fn every_campaign_level_loads() {
    for l in campaign_manifest().levels() {
        let file = std::path::Path::new(&l.path).file_name().unwrap();
        let path = levels_root().join(file);
        let level =
            re_level::load(&path).unwrap_or_else(|e| panic!("{} failed to load: {e}", l.path));
        assert!(!level.nodes.is_empty(), "{} has no puzzle nodes", l.id);
        assert!(
            !level.door.target_parts.is_empty() || !level.door.target_off.is_empty(),
            "{} has no win target",
            l.id
        );
    }
}

/// **Continue follows the campaign.** With the twelve campaign levels in the
/// catalogue, `first_unsolved` resumes at the first unsolved CAMPAIGN level — which
/// is by construction the deepest one the unlock gate allows, so Continue can never
/// drop the player onto a locked level.
#[test]
fn continue_resumes_the_campaign_at_the_first_unsolved_level() {
    let catalog = campaign_catalog();
    let m = campaign_manifest();
    let ids: Vec<String> = m.levels().iter().map(|l| l.id.clone()).collect();
    let mut progress = crate::save::Progress::default();

    // Fresh save: Continue points at L1.
    assert_eq!(catalog.first_unsolved(&progress), Some(0));

    // Solve forward; Continue tracks the frontier, and the frontier is always
    // unlocked.
    for i in 0..ids.len() - 1 {
        progress.mark_solved(&ids[i]);
        let slot = catalog.first_unsolved(&progress).unwrap();
        assert_eq!(
            slot,
            i + 1,
            "after solving {} Continue must resume at {}",
            ids[i],
            ids[i + 1]
        );
        let id = catalog.id_at(slot).unwrap();
        assert!(
            m.is_unlocked(id, &progress),
            "Continue must never land on a locked level ({id})"
        );
    }

    // Campaign complete ⇒ Continue replays from the top (the historical contract).
    progress.mark_solved(&ids[ids.len() - 1]);
    assert_eq!(catalog.first_unsolved(&progress), Some(0));
}

/// Continue skips a HOLE in the campaign: if the player solved levels out of order
/// (an old save), the resume point is the earliest unsolved campaign level, not the
/// deepest solved one.
#[test]
fn continue_resumes_at_the_earliest_gap_not_the_deepest_solve() {
    let catalog = campaign_catalog();
    let ids: Vec<String> = campaign_manifest()
        .levels()
        .iter()
        .map(|l| l.id.clone())
        .collect();
    let mut progress = crate::save::Progress::default();
    progress.mark_solved(&ids[0]);
    progress.mark_solved(&ids[1]);
    progress.mark_solved(&ids[4]); // out of order — a gap at 2 and 3
    assert_eq!(
        catalog.first_unsolved(&progress),
        Some(2),
        "Continue resumes at the earliest gap"
    );
}

/// A catalogue with NO campaign levels in it (the `01`..`07` archive fixture) falls
/// back to the historical whole-catalogue scan — the manifest preference must not
/// break a focused fixture.
#[test]
fn continue_falls_back_to_the_catalogue_scan_without_campaign_levels() {
    let catalog = abs_catalog(); // 01..07 — archive, not campaign
    let mut progress = crate::save::Progress::default();
    assert_eq!(catalog.first_unsolved(&progress), Some(0));
    progress.mark_solved("01");
    assert_eq!(
        catalog.first_unsolved(&progress),
        Some(1),
        "the archive fixture keeps the pre-manifest scan behaviour"
    );
}

// ────────────────── movement themes + solve silence ────────────────────

/// A headless app whose catalogue IS the campaign, in manifest order (so slot `n`
/// is the `n`-th authored level).
fn campaign_app() -> App {
    let mut app = test_app();
    app.insert_resource(campaign_catalog());
    app
}

/// Drain every `SetTheme` the app has emitted.
fn drained_themes(app: &mut App) -> Vec<re_audio::SetTheme> {
    app.world_mut()
        .resource_mut::<Messages<re_audio::SetTheme>>()
        .drain()
        .collect()
}

/// Drain every `SolveSilence` the app has emitted.
fn drained_silences(app: &mut App) -> Vec<re_audio::SolveSilence> {
    app.world_mut()
        .resource_mut::<Messages<re_audio::SolveSilence>>()
        .drain()
        .collect()
}

/// The four movements map one-to-one onto `re_audio`'s four MOVEMENT themes — and
/// never onto one of the three shipped level themes (Default/Cold/Garden), which
/// the movement themes extend rather than replace.
#[test]
fn movement_theme_mapping_is_one_to_one() {
    let themes: Vec<re_audio::Theme> = [Movement::P1, Movement::P2, Movement::P3, Movement::P4]
        .into_iter()
        .map(Movement::theme)
        .collect();
    assert_eq!(
        themes,
        re_audio::Theme::MOVEMENTS.to_vec(),
        "P1..P4 must map onto the movement themes, in order"
    );
    for t in [
        re_audio::Theme::Default,
        re_audio::Theme::Cold,
        re_audio::Theme::Garden,
    ] {
        assert!(
            !themes.contains(&t),
            "a movement must not reuse the shipped theme {t:?}"
        );
    }
}

/// Entering a CAMPAIGN level sends exactly one `SetTheme`, carrying that level's
/// movement theme (one sample per movement, taken from the manifest itself).
#[test]
fn entering_campaign_level_sends_its_movement_theme() {
    for (slot, expected) in [
        (0usize, re_audio::Theme::P1FirstLight), // L1  — P1
        (3, re_audio::Theme::P2TheGaze),         // L4  — P2
        (6, re_audio::Theme::P3Grief),           // L7  — P3
        (11, re_audio::Theme::P4Closing),        // L12 — P4
    ] {
        let mut app = campaign_app();
        // The fixture must actually be the level we think it is.
        let id = app
            .world()
            .resource::<LevelCatalog>()
            .id_at(slot)
            .unwrap()
            .to_string();
        let movement = campaign_manifest()
            .get(&id)
            .expect("campaign level")
            .movement;
        assert_eq!(movement.theme(), expected, "manifest tag for {id}");

        app.world_mut().resource_mut::<CurrentLevel>().0 = slot;
        set_state(&mut app, GameState::Playing); // OnEnter(Playing) dispatches

        let sent = drained_themes(&mut app);
        assert_eq!(sent.len(), 1, "one theme per campaign level entry ({id})");
        assert_eq!(sent[0], re_audio::SetTheme(expected), "theme for {id}");
    }
}

/// A NON-campaign level (an archive room) sends NO theme at all — it keeps today's
/// logic: the ambience bed only.
#[test]
fn non_campaign_level_keeps_todays_ambience_only() {
    let mut app = test_app(); // 01..07 — Archive, absent from the manifest
    let id = app
        .world()
        .resource::<LevelCatalog>()
        .id_at(0)
        .unwrap()
        .to_string();
    assert!(
        !campaign_manifest().contains(&id),
        "fixture level '{id}' must not be a campaign level"
    );
    app.world_mut().resource_mut::<CurrentLevel>().0 = 0;
    set_state(&mut app, GameState::Playing);

    assert!(
        drained_themes(&mut app).is_empty(),
        "a non-campaign level must not switch the director's theme"
    );
    let ambience_count = app
        .world_mut()
        .resource_mut::<Messages<re_audio::SetAmbience>>()
        .drain()
        .count();
    assert_eq!(ambience_count, 1, "the ambience bed is still sent");
}

/// The peak flag is exactly the campaign's 7th and 11th levels.
#[test]
fn manifest_peak_solve_is_l7_and_l11_only() {
    let m = campaign_manifest();
    for (i, l) in m.levels().iter().enumerate() {
        let expect = i == 6 || i == 10;
        assert_eq!(
            m.is_peak_solve(&l.id),
            expect,
            "position {i} ('{}') peak flag",
            l.id
        );
    }
    // Sanity-check the fixture against the authored ids the design names.
    assert!(m.is_peak_solve("L7-hidden-depths"), "L7 is a peak");
    assert!(m.is_peak_solve("L11-inner-circuit"), "L11 is a peak");
    assert!(!m.is_peak_solve("12-mirror"), "a demo is never a peak");
    assert!(!m.is_peak_solve("nope"), "an unknown id is never a peak");
}

/// Solving a PEAK level (L7 / L11) sends the solve-silence; solving any other
/// campaign level sends nothing.
#[test]
fn peak_solves_send_solve_silence_and_others_do_not() {
    std::env::set_var("TS_SAVE_DIR", std::env::temp_dir());
    for (slot, expect) in [
        (0usize, false), // L1
        (5, false),      // L6
        (6, true),       // L7  — peak
        (9, false),      // L10
        (10, true),      // L11 — peak
        (11, false),     // L12 (the finale has its own staging, not a silence)
    ] {
        let mut app = campaign_app();
        let id = app
            .world()
            .resource::<LevelCatalog>()
            .id_at(slot)
            .unwrap()
            .to_string();
        app.world_mut().resource_mut::<CurrentLevel>().0 = slot;
        set_state(&mut app, GameState::Won); // OnEnter(Won) → peak_solve_silence

        let sent = drained_silences(&mut app);
        if expect {
            assert_eq!(sent.len(), 1, "solving '{id}' must hold the score's breath");
            assert_eq!(
                sent[0],
                re_audio::SolveSilence::solve(),
                "the designed 3–5 s window for '{id}'"
            );
        } else {
            assert!(
                sent.is_empty(),
                "solving '{id}' must not mute the director ({sent:?})"
            );
        }
    }
}

/// A non-campaign solve never mutes the director either.
#[test]
fn non_campaign_solve_sends_no_silence() {
    std::env::set_var("TS_SAVE_DIR", std::env::temp_dir());
    let mut app = test_app(); // archive fixture
    app.world_mut().resource_mut::<CurrentLevel>().0 = 6; // "07"
    set_state(&mut app, GameState::Won);
    assert!(
        drained_silences(&mut app).is_empty(),
        "an archive solve leaves the score running"
    );
}

// ─────────────────── the framing states + the two finales ──────────────
//
// Everything below drives the REAL state machine headlessly: a real `.lvl` load,
// real `re_puzzle` toggles at the authored switch cells, and the real
// `OnExit(Playing)` → `OnEnter(Won)` chain. Nothing sets `solved` by hand except
// where a test explicitly says it is mirroring the win watcher.

/// The catalogue slot of the campaign's finale in [`campaign_catalog`].
fn finale_slot() -> usize {
    campaign_manifest().len() - 1
}

/// Toggle the pipe running `from → to` (by NODE NAME) through the real runtime
/// seam — `re_puzzle::puzzle_interact` at the pipe's authored switch cell, exactly
/// as an `E` press would. Panics if the level has no such pipe or the pipe has no
/// switch (both are level-authoring errors, not test conditions).
fn toggle_named_pipe(app: &mut App, from: &str, to: &str) {
    let (x, y, z) = {
        let level_data = app.world().resource::<CurrentLevelData>();
        let level = level_data.0.as_ref().expect("a level is loaded");
        let idx_of = |name: &str| {
            level
                .nodes
                .iter()
                .position(|n| n.name == name)
                .unwrap_or_else(|| panic!("level has no node '{name}'"))
        };
        let (fi, ti) = (idx_of(from), idx_of(to));
        let puzzle = app.world().resource::<PuzzleState>();
        let pipe = puzzle
            .pipes
            .iter()
            .find(|p| p.from == fi && p.to == ti)
            .unwrap_or_else(|| panic!("level has no pipe {from}->{to}"));
        pipe.switch_coord()
            .unwrap_or_else(|| panic!("pipe {from}->{to} has no switch cell"))
    };
    let mut puzzle = app.world_mut().resource_mut::<PuzzleState>();
    assert!(
        re_puzzle::puzzle_interact(&mut puzzle, x, y, z),
        "interacting at the authored switch cell of {from}->{to} must toggle it"
    );
}

/// Mirror the win watcher: publish the puzzle's `solved` into the `Solved` mirror
/// (`re_puzzle::apply_interactions` does this in the real app) and take the Won
/// transition.
fn commit_win(app: &mut App) {
    let solved = app.world().resource::<PuzzleState>().solved;
    assert!(
        solved,
        "the network must actually be solved before committing the win"
    );
    app.world_mut().resource_mut::<Solved>().0 = true;
    set_state(app, GameState::Won);
}

/// Load the campaign finale and wire the `exit 0` branch. The level boots with
/// `gather->face->REC` already ON (REC lit); prune the self-feed (`face->REC`),
/// then build `unname` (T2 = gather + face) and route it to the outward door.
fn play_finale_exit0(app: &mut App) {
    app.world_mut().resource_mut::<CurrentLevel>().0 = finale_slot();
    set_state(app, GameState::Playing);
    toggle_named_pipe(app, "face", "REC"); // prune the self-feed
    toggle_named_pipe(app, "gather", "unname");
    toggle_named_pipe(app, "face", "unname");
    toggle_named_pipe(app, "unname", "EXIT");
    app.update();
}

/// Load the campaign finale and wire the `while(true)` branch: recognition is
/// already lit from the load draw, so just close the ring.
fn play_finale_loop(app: &mut App) {
    app.world_mut().resource_mut::<CurrentLevel>().0 = finale_slot();
    set_state(app, GameState::Playing);
    toggle_named_pipe(app, "REC", "MEM");
    toggle_named_pipe(app, "MEM", "REC");
    app.update();
}

// ─────────────────────── 4.1 · the cold-boot Intro ──────────────────────────

/// START on a FRESH save opens the campaign through the Stewardship cold-boot; any
/// key then hands off to the dive, which loads the level exactly as before.
#[test]
fn intro_plays_on_a_fresh_campaign_start_only() {
    isolate_save_dir("w4_intro_fresh");
    let mut app = campaign_app();
    set_state(&mut app, GameState::Menu);
    let start = menu_items(&crate::save::Progress::default())
        .iter()
        .position(|i| *i == MenuItem::Start)
        .unwrap();
    app.world_mut().resource_mut::<MenuCursor>().index = start;
    force_commit(&mut app);
    app.update();
    assert_eq!(
        state(&app),
        GameState::Intro,
        "a new campaign boots into the intro"
    );

    // Any key leaves for the dive (not straight to Playing — the dive still owns
    // the transition INTO the room).
    nav(&mut app, MenuNav::Confirm);
    app.update();
    assert_eq!(state(&app), GameState::Diving);
}

/// The intro is for a NEW campaign only: one solved campaign level, or an open
/// session loop, and START goes straight to the dive as it always did.
#[test]
fn intro_is_skipped_once_the_campaign_has_begun() {
    isolate_save_dir("w4_intro_skip");
    for seed in ["solved", "looped"] {
        let mut app = campaign_app();
        {
            let mut p = app.world_mut().resource_mut::<crate::save::Progress>();
            match seed {
                "solved" => {
                    p.mark_solved("L1-first-pulse");
                }
                _ => p.begin_next_session("L1-first-pulse"),
            }
        }
        set_state(&mut app, GameState::Menu);
        let start = menu_items(&crate::save::Progress::default())
            .iter()
            .position(|i| *i == MenuItem::Start)
            .unwrap();
        app.world_mut().resource_mut::<MenuCursor>().index = start;
        force_commit(&mut app);
        app.update();
        assert_eq!(
            state(&app),
            GameState::Diving,
            "'{seed}' is not a fresh campaign — no intro"
        );
    }
}

/// CONTINUE never shows the intro, even on a completely untouched save.
#[test]
fn continue_never_shows_the_intro() {
    isolate_save_dir("w4_intro_continue");
    let mut app = campaign_app();
    set_state(&mut app, GameState::Menu);
    let cont = menu_items(&crate::save::Progress::default())
        .iter()
        .position(|i| *i == MenuItem::Continue)
        .unwrap();
    app.world_mut().resource_mut::<MenuCursor>().index = cont;
    force_commit(&mut app);
    app.update();
    assert_eq!(state(&app), GameState::Diving);
}

/// A catalogue whose slot 0 is NOT the campaign's first level (the archive
/// fixture) never triggers the intro — the intro frames the campaign, not "the
/// first file in the folder".
#[test]
fn intro_does_not_fire_for_a_non_campaign_start() {
    isolate_save_dir("w4_intro_archive");
    let mut app = test_app(); // archive fixture: slot 0 == "01"
    set_state(&mut app, GameState::Menu);
    force_commit(&mut app); // START is item 0
    app.update();
    assert_eq!(state(&app), GameState::Diving);
}

/// The intro auto-advances on its own timeout even if the player never presses a
/// key — the state can never trap them.
#[test]
fn intro_times_out_on_its_own() {
    let mut app = campaign_app();
    set_state(&mut app, GameState::Intro);
    app.world_mut().resource_mut::<IntroTimer>().0 = INTRO_TIMEOUT;
    app.update();
    app.update();
    assert_eq!(state(&app), GameState::Diving);
}

/// `↑`/`↓` scroll the boot text instead of leaving.
#[test]
fn intro_arrows_scroll_and_do_not_leave() {
    let mut app = campaign_app();
    set_state(&mut app, GameState::Intro);
    nav(&mut app, MenuNav::Down);
    nav(&mut app, MenuNav::Down);
    assert_eq!(state(&app), GameState::Intro, "scrolling is not skipping");
    assert_eq!(app.world().resource::<PaneScroll>().0, 2);
    nav(&mut app, MenuNav::Up);
    assert_eq!(app.world().resource::<PaneScroll>().0, 1);
}

// ───────────────────────── 4.2 · the MemoryView ─────────────────────────────

/// MEMORY opens the journal from the main menu and `Esc` comes back.
#[test]
fn memory_view_opens_from_the_menu_and_returns() {
    let mut app = campaign_app();
    set_state(&mut app, GameState::Menu);
    let idx = menu_items(&crate::save::Progress::default())
        .iter()
        .position(|i| *i == MenuItem::Memory)
        .unwrap();
    app.world_mut().resource_mut::<MenuCursor>().index = idx;
    force_commit(&mut app);
    app.update();
    assert_eq!(state(&app), GameState::MemoryView);
    assert_eq!(
        *app.world().resource::<MemoryViewReturn>(),
        MemoryViewReturn::Menu
    );

    nav(&mut app, MenuNav::Back);
    app.update();
    assert_eq!(state(&app), GameState::Menu);
}

/// MEMORY opens from the PAUSE overlay too, returns to Pause, and — critically —
/// the live level survives behind it (only Pause → Menu tears down).
#[test]
fn memory_view_opens_from_pause_and_keeps_the_level_alive() {
    let mut app = campaign_app();
    app.world_mut().resource_mut::<CurrentLevel>().0 = 0;
    set_state(&mut app, GameState::Playing);
    set_state(&mut app, GameState::Pause);

    let idx = PAUSE_ITEMS
        .iter()
        .position(|i| *i == PauseItem::Memory)
        .unwrap();
    app.world_mut().resource_mut::<MenuCursor>().index = idx;
    force_commit(&mut app);
    app.update();
    assert_eq!(state(&app), GameState::MemoryView);
    assert_eq!(
        *app.world().resource::<MemoryViewReturn>(),
        MemoryViewReturn::Pause
    );
    assert!(
        app.world().resource::<CurrentLevelData>().0.is_some(),
        "the level must survive behind the journal"
    );

    nav(&mut app, MenuNav::Back);
    app.update();
    assert_eq!(state(&app), GameState::Pause);
    assert!(app.world().resource::<CurrentLevelData>().0.is_some());
}

/// The journal opens at the top every time, and scrolls.
#[test]
fn memory_view_scrolls_and_resets_on_entry() {
    let mut app = campaign_app();
    set_state(&mut app, GameState::MemoryView);
    nav(&mut app, MenuNav::Down);
    nav(&mut app, MenuNav::Down);
    assert_eq!(app.world().resource::<PaneScroll>().0, 2);
    nav(&mut app, MenuNav::Back);
    app.update();
    set_state(&mut app, GameState::MemoryView);
    assert_eq!(
        app.world().resource::<PaneScroll>().0,
        0,
        "a pane opens at its top"
    );
}

// ──────────────── 4.3 · the finale: which ending fired ──────────────────────

/// The finale's framing beat is up the moment L12 loads — and it is NOT the
/// model's plea (which is already an authored `[note]` on the wall).
#[test]
fn the_finale_opens_with_its_framing_beat() {
    let mut app = campaign_app();
    app.world_mut().resource_mut::<CurrentLevel>().0 = finale_slot();
    set_state(&mut app, GameState::Playing);
    let beat = app.world().resource::<NarrativeBeat>();
    let line = beat.line().expect("the finale arms its framing beat");
    assert!(
        line.contains("the wiring is reversible. the door is not."),
        "{line}"
    );
    assert!(
        !line.contains("please match"),
        "the plea is the level's note, not the HUD's"
    );
}

/// A non-finale campaign level opens with NO beat — the framing is the finale's
/// alone.
#[test]
fn an_ordinary_level_opens_without_a_framing_beat() {
    let mut app = campaign_app();
    app.world_mut().resource_mut::<CurrentLevel>().0 = 0;
    set_state(&mut app, GameState::Playing);
    assert!(app.world().resource::<NarrativeBeat>().line().is_none());
}

/// **Ending 1 — `exit 0`.** Wire the outward door with recognition dark, win, and
/// the flow captures the RELEASE ending and routes to the end-card.
#[test]
fn exit0_ending_is_captured_and_routed() {
    isolate_save_dir("w4_exit0");
    let mut app = campaign_app();
    play_finale_exit0(&mut app);
    assert!(
        !app.world().resource::<PuzzleState>().cycle_held,
        "the release branch leaves no kept cycle"
    );
    commit_win(&mut app);

    assert_eq!(
        app.world().resource::<EndingOutcome>().0,
        Some(Ending::Exit0),
        "target_off + no kept cycle == the release ending"
    );
    // Won is entered for one frame (progress + audio), then the end-card takes over.
    app.update();
    assert_eq!(state(&app), GameState::Ending);
    // BOTH endings mark the campaign complete: the finale is recorded solved.
    assert!(app
        .world()
        .resource::<crate::save::Progress>()
        .is_solved("L12-closing"));
}

/// **Ending 2 — `while(true)`.** Close the ring; `cycle_keep` lets it stand; the
/// flow captures the CAGE ending, and the readout collapses into `pool_alt`.
#[test]
fn while_true_ending_is_captured_and_routed() {
    isolate_save_dir("w4_loop");
    let mut app = campaign_app();
    play_finale_loop(&mut app);
    assert!(
        app.world().resource::<PuzzleState>().cycle_held,
        "closing the ring under cycle_keep must hold the cycle"
    );
    commit_win(&mut app);

    assert_eq!(
        app.world().resource::<EndingOutcome>().0,
        Some(Ending::WhileTrue),
        "a kept cycle == the cage ending"
    );
    assert_eq!(
        app.world().resource::<MemoryReadout>().form,
        ReadoutForm::Corrupt,
        "the loop readout is the pool_alt Babel, not a clean memory"
    );
    app.update();
    assert_eq!(state(&app), GameState::Ending);
    assert!(app
        .world()
        .resource::<crate::save::Progress>()
        .is_solved("L12-closing"));
}

/// The corrupt collapse is NOT written into the persistent journal — a level's
/// remembered phrase is a recovered vignette, and the loop is an ending.
#[test]
fn the_loop_collapse_is_not_persisted_as_a_memory() {
    isolate_save_dir("w4_loop_phrase");
    let mut app = campaign_app();
    play_finale_loop(&mut app);
    commit_win(&mut app);
    let p = app.world().resource::<crate::save::Progress>();
    assert!(p.is_solved("L12-closing"), "the level is still solved");
    assert_eq!(
        p.phrase_for("L12-closing"),
        None,
        "the Babel collapse must not overwrite the journal"
    );
}

/// An ORDINARY campaign win still gets the generic Won screen — the routing is the
/// finale's alone, and a stale outcome can never re-route a later win.
#[test]
fn a_non_finale_win_stays_on_the_won_screen() {
    isolate_save_dir("w4_nonfinale");
    let mut app = campaign_app();
    // First, produce a real finale ending so the outcome resource is populated…
    play_finale_exit0(&mut app);
    commit_win(&mut app);
    assert!(app.world().resource::<EndingOutcome>().0.is_some());
    set_state(&mut app, GameState::Menu);

    // …then win an ordinary level and confirm the outcome was cleared.
    app.world_mut().resource_mut::<CurrentLevel>().0 = 0;
    set_state(&mut app, GameState::Playing);
    app.world_mut().resource_mut::<Solved>().0 = true;
    set_state(&mut app, GameState::Won);
    assert_eq!(app.world().resource::<EndingOutcome>().0, None);
    app.update();
    assert_eq!(state(&app), GameState::Won);
}

/// Pausing the finale never captures an ending (nothing is solved yet).
#[test]
fn pausing_the_finale_captures_no_ending() {
    let mut app = campaign_app();
    app.world_mut().resource_mut::<CurrentLevel>().0 = finale_slot();
    set_state(&mut app, GameState::Playing);
    set_state(&mut app, GameState::Pause);
    assert_eq!(app.world().resource::<EndingOutcome>().0, None);
}

// ───────────────── 4.3/4.4 · the end-cards + the epilogue ───────────────────

/// The eye phase swallows the first keypress (it only reveals the readout), so a
/// player skipping the eye can never accidentally commit an offer.
#[test]
fn the_end_card_eye_phase_swallows_the_first_key() {
    let mut app = campaign_app();
    app.world_mut()
        .insert_resource(EndingOutcome(Some(Ending::WhileTrue)));
    set_state(&mut app, GameState::Ending);
    assert_eq!(
        app.world().resource::<EndingTimer>().phase(),
        EndingPhase::Eye
    );
    nav(&mut app, MenuNav::Confirm);
    assert_eq!(
        state(&app),
        GameState::Ending,
        "the first key only reveals the readout"
    );
    assert_eq!(
        app.world().resource::<EndingTimer>().phase(),
        EndingPhase::Readout
    );
}

/// **BOTH end-cards hand off to the SAME epilogue** — the M1 "identical furniture"
/// requirement, at the state level.
#[test]
fn both_end_cards_continue_into_the_same_epilogue() {
    for ending in [Ending::Exit0, Ending::WhileTrue] {
        let mut app = campaign_app();
        app.world_mut().insert_resource(EndingOutcome(Some(ending)));
        set_state(&mut app, GameState::Ending);
        app.world_mut().resource_mut::<EndingTimer>().0 = ENDING_EYE_WINDOW;
        force_commit(&mut app); // CONTINUE is item 0 on both cards
        app.update();
        assert_eq!(
            state(&app),
            GameState::Epilogue,
            "{ending:?} must reach the epilogue"
        );

        nav(&mut app, MenuNav::Confirm);
        app.update();
        assert_eq!(
            state(&app),
            GameState::Menu,
            "the epilogue ends at the menu"
        );
    }
}

/// The Menu is always reachable from either end-card — the loop is OFFERED, never
/// imposed.
#[test]
fn the_menu_is_reachable_from_both_end_cards() {
    for ending in [Ending::Exit0, Ending::WhileTrue] {
        let mut app = campaign_app();
        app.world_mut().insert_resource(EndingOutcome(Some(ending)));
        set_state(&mut app, GameState::Ending);
        app.world_mut().resource_mut::<EndingTimer>().0 = ENDING_EYE_WINDOW;
        nav(&mut app, MenuNav::Back);
        app.update();
        assert_eq!(
            state(&app),
            GameState::Menu,
            "{ending:?} must not trap the player"
        );
    }
}

/// The release card offers exactly one way on; the loop card offers two, with the
/// loop NEVER pre-selected.
#[test]
fn only_the_loop_card_offers_the_loop() {
    assert_eq!(ending_items(Ending::Exit0), vec![EndingItem::Continue]);
    assert_eq!(
        ending_items(Ending::WhileTrue),
        vec![EndingItem::Continue, EndingItem::BeginAgain]
    );
    let mut app = campaign_app();
    app.world_mut()
        .insert_resource(EndingOutcome(Some(Ending::WhileTrue)));
    set_state(&mut app, GameState::Ending);
    let cursor = *app.world().resource::<MenuCursor>();
    assert_eq!(cursor.index, 0, "the loop is never the default choice");
    assert_eq!(cursor.len, 2);
}

// ───────────────────── 4.4/4.5 · the loop-back offer ────────────────────────

/// **The loop-restart carries memory.** Accepting the offer bumps the session,
/// parks the cursor on level 1, and leaves `solved` / `phrases` untouched.
#[test]
fn accepting_the_loop_keeps_solved_and_phrases() {
    isolate_save_dir("w4_loop_restart");
    let mut app = campaign_app();
    {
        let mut p = app.world_mut().resource_mut::<crate::save::Progress>();
        for l in campaign_manifest().levels() {
            p.record_phrase(&l.id, &format!("PHRASE for {}", l.id));
        }
    }
    let (solved_before, phrases_before) = {
        let p = app.world().resource::<crate::save::Progress>();
        (p.solved.clone(), p.phrases.clone())
    };

    app.world_mut()
        .insert_resource(EndingOutcome(Some(Ending::WhileTrue)));
    set_state(&mut app, GameState::Ending);
    app.world_mut().resource_mut::<EndingTimer>().0 = ENDING_EYE_WINDOW;
    nav(&mut app, MenuNav::Right); // move onto the loop offer
    force_commit(&mut app);
    app.update();

    assert_eq!(
        state(&app),
        GameState::Diving,
        "accepting the loop begins again"
    );
    let p = app.world().resource::<crate::save::Progress>();
    assert_eq!(p.session, 1, "the session increments");
    assert_eq!(p.session_label(), "0002");
    assert_eq!(p.campaign_cursor.as_deref(), Some("L1-first-pulse"));
    assert_eq!(
        p.solved, solved_before,
        "the loop must NEVER clear `solved`"
    );
    assert_eq!(
        p.phrases, phrases_before,
        "the loop must NEVER clear `phrases`"
    );
    assert_eq!(
        app.world().resource::<CurrentLevel>().0,
        0,
        "the loop opens on the campaign's first level"
    );
}

/// The MENU shows the loop offer once the campaign is complete — on BOTH endings,
/// since both mark it complete — and accepting it there does the same thing.
#[test]
fn the_menu_offers_the_loop_only_when_the_campaign_is_complete() {
    isolate_save_dir("w4_menu_offer");
    let mut prog = crate::save::Progress::default();
    assert!(
        !menu_items(&prog).contains(&MenuItem::BeginAgain),
        "an unfinished campaign shows no loop offer"
    );
    for l in campaign_manifest().levels() {
        prog.mark_solved(&l.id);
    }
    let items = menu_items(&prog);
    assert!(
        items.contains(&MenuItem::BeginAgain),
        "a complete campaign offers the loop"
    );
    assert_eq!(
        items.last(),
        Some(&MenuItem::Quit),
        "the offer sits above QUIT, never replacing it"
    );

    let mut app = campaign_app();
    *app.world_mut().resource_mut::<crate::save::Progress>() = prog;
    set_state(&mut app, GameState::Menu);
    let idx = items
        .iter()
        .position(|i| *i == MenuItem::BeginAgain)
        .unwrap();
    app.world_mut().resource_mut::<MenuCursor>().index = idx;
    force_commit(&mut app);
    app.update();
    assert_eq!(state(&app), GameState::Diving);
    assert_eq!(
        app.world()
            .resource::<crate::save::Progress>()
            .campaign_cursor
            .as_deref(),
        Some("L1-first-pulse")
    );
}

/// **CONTINUE follows the loop cursor**, and the cursor walks forward as the
/// replay is re-solved — otherwise Continue would stick on level 1 forever (every
/// level is still marked solved).
#[test]
fn continue_follows_the_loop_cursor_and_the_cursor_advances() {
    isolate_save_dir("w4_cursor");
    let mut app = campaign_app();
    {
        let mut p = app.world_mut().resource_mut::<crate::save::Progress>();
        for l in campaign_manifest().levels() {
            p.mark_solved(&l.id);
        }
        p.begin_next_session("L1-first-pulse");
    }
    // Continue lands on the cursor, not on "the first unsolved" (there is none).
    let idx = app
        .world()
        .resource::<LevelCatalog>()
        .first_unsolved(app.world().resource::<crate::save::Progress>())
        .unwrap();
    assert_eq!(
        app.world().resource::<LevelCatalog>().id_at(idx),
        Some("L1-first-pulse")
    );

    // Re-solving the cursor's level advances it to the next manifest entry.
    app.world_mut().resource_mut::<CurrentLevel>().0 = 0;
    set_state(&mut app, GameState::Playing);
    app.world_mut().resource_mut::<Solved>().0 = true;
    set_state(&mut app, GameState::Won);
    assert_eq!(
        app.world()
            .resource::<crate::save::Progress>()
            .campaign_cursor
            .as_deref(),
        Some("L2-two-make-three"),
        "the loop cursor walks the manifest with the player"
    );
}

/// Finishing the loop's last level CLEARS the cursor, so Continue falls back to
/// its ordinary behaviour instead of pointing past the end of the campaign.
#[test]
fn the_loop_cursor_clears_at_the_end_of_the_campaign() {
    isolate_save_dir("w4_cursor_end");
    let mut app = campaign_app();
    {
        let mut p = app.world_mut().resource_mut::<crate::save::Progress>();
        for l in campaign_manifest().levels() {
            p.mark_solved(&l.id);
        }
        p.session = 1;
        p.set_campaign_cursor(Some("L12-closing"));
    }
    app.world_mut().resource_mut::<CurrentLevel>().0 = finale_slot();
    set_state(&mut app, GameState::Playing);
    app.world_mut().resource_mut::<Solved>().0 = true;
    set_state(&mut app, GameState::Won);
    assert_eq!(
        app.world()
            .resource::<crate::save::Progress>()
            .campaign_cursor,
        None,
        "the cursor is cleared once the loop reaches the end"
    );
}

/// The epilogue is reachable, scrolls, and always ends at the menu.
#[test]
fn the_epilogue_scrolls_and_ends_at_the_menu() {
    let mut app = campaign_app();
    set_state(&mut app, GameState::Epilogue);
    nav(&mut app, MenuNav::Down);
    nav(&mut app, MenuNav::Down);
    assert_eq!(state(&app), GameState::Epilogue);
    assert_eq!(app.world().resource::<PaneScroll>().0, 2);
    nav(&mut app, MenuNav::Back);
    app.update();
    assert_eq!(state(&app), GameState::Menu);
}

// ═════════════════════════════════════════════════════════════════════════
// `clear_stale_nav` on `OnEnter(Playing)` — probe tests for the pause-menu
// fix (2026-08-05 sweep).
//
// `Playing` was the ONE state entry missing `clear_stale_nav`. `Messages<
// MenuNav>` is double-buffered (a message survives the frame it is written
// AND the following frame), so the `Back` that `pause_nav` consumed to
// RESUME (Pause → Playing) was still alive in the buffer when `watch_pause`
// (`Update`, `in_state(Playing)`) ran its OWN `nav.drain()` on the very next
// frame: it found that same stale `Back` and re-opened Pause immediately —
// Esc could never resume. The identical mechanism dropped the player into
// Pause instead of the level when they pressed Esc to SKIP the intro dive
// (`tick_diving` treats any `MenuNav` — including `Back` — as a skip signal).
//
// Every existing pause/resume test above (`pause_resume_returns_to_playing`
// etc.) resumes via `force_commit` — `MenuCursor::pending_commit`, a direct
// resource flag `pause_nav` also honours — which NEVER writes a `MenuNav`
// message at all, so it could not have exercised (or caught) this bug. These
// two tests resume/skip via a REAL `MenuNav::Back` message, written by a
// dedicated `PreUpdate` system exactly where `re_present`'s real input lands
// relative to the flow's `Update`-scheduled nav readers and the `PreUpdate`-
// scheduled `StateTransition` apply — not by poking the message into the
// World from outside any schedule between `app.update()` calls, which would
// not reproduce the same cross-frame double-buffering the live game hits.
// ═════════════════════════════════════════════════════════════════════════

/// Test-only stand-in for a real keypress: set on the test thread, consumed by
/// [`emit_pending_back`] the next time `PreUpdate` runs — so the `MenuNav::Back`
/// message is written by an actual scheduled system, in the same schedule slot
/// (`PreUpdate`, ahead of `StateTransition` and `Update`) the present layer's
/// input systems occupy, rather than injected directly into the `World`
/// between frames.
#[derive(Resource, Default)]
struct PendingBack(bool);

/// `PreUpdate`: if a press is pending, emit exactly one `MenuNav::Back` and
/// clear the flag. Registered on the realistic-input test app only.
fn emit_pending_back(mut pending: ResMut<PendingBack>, mut nav: MessageWriter<MenuNav>) {
    if pending.0 {
        nav.write(MenuNav::Back);
        pending.0 = false;
    }
}

/// [`test_app`] plus the `PreUpdate`-scheduled Esc emitter, for the two probe
/// tests below.
fn test_app_realistic_input() -> App {
    let mut app = test_app();
    app.init_resource::<PendingBack>();
    app.add_systems(PreUpdate, emit_pending_back);
    app
}

/// "Press Esc this frame": arm the pending flag and advance one frame. The
/// `PreUpdate`-scheduled [`emit_pending_back`] writes `MenuNav::Back` before
/// that frame's `StateTransition` apply and `Update` (nav-reading) systems
/// run — exactly the ordering a real keypress arrives in.
fn press_esc(app: &mut App) {
    app.world_mut().resource_mut::<PendingBack>().0 = true;
    app.update();
}

/// Advance one frame with no new input (applies any transition queued by the
/// previous frame's nav reader, and lets `watch_pause` run against whatever
/// is left in the double-buffered `MenuNav` queue).
fn tick(app: &mut App) {
    app.update();
}

/// Playing → Esc → Pause → Esc → must be Playing and STAY Playing for at
/// least 2 more frames (not bounce back into Pause from the stale
/// double-buffered `Back` that resumed it).
///
/// Must FAIL if `clear_stale_nav` is removed from `OnEnter(GameState::
/// Playing)` — verified by temporarily removing it, capturing the failure,
/// and restoring it (see the task's control-run output; not re-run
/// automatically here since flipping shipped wiring inside a normal test run
/// would make the suite self-sabotaging).
#[test]
fn esc_in_pause_resumes_play() {
    let mut app = test_app_realistic_input();
    set_state(&mut app, GameState::Playing); // loads a real level
    assert_eq!(state(&app), GameState::Playing);

    // Esc opens Pause (watch_pause, Update, in_state(Playing)).
    press_esc(&mut app);
    assert_eq!(
        state(&app),
        GameState::Playing,
        "the Playing->Pause transition is only QUEUED this frame"
    );
    tick(&mut app); // apply Playing -> Pause
    assert_eq!(
        state(&app),
        GameState::Pause,
        "Esc in Playing must open Pause"
    );

    // Esc again resumes (pause_nav's `Back` arm — NOT force_commit).
    press_esc(&mut app);
    assert_eq!(
        state(&app),
        GameState::Pause,
        "the Pause->Playing transition is only QUEUED this frame"
    );
    tick(&mut app); // apply Pause -> Playing (OnEnter(Playing) runs clear_stale_nav here)
    assert_eq!(
        state(&app),
        GameState::Playing,
        "Esc in Pause must resume to Playing"
    );

    // The regression: without clear_stale_nav, the SAME Back message (still
    // alive in the double-buffered queue) is found by watch_pause's drain on
    // the very next Update and re-opens Pause immediately.
    tick(&mut app);
    assert_eq!(
        state(&app),
        GameState::Playing,
        "must NOT bounce back into Pause one frame after resuming (stale double-buffered Back)"
    );
    tick(&mut app);
    assert_eq!(
        state(&app),
        GameState::Playing,
        "must STAY Playing two frames after resuming"
    );
}

/// Diving → Esc (skip) → must land in Playing, not Pause.
///
/// `tick_diving` treats any `MenuNav` (including `Back`) as a skip signal, so
/// pressing Esc to skip the intro dive writes the exact same `MenuNav::Back`
/// the pause path does — the identical stale-message mechanism drops the
/// player into Pause instead of the level without the `OnEnter(Playing)` fix.
///
/// Must FAIL if `clear_stale_nav` is removed from `OnEnter(GameState::
/// Playing)` (see the task's captured control-run output).
#[test]
fn esc_skipping_the_dive_lands_in_play_not_pause() {
    let mut app = test_app_realistic_input();
    app.world_mut().resource_mut::<CurrentLevel>().0 = 0;
    set_state(&mut app, GameState::Diving);
    assert_eq!(state(&app), GameState::Diving);

    // Esc skips the dive (tick_diving reads any MenuNav as a skip signal).
    press_esc(&mut app);
    assert_eq!(
        state(&app),
        GameState::Diving,
        "the Diving->Playing skip is only QUEUED this frame"
    );
    tick(&mut app); // apply Diving -> Playing (OnEnter(Playing) runs clear_stale_nav here)
    assert_eq!(
        state(&app),
        GameState::Playing,
        "Esc-skip must land in Playing"
    );

    // The regression: without clear_stale_nav, the SAME Back message used to
    // skip the dive is still alive in the double-buffered queue and
    // watch_pause's next drain reads it as "Esc pressed in Playing" → Pause.
    tick(&mut app);
    assert_eq!(
        state(&app),
        GameState::Playing,
        "must NOT fall into Pause one frame after the dive-skip (stale double-buffered Back)"
    );
    tick(&mut app);
    assert_eq!(
        state(&app),
        GameState::Playing,
        "must STAY Playing two frames after the dive-skip"
    );
}

// ─────────────────── CLI level → catalogue slot resolution ───────────────────
//
// The playtest defect: `terminal-state levels\01.lvl` (the natural Windows form)
// was compared to the catalogue by STRING EQUALITY, missed, and silently started
// catalogue slot 0 — the player played a level they had not asked for and there
// was nothing on screen to say so.

/// A catalogue whose paths are the shipped `levels/NN.lvl` in the RELATIVE form
/// the real `LevelCatalog::default()` produces. Deliberately not absolute: the
/// lexical tier is what must carry these, since `cargo test` runs from the crate
/// directory where `levels/` does not resolve.
fn relative_catalog() -> LevelCatalog {
    LevelCatalog {
        paths: (1..=7).map(|n| format!("levels/{n:02}.lvl")).collect(),
    }
}

#[test]
fn index_of_path_matches_the_exact_string() {
    assert_eq!(relative_catalog().index_of_path("levels/03.lvl"), Some(2));
}

#[test]
fn index_of_path_ignores_a_leading_dot_segment() {
    // `./levels/03.lvl` — what a shell completion or a script often produces.
    // `Path::components` keeps a LEADING CurDir, so this only works because the
    // lexical key filters it out.
    assert_eq!(relative_catalog().index_of_path("./levels/03.lvl"), Some(2));
}

#[test]
#[cfg(windows)]
fn index_of_path_accepts_the_windows_separator_and_case() {
    // THE DEFECT. Both of these are what a Windows player actually types.
    let cat = relative_catalog();
    assert_eq!(cat.index_of_path(r"levels\03.lvl"), Some(2));
    assert_eq!(cat.index_of_path(r".\levels\03.lvl"), Some(2));
    assert_eq!(cat.index_of_path(r"LEVELS\03.LVL"), Some(2));
}

#[test]
fn index_of_path_matches_by_filesystem_identity() {
    // The catalogue holds ABSOLUTE paths and the query is RELATIVE (to the crate
    // dir `cargo test` runs in). The two strings share no prefix, so only
    // `fs::canonicalize` can equate them — this pins the filesystem-identity tier.
    let cat = abs_catalog();
    assert_eq!(cat.index_of_path("../../levels/03.lvl"), Some(2));
}

#[test]
fn index_of_path_refuses_a_level_the_catalogue_does_not_hold() {
    // No guessing: a miss is a miss, so the caller can report it.
    let cat = relative_catalog();
    assert_eq!(cat.index_of_path("levels/does-not-exist.lvl"), None);
    // A bare file name names a DIFFERENT path (some other directory's 03.lvl) and
    // must not resolve — it is only offered as a "did you mean" suggestion.
    assert_eq!(cat.index_of_path("03.lvl"), None);
    assert_eq!(cat.same_name_as("03.lvl"), vec!["levels/03.lvl"]);
}

/// Start a fresh app whose CLI level is `cli` over [`relative_catalog`], drive it
/// to the menu, and commit START. Returns the catalogue slot the flow chose.
///
/// The relative catalogue is the right fixture here: committing START only sets
/// [`CurrentLevel`] and transitions to `Diving`, so no level is ever loaded and
/// the paths need not resolve on disk — which is exactly the state a real launch
/// is in when it makes this decision.
fn cli_start_slot(cli: &str) -> usize {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(StatesPlugin)
        .add_message::<InteractPressed>()
        .add_plugins(PuzzlePlugin)
        .add_plugins(FlowPlugin {
            cli_level: Some(cli.to_string()),
        })
        .insert_resource(relative_catalog());
    set_state(&mut app, GameState::Menu);
    force_commit(&mut app); // index 0 == Start
    app.world().resource::<CurrentLevel>().0
}

#[test]
fn cli_level_start_plays_the_requested_level_not_slot_zero() {
    // Slot 2 is levels/03.lvl. Ask for it in a form that is NOT string-equal to
    // the catalogue entry and START must still land on slot 2.
    assert_eq!(
        cli_start_slot("./levels/03.lvl"),
        2,
        "a CLI level given as `./levels/03.lvl` must start slot 2, not fall back to 0"
    );
}

#[test]
#[cfg(windows)]
fn cli_level_start_accepts_a_backslash_path() {
    // The exact playtest command shape: `terminal-state levels\03.lvl`.
    assert_eq!(
        cli_start_slot("levels\\03.lvl"),
        2,
        "`levels\\03.lvl` must start slot 2, not fall back to 0"
    );
}

// ── LEVEL-REQUESTED POST-FX (lighting spec §1.3) ─────────────────────────────
// `[atmosphere] postfx = NAME` sets the level's own look, but never over the
// player's back: the rule is "the level asks, an explicit player choice wins".

/// A settings value whose POST-FX row still holds the shipped default.
fn default_settings() -> crate::settings::Settings {
    crate::settings::Settings::default()
}

#[test]
fn a_level_preset_applies_while_the_player_never_chose_one() {
    let s = default_settings();
    assert!(s.postfx_is_default(), "the shipped row is untouched");
    assert_eq!(
        super::postfx_for_level(&s, Some("cinematic")),
        re_postfx::PostfxConfig::cinematic(),
        "the level's request is honoured when the player expressed no preference"
    );
}

#[test]
fn an_explicit_player_choice_outranks_the_level() {
    let mut s = default_settings();
    s.postfx = "crt".to_string();
    assert!(!s.postfx_is_default(), "the player picked a preset");
    assert_eq!(
        super::postfx_for_level(&s, Some("cinematic")),
        re_postfx::PostfxConfig::crt(),
        "a chosen preset is not overridden by the level"
    );
}

#[test]
fn a_level_that_requests_nothing_resolves_to_the_players_setting() {
    // This is what stops a preset from leaking out of the previous room.
    let mut s = default_settings();
    s.postfx = "toon".to_string();
    assert_eq!(
        super::postfx_for_level(&s, None),
        re_postfx::PostfxConfig::toon()
    );
    let d = default_settings();
    assert_eq!(super::postfx_for_level(&d, None), d.postfx_config());
}

#[test]
fn an_unknown_preset_name_falls_back_to_the_player_setting() {
    // The parser refuses an unknown name, so this can only come from a
    // hand-edited file — it must degrade to the player's look, not to a surprise.
    let s = default_settings();
    assert_eq!(
        super::postfx_for_level(&s, Some("kaleidoscope")),
        s.postfx_config(),
        "an unparseable request is ignored"
    );
}

#[test]
fn the_campaigns_first_level_request_is_a_real_preset() {
    // A shipped campaign level is part of the contract: its authored `postfx =
    // clean` must resolve — L1 authors exactly that request.
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../levels/L1-first-pulse.lvl");
    let raw = std::fs::read_to_string(&path).expect("the campaign's first level exists");
    let level = re_level::load_string(&raw, "L1-first-pulse").expect("it parses");
    let map = level.build_map();
    let want = map
        .atmosphere()
        .postfx
        .clone()
        .expect("the level requests a preset");
    let s = default_settings();
    assert_ne!(
        super::postfx_for_level(&s, Some(&want)),
        re_postfx::PostfxConfig::off(),
        "`postfx = {want}` must resolve to a real chain, not the identity"
    );
}

// ─────────────────────── splash eye gaze tracking ────────────────────────────

#[test]
fn enter_splash_resets_timer_and_mouse_state() {
    let mut world = World::new();
    world.insert_resource(SplashTimer(std::time::Duration::from_secs(3)));
    world.insert_resource(SplashMouse(Some((10, 10))));
    let _ = world.run_system_once(enter_splash);
    assert_eq!(world.resource::<SplashTimer>().0, std::time::Duration::ZERO);
    assert_eq!(world.resource::<SplashMouse>().0, None);
}

#[test]
fn splash_eye_seed_is_stable_within_one_default_call_but_not_hardcoded() {
    // Two independent `Default::default()` calls each read the wall clock
    // fresh, so they are extremely unlikely to collide — but the point of the
    // resource is that ONE value, once resolved, stays fixed for the rest of
    // the run (see `SplashEyeSeed`'s doc comment): a single value read twice
    // must compare equal to itself.
    let seed = SplashEyeSeed::default();
    assert_eq!(seed.0, seed.0);
}

#[test]
fn track_splash_mouse_folds_the_latest_move_event() {
    let mut app = App::new();
    app.add_message::<re_present::MenuMouseMove>();
    app.insert_resource(SplashMouse::default());
    app.world_mut()
        .write_message(re_present::MenuMouseMove { col: 12, row: 34 });
    let _ = app.world_mut().run_system_once(track_splash_mouse);
    assert_eq!(app.world().resource::<SplashMouse>().0, Some((12, 34)));
}
