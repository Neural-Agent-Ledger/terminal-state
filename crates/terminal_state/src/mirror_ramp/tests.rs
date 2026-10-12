//! Unit tests for the MirrorClarity ramp (W3 3.1).
//!
//! The mapping function is pinned to the design curve from the story-expansion
//! plan (`clarity = shards/7 · 0.85`, `+0.15` on the campaign's final level), and
//! the resource-driving system is exercised headlessly — no renderer, no
//! terminal, no save file.

use super::*;
use bevy::state::app::StatesPlugin;
use bevy::MinimalPlugins;

/// Tolerance for the ramp's float comparisons (the curve is a ratio, so exact
/// equality only holds at the endpoints).
const EPS: f32 = 1e-4;

/// THE CURVE, at the four points the plan names: 0 shards → 0, 4 → ~0.49,
/// 7 → 0.85, 7 + final level → 1.0.
#[test]
fn clarity_curve_matches_the_designed_points() {
    assert_eq!(clarity_for(0, false), 0.0, "no shards ⇒ a noisy reflection");
    let four = clarity_for(4, false);
    assert!(
        (four - 4.0 / 7.0 * 0.85).abs() < EPS,
        "4 shards ⇒ 4/7·0.85, got {four}"
    );
    assert!((four - 0.4857).abs() < 1e-3, "4 shards ≈ 0.49, got {four}");
    assert!(
        (clarity_for(7, false) - SHARD_CEILING).abs() < EPS,
        "all seven shards ⇒ the 0.85 ceiling"
    );
    assert!(
        (clarity_for(7, true) - 1.0).abs() < EPS,
        "seven shards on the final level ⇒ fully crisp"
    );
}

/// The ramp never goes backwards as shards accumulate, and the final-level bonus
/// is strictly a bonus at every shard count.
#[test]
fn clarity_is_monotone_and_the_bonus_only_adds() {
    let mut prev = -1.0_f32;
    for n in 0..=SHARD_TOTAL {
        let c = clarity_for(n, false);
        assert!(
            c > prev,
            "clarity must rise with shards ({n}: {c} <= {prev})"
        );
        assert!(
            clarity_for(n, true) > c,
            "the final-level bonus must add at {n} shards"
        );
        prev = c;
    }
}

/// Out-of-range shard counts saturate instead of overshooting, and the result is
/// always a valid clarity in `[0, 1]`.
#[test]
fn clarity_is_clamped_at_both_ends() {
    assert!(
        (clarity_for(99, false) - SHARD_CEILING).abs() < EPS,
        "more shards than exist cannot exceed the ceiling"
    );
    assert!(
        (clarity_for(99, true) - 1.0).abs() < EPS,
        "ceiling + bonus is exactly 1.0, never more"
    );
    for n in 0..=12 {
        for final_level in [false, true] {
            let c = clarity_for(n, final_level);
            assert!((0.0..=1.0).contains(&c), "clarity {c} out of range");
        }
    }
}

/// The finale is identified through the MANIFEST (last entry), not a hardcoded id.
#[test]
fn final_campaign_level_is_the_manifest_tail() {
    let manifest = campaign_manifest();
    assert!(!manifest.is_empty(), "the campaign manifest must parse");
    let last = &manifest.levels()[manifest.len() - 1];
    assert!(
        is_final_campaign_level(&last.id),
        "the manifest's last level ('{}') is the finale",
        last.id
    );
    for l in &manifest.levels()[..manifest.len() - 1] {
        assert!(
            !is_final_campaign_level(&l.id),
            "'{}' is not the finale",
            l.id
        );
    }
    assert!(
        !is_final_campaign_level("12-mirror"),
        "a demo level is never the campaign finale"
    );
}

/// Shards are derived from solved ids: L1–L7 each carry one, L8+ carry none.
#[test]
fn shards_held_counts_only_the_seven_shard_levels() {
    let mut p = Progress::default();
    assert_eq!(shards_held(&p), 0, "a fresh save holds no shards");
    for id in ["L1-first-pulse", "L2-two-make-three", "L3-full-spectrum"] {
        p.mark_solved(id);
    }
    assert_eq!(shards_held(&p), 3, "three shard levels ⇒ three shards");
    // L8..L12 recontextualise; they add no new shard.
    for id in ["L8-cold-room", "L12-closing"] {
        p.mark_solved(id);
    }
    assert_eq!(shards_held(&p), 3, "L8+ adds no shard");
    for id in [
        "L4-the-angle",
        "L5-soft-footstep",
        "L6-polysemantic",
        "L7-hidden-depths",
    ] {
        p.mark_solved(id);
    }
    assert_eq!(shards_held(&p), SHARD_TOTAL, "L1..L7 completes the name");
}

