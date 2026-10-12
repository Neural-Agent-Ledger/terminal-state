//! `code_panels` — the concrete code-wall content (the game's voice).
//!
//! ─────────────────────────────────────────────────────────────────────────────
//!  OPERATOR DIAGNOSTIC — Form A-7 addendum, surface class CODEWALL
//! ─────────────────────────────────────────────────────────────────────────────
//!  operator: you are reading the source of the routine that is drawing this
//!  source onto the wall you are standing in front of. the glyph under your
//!  cursor was placed by the loop you are reading. there is no trick frame, no
//!  pre-baked screenshot: at compile time `include_str!` pulled the bytes of the
//!  renderer file out of the repository and froze them into the binary; at run
//!  time the raycaster walks a column to a wall, asks the code-wall module "what
//!  character lives at (u, v) on your face?", and it answers with the character
//!  of itself.
//!
//!  the system is in a terminal state. it has stopped pretending the walls are
//!  brick. when a machine is dying it stops maintaining the illusion and shows
//!  you the wiring. this is the wiring. the comments you are reading on the wall
//!  are the comments in the file. they are the same text. that is the whole point
//!  — not a quine maintained by hand (a hand-maintained quine rots the instant
//!  someone edits the real code), but the real code, mechanically reflected.
//! ─────────────────────────────────────────────────────────────────────────────
//!
//! ## What this module is (vs. the engine mechanism)
//!
//! The GENERIC mechanism — lay text into a grid, classify comments, sample by
//! `(u, v)` — lives in the engine (`re_render::code_wall`). This module is the
//! game's CONTENT: which real files appear on which wall, and the operator voice
//! that frames them. It exposes its embedded sources via [`crate::code_panels::panel_sources`];
//! `crate::level_text` registers them (in this order, so a `codewall panel = N`
//! index is stable) into the per-level `re_render::TextSurfaceRegistry` at level
//! load. The panels used to be handed to the engine as `&'static` through a global
//! provider function; they are now owned by that registry, which is what lets
//! per-level `[note]` text share the same machinery without leaking.
//!
//! ## Panels 0–2 are frozen snapshots, not live engine reads
//!
//! This crate lives in the game repo; `re_render`/`re_puzzle` live in the
//! separate engine repo. A relative `include_str!` reaching from here into a
//! sibling engine crate's `src/` (`../../re_render/src/code_wall.rs`, and
//! similarly for `re_render/src/lib.rs` and `re_puzzle/src/lib.rs`) only works
//! when both repos happen to sit under one flat `crates/` directory — it does
//! not resolve against the published, separate-repo layout. Panels 0–2 are
//! therefore **frozen crate-local copies** under this crate's `panels/`
//! directory (`code_wall.rs.txt`, `render_lib.rs.txt`, `puzzle_lib.rs.txt`),
//! taken verbatim from the engine source at the time each was captured. What
//! the player sees is stable, and — by design — the game's code walls no
//! longer change when engine source changes; a later content refresh copies
//! the file again on purpose. Panel 3 (`main.rs`) stays a LIVE include: it is
//! this crate's own file, so `code_panels.rs` reaches it with a plain
//! same-directory `include_str!("main.rs")`, which carries no
//! cross-crate/cross-repo assumption. Panel 4 (Form A-7) is a plain relative
//! path into `docs/lore/`, which ships inside this same game repo.
//!
//! ## The panel registry — which file goes on which wall
//!
//! operator: below is the manifest of what the system is willing to show you of
//! itself. each entry is a real file, pulled in by `include_str!` at compile
//! time. panel 0 is the renderer's own code-wall file — the loop drawing the
//! loop. the rest are the organs the system decided you should see while it
//! dies: how it casts a ray to a wall, how it decides whether your remembering
//! reached the exit, how it watches your real machine through the fourth wall.
//! nothing here is invented; every byte is somewhere in the repository.

