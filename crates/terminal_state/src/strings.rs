//! `strings` — the one table every player-facing string in the guide and the
//! first-steps checklist lives in.
//!
//! Owner decision: **English only, for now** — this project has no locale
//! layer yet. What is already in place is the shape a future localization
//! pass needs: every string a screen renders gets a stable, documented,
//! symbolic identity up front, so that pass finds one table instead of prose
//! scattered across `guide.rs`/`first_steps.rs`. Each constant's doc comment
//! carries its symbolic key (`guide.goal.title`, dotted, lowercase) — the name
//! a future `strings.csv`/locale table would use as the row key; the Rust
//! constant itself is the identifier, since there is no runtime lookup to key
//! by string yet.
//!
//! Every string here is genuinely wired — read by `guide.rs` or
//! `first_steps.rs` and painted onto a real screen. Nothing is added here in
//! advance of a screen that renders it: an untranslated key with no reader
//! has no corrective pressure and drifts into fiction for free.

// ─────────────────────────────── the GUIDE ──────────────────────────────────
// `guide.rs` — chapters: Goal, Controls, Nodes & switches, The forward pass,
// Materials / reading the world (study report item 1).

/// `guide.title`
pub const GUIDE_TITLE: &str = "G U I D E";

/// `guide.chapter.goal`
pub const GUIDE_CHAPTER_GOAL: &str = "GOAL";
/// `guide.chapter.controls`
pub const GUIDE_CHAPTER_CONTROLS: &str = "CONTROLS";
/// `guide.chapter.nodes_switches`
pub const GUIDE_CHAPTER_NODES_SWITCHES: &str = "NODES & SWITCHES";
/// `guide.chapter.forward_pass`
pub const GUIDE_CHAPTER_FORWARD_PASS: &str = "THE FORWARD PASS";
/// `guide.chapter.materials`
pub const GUIDE_CHAPTER_MATERIALS: &str = "MATERIALS";

/// `guide.goal.body` (one entry per line)
pub const GUIDE_GOAL_BODY: &[&str] = &[
    "Model 07 is a binary neural network wired into these rooms.",
    "",
    "Each room holds a set of NODES. A node fires once enough of its",
    "wired inputs are ON.",
    "",
    "Toggle switches to wire pipes ON or OFF. Power runs through them",
    "to the nodes they feed.",
    "",
    "The room is solved once every required node has fired — watch the",
    "[lit/total] readout on the objective line.",
    "",
    "Some rooms cap how many pipes may be ON at once. When that applies",
    "the HUD shows power N/M, and it turns amber once the grid is full.",
    "",
    "Once solved, walk to the door. It opens.",
];

/// `guide.controls.body`
pub const GUIDE_CONTROLS_BODY: &[&str] = &[
    "WASD          move forward / back / strafe left / right",
    "<- / ->       turn left / right",
    "^ / v         look up / down (pitch)",
    "SPACE         jump",
    "E or ENTER    interact with a faced switch",
    "SHIFT (held)  sprint — a pure speed modifier, never blocks a turn",
    "M             toggle mouse-look on / off",
    "[ / ]         cycle field of view",
    "Z             toggle zoom",
    "G             cycle post-fx",
    "ESC or Q      pause (in play) / back (in a menu)",
];

/// `guide.nodes_switches.body`
pub const GUIDE_NODES_SWITCHES_BODY: &[&str] = &[
    "A PIPE is a directed wire from one node to another. Every pipe carries",
    "a SWITCH cell somewhere along its run.",
    "",
    "Face a switch — the crosshair brightens from a faint + to a bright",
    "diamond — and press E to flip its pipe ON or OFF.",
    "",
    "An ON pipe carries its source node's activation forward; an OFF",
    "pipe carries nothing.",
    "",
    "A node FIRES once the sum of its active incoming ON-pipes reaches",
    "its threshold.",
    "",
    "Some nodes are INPUTS: fixed by the room itself, never toggled",
    "directly by a switch.",
];

/// `guide.forward_pass.body`
pub const GUIDE_FORWARD_PASS_BODY: &[&str] = &[
    "Every switch you flip re-runs a FORWARD PASS over the whole network,",
    "node by node, in wiring order — the same instant, not a delay.",
    "",
    "If your wiring closes a loop, the pass cannot resolve it: the toggle",
    "is refused and rolled back, as if it never happened.",
    "",
    "A few rooms deliberately want a held loop instead of an ordinary",
    "resolution — those are the rare exception, not the rule.",
    "",
    "You win by driving every required node to fire. The objective line",
    "tracks it live, in real time, as you toggle.",
];

/// `guide.materials.body`
pub const GUIDE_MATERIALS_BODY: &[&str] = &[
    "Brightness and glyph density read as DEPTH: dimmer, denser glyphs",
    "sit farther from the camera.",
    "",
    "An idle switch reads as a faint +; a switch you can act on right",
    "now reads as a bright diamond.",
    "",
    "A surface that visibly PULSES or animates is carrying live current.",
    "A dead surface sits still.",
    "",
    "Different glyph families mark different materials — walls, glass,",
    "circuitry, and code-text all render distinctly once you learn to",
    "read them.",
];

/// `guide.footer.hint`
pub const GUIDE_FOOTER_HINT: &str = "up/down chapter  ·  left/right scroll  ·  Esc/Enter — back";

// ────────────────────────── FIRST STEPS checklist ───────────────────────────
// `first_steps.rs` (study report item 2).

/// `first_steps.moved`
pub const FIRST_STEP_MOVED: &str = "move";
/// `first_steps.looked`
pub const FIRST_STEP_LOOKED: &str = "look around";
/// `first_steps.jumped`
pub const FIRST_STEP_JUMPED: &str = "jump";
/// `first_steps.faced_switch`
pub const FIRST_STEP_FACED_SWITCH: &str = "face a switch";
/// `first_steps.toggled_switch`
pub const FIRST_STEP_TOGGLED_SWITCH: &str = "toggle a switch";
/// `first_steps.watched_pulse`
pub const FIRST_STEP_WATCHED_PULSE: &str = "watch a pulse travel";
/// `first_steps.opened_guide`
pub const FIRST_STEP_OPENED_GUIDE: &str = "open the guide";
