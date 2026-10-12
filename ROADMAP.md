# Roadmap

Where the game stands and what comes next. This page covers the **game**: campaign,
story, menus, onboarding, settings and localisation. Renderer, audio, input, the level
format and the editor have their own roadmap in
[terminal3d](https://github.com/Neural-Agent-Ledger/terminal3d).

Items under **Good first issues** are small and self-contained, and you can do them
without knowing the whole codebase. Comment on the issue, or open one, before you
start, so two people do not take the same item.

---

## Done

**Game**
- A twelve-level campaign, `L1` to `L12`, in four movements, with unlock gating and a
  fog curve that closes in as you descend (`levels/campaign.toml`).
- An archive of eight earlier rooms, `01` to `08`, playable from the level browser.
- The binary neural-network puzzle loop: switches, a forward pass through threshold
  nodes, power budgets and door targets.
- Story layer: in-level notes, code walls, fourth-wall tokens (local only, opt-out),
  endings and epilogue.
- Boot-console intro, an eye splash that follows the mouse, animated menu backdrops
  and mouse-driven menus.
- Save profile, persistent settings, quality presets, FOV zoom, pause menu and a
  performance overlay.

**Landed on 2026-09-28**
- **In-game guide** reachable from both the main menu and the pause menu: goal,
  controls, nodes and switches, the forward pass, and how to read the world.
- **First-steps checklist** that ticks itself off as you move, look, jump, face and
  flip a switch, watch a pulse and open the guide. It never blocks input, it persists,
  and a setting hides it.
- **Proximity interaction prompt** that shows what you are facing and which key acts
  on it.
- **Settings polish:** a scrolling settings window, a colour-steps option, and
  brightness.
- **Movement feel:** coyote time and a jump buffer, so a jump pressed a moment late or
  a moment early still counts.
- **The engine split.** The engine and editor now live in their own repository,
  [terminal3d](https://github.com/Neural-Agent-Ledger/terminal3d), and this repository holds only
  the game. The campaign's tests, golden hashes and content moved with it.

---

## Next

**Polish**
- Campaign visual review fixes, level by level, from `--dump` frames and live runs.
- Story staging: the clarity ramp across the four movements, per-movement audio themes
  and full ending sequences.
- A photosensitivity group in the settings for flashing effects.

**Playtest feedback**
- First-time player sessions on Windows Terminal and on Linux terminals, watching where
  people stall in `L1` to `L3`, and whether the guide and checklist answer the question
  they had at that moment.
- Terminal compatibility reports: which emulators, fonts and sizes render correctly.
  Reports from your own setup are very welcome.
- Tune the first-steps thresholds and the interaction prompt from what those sessions
  show.

**Localisation**

The game is English-only today, and it has **no locale layer yet**. The guide and the
first-steps checklist already keep their text in one table,
`crates/terminal_state/src/strings.rs`, under symbolic keys such as
`guide.goal.title`. Other screens still carry inline text. The order is fixed,
because translating text that no screen reads only produces text that drifts:

1. Route every player-facing string through the table: menus, settings, HUD, story
   screens, level notes.
2. Add the locale layer: a translation table keyed by those symbolic keys, with English
   as the source language.
3. Add a language picker to the settings that applies immediately, persists, and is
   applied again at start-up.
4. Add a test that rejects empty cells, cells that are byte-identical to English
   (except for an allowlist of proper nouns), keys missing from the table, and
   mismatched format specifiers.
5. Then translate. The target set is fourteen locales: `en ru de fr es it pt_BR pl tr
   uk zh_CN zh_TW ja ko`. The guide and the controls go first, because they are what a
   player who does not read English cannot work around.

**Level ideas**
- More multi-storey rooms built with the editor's stairs and storeys.
- Abstract, surreal rooms built on portals, self-similar spaces, holograms and code
  walls.
- Rooms that teach one idea each, for players who want more practice between campaign
  levels: a single threshold, a decoy pipe, a power budget, a lying door.
- A community archive: standalone rooms from contributors, shown in the level browser
  next to the original archive.

---

## Good first issues

- **Move menu and settings labels into `strings.rs`.** `menu.rs` still writes labels
  such as `START`, `SETTINGS` and the settings row names inline. Give each a symbolic
  key and a constant in `strings.rs`, following the existing guide entries. This is
  step 1 of the localisation plan above.
- **Fix the launcher's log hint.** `TERMINAL-STATE.bat` tells the player to look at
  `terminal-state.log` after an error, but the game does not write that file. Either
  change the message to say where error output actually goes, or write the log.
- **Document every CLI flag in one place.** `--help` in `src/main.rs` is the source.
  Check that [INSTALL.md](INSTALL.md) covers the same flags and nothing it no longer
  supports.
- **Guide shortcut.** The guide opens from the main menu and the pause menu. Propose,
  in an issue, a direct key for it during play, checking first that the key is free in
  the terminal3d key map.
- **A small archive room.** Build one standalone room with the terminal3d editor, make
  it pass `lvlcheck`, and submit it as described in
  [CONTRIBUTING.md](CONTRIBUTING.md#submitting-a-level).
- **Terminal report.** Play the first three levels in a terminal we have not listed and
  open an issue with what worked, what did not, and a `--dump --ansi` frame.

---

## Open design questions

These need a design decision before any code is written. Discussion is welcome in the
issues.

- Should the fourth-wall opt-out (`lore_fourth_wall`) also get a row in the settings
  menu, or stay a settings-file key as it is today?
- Which new puzzle mechanic comes next? Several are written up; none is chosen.
- `L6`: the seat rows are hard to read from the balcony. Change the geometry, move the
  spawn, or accept it?
- Should a community archive live in this repository, or in a separate content
  repository with its own review flow?
