//! `fog` — **P5 "fog as the death-timer"** (reactive-materials spec §6, Story
//! Bible §1/§7).
//!
//! The campaign's fog should TIGHTEN + BLACKEN as the player advances: the
//! world contracts and darkens level→level, so the late campaign feels like the
//! system dying — the literal meaning of "TERMINAL STATE". This module owns the
//! *input* side only: it derives a **campaign-progress scalar** (`0.0` at the
//! first level → `1.0` at the final level) and a pure curve that modulates a
//! level's authored [`Atmosphere`] by that progress. The render fog MATH
//! (`re_render::ascii_shader`) is untouched — we only drive its `fog_far` /
//! `fog_color` inputs.
//!
//! ## Where it sits in the load
//!
//! `enter_playing` (see [`crate::flow`]) calls `level.build_map()`, which
//! stamps the level's *authored* fog onto the [`re_world::Map`] via `set_atmosphere`. We
//! then read that atmosphere back, run [`modulate_atmosphere`], and write the
//! tightened result back with `set_atmosphere` — so **the authored fog is the
//! baseline mood and progress only contracts it further**.
//!
//! ## Back-compat (the hard rule)
//!
//! At `progress == 0.0` (level 1, a single-level catalogue, or a standalone
//! `--dump` with no campaign context) [`modulate_atmosphere`] returns its input
//! **unchanged, byte-for-byte**. So existing levels and golden tests render
//! exactly as before; the tightening is purely additive on the later campaign.
//!
//! ## Tasteful + bounded
//!
//! The curve is deliberately gentle and clamped:
//! * `fog_far` shrinks toward — never below — [`FogCurve::min_far_frac`] of its
//!   baseline (default `0.45`), so the *last* level is hazy/claustrophobic but
//!   still playable, never a near-zero render distance.
//! * `fog_color` darkens toward black by at most [`FogCurve::max_darken`]
//!   (default `0.6`), so the late palette is grim but not a pure-black void.
//! * Both use a slight ease-in ([`FogCurve::ease`]) so the *early* campaign is
//!   barely touched and the contraction concentrates in the back half — the
//!   dread *ramps*.
//!
//! The modulation itself ([`modulate_atmosphere`] / [`progress_for`]) is a pure
//! function of `(authored atmosphere, progress, curve)` — no terminal, no I/O —
//! so it is fully headless-testable. Only the tunable [`FogCurve`] config carries
//! a Bevy `Resource` derive so the ECS can hold/retune it; the math never needs
//! an ECS.

use bevy::prelude::Resource;
use re_world::Atmosphere;

/// The renderer's built-in fog band (`re_render::ascii_shader`'s `FOG_NEAR` /
/// `FOG_FAR`). Mirrored here so that, when progress modulation engages on a
/// level that authored **no** explicit fog distance, we have a concrete
/// baseline to contract from instead of leaving `fog_far = None` (which the
/// renderer would read as the full built-in distance, i.e. no tightening at
/// all). Kept in sync with the render constants; if those change this is a
/// tuning value, not a correctness coupling (the render math still owns the
/// real default).
const BUILTIN_FOG_NEAR: f32 = 1.0;
/// See [`BUILTIN_FOG_NEAR`].
const BUILTIN_FOG_FAR: f32 = 26.0;
/// The renderer's built-in fog tint when none is authored: a fade toward black.
const BUILTIN_FOG_COLOR: u32 = 0x0000_0000;

/// The tunable death-timer fog curve: how hard campaign progress contracts and
/// darkens the fog. Defaults give a gentle, bounded ramp (see the module docs);
/// every field can be overridden if the campaign wants more or less dread.
///
/// Held as a Bevy [`Resource`] so the campaign (or a future settings hook) can
/// retune the death-timer at runtime; `enter_playing` reads it on each level
/// load. The pure modulation logic ([`modulate_atmosphere`]) takes it by `&`, so
/// it stays headlessly testable without an ECS.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct FogCurve {
    /// The floor `fog_far` may shrink to, as a fraction of the baseline far
    /// distance, at `progress == 1.0`. `0.45` ⇒ the final level's render
    /// distance is 45% of its baseline — tight and claustrophobic, but still
    /// playable. Clamped into `(0, 1]`; `1.0` disables the contraction.
    pub min_far_frac: f32,
    /// How far `fog_color` is dragged toward black at `progress == 1.0`, as a
    /// blend fraction `[0, 1]`. `0.6` ⇒ 60% of the way to black — grim, not a
    /// pure-black void. `0.0` disables darkening.
    pub max_darken: f32,
    /// Ease exponent applied to `progress` before it drives the contraction.
    /// `> 1.0` back-loads the effect (early levels barely change, the dread
    /// ramps toward the end); `1.0` is linear. Clamped `>= 1.0`.
    pub ease: f32,
}

