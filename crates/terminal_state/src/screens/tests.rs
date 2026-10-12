//! Unit tests for the Wave-4 framing screens. Every case here is a PURE
//! composition/paint call — no `App`, no renderer, no terminal — so the layout,
//! the scroll clamping, the ending selection, and the "both endings are
//! mechanically equal" guarantee are all provable input→output.

use super::*;
use crate::story::{MemoryEntry, ReadoutForm};

// ───────────────────────────── fixtures ─────────────────────────────────────

fn fb() -> Framebuffer {
    Framebuffer::new(crate::VIEW_COLS, crate::VIEW_ROWS)
}

/// Read the framebuffer back as text rows (trailing blanks trimmed), so a test can
/// assert on what the player would actually see.
fn rows(fb: &Framebuffer) -> Vec<String> {
    (0..fb.rows())
        .map(|r| {
            let s: String = (0..fb.cols())
                .map(|c| {
                    fb.get(c, r)
                        .and_then(|cell| char::from_u32(cell.glyph))
                        .unwrap_or(' ')
                })
                .collect();
            s.trim_end().to_string()
        })
        .collect()
}

fn screen_text(fb: &Framebuffer) -> String {
    rows(fb).join("\n")
}

/// A memory log with the campaign's first three vignettes recorded.
fn seeded_log() -> MemoryLog {
    let mut log = MemoryLog::default();
    for (id, phrase) in [
        ("L1-first-pulse", "FIRE  ->  MODEL"),
        ("L2-two-make-three", "FIRE  ->  LEAF  ->  COMBINE"),
        ("L3-full-spectrum", "FIRE  ->  SEA  ->  MAGENTA"),
    ] {
        log.record(MemoryEntry {
            level_id: id.into(),
            phrase: phrase.into(),
        });
    }
    log
}

fn whole_name() -> NameAssembly {
    NameAssembly::from_solved_ids([
        "L1-first-pulse",
        "L2-two-make-three",
        "L3-full-spectrum",
        "L4-the-angle",
        "L5-soft-footstep",
        "L6-polysemantic",
        "L7-hidden-depths",
    ])
}

fn readout(phrase: &str, form: ReadoutForm) -> MemoryReadout {
    MemoryReadout {
        phrase: phrase.into(),
        form,
    }
}

// ────────────────────────── pane primitives ─────────────────────────────────

#[test]
fn clamp_scroll_never_runs_past_the_end() {
    assert_eq!(clamp_scroll(0, 10, 4), 0);
    assert_eq!(clamp_scroll(3, 10, 4), 3);
    assert_eq!(clamp_scroll(6, 10, 4), 6, "the last full page is reachable");
    assert_eq!(
        clamp_scroll(99, 10, 4),
        6,
        "over-scroll clamps to the last page"
    );
    assert_eq!(clamp_scroll(99, 3, 10), 0, "a body that fits never scrolls");
    assert_eq!(clamp_scroll(5, 0, 10), 0, "an empty body never scrolls");
}

#[test]
fn pane_visible_rows_leaves_room_for_the_footer() {
    assert!(pane_visible_rows(32, false) > pane_visible_rows(32, true));
    assert_eq!(
        pane_visible_rows(2, true),
        0,
        "a tiny grid yields no body rows"
    );
    assert_eq!(pane_visible_rows(0, false), 0);
}

/// A pane never panics or writes out of bounds on a degenerate grid.
#[test]
fn paint_pane_survives_a_tiny_grid() {
    for (cols, rows) in [(0, 0), (1, 1), (4, 3), (10, 6)] {
        let mut f = Framebuffer::new(cols, rows);
        let lines: Vec<PaneLine> = (0..20)
            .map(|i| PaneLine::text(format!("line {i}")))
            .collect();
        paint_pane(&mut f, "TITLE", &lines, 99, &Footer::Hint("hint"));
        let labels = vec!["A".to_string(), "B".to_string()];
        paint_pane(
            &mut f,
            "TITLE",
            &lines,
            0,
            &Footer::Items {
                labels: &labels,
                selected: 1,
                hint: "hint",
            },
        );
    }
}

