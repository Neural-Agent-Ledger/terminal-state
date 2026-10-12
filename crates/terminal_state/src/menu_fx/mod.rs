//! Main-menu visual effects layer — currently just [`crate::menu_fx::overgrowth`], the
//! deterministic glyph-vine growth. A second design (a slower bit-rot decay)
//! could land here later as its own sibling module, following the same
//! pattern as `re_glitch`'s per-phase modules — such designs share one common
//! foundation and would differ only in one `menu_fx/*.rs` file each.
//!
//! Everything here is pure data + pure functions (no ECS, no terminal) so the
//! growth simulation is exhaustively unit-testable and reproducible from a
//! `(seed, step)` pair alone — the wiring into `Res<Framebuffer>` lives in
//! `crate::menu`'s `draw_menu` system, which owns the ECS side of this file
//! partition.
//!
//! `overgrowth` itself lives in `re_asciifx` — it is engine-pure and has no
//! menu-specific type, so it is re-exported here rather than kept as a
//! game-local module, and every `crate::menu_fx::overgrowth::…` path in the
//! game crate keeps compiling unchanged.

pub mod backdrop;
pub use re_asciifx::overgrowth;