impl Default for FogCurve {
    fn default() -> Self {
        // A deliberately gentle, back-loaded ramp: the first third of the
        // campaign is almost untouched, the contraction concentrates in the
        // final stretch (the closing `while(true)`), and even the last level
        // keeps ~45% of its render distance and ~40% of its colour.
        FogCurve {
            min_far_frac: 0.45,
            max_darken: 0.60,
            ease: 1.6,
        }
    }
}

impl FogCurve {
    /// The eased progress `[0, 1]` that actually drives the contraction:
    /// `progress^ease`, with `progress` clamped to `[0, 1]` and `ease` to
    /// `>= 1`. `0` in → `0` out (so back-compat at progress 0 is exact).
    #[inline]
    fn eased(&self, progress: f32) -> f32 {
        let p = progress.clamp(0.0, 1.0);
        let e = self.ease.max(1.0);
        p.powf(e)
    }
}

/// The campaign-progress scalar for a level at catalogue `index` in a campaign
/// of `len` levels: `0.0` at the first level, `1.0` at the final one, linear in
/// between. A single-level (or empty) campaign yields `0.0` — there is no "later"
/// to contract toward, so the level renders with its authored fog untouched.
///
/// This is the *position-in-sequence* progress the spec calls for (level→level
/// tightening), independent of which levels the player has actually solved: the
/// fog reflects *how deep into the dying system you are*, not your save state.
#[inline]
pub fn progress_for(index: usize, len: usize) -> f32 {
    if len <= 1 {
        return 0.0;
    }
    let last = (len - 1) as f32;
    (index.min(len - 1) as f32 / last).clamp(0.0, 1.0)
}

/// Modulate an authored [`Atmosphere`] by campaign `progress` `[0, 1]` under
/// `curve`: shrink `fog_far` (the world contracts) and darken `fog_color`
/// (toward black) as progress rises. The authored values are the BASELINE;
/// progress only ever tightens from there.
///
/// Rules:
/// * `progress <= 0` ⇒ the input is returned **unchanged** (back-compat: a
///   standalone load / the first level / a single-level campaign render exactly
///   as authored — `fog_near`/`fog_far`/`fog_color`/`matrix` byte-identical).
/// * Otherwise, any *unauthored* fog field engaged by the contraction is first
///   seeded from the renderer's built-in baseline (so a level with no `[palette]`
///   fog still tightens), then contracted. `fog_near` and `matrix` are never
///   touched (near distance and ambient density are not part of the death-timer).
///
/// Pure: no I/O, no globals — `(atmo, progress, curve) → atmo`.
pub fn modulate_atmosphere(atmo: &Atmosphere, progress: f32, curve: &FogCurve) -> Atmosphere {
    // Back-compat fast path: no campaign progress ⇒ authored fog, untouched.
    if progress <= 0.0 {
        return atmo.clone();
    }
    let t = curve.eased(progress);
    let mut out = atmo.clone();

    // fog_far: contract toward `min_far_frac` of the baseline. Seed the built-in
    // far distance when the level authored none, so progress tightens even an
    // unfogged level.
    let far_base = atmo.fog_far.unwrap_or(BUILTIN_FOG_FAR);
    let frac = curve.min_far_frac.clamp(0.0, 1.0);
    // lerp(1, frac, t): factor 1 at t=0, `frac` at t=1.
    let factor = 1.0 + (frac - 1.0) * t;
    let new_far = far_base * factor;
    // Keep the band valid: never let `fog_far` collapse to/under `fog_near`.
    let near = atmo.fog_near.unwrap_or(BUILTIN_FOG_NEAR);
    out.fog_far = Some(new_far.max(near + 0.5));

    // fog_color: drag toward black by up to `max_darken` of the way.
    let color_base = atmo.fog_color.unwrap_or(BUILTIN_FOG_COLOR);
    let darken = (curve.max_darken.clamp(0.0, 1.0)) * t;
    out.fog_color = Some(darken_toward_black(color_base, darken));

    out
}