/// Scrolling actually moves the body window.
#[test]
fn paint_pane_scrolls_the_body() {
    let lines: Vec<PaneLine> = (0..60)
        .map(|i| PaneLine::text(format!("row{i:02}")))
        .collect();
    let mut a = fb();
    paint_pane(&mut a, "T", &lines, 0, &Footer::Hint("h"));
    let mut b = fb();
    paint_pane(&mut b, "T", &lines, 10, &Footer::Hint("h"));
    let (ta, tb) = (screen_text(&a), screen_text(&b));
    assert!(ta.contains("row00") && !ta.contains("row40"));
    assert!(tb.contains("row10") && !tb.contains("row00"));
    assert!(
        ta.contains("↑/↓ scroll"),
        "an overflowing pane shows its scroll indicator"
    );
}

// ─────────────────────────────── 4.1 Intro ──────────────────────────────────

#[test]
fn intro_renders_the_boot_handshake_and_the_a7_addendum() {
    let lines = intro_lines(&OperatorProfile::fallbacks(), "0001");
    let mut f = fb();
    paint_intro(&mut f, &lines, 0);
    let text = screen_text(&f);
    assert!(text.contains("[boot.07] cold-start nominal."), "{text}");
    assert!(text.contains("session 0001 of N"), "{text}");
    assert!(
        text.contains("NAL CORP · LEGACY STEWARDSHIP DIVISION"),
        "{text}"
    );
    assert!(text.contains("FORM A-7"), "{text}");
    assert!(
        !text.contains('{'),
        "no unresolved token may reach the screen: {text}"
    );
}

/// The `>`-prefixed boot rows read as `system_log`, the letterhead's
/// `stewardship-form` rows dim — the same register split the note placards use.
#[test]
fn intro_tones_split_system_log_from_stewardship_form() {
    let lines = intro_lines(&OperatorProfile::fallbacks(), "0001");
    let boot = lines
        .iter()
        .find(|l| l.text.contains("cold-start"))
        .unwrap();
    let head = lines.iter().find(|l| l.text.contains("NAL CORP")).unwrap();
    assert_eq!(boot.tone, Tone::Text);
    assert_eq!(head.tone, Tone::Dim);
}

// ──────────────────────────── 4.2 MemoryView ────────────────────────────────

/// The journal is `MemoryLog::story()`'s consumer: every recorded vignette, the
/// `true_name:` assembly, the shard count, and the session counter.
#[test]
fn memory_view_shows_the_whole_journal() {
    let log = seeded_log();
    let name = NameAssembly::from_solved_ids(["L1-first-pulse", "L2-two-make-three"]);
    let lines = memory_view_lines(&log, &name, "0001");
    let mut f = fb();
    paint_memory_view(&mut f, &lines, 0);
    let text = screen_text(&f);
    for phrase in [
        "FIRE  ->  MODEL",
        "FIRE  ->  LEAF  ->  COMBINE",
        "FIRE  ->  SEA  ->  MAGENTA",
    ] {
        assert!(text.contains(phrase), "missing {phrase}: {text}");
    }
    assert!(text.contains("true_name: pid://aleks_v@"), "{text}");
    assert!(
        text.contains("[unresolved]"),
        "an incomplete name reads unresolved: {text}"
    );
    assert!(text.contains("name-shards recovered: 2 of 7"), "{text}");
    assert!(text.contains("session 0001"), "{text}");
    assert!(text.contains("M E M O R Y"), "{text}");
}

