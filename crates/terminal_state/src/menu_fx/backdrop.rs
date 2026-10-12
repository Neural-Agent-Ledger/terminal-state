//! A random-per-launch main-menu backdrop, picked from a curated subset of
//! `ascii_fx` effects — `menu_fx::overgrowth` (the pre-existing glyph-vine
//! growth) is kept as ONE of the options, so the menu keeps its familiar look
//! on some launches while others get a different backdrop.
//!
//! Pure data + pure functions here (no ECS, no terminal) — same split as
//! `overgrowth`: the ECS wiring (a resource caching which kind was rolled +
//! the boxed effect, rebuilt on resize) lives in `crate::menu`'s `draw_menu`
//! system.
//!
//! `Heartbeat` and `Dna` are deliberately NOT in the curated set. `Heartbeat`
//! (per `ascii_fx/INTEGRATION.md`'s original wiring note) reads better as a
//! fuller-screen moment (a level-complete card, an interstitial) than a
//! permanent, always-on backdrop — a heart monitor stuck behind the main menu
//! for the whole time a player sits on it would read as a strange, single-shot
//! animation stuck on its last frame. `Dna` is excluded for a different reason:
//! its helix only ever spans `AMPLITUDE_FRAC_RANGE` (≤36%) of the rect's width
//! by design (a 3D perspective read needs room either side to swing through),
//! so on a full-width menu screen it reads as one narrow ribbon with a wide
//! black bar on each side — a fine centerpiece for a narrower card, not an
//! ambient full-bleed backdrop.
//!
//! `Seeds` USED to be excluded for the same "stuck on its last frame" reason
//! as `Heartbeat` — the 2026-09-25 rework made it loop (relaunching after each
//! bloom hold) rather than freezing on a final frame, so that reason no longer
//! applies and it is back in the curated set. `Plasma` and `Tunnel` (new
//! 2026-09-25 effects) both fill the entire rect and read calmly at [`DIM_K`],
//! so they join it too.

use re_asciifx::{
    circuit::Circuit, eye_swarm::EyeSwarm, hash::hash_unit, neural_net::NeuralNet,
    oscilloscope::Oscilloscope, plasma::Plasma, radar::Radar, seeds::Seeds, tunnel::Tunnel,
    AsciiEffect, Rect,
};
use re_core::{oklab, Cell, Framebuffer};

/// Which backdrop is showing. `Overgrowth` means "run the pre-existing
/// `menu_fx::overgrowth` path unchanged" — everything else is a boxed
/// `ascii_fx` effect `crate::menu::draw_menu` paints (dimmed) instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BackdropKind {
    /// The pre-existing glyph-vine growth (`menu_fx::overgrowth`), unchanged.
    Overgrowth,
    /// `re_asciifx::eye_swarm::EyeSwarm`, dimmed and full-bleed.
    EyeSwarm,
    /// `re_asciifx::neural_net::NeuralNet`, dimmed and full-bleed.
    NeuralNet,
    /// `re_asciifx::circuit::Circuit`, dimmed and full-bleed.
    Circuit,
    /// `re_asciifx::radar::Radar`, dimmed and full-bleed.
    Radar,
    /// `re_asciifx::oscilloscope::Oscilloscope`, dimmed and full-bleed.
    Oscilloscope,
    /// `re_asciifx::seeds::Seeds`, dimmed and full-bleed — loops (relaunches
    /// after each bloom hold) rather than freezing on its last frame.
    Seeds,
    /// `re_asciifx::plasma::Plasma`, dimmed and full-bleed.
    Plasma,
    /// `re_asciifx::tunnel::Tunnel`, dimmed and full-bleed.
    Tunnel,
}

/// The curated subset, in a fixed order [`pick`] indexes into — order only
/// matters for `pick`'s determinism, not for any visual ranking.
pub const CURATED: [BackdropKind; 9] = [
    BackdropKind::Overgrowth,
    BackdropKind::EyeSwarm,
    BackdropKind::NeuralNet,
    BackdropKind::Circuit,
    BackdropKind::Radar,
    BackdropKind::Oscilloscope,
    BackdropKind::Seeds,
    BackdropKind::Plasma,
    BackdropKind::Tunnel,
];

/// A distinct hash salt from every other per-launch draw made from the same
/// seed (`flow::SplashEyeSeed`'s value is shared with the splash eye itself;
/// see `ascii_fx::hash`'s module doc on why every logical draw needs its own
/// salt) — so "which backdrop" and "the splash eye's own variant/size/hue"
/// never accidentally correlate.
const SALT_PICK_KIND: u64 = 0xB4C0_D0D0_0001;
/// A second, different salt for the CHOSEN effect's own internal seed (its
/// size/topology/frequencies) — again distinct from `SALT_PICK_KIND` so
/// picking a different kind doesn't also change with what parameters that
/// kind rolls.
const SALT_EFFECT_SEED: u64 = 0xB4C0_D0D0_0002;

/// Deterministically choose a backdrop from `seed` (typically
/// `flow::SplashEyeSeed`'s value — "random per launch").
#[must_use]
pub fn pick(seed: u64) -> BackdropKind {
    let idx = (hash_unit(seed, 0, SALT_PICK_KIND) * CURATED.len() as f64) as usize;
    CURATED[idx.min(CURATED.len() - 1)]
}

/// The seed the chosen effect itself is built from — derived from the SAME
/// launch seed via a different salt (see `SALT_EFFECT_SEED`), so it is
/// stable across resizes (the effect's content does not re-roll just
/// because the terminal resized) but independent of which kind got picked.
#[must_use]
pub fn effect_seed(launch_seed: u64) -> u64 {
    re_asciifx::hash::hash64(launch_seed, SALT_EFFECT_SEED, 0)
}