/// Blend a packed `0x00RRGGBB` colour `amount` `[0, 1]` of the way toward black
/// (per-channel scale by `1 - amount`). `0` ⇒ unchanged; `1` ⇒ black.
#[inline]
fn darken_toward_black(rgb: u32, amount: f32) -> u32 {
    let a = amount.clamp(0.0, 1.0);
    let scale = 1.0 - a;
    let r = ((rgb >> 16) & 0xFF) as f32;
    let g = ((rgb >> 8) & 0xFF) as f32;
    let b = (rgb & 0xFF) as f32;
    // round-to-nearest keeps the curve symmetric and stable across runs.
    let rr = (r * scale).round().clamp(0.0, 255.0) as u32;
    let gg = (g * scale).round().clamp(0.0, 255.0) as u32;
    let bb = (b * scale).round().clamp(0.0, 255.0) as u32;
    (rr << 16) | (gg << 8) | bb
}

#[cfg(test)]
mod tests {
    use super::*;

    fn far(a: &Atmosphere) -> f32 {
        a.fog_far.expect("fog_far set")
    }
    fn color(a: &Atmosphere) -> u32 {
        a.fog_color.expect("fog_color set")
    }

    /// progress_for: first level 0.0, last 1.0, midpoint 0.5; degenerate sizes 0.
    #[test]
    fn progress_scalar_spans_zero_to_one() {
        assert_eq!(progress_for(0, 7), 0.0);
        assert_eq!(progress_for(6, 7), 1.0);
        assert!((progress_for(3, 7) - 0.5).abs() < 1e-6);
        // Degenerate campaigns: no "later" to tighten toward → 0.
        assert_eq!(progress_for(0, 1), 0.0);
        assert_eq!(progress_for(0, 0), 0.0);
        // Out-of-range index clamps to the last level (1.0), never panics.
        assert_eq!(progress_for(99, 7), 1.0);
    }

    /// THE spec test: the same authored fog at progress 0.0 vs 0.9 →
    /// SMALLER fog_far (world contracts) and DARKER fog_color (toward black) at
    /// 0.9; at 0.0 the authored values are returned UNCHANGED.
    #[test]
    fn progress_zero_vs_high_contracts_and_darkens() {
        let authored = Atmosphere {
            fog_color: Some(0x0044_88CC),
            fog_near: Some(1.0),
            fog_far: Some(20.0),
            matrix: None,
            ..Atmosphere::default()
        };
        let curve = FogCurve::default();

        // progress 0.0 → byte-identical (back-compat).
        let at0 = modulate_atmosphere(&authored, 0.0, &curve);
        assert_eq!(
            at0, authored,
            "progress 0 must leave authored fog unchanged"
        );

        // progress 0.9 → tighter + darker.
        let at9 = modulate_atmosphere(&authored, 0.9, &curve);
        assert!(
            far(&at9) < far(&at0),
            "fog_far must shrink: {} !< {}",
            far(&at9),
            far(&at0)
        );
        // Darker = each channel <= the baseline, and at least one strictly less.
        let (c0, c9) = (color(&at0), color(&at9));
        let ch = |c: u32, s: u32| (c >> s) & 0xFF;
        for s in [0, 8, 16] {
            assert!(ch(c9, s) <= ch(c0, s), "channel must not brighten");
        }
        assert!(
            c9 < c0,
            "fog_color must darken overall: {c9:06X} !< {c0:06X}"
        );
        // fog_near + matrix are never touched by the death-timer.
        assert_eq!(at9.fog_near, authored.fog_near);
        assert_eq!(at9.matrix, authored.matrix);
    }