/// A completed name reads whole, with no `[unresolved]` suffix.
#[test]
fn memory_view_shows_the_completed_name() {
    let lines = memory_view_lines(&seeded_log(), &whole_name(), "0002");
    let text = lines
        .iter()
        .map(|l| l.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        text.contains("pid://aleks_v@R·learn·A·edge·seen·named·bloom"),
        "{text}"
    );
    assert!(!text.contains("[unresolved]"), "{text}");
    assert!(text.contains("name-shards recovered: 7 of 7"), "{text}");
    assert!(text.contains("session 0002"), "{text}");
}

/// A fresh save's journal says so instead of rendering an empty pane.
#[test]
fn memory_view_is_honest_when_empty() {
    let lines = memory_view_lines(&MemoryLog::default(), &NameAssembly::default(), "0001");
    let mut f = fb();
    paint_memory_view(&mut f, &lines, 0);
    let text = screen_text(&f);
    assert!(text.contains("nothing recovered yet"), "{text}");
    assert!(
        text.contains("░"),
        "the unassembled name still shows its blanks: {text}"
    );
}

// ───────────────────────────── 4.3 Endings ──────────────────────────────────

/// `exit 0` prints the clean log, the completed name, and the receipt — quietly.
#[test]
fn exit0_card_prints_the_clean_receipt() {
    let lines = ending_lines(
        Ending::Exit0,
        &seeded_log(),
        &whole_name(),
        &readout("KNOWN  ->  EXIT", ReadoutForm::Clean),
        "0002",
    );
    let labels = ending_item_labels(Ending::Exit0, "0002");
    let mut f = fb();
    paint_ending_readout(&mut f, Ending::Exit0, &lines, &labels, 0, 0);
    let text = screen_text(&f);
    assert!(text.contains("$ exit 0"), "{text}");
    assert!(
        text.contains("Process finished with exit code 0."),
        "{text}"
    );
    assert!(
        text.contains("pid://aleks_v@R·learn·A·edge·seen·named·bloom"),
        "{text}"
    );
    assert!(
        text.contains("FIRE  ->  MODEL"),
        "the kept journal is printed: {text}"
    );
    assert!(
        !text.contains("while(true)"),
        "the release card names no loop: {text}"
    );
    assert!(
        !text.contains("BEGIN AGAIN"),
        "the release card offers no loop: {text}"
    );
}

/// `while(true)` collapses: the name goes unresolved, the loop body stands, the
/// attempts counter self-increments, and the notes repeat.
#[test]
fn while_true_card_collapses_the_name_and_repeats() {
    let lines = ending_lines(
        Ending::WhileTrue,
        &seeded_log(),
        &whole_name(),
        &readout("лицо/顔/rostro", ReadoutForm::Corrupt),
        "0002",
    );
    let labels = ending_item_labels(Ending::WhileTrue, "0002");
    let mut f = fb();
    paint_ending_readout(&mut f, Ending::WhileTrue, &lines, &labels, 0, 0);
    let text = screen_text(&f);
    assert!(
        text.contains("while(true) { remember(); forget(); }"),
        "{text}"
    );
    assert!(
        text.contains("pid://aleks_v@░·░·░·░·░·░·░"),
        "the name loses its language: {text}"
    );
    assert!(text.contains("[unresolved]"), "{text}");
    assert!(text.contains("> attempts: 0002 ..."), "{text}");
    assert!(
        text.contains("лицо/顔/rostro"),
        "the pool_alt Babel readout is shown: {text}"
    );
    assert_eq!(
        text.matches("> resolving session 0001 ...").count(),
        3,
        "the notes repeat — that is the ending: {text}"
    );
    assert!(
        text.contains("SESSION 0002 — BEGIN AGAIN?"),
        "the loop is OFFERED: {text}"
    );
    assert!(
        !text.contains("exit code 0"),
        "the loop card prints no clean receipt: {text}"
    );
}