/// The override is installed only for a live CAMPAIGN level, in Playing or Pause.
#[test]
fn override_applies_to_campaign_levels_only() {
    let mut p = Progress::default();
    for id in ["L1-first-pulse", "L2-two-make-three"] {
        p.mark_solved(id);
    }
    let expected = clarity_for(2, false);

    assert_eq!(
        override_for(GameState::Playing, &p, Some("L3-full-spectrum")),
        Some(expected),
        "a campaign level ramps"
    );
    assert_eq!(
        override_for(GameState::Pause, &p, Some("L3-full-spectrum")),
        Some(expected),
        "pausing must not pop the mirror"
    );
    assert_eq!(
        override_for(GameState::Playing, &p, Some("12-mirror")),
        None,
        "the mirror DEMO keeps its authored clarity"
    );
    assert_eq!(
        override_for(GameState::Playing, &p, Some("03")),
        None,
        "an archive room keeps its authored clarity"
    );
    assert_eq!(
        override_for(GameState::Playing, &p, None),
        None,
        "no live level ⇒ no override"
    );
    for state in [
        GameState::Splash,
        GameState::Menu,
        GameState::LevelSelect,
        GameState::Diving,
        GameState::Won,
    ] {
        assert_eq!(
            override_for(state, &p, Some("L3-full-spectrum")),
            None,
            "{state:?} must leave mirrors authored"
        );
    }
}

/// The finale gets the full ramp: seven shards + L12 ⇒ 1.0.
#[test]
fn override_on_the_finale_with_the_whole_name_is_fully_crisp() {
    let mut p = Progress::default();
    for l in campaign_manifest().levels() {
        p.mark_solved(&l.id);
    }
    let got = override_for(GameState::Playing, &p, Some("L12-closing")).expect("finale ramps");
    assert!((got - 1.0).abs() < EPS, "expected 1.0, got {got}");
}

/// Build a headless app with only the ramp plugin: no renderer, no terminal, no
/// flow. `MirrorClarity` is init'd by the plugin itself.
fn ramp_app(level_path: &str) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(StatesPlugin)
        .init_state::<GameState>()
        .init_resource::<CurrentLevel>()
        .init_resource::<Progress>()
        .insert_resource(LevelCatalog {
            paths: vec![level_path.to_string()],
        })
        .add_plugins(MirrorRampPlugin);
    app
}

fn go(app: &mut App, s: GameState) {
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(s);
    app.update();
    app.update(); // one more frame so Update runs after the transition applied
}

/// The SYSTEM end-to-end: entering a campaign level installs the ramp value;
/// leaving for the menu clears it back to the authored clarity.
#[test]
fn system_installs_and_clears_the_override() {
    let mut app = ramp_app("levels/L12-closing.lvl");
    {
        let mut p = app.world_mut().resource_mut::<Progress>();
        for id in [
            "L1-first-pulse",
            "L2-two-make-three",
            "L3-full-spectrum",
            "L4-the-angle",
            "L5-soft-footstep",
            "L6-polysemantic",
            "L7-hidden-depths",
        ] {
            p.mark_solved(id);
        }
    }
    app.update();
    assert_eq!(
        app.world().resource::<MirrorClarity>().override_value,
        None,
        "the splash leaves mirrors authored"
    );

    go(&mut app, GameState::Playing);
    let v = app
        .world()
        .resource::<MirrorClarity>()
        .override_value
        .expect("playing the finale installs the override");
    assert!((v - 1.0).abs() < EPS, "expected 1.0 on L12, got {v}");

    go(&mut app, GameState::Menu);
    assert_eq!(
        app.world().resource::<MirrorClarity>().override_value,
        None,
        "leaving play clears the override"
    );
}

/// A non-campaign level (an archive room, standing in for any technique demo)
/// is never overridden by the system either — it shows its own authored clarity.
/// (`override_applies_to_campaign_levels_only` above pins the same rule for the
/// pure `override_for` mapping; this is the system end-to-end.)
#[test]
fn system_leaves_a_demo_level_authored() {
    let mut app = ramp_app("levels/03.lvl");
    go(&mut app, GameState::Playing);
    assert_eq!(
        app.world().resource::<MirrorClarity>().override_value,
        None,
        "an archive room keeps its authored clarity while played"
    );
}