    /// The contraction is bounded: even at progress 1.0 the world keeps
    /// `min_far_frac` of its render distance and the colour is not pure black.
    #[test]
    fn full_progress_is_bounded_not_unplayable() {
        let authored = Atmosphere {
            fog_color: Some(0x00FF_FFFF),
            fog_near: Some(1.0),
            fog_far: Some(20.0),
            matrix: None,
            ..Atmosphere::default()
        };
        let curve = FogCurve::default();
        let at1 = modulate_atmosphere(&authored, 1.0, &curve);
        // fog_far still ~45% of baseline (not near-zero).
        assert!(far(&at1) >= 20.0 * curve.min_far_frac - 0.01);
        assert!(far(&at1) > far(&at1).min(2.0)); // sanity: not collapsed
                                                 // Colour darkened by at most max_darken (not pure black).
        let c = color(&at1);
        assert!(c > 0, "final fog colour must not be pure black");
    }

    /// Monotonicity: as progress rises the fog only ever tightens (fog_far
    /// non-increasing, fog_color non-brightening). The dread never reverses.
    #[test]
    fn tightening_is_monotonic() {
        let authored = Atmosphere {
            fog_color: Some(0x0080_C0FF),
            fog_near: Some(1.0),
            fog_far: Some(24.0),
            matrix: None,
            ..Atmosphere::default()
        };
        let curve = FogCurve::default();
        let mut prev_far = f32::INFINITY;
        let mut prev_color_sum = u32::MAX;
        for i in 0..=10 {
            let p = i as f32 / 10.0;
            let a = modulate_atmosphere(&authored, p, &curve);
            let f = a.fog_far.unwrap_or(BUILTIN_FOG_FAR);
            assert!(f <= prev_far + 1e-4, "fog_far must not grow with progress");
            prev_far = f;
            let c = a.fog_color.unwrap_or(0);
            let sum = ((c >> 16) & 0xFF) + ((c >> 8) & 0xFF) + (c & 0xFF);
            assert!(
                sum <= prev_color_sum,
                "fog_color must not brighten with progress"
            );
            prev_color_sum = sum;
        }
    }

    /// A level that authored NO fog still tightens once progress engages (the
    /// built-in baseline is seeded), but stays untouched at progress 0.
    #[test]
    fn unfogged_level_seeds_baseline_when_progress_engages() {
        let none = Atmosphere::default();
        assert!(none.is_none());
        // progress 0 → still none (back-compat: an unfogged level dumps clean).
        assert_eq!(modulate_atmosphere(&none, 0.0, &FogCurve::default()), none);
        // progress > 0 → seeded + contracted from the built-in far baseline.
        let at = modulate_atmosphere(&none, 0.8, &FogCurve::default());
        assert!(
            at.fog_far.unwrap() < BUILTIN_FOG_FAR,
            "must contract built-in far"
        );
        assert!(at.fog_color.is_some(), "darkened colour seeded");
        // fog_near stays None (death-timer never invents a near distance).
        assert_eq!(at.fog_near, None);
    }

    /// darken_toward_black endpoints + a midpoint.
    #[test]
    fn darken_helper_endpoints() {
        assert_eq!(darken_toward_black(0x00FF_FFFF, 0.0), 0x00FF_FFFF);
        assert_eq!(darken_toward_black(0x00FF_FFFF, 1.0), 0x0000_0000);
        assert_eq!(darken_toward_black(0x0080_8080, 0.5), 0x0040_4040);
    }

    /// A disabled curve (frac 1, darken 0) is a no-op at every progress.
    #[test]
    fn disabled_curve_is_identity() {
        let authored = Atmosphere {
            fog_color: Some(0x0012_3456),
            fog_near: Some(2.0),
            fog_far: Some(15.0),
            matrix: Some(0.3),
            ..Atmosphere::default()
        };
        let curve = FogCurve {
            min_far_frac: 1.0,
            max_darken: 0.0,
            ease: 1.0,
        };
        let at = modulate_atmosphere(&authored, 1.0, &curve);
        assert_eq!(at.fog_far, authored.fog_far);
        assert_eq!(at.fog_color, authored.fog_color);
        assert_eq!(at.matrix, authored.matrix);
    }
}