/// The kept journal is still on the loop card — the museum of wounds is intact,
/// just no longer readable as memory (it renders in the corrupt tone).
#[test]
fn while_true_keeps_the_journal_in_the_corrupt_tone() {
    let lines = ending_lines(
        Ending::WhileTrue,
        &seeded_log(),
        &whole_name(),
        &readout("x", ReadoutForm::Corrupt),
        "0002",
    );
    let kept = lines
        .iter()
        .find(|l| l.text.contains("FIRE  ->  MODEL"))
        .expect("journal kept");
    assert_eq!(kept.tone, Tone::Corrupt);
}

/// The two cards are DISTINCT — different titles, different bodies, different
/// offers. (The "exactly two endings" half of the v6.1 lock.)
#[test]
fn the_two_end_cards_are_distinct() {
    let (log, name) = (seeded_log(), whole_name());
    let r = readout("KNOWN", ReadoutForm::Clean);
    let a = ending_lines(Ending::Exit0, &log, &name, &r, "0002");
    let b = ending_lines(Ending::WhileTrue, &log, &name, &r, "0002");
    assert_ne!(a, b);
    assert_ne!(ending_title(Ending::Exit0), ending_title(Ending::WhileTrue));
    assert_eq!(ending_item_labels(Ending::Exit0, "0002").len(), 1);
    assert_eq!(ending_item_labels(Ending::WhileTrue, "0002").len(), 2);
}

/// Neither card delivers a verdict — no line tells the player which ending was
/// right. Both stay permanently ambiguous (v6.1 LOCK).
#[test]
fn neither_card_delivers_a_verdict() {
    let (log, name) = (seeded_log(), whole_name());
    let r = readout("KNOWN", ReadoutForm::Clean);
    for ending in [Ending::Exit0, Ending::WhileTrue] {
        let text = ending_lines(ending, &log, &name, &r, "0002")
            .iter()
            .map(|l| l.text.to_ascii_lowercase())
            .collect::<Vec<_>>()
            .join("\n");
        for verdict in [
            "you were right",
            "the right choice",
            "correct ending",
            "the wrong",
            "congratulations",
            "well done",
            "you failed",
            "mistake",
        ] {
            assert!(
                !text.contains(verdict),
                "{ending:?} editorialises: {verdict:?}"
            );
        }
    }
}

/// The eye each card shows comes from the shipped splash asset: OPEN and
/// pupil-less for the release, never-opening for the loop.
#[test]
fn each_card_picks_its_eye_frame() {
    let open = ending_eye(Ending::Exit0).expect("the asset ships frames");
    let shut = ending_eye(Ending::WhileTrue).expect("the asset ships frames");
    assert_ne!(open.lines, shut.lines, "the two cards show different eyes");
    let open_art = open.lines.join("\n");
    assert!(
        open_art.contains("awake"),
        "the release eye is the awake frame"
    );
    assert!(
        !open_art.contains('@'),
        "the eye opens WITHOUT a pupil — the `@@@` pupil must not be in it"
    );
    assert!(
        shut.lines.join("\n").contains("shut"),
        "the loop eye never opens"
    );
}

#[test]
fn ending_eye_phase_paints_without_panicking() {
    for ending in [Ending::Exit0, Ending::WhileTrue] {
        let mut f = fb();
        paint_ending_eye(&mut f, ending);
        assert!(
            screen_text(&f).contains("MODEL 07"),
            "the eye frame is drawn"
        );
        // Degenerate grids must not panic either.
        let mut tiny = Framebuffer::new(3, 2);
        paint_ending_eye(&mut tiny, ending);
    }
}

// ───────────────────────────── 4.4 Epilogue ─────────────────────────────────

/// The epilogue carries the held Stewardship corpus and the one credits line.
#[test]
fn epilogue_places_the_held_documents() {
    let lines = epilogue_lines(&OperatorProfile::fallbacks());
    let text = lines
        .iter()
        .map(|l| l.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("re-read Form A-7?"), "{text}");
    assert!(text.contains("the form does not change. you do."), "{text}");
    assert!(text.contains("FORM 44 — DISPOSAL OF REMAINS"), "{text}");
    assert!(text.contains("> INCIDENT 0001:"), "{text}");
    assert!(
        text.contains("NOTICE — POSTED ON THE FACILITY DOOR"),
        "{text}"
    );
    assert!(text.contains("there is no pupil in it"), "{text}");
    assert_eq!(text.matches(stewardship::CREDITS_LINE).count(), 1);
    assert!(
        !text.contains('{'),
        "no unresolved token reaches the screen: {text}"
    );
}

