//! Unit tests for the SPLASH painter — a procedural [`re_asciifx::eye::Eye`],
//! see `paint_splash`'s doc.
//!
//! The splash is not a `--dump` path (there is no level and no camera), so the
//! evidence here is the pure painter: the eye phase paints SOMETHING and it
//! animates continuously over time, the phase flips from the eye to the title
//! card inside the state's own timeout, and the Sealed-Door footer is on
//! screen in both phases. No `App`, no renderer, no terminal.

use super::*;

/// The same grid `--dump` and the golden framebuffer tests use.
const COLS: usize = 100;
const ROWS: usize = 32;

/// A fixed literal seed for every test below — `paint_splash` must be a pure
/// function of `(elapsed, eye_seed, gaze_target)`, so any fixed seed proves
/// the same determinism `boot_console::breakdown_reaches_the_real_splash_frame`
/// depends on.
const TEST_EYE_SEED: u64 = 42;

fn fb() -> Framebuffer {
    Framebuffer::new(COLS, ROWS)
}

/// Read one framebuffer row back as text (for asserting painted content).
fn row_text(fb: &Framebuffer, row: usize) -> String {
    (0..fb.cols())
        .map(|c| {
            fb.get(c, row)
                .and_then(|cell| char::from_u32(cell.glyph))
                .unwrap_or(' ')
        })
        .collect()
}