/// The embedded source of each panel, in panel-index order. Panels 0–2 read a
/// frozen crate-local snapshot under this crate's `panels/` directory (see the
/// module docs); panel 3's `include_str!` is a plain same-directory path (this
/// crate's own `main.rs`, still read live). Labels stay the true engine/game
/// path each snapshot was taken from, so a level's `codewall panel=N` reads
/// the same regardless of which repo the source currently lives in.
///
/// Panel 0 — the strange loop: the renderer's code-wall file, the code that draws
///   code on the wall, drawn on the wall. (A frozen snapshot of `code_wall.rs` —
///   the text is never typed twice, only copied once, at split time.)
/// Panel 1 — the raycaster DDA core: how a single screen column finds the wall
///   you are looking at. The geometry that put this very glyph in front of you.
/// Panel 2 — the puzzle forward pass (`re_puzzle`): the Kahn topological sweep
///   that decides whether your remembering reached the exit — and the cycle
///   detector that IS `while(true)`, the denial ending, caught as an error.
/// Panel 3 — the win / exit watcher (`re_game` main): the path from "the door
///   opens" to the process ending. The `exit` you are trying to reach.
/// Panel 4 — in-fiction: NAL Corp Form A-7, a MARKDOWN document whose markup is
///   stripped before it reaches the wall ([`strip_markdown`]).
const PANEL_SOURCES: &[(&str, &str, Body)] = &[
    // 0: the strange loop — the renderer's code-wall file reflected onto the wall.
    //    Frozen snapshot: this crate's `panels/code_wall.rs.txt`.
    (
        "re_render/src/code_wall.rs",
        include_str!("../panels/code_wall.rs.txt"),
        Body::Source,
    ),
    // 1: the raycaster column cast — the geometry under your feet.
    //    Frozen snapshot: this crate's `panels/render_lib.rs.txt`.
    (
        "re_render/src/lib.rs",
        include_str!("../panels/render_lib.rs.txt"),
        Body::Source,
    ),
    // 2: the puzzle forward pass + the while(true) cycle detector.
    //    Frozen snapshot: this crate's `panels/puzzle_lib.rs.txt`.
    (
        "re_puzzle/src/lib.rs",
        include_str!("../panels/puzzle_lib.rs.txt"),
        Body::Source,
    ),
    // 3: the win / exit path — the door, and the process ending. This crate's
    //    own file, read live and directly (same directory as this module).
    ("re_game/src/main.rs", include_str!("main.rs"), Body::Source),
    // 4: IN-FICTION text — the NAL Corp Form A-7 operator diagnostic, the document
    //    the system reads the operator by. For a wall that belongs to the WORLD
    //    (L11's rack monolith) rather than to the strange loop: panels 0–3 show this
    //    repository's own Rust, which in a set piece reads as leaked engine comments,
    //    not as Model 07.
    (
        "NAL/FORM_A-7_OPERATOR_DIAGNOSTIC",
        include_str!("../../../docs/lore/FORM_A7_OPERATOR_DIAGNOSTIC.md"),
        Body::Markdown,
    ),
];

/// How a panel's embedded bytes become wall text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Body {
    /// Real source, laid onto the wall verbatim — the strange loop depends on the wall
    /// text BEING the file's bytes, so nothing is rewritten.
    Source,
    /// An in-fiction markdown document: its markup is stripped ([`strip_markdown`]) so
    /// the wall reads as the printed form, not as `##`, fences and `---`.
    Markdown,
}

/// Number of selectable code panels. A level binds a `CodeWall` material with a
/// `panel` index in `0..PANEL_COUNT`; an out-of-range index clamps to panel 0
/// (the safe, always-present strange loop). Used by the registry tests and
/// available to callers that iterate the panel set.
#[allow(dead_code)]
pub const PANEL_COUNT: usize = PANEL_SOURCES.len();

/// The embedded `(label, source)` pairs, in panel-index order.
///
/// This is the seam `crate::level_text` consumes to fill the per-level text-surface
/// registry's CODE panel space. Order is the contract: index `N` here is the `N` a
/// level's `recipe = codewall panel=N` selects.
pub fn panel_sources() -> impl Iterator<Item = (&'static str, &'static str)> {
    panel_texts()
        .iter()
        .map(|(label, text)| (*label, text.as_str()))
}