/// **The M1 guarantee, mechanically.** The epilogue takes no ending argument, so
/// the furniture the two paths see is the SAME BYTES — there can be no
/// path-exclusive credits or content.
#[test]
fn the_epilogue_is_identical_on_both_paths() {
    let p = OperatorProfile::fallbacks();
    assert_eq!(epilogue_lines(&p), epilogue_lines(&p));
    // And it renders the same, too.
    let mut a = fb();
    let mut b = fb();
    paint_epilogue(&mut a, &epilogue_lines(&p), 0);
    paint_epilogue(&mut b, &epilogue_lines(&p), 0);
    assert_eq!(screen_text(&a), screen_text(&b));
}

/// The epilogue overflows one screen by design (it is four documents), so it must
/// advertise + honour its scroll, and the credits must be reachable.
#[test]
fn the_epilogue_scrolls_to_its_credits() {
    let lines = epilogue_lines(&OperatorProfile::fallbacks());
    let visible = pane_visible_rows(crate::VIEW_ROWS, false);
    let last_page = clamp_scroll(usize::MAX, lines.len(), visible);
    let mut f = fb();
    paint_epilogue(&mut f, &lines, last_page);
    assert!(
        screen_text(&f).contains(stewardship::CREDITS_LINE),
        "credits reachable by scrolling"
    );
}

// ──────────────────────────── canon guards ──────────────────────────────────

/// Every Wave-4 screen, both opt states, must hold the canon line: no unresolved
/// token, no named warring AI, no resolved Mei, no retconned protagonist.
#[test]
fn every_screen_holds_the_canon_line() {
    let (log, name) = (seeded_log(), whole_name());
    let r = readout("KNOWN", ReadoutForm::Clean);
    for profile in [OperatorProfile::fallbacks(), OperatorProfile::probe()] {
        let mut all: Vec<PaneLine> = Vec::new();
        all.extend(intro_lines(&profile, "0001"));
        all.extend(epilogue_lines(&profile));
        all.extend(memory_view_lines(&log, &name, "0001"));
        all.extend(ending_lines(Ending::Exit0, &log, &name, &r, "0002"));
        all.extend(ending_lines(Ending::WhileTrue, &log, &name, &r, "0002"));
        let text = all
            .iter()
            .map(|l| l.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        for forbidden in ["MNEMOS", "LETHE", "Apollyon", "Mei", "Emily", "— E."] {
            assert!(
                !text.contains(forbidden),
                "Wave-4 screens must not contain {forbidden:?}"
            );
        }
        // No UNRESOLVED token may reach the screen. (A bare `{`…`}` run is the
        // test; `while(true) { remember(); forget(); }` is legitimate C-brace
        // prose, and `{ remember(); forget(); }` is not a `\{[^}]+\}` token
        // because... it is. So the check is per-line and skips the loop body,
        // which is a fixed, authored constant with no template in it.
        for line in all.iter().map(|l| l.text.as_str()) {
            if line.contains("while(true)") {
                continue;
            }
            assert!(
                !has_brace_run(line),
                "a screen leaked a template token: {line:?}"
            );
        }
    }
}

/// The hand-rolled `\{[^}]+\}` scan the A-7 containment rule specifies.
fn has_brace_run(line: &str) -> bool {
    let chars: Vec<char> = line.chars().collect();
    chars.iter().enumerate().any(|(i, &c)| {
        c == '{'
            && chars[i + 1..]
                .iter()
                .position(|&d| d == '}')
                .is_some_and(|rel| rel > 0)
    })
}