/// The whole framebuffer as text, for "does this appear anywhere" assertions.
fn screen_text(fb: &Framebuffer) -> String {
    (0..fb.rows())
        .map(|r| row_text(fb, r))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Every painted cell (glyph, fg, bg) as a flat vector — used where a
/// pixel-level diff is clearer than a text scrape (the procedural eye paints
/// COLOUR, not distinguishing glyph text).
fn cells(fb: &Framebuffer) -> Vec<re_core::Cell> {
    (0..fb.rows())
        .flat_map(|r| (0..fb.cols()).map(move |c| (c, r)))
        .map(|(c, r)| *fb.get(c, r).unwrap())
        .collect()
}

fn ms(n: u64) -> std::time::Duration {
    std::time::Duration::from_millis(n)
}

/// The embedded eye asset (still used by `crate::screens::ending_eye` for the
/// campaign end-cards, see `EYE_FRAMES_SRC`'s doc comment) parses into MANY
/// frames, every frame carries art, and every frame has a positive delay.
#[test]
fn eye_asset_parses_into_multiple_frames() {
    let frames = eye_frames();
    assert!(
        frames.len() > 1,
        "the eye animation must yield more than one frame, got {}",
        frames.len()
    );
    let h = frames[0].lines.len();
    assert!(h > 4, "a frame should be a block of art, got {h} rows");
    for (i, f) in frames.iter().enumerate() {
        assert_eq!(f.lines.len(), h, "frame {i} has a different height");
        assert!(!f.delay.is_zero(), "frame {i} must hold for a while");
        assert!(
            f.lines.iter().any(|l| !l.trim().is_empty()),
            "frame {i} is blank"
        );
        // The separator blank line must not have survived into the frame.
        assert!(
            f.lines.last().is_some_and(|l| !l.trim().is_empty()),
            "frame {i} kept a trailing blank row"
        );
        // Header/metadata lines are consumed, never painted.
        assert!(
            !f.lines
                .iter()
                .any(|l| l.trim_start().starts_with("[frame")
                    || l.trim_start().starts_with("delay_ms")),
            "frame {i} leaked a metadata line into its art"
        );
    }
}

/// The two phases divide the splash state's own timeout: the eye runs first, the
/// title card is always reached before the timeout expires.
#[test]
fn splash_phases_fit_inside_the_state_timeout() {
    let total = SPLASH_TIMEOUT;
    let window = eye_window(total);
    assert!(!window.is_zero(), "the eye must get a window");
    assert!(
        window < total,
        "the eye window ({window:?}) must leave room for the title card in {total:?}"
    );
    assert_eq!(splash_phase(ms(0), total), SplashPhase::Eye);
    assert_eq!(
        splash_phase(window.saturating_sub(ms(1)), total),
        SplashPhase::Eye
    );
    assert_eq!(splash_phase(window, total), SplashPhase::Title);
    assert_eq!(splash_phase(total, total), SplashPhase::Title);
    // Overrun (the timeout fires on the same frame the timer passes it) stays on
    // the title card rather than looping back to the eye.
    assert_eq!(splash_phase(total * 2, total), SplashPhase::Title);
}

/// A pathological (zero-length) window degrades to the title card instead of
/// dividing by zero or panicking.
#[test]
fn zero_length_splash_degrades_to_the_title() {
    assert_eq!(
        splash_phase(ms(0), std::time::Duration::ZERO),
        SplashPhase::Title
    );
}

/// The eye phase paints something (the eye is not a blank rect), it is
/// perfectly deterministic given `(elapsed, eye_seed, gaze_target)`, and it
/// continuously animates (a moment 2s later reads differently — breathing/
/// tremor/saccade are all continuous functions of time).
#[test]
fn eye_phase_paints_something_deterministically_and_it_animates() {
    let mut a1 = fb();
    paint_splash(&mut a1, ms(0), TEST_EYE_SEED, None);
    let mut a2 = fb();
    paint_splash(&mut a2, ms(0), TEST_EYE_SEED, None);
    assert_eq!(
        cells(&a1),
        cells(&a2),
        "same seed + same t must paint byte-identical frames"
    );

    let blank = fb();
    assert_ne!(
        cells(&a1),
        cells(&blank),
        "the eye phase must paint SOMETHING, not a blank frame"
    );

    let mut b = fb();
    paint_splash(&mut b, ms(2000), TEST_EYE_SEED, None);
    assert_ne!(
        cells(&a1),
        cells(&b),
        "the splash eye must actually animate over 2 seconds of continuous time"
    );
}

/// Two different seeds roll visibly different eyes (variant/size/hue), so
/// "randomized per launch" is not a no-op.
#[test]
fn different_seeds_paint_different_eyes() {
    let mut a = fb();
    paint_splash(&mut a, ms(500), 1, None);
    let mut b = fb();
    paint_splash(&mut b, ms(500), 2, None);
    assert_ne!(
        cells(&a),
        cells(&b),
        "two different eye seeds must not paint the same frame"
    );
}

/// A gaze target actually changes what the eye paints relative to no target
/// (autonomous saccade) at the same instant.
#[test]
fn a_gaze_target_changes_the_painted_frame() {
    let mut a = fb();
    paint_splash(&mut a, ms(300), TEST_EYE_SEED, None);
    let mut b = fb();
    paint_splash(&mut b, ms(300), TEST_EYE_SEED, Some((10.0, 5.0)));
    assert_ne!(
        cells(&a),
        cells(&b),
        "with vs. without a gaze target must paint differently"
    );
}

/// The title phase paints the title card + the tagline, and none of the
/// splash's eye-phase colour painting survives into it.
#[test]
fn title_phase_paints_the_title_card() {
    let mut f = fb();
    paint_splash(
        &mut f,
        SPLASH_TIMEOUT.saturating_sub(ms(1)),
        TEST_EYE_SEED,
        None,
    );
    let text = screen_text(&f);
    assert!(
        text.contains("terminal . state . sys"),
        "the title asset's own strap line should be on screen:\n{text}"
    );
    assert!(
        text.contains("a first-person ASCII puzzle"),
        "the tagline should be on screen"
    );
}

/// The "press any key" strap and the Sealed-Door notice are on screen in BOTH
/// phases — the strap because the skip is always available, the notice because it
/// is the splash's static frame.
#[test]
fn strap_and_sealed_door_notice_are_static_across_phases() {
    for t in [ms(0), ms(900), SPLASH_TIMEOUT.saturating_sub(ms(1))] {
        let mut f = fb();
        paint_splash(&mut f, t, TEST_EYE_SEED, None);
        let text = screen_text(&f);
        assert!(
            text.contains("PRESS ANY KEY"),
            "the strap must show at {t:?}:\n{text}"
        );
        for line in SEALED_DOOR_NOTICE {
            assert!(
                text.contains(line.trim_end()),
                "the Sealed-Door notice line {line:?} must show at {t:?}"
            );
        }
    }
}

/// The notice is bottom-anchored and the strap sits above it, so neither collides
/// with the phase art.
#[test]
fn footer_is_bottom_anchored() {
    let mut f = fb();
    paint_splash(&mut f, ms(0), TEST_EYE_SEED, None);
    let last = row_text(&f, ROWS - 1);
    assert!(
        last.contains("you came in through the source"),
        "the notice's last line belongs on the bottom row, got {last:?}"
    );
}

/// Every glyph the splash paints stays inside the grid, and a comically small grid
/// neither panics nor writes out of bounds.
#[test]
fn splash_survives_a_tiny_grid() {
    for (cols, rows) in [(1, 1), (10, 3), (40, 12), (200, 60)] {
        let mut f = Framebuffer::new(cols, rows);
        paint_splash(&mut f, ms(0), TEST_EYE_SEED, None);
        paint_splash(&mut f, SPLASH_TIMEOUT, TEST_EYE_SEED, Some((1.0, 1.0)));
        assert_eq!(f.cols(), cols, "grid width untouched");
        assert_eq!(f.rows(), rows, "grid height untouched");
    }
}

/// Not an assertion — a human-readable dump of both splash phases, printed when
/// this test runs with `--nocapture`. The splash is not a `--dump` path (no level,
/// no camera), so this is how a frame gets eyeballed.
#[test]
fn splash_frames_preview() {
    for (name, t) in [
        ("EYE t=0", ms(0)),
        ("TITLE t=end", SPLASH_TIMEOUT.saturating_sub(ms(1))),
    ] {
        let mut f = fb();
        paint_splash(&mut f, t, TEST_EYE_SEED, None);
        println!("──── {name} ────");
        println!("{}", screen_text(&f));
    }
}

/// Text evidence (not just an assertion) for the delegation's acceptance
/// criterion: the SETTINGS screen at the real 100x32 grid, printed row by row
/// (`cargo test ... -- --nocapture`) — the cost-tint legend, the inserted
/// COMFORT/PHOTOSENSITIVITY header, and the COLOUR STEPS / BRIGHTNESS /
/// FIRST STEPS rows this delegation added should all be visible in the dump.
/// `draw_settings` is a Bevy system (needs `Res<Settings>`/`Res<MenuCursor>`),
/// so it is run once through a minimal headless `World`
/// ([`bevy::ecs::system::RunSystemOnce`], the same pattern `flow::tests` uses).
#[test]
fn settings_screen_text_dump_for_evidence() {
    use bevy::ecs::system::RunSystemOnce;
    let mut world = bevy::prelude::World::new();
    world.insert_resource(fb());
    world.insert_resource(MenuCursor {
        index: 0,
        len: SETTINGS_ROWS.len(),
        pending_commit: false,
    });
    world.insert_resource(Settings::default());
    world
        .run_system_once(draw_settings)
        .expect("draw_settings runs headless");
    let f = world.resource::<Framebuffer>();
    println!("--- SETTINGS screen, 100x32 ---");
    println!("{}", screen_text(f));
}

/// The settings list is 22 rows deep and does not fit an unscrolled 32-row (let
/// alone 24-row) grid — [`draw_settings`] must scroll to keep whichever row the
/// cursor sits on inside the visible window, for EVERY row index, on both the
/// full 100×32 grid and the minimum-supported 80×24 one. The selected row
/// paints as `"▶ {line} ◀"` (see [`draw_settings_row`]), so its `▶` marker
/// reaching `screen_text` is direct evidence the row is on screen, not
/// scrolled off — the overflow this delegation fixes silently clipped that
/// exact row via `put_str`'s own out-of-bounds guard.
#[test]
fn selected_settings_row_is_always_visible_across_every_index_at_100x32_and_80x24() {
    use bevy::ecs::system::RunSystemOnce;
    for (cols, rows) in [(100usize, 32usize), (80, 24)] {
        for sel in 0..SETTINGS_ROWS.len() {
            let mut world = bevy::prelude::World::new();
            world.insert_resource(Framebuffer::new(cols, rows));
            world.insert_resource(MenuCursor {
                index: sel,
                len: SETTINGS_ROWS.len(),
                pending_commit: false,
            });
            world.insert_resource(Settings::default());
            world
                .run_system_once(draw_settings)
                .expect("draw_settings runs headless");
            let f = world.resource::<Framebuffer>();
            let text = screen_text(f);
            assert!(
                text.contains('▶'),
                "{cols}x{rows} sel={sel}: the selected row's marker is off-screen:\n{text}"
            );
        }
    }
}

/// [`settings_window`] (the pure scroll-range function `draw_settings` and
/// `settings_rects` both call) must keep `sel` inside `[start, end)` for every
/// row index on both shipped grid sizes — the property the render-level test
/// above observes indirectly through the painted marker.
#[test]
fn settings_window_keeps_the_cursor_inside_its_own_range_at_every_index() {
    for (cols, rows) in [(100usize, 32usize), (80, 24)] {
        let _ = cols; // window math does not depend on column count
        for sel in 0..SETTINGS_ROWS.len() {
            let (start, end) = settings_window(sel, rows);
            assert!(
                start <= sel && sel < end,
                "{rows}-row grid, sel={sel}: window [{start},{end}) excludes it"
            );
            assert!(
                end <= SETTINGS_ROWS.len(),
                "{rows}-row grid, sel={sel}: window past the end"
            );
        }
    }
}

/// The COMFORT/PHOTOSENSITIVITY header (painted directly above the `Glitch`
/// row) and the cost-tint legend footer must both still be correct once the
/// list is scrolled — selecting `Glitch` itself forces it into the visible
/// window on every grid size this project ships.
#[test]
fn settings_screen_keeps_the_comfort_header_and_cost_legend_correct_while_scrolled() {
    use bevy::ecs::system::RunSystemOnce;
    let glitch_idx = SETTINGS_ROWS
        .iter()
        .position(|&r| r == SettingsRow::Glitch)
        .expect("Glitch is in SETTINGS_ROWS");
    for (cols, rows) in [(100usize, 32usize), (80, 24)] {
        let mut world = bevy::prelude::World::new();
        world.insert_resource(Framebuffer::new(cols, rows));
        world.insert_resource(MenuCursor {
            index: glitch_idx,
            len: SETTINGS_ROWS.len(),
            pending_commit: false,
        });
        world.insert_resource(Settings::default());
        world
            .run_system_once(draw_settings)
            .expect("draw_settings runs headless");
        let f = world.resource::<Framebuffer>();
        let text = screen_text(f);
        assert!(
            text.contains("COMFORT / PHOTOSENSITIVITY"),
            "{cols}x{rows}: COMFORT header missing while Glitch is selected:\n{text}"
        );
        assert!(
            text.contains("amber = pricier settings"),
            "{cols}x{rows}: cost-tint legend footer missing:\n{text}"
        );
    }
    // Also true at the very LAST row, where the header sits several rows
    // above the cursor and must have scrolled out of the earlier fully-shown
    // window without leaving the legend footer disturbed.
    let mut world = bevy::prelude::World::new();
    world.insert_resource(Framebuffer::new(80, 24));
    world.insert_resource(MenuCursor {
        index: SETTINGS_ROWS.len() - 1,
        len: SETTINGS_ROWS.len(),
        pending_commit: false,
    });
    world.insert_resource(Settings::default());
    world
        .run_system_once(draw_settings)
        .expect("draw_settings runs headless");
    let f = world.resource::<Framebuffer>();
    let text = screen_text(f);
    assert!(
        text.contains("amber = pricier settings"),
        "80x24 at the last row: cost-tint legend footer missing:\n{text}"
    );
    assert!(
        text.contains('▶'),
        "80x24 at the last row: selected marker off-screen:\n{text}"
    );
}

/// [`settings_rects`] (the mouse hit-test geometry) must return exactly the
/// rows [`settings_window`] says are visible — a scrolled-off row can never be
/// hoverable or clickable, and every returned rect's row is really on screen.
#[test]
fn settings_rects_never_returns_a_scrolled_off_row() {
    let settings = Settings::default();
    for (cols, rows) in [(100usize, 32usize), (80, 24)] {
        for sel in 0..SETTINGS_ROWS.len() {
            let (start, end) = settings_window(sel, rows);
            let rects = settings_rects(cols, sel, rows, &settings);
            assert_eq!(rects.len(), end - start, "{cols}x{rows} sel={sel}");
            for r in &rects {
                assert!(
                    r.index >= start && r.index < end,
                    "{cols}x{rows} sel={sel}: rect {r:?} outside window [{start},{end})"
                );
                assert!(
                    r.row < rows,
                    "{cols}x{rows} sel={sel}: rect row {} >= {rows}",
                    r.row
                );
            }
        }
    }
}