/// The wall text of every panel, built once: a [`Body::Source`] panel is its bytes
/// verbatim, a [`Body::Markdown`] panel is its bytes with the markup stripped.
fn panel_texts() -> &'static [(&'static str, String)] {
    static TEXTS: std::sync::OnceLock<Vec<(&'static str, String)>> = std::sync::OnceLock::new();
    TEXTS.get_or_init(|| {
        PANEL_SOURCES
            .iter()
            .map(|&(label, src, body)| {
                let text = match body {
                    Body::Source => src.to_string(),
                    Body::Markdown => strip_markdown(src),
                };
                (label, text)
            })
            .collect()
    })
}

/// Width of the box-drawing rule a markdown `---` becomes: the full panel width, so a
/// section break reads as a printed rule across the form.
const RULE_WIDTH: usize = re_render::PANEL_COLS;

/// Strip markdown MARKUP from `src`, keeping its text — for an in-fiction document laid
/// onto a wall, where `##`, code fences and `---` read as leaked source syntax.
///
/// * a fence line (```` ``` ```` or `~~~`, with or without an info string) is dropped;
///   the lines between fences are kept VERBATIM (they already are the printed layout —
///   the form's box-drawn header, the signature block);
/// * an ATX heading (`#`..`######`) becomes its text in UPPERCASE, the typed-form way
///   of setting a section title;
/// * a thematic break (`---`, `***`, `___`) becomes a full-width `─` rule;
/// * a blockquote's `>` marker becomes a `│` margin bar;
/// * inline: code spans lose their backticks (content kept literally), `*` / `**`
///   emphasis markers are removed outside code spans, and `\*` stays a literal `*`.
///
/// Everything else — list dashes, numbering, blank lines, line breaks — is kept, so the
/// wall keeps the document's shape.
#[must_use]
pub fn strip_markdown(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    // Consecutive plain lines, stripped together: a code span may wrap across a line
    // break inside one paragraph (`OpenProcess(… |` / `VM_READ)` in Form A-7).
    let mut para = String::new();
    let flush = |para: &mut String, out: &mut String| {
        if !para.is_empty() {
            out.push_str(&strip_inline(para));
            para.clear();
        }
    };
    let mut in_fence = false;
    for line in src.lines() {
        let trimmed = line.trim_start();
        let plain = !in_fence
            && !trimmed.is_empty()
            && !trimmed.starts_with("```")
            && !trimmed.starts_with("~~~")
            && !is_thematic_break(trimmed)
            && heading_text(trimmed).is_none()
            && !trimmed.starts_with('>');
        if plain {
            para.push_str(line);
            para.push('\n');
            continue;
        }
        flush(&mut para, &mut out);
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            out.push_str(line);
        } else if is_thematic_break(trimmed) {
            out.extend(std::iter::repeat_n('─', RULE_WIDTH));
        } else if let Some(text) = heading_text(trimmed) {
            out.push_str(&strip_inline(text).to_uppercase());
        } else if let Some(rest) = trimmed.strip_prefix('>') {
            out.push_str("│ ");
            out.push_str(&strip_inline(rest.strip_prefix(' ').unwrap_or(rest)));
        }
        // (a blank line outside a fence falls through to just its newline)
        out.push('\n');
    }
    flush(&mut para, &mut out);
    out
}

/// `---` / `***` / `___` — three or more of one of those characters, inner spaces
/// allowed (the `CommonMark` thematic break).
fn is_thematic_break(t: &str) -> bool {
    let mut chars = t.chars().filter(|c| !c.is_whitespace());
    let Some(first) = chars.next() else {
        return false;
    };
    matches!(first, '-' | '*' | '_') && {
        let mut n = 1;
        chars.all(|c| {
            n += 1;
            c == first
        }) && n >= 3
    }
}