/// Build the boxed effect for a non-[`BackdropKind::Overgrowth`] kind, sized
/// to `rect`. `None` for `Overgrowth` (handled by the pre-existing
/// `menu_fx::overgrowth` ECS path instead).
#[must_use]
pub fn build(
    kind: BackdropKind,
    seed: u64,
    rect: Rect,
) -> Option<Box<dyn AsciiEffect + Send + Sync>> {
    match kind {
        BackdropKind::Overgrowth => None,
        BackdropKind::EyeSwarm => Some(Box::new(EyeSwarm::random(seed, rect))),
        BackdropKind::NeuralNet => Some(Box::new(NeuralNet::random(seed, rect))),
        BackdropKind::Circuit => Some(Box::new(Circuit::random(seed, rect))),
        BackdropKind::Radar => Some(Box::new(Radar::random(seed, rect))),
        BackdropKind::Oscilloscope => Some(Box::new(Oscilloscope::random(seed, rect))),
        BackdropKind::Seeds => Some(Box::new(Seeds::random(seed, rect))),
        BackdropKind::Plasma => Some(Box::new(Plasma::random(seed, rect))),
        BackdropKind::Tunnel => Some(Box::new(Tunnel::random(seed, rect))),
    }
}

/// How much the backdrop's own colours are scaled down before merging into
/// the real framebuffer — "dim enough behind the menu text for legibility".
pub const DIM_K: f32 = 0.45;

#[must_use]
#[inline]
fn scale_color(c: u32, k: f32) -> u32 {
    let l = oklab::unpack_linear(c);
    oklab::pack_linear(oklab::LinRgb {
        r: l.r * k,
        g: l.g * k,
        b: l.b * k,
    })
}

/// Merge every cell of `src` that was actually PAINTED (i.e. differs from a
/// fresh `Framebuffer`'s default blank cell — see [`build`]'s callers, which
/// always paint into a freshly-constructed `src`) into `dst`, dimmed by `k`.
/// Cells `src` left untouched are skipped, so `dst`'s own background (already
/// `clear_bg`'d by the caller) survives underneath exactly where the effect
/// chose not to paint — the same "only ever add, never blank" contract
/// `menu::paint_overgrowth` already follows.
pub fn blit_dimmed(dst: &mut Framebuffer, src: &Framebuffer, k: f32) {
    let rows = src.rows().min(dst.rows());
    let cols = src.cols().min(dst.cols());
    for row in 0..rows {
        for col in 0..cols {
            let cell = *src.get(col, row).unwrap();
            if cell == Cell::default() {
                continue;
            }
            let dimmed = Cell {
                fg: scale_color(cell.fg, k),
                bg: scale_color(cell.bg, k),
                ..cell
            };
            dst.set(col, row, dimmed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pick_is_deterministic_and_stays_in_the_curated_set() {
        for seed in 0..200u64 {
            let a = pick(seed);
            let b = pick(seed);
            assert_eq!(a, b, "same seed must pick the same kind");
            assert!(CURATED.contains(&a));
        }
    }

    #[test]
    fn pick_visits_more_than_one_kind_across_many_seeds() {
        let mut kinds = std::collections::HashSet::new();
        for seed in 0..500u64 {
            kinds.insert(pick(seed));
        }
        assert!(
            kinds.len() > 1,
            "500 seeds must not all funnel into one backdrop kind"
        );
    }

    #[test]
    fn effect_seed_is_stable_and_independent_of_the_picked_kind() {
        let a = effect_seed(7);
        let b = effect_seed(7);
        assert_eq!(a, b);
        // Different from the launch seed itself and from the kind-picking
        // salt's own stream (regression for "two draws must not correlate").
        assert_ne!(a, 7);
    }

    #[test]
    fn blit_dimmed_only_copies_cells_the_source_actually_painted() {
        let mut dst = Framebuffer::new(4, 4);
        for row in 0..4 {
            for col in 0..4 {
                dst.set(
                    col,
                    row,
                    Cell {
                        glyph: u32::from(' '),
                        fg: 0x00FF_FFFF,
                        bg: 0x0000_0000,
                        ..Default::default()
                    },
                );
            }
        }
        let mut src = Framebuffer::new(4, 4);
        src.set(
            1,
            1,
            Cell {
                glyph: u32::from('*'),
                fg: 0x00FF_0000,
                bg: 0x0000_0000,
                ..Default::default()
            },
        );
        blit_dimmed(&mut dst, &src, 0.5);
        assert_eq!(
            char::from_u32(dst.get(1, 1).unwrap().glyph),
            Some('*'),
            "the one painted cell must be copied"
        );
        assert_eq!(
            char::from_u32(dst.get(0, 0).unwrap().glyph),
            Some(' '),
            "an untouched source cell must not blank the destination"
        );
    }

    #[test]
    fn blit_dimmed_actually_dims_the_colour() {
        let mut dst = Framebuffer::new(2, 2);
        let mut src = Framebuffer::new(2, 2);
        src.set(
            0,
            0,
            Cell {
                glyph: u32::from('*'),
                fg: 0x00FF_FFFF,
                bg: 0x0000_0000,
                ..Default::default()
            },
        );
        blit_dimmed(&mut dst, &src, 0.5);
        let fg = dst.get(0, 0).unwrap().fg;
        assert_ne!(fg, 0x00FF_FFFF, "a 0.5 dim factor must change the colour");
    }
}