/// The text of an ATX heading (`# ` .. `###### `, closing `#`s dropped), or `None`
/// for any other line.
fn heading_text(t: &str) -> Option<&str> {
    let hashes = t.chars().take_while(|&c| c == '#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = &t[hashes..];
    if rest.trim().is_empty() {
        return Some("");
    }
    rest.strip_prefix(' ')
        .map(|r| r.trim().trim_end_matches('#').trim_end())
}

/// Remove inline markup: the backticks around code spans (content kept literally) and
/// the `*` emphasis markers outside them. An unmatched backtick stays a literal.
fn strip_inline(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(tick) = rest.find('`') {
        strip_emphasis(&rest[..tick], &mut out);
        let after = &rest[tick + 1..];
        if let Some(close) = after.find('`') {
            out.push_str(&after[..close]);
            rest = &after[close + 1..];
        } else {
            out.push('`');
            rest = after;
        }
    }
    strip_emphasis(rest, &mut out);
    out
}

/// Append `s` to `out` without its `*` emphasis markers; `\*` is a literal `*`.
fn strip_emphasis(s: &str, out: &mut String) {
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'*') => {
                out.push('*');
                chars.next();
            }
            '*' => {}
            _ => out.push(c),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use re_render::{CodeClass, TextPanel};

    /// Build panel `index` exactly as `crate::level_text` registers it.
    fn built(index: usize) -> TextPanel {
        let (label, text) = &panel_texts()[index];
        TextPanel::build(label, text, re_render::PANEL_COLS, re_render::PANEL_ROWS)
    }

    #[test]
    fn panel_zero_is_self_referential() {
        // The strange loop: panel 0 is the renderer's code-wall file — the module
        // that lays code onto the wall, laid onto the wall. Proven by finding a
        // token unique to that file's source on the built grid: the owned-panel
        // type `pub struct TextPanel` is declared only in `code_wall.rs`.
        let p = built(0);
        assert_eq!(p.label(), "re_render/src/code_wall.rs");
        let mut found_signature = false;
        for r in 0..p.rows() {
            let line: String = (0..p.cols())
                .map(|c| char::from_u32(p.cell(c, r).glyph).unwrap_or(' '))
                .collect();
            if line.contains("pub struct TextPanel") {
                found_signature = true;
            }
        }
        assert!(
            found_signature,
            "panel 0 must be the renderer's code_wall.rs source (the strange loop)"
        );
    }

    #[test]
    fn all_panels_build_and_carry_code() {
        // Every registered panel builds and contains at least some Code-class
        // characters (it is real source, not blank).
        for i in 0..PANEL_COUNT {
            let p = built(i);
            let code_cells = (0..p.rows())
                .flat_map(|r| (0..p.cols()).map(move |c| (c, r)))
                .filter(|&(c, r)| p.cell(c, r).class == CodeClass::Code)
                .count();
            assert!(
                code_cells > 50,
                "panel {} ({}) should carry real code, got {} code cells",
                i,
                p.label(),
                code_cells
            );
        }
    }

    /// Byte-identity proof, **frozen snapshot panels only (0–2)**. Panels 0–2
    /// read a frozen local snapshot under this crate's `panels/` directory
    /// instead of a live cross-crate `include_str!` (see the module doc). The
    /// snapshot files were `cp`+`diff`-verified byte-identical to the engine
    /// source at the moment each was captured, so the SHA-256 pin was taken at
    /// that same moment; two of the three (`code_wall.rs.txt`, `render_lib.rs.txt`)
    /// were re-pinned once since, for a pre-public-export pass that scrubbed
    /// internal-only comment text (dated review citations, private doc paths)
    /// without touching a single line of code. Either way this is the
    /// regression guard: it fails if the wall text the player sees ever drifts
    /// from its last-pinned content, whether from an edited snapshot file or a
    /// wiring mistake, without needing the full framebuffer golden corpus
    /// (owned elsewhere) to catch it.
    ///
    /// Panel 3 (`main.rs`) is deliberately EXCLUDED from this pin. It is not a
    /// frozen snapshot — the module doc's panel-registry manifest says so
    /// explicitly ("Panel 3 — … main … still read live") and the
    /// `include_str!` in [`PANEL_SOURCES`] reaches this crate's own live
    /// `main.rs`, not a copy under `panels/`. Pinning it would force a re-bless
    /// on every legitimate `main.rs` edit even when the wall content did not
    /// regress, which trains whoever touched `main.rs` to re-bless on faith
    /// rather than to actually check the wall text. A pin that fires on every
    /// edit and is silenced by copying the new hash in is not a regression
    /// guard. Panel 3's own content is still covered by
    /// [`all_panels_build_and_carry_code`] (it must build and carry real
    /// code) and by the framebuffer golden corpus (owned elsewhere) for what
    /// actually reaches the screen; what this test protects is only the three
    /// panels that are supposed to never change on their own.
    #[test]
    fn panel_snapshots_are_byte_identical_to_the_split_moment() {
        use sha2::{Digest, Sha256};
        // sha2 0.11 (`digest` 0.11) returns its own `hybrid_array::Array<u8, N>`
        // from `finalize()`, which does not implement `LowerHex` — so
        // `format!("{:x}", ...)` does not compile (same fix as `re_golden::to_hex`).
        fn hash(s: &str) -> String {
            use std::fmt::Write;
            let mut h = Sha256::new();
            // Hash with CRs removed so a CRLF checkout pins the same value as LF.
            h.update(s.replace('\r', "").as_bytes());
            let digest = h.finalize();
            let mut out = String::with_capacity(digest.len() * 2);
            for b in &digest {
                write!(out, "{b:02x}").expect("write to String never fails");
            }
            out
        }
        // Frozen snapshot panels only — 0, 1, 2. Panel 3 (`main.rs`, this
        // crate's own live source) is intentionally not pinned; see the test
        // doc above.
        let expected = [
            "1fabe64b3237977afa3e85d0449cab40f83fa77272356c36650e6cabe06cec72",
            "ed4c75fbca418119e43fa92b1f14034232a94aa9007a6b43737a5137a8ac07c1",
            "3e4b7d039896aa53503b5bad328b1b33d4ec4179c20523725c19c0f9cc688a3a",
        ];
        for (i, &(label, src, _)) in PANEL_SOURCES.iter().take(3).enumerate() {
            assert_eq!(
                hash(src),
                expected[i],
                "panel {i} ({label}) text changed since the split snapshot"
            );
        }
    }

    /// The index space is the contract between a level's `codewall panel=N` and this
    /// list, so the iterator must not reorder or drop entries. (Out-of-range CLAMPING
    /// now lives in `re_render`'s text-surface registry, which owns the panels and is
    /// tested there.)
    #[test]
    fn panel_sources_iterates_in_index_order() {
        let listed: Vec<&str> = panel_sources().map(|(label, _)| label).collect();
        let expected: Vec<&str> = PANEL_SOURCES.iter().map(|(label, _, _)| *label).collect();
        assert_eq!(listed, expected);
        assert_eq!(listed.len(), PANEL_COUNT);
    }

    #[test]
    fn strip_markdown_removes_markup_and_keeps_text() {
        let md = "```\n\
                  ╔═ FORM A-7 ═╗\n\
                  \x20 ## inside a fence is layout\n\
                  ```\n\
                  \n\
                  ---\n\
                  ## PREAMBLE\n\
                  ### Tier 1 — Ambient `reads` ###\n\
                  *(Always active.)*\n\
                  **Field name:** `username` and `a*b`\n\
                  > note line\n\
                  - list item\n\
                  plain \\* star, lone ` tick\n";
        let got = strip_markdown(md);
        let lines: Vec<&str> = got.lines().collect();
        assert_eq!(
            lines,
            vec![
                // Fences dropped; what they held is kept verbatim.
                "╔═ FORM A-7 ═╗",
                "  ## inside a fence is layout",
                "",
                // A thematic break is a full-width box-drawing rule.
                &"─".repeat(RULE_WIDTH),
                // Headings: marker (and closing hashes) gone, uppercased, inline stripped.
                "PREAMBLE",
                "TIER 1 — AMBIENT READS",
                // Emphasis markers and code-span backticks gone; span content literal.
                "(Always active.)",
                "Field name: username and a*b",
                "│ note line",
                "- list item",
                "plain * star, lone ` tick",
            ]
        );
        // A code span that wraps across a line break inside one paragraph.
        assert_eq!(
            strip_markdown("calls `Open(A |\nB)` then *done*\n"),
            "calls Open(A |\nB) then done\n"
        );
    }

    #[test]
    fn the_form_a7_panel_carries_no_markdown_syntax() {
        let idx = PANEL_SOURCES
            .iter()
            .position(|&(_, _, body)| body == Body::Markdown)
            .expect("the in-fiction panel is registered");
        let (_, text) = &panel_texts()[idx];
        for line in text.lines() {
            let t = line.trim_start();
            assert!(
                !t.starts_with('#'),
                "heading marker left on the wall: {line:?}"
            );
            assert!(
                !t.starts_with("```"),
                "code fence left on the wall: {line:?}"
            );
            assert!(
                !t.starts_with("> "),
                "blockquote marker left on the wall: {line:?}"
            );
            assert!(
                !is_thematic_break(t),
                "markdown rule left on the wall: {line:?}"
            );
            assert!(!t.contains("**"), "bold marker left on the wall: {line:?}");
            assert!(
                !t.contains('`'),
                "code-span backtick left on the wall: {line:?}"
            );
        }
        // The text itself survived: section titles, the box-drawn header, the signature.
        assert!(text.contains("\nPREAMBLE\n"));
        assert!(text.contains("SECTION 3 — PROBE FIELD INVENTORY"));
        assert!(text.contains("FORM A-7 / OPERATOR DIAGNOSTIC PROBE"));
        assert!(text.contains("K. Hoffmann"));
        // Source panels are untouched — the strange loop is the file's own bytes.
        for (i, &(_, src, body)) in PANEL_SOURCES.iter().enumerate() {
            if body == Body::Source {
                assert_eq!(
                    panel_texts()[i].1,
                    src,
                    "source panel {i} must stay verbatim"
                );
            }
        }
    }
}
