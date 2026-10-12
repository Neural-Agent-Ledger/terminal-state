# CLAUDE.md

Guidance for Claude Code working in this repository.

TERMINAL STATE is a first-person 3D ASCII puzzle game that runs in a terminal. This
repository is **the game only**: one cargo workspace with one crate, `terminal_state`
(library plus the binary `terminal-state`), the campaign levels, the lore and the
splash art. The engine and the level editor live in a separate repository,
[terminal3d](https://github.com/Neural-Agent-Ledger/terminal3d), and arrive here as git
dependencies on a tagged release. Both run on **headless Bevy 0.19** (no wgpu, winit
or `bevy_render`).

Read [README.md](README.md) for the overview and [CONTRIBUTING.md](CONTRIBUTING.md)
for the rules PRs are held to. This file is the short operational version.

## The boundary with the engine

```
terminal-state (this repo)     story shell: game states, menus, guide, first steps,
      │                        story/lore text, save and settings, campaign content
      │ depends on (git tag)
terminal3d                     re_core re_world re_level re_puzzle re_render re_postfx
                               re_glitch re_asciifx re_audio re_runtime re_present
                               dev only: re_lvlcheck re_golden re_voximport
```

Rules you must not break:

1. **Dependencies point one way: game → engine.** Never ask the engine to know about
   the game. If the game needs a new engine capability, it is an engine PR first.
2. **Only the engine's `re_present` touches the terminal.** No crossterm or ratatui in
   this crate. `clippy.toml` bans their types. The game paints into a `Framebuffer`
   and reads backend-neutral input events.
3. **No networking in the game.** `tests/no_net_in_game_build.rs` fails if a networking
   crate becomes reachable. `no_gpu_in_default_build.rs` (ignored by default) does the
   same for GPU crates.
4. **Do not hand-edit the `# BEGIN engine-deps` … `# END engine-deps` block** in
   `Cargo.toml` to point at a local path. To work against a local engine, use the
   override in [docs/DEVELOPING_WITH_LOCAL_ENGINE.md](docs/DEVELOPING_WITH_LOCAL_ENGINE.md).

## Where things live

| Area | Location |
|---|---|
| Binary entry, CLI (`HELP`, `KNOWN_FLAGS`), `--dump` | `crates/terminal_state/src/main.rs` |
| Game states and level flow | `crates/terminal_state/src/flow.rs` (+ `flow/tests.rs`) |
| Menus, HUD, settings screen | `crates/terminal_state/src/menu.rs`, `menu_fx/`, `menu_mouse.rs`, `screens.rs` |
| In-game guide | `crates/terminal_state/src/guide.rs` |
| First-steps checklist | `crates/terminal_state/src/first_steps.rs` |
| Player-facing strings (symbolic keys, English only) | `crates/terminal_state/src/strings.rs` |
| Save / settings | `crates/terminal_state/src/save.rs`, `settings.rs` |
| Story, fourth-wall interpolation | `crates/terminal_state/src/story/` (`interp.rs` is the privacy-relevant one) |
| Boot console, entry effect | `crates/terminal_state/src/boot_console.rs`, `entry_fx.rs` |
| Code-wall text snapshots | `crates/terminal_state/panels/*.txt` |
| Campaign tests | `crates/terminal_state/tests/` |
| Levels and campaign order | `levels/*.lvl`, `levels/campaign.toml` |
| Golden hashes | `tests/golden/campaign_fb_hashes.txt`, `tests/golden/README.md` |
| Story canon (spoilers) | `docs/lore/` |

## Commands

```sh
cargo build
cargo run --release -- --dump levels/L1-first-pulse.lvl --size 100x32   # one frame, headless
cargo clippy -p terminal_state --all-targets
cargo test -p terminal_state --lib                 # fast
cargo test --workspace --no-fail-fast              # everything
```

**Always set these when running tests or the game from a non-interactive shell:**

- `TS_AUDIO_SILENT=1`: no audio device is opened. Use it unless the user asks for sound.
- `TS_SAVE_DIR=<fresh temp dir>`: keeps tests and runs away from the user's real save
  and settings.
- `TS_NO_INTRO=1`: skips the boot-console intro on live runs.

The interactive game takes over the terminal and needs a real TTY. Do not start it
from a non-interactive shell. Use `--dump` instead. The level editor and `lvlcheck`
are in terminal3d, not here.

## Verifying visual changes

You cannot see the terminal, so verify with `--dump`:

1. Dump the affected levels **before** the change (`--size 100x32`, plus `--ansi` if
   colour matters).
2. Make the change, dump again, and diff the two files.
3. Tell the user what moved and show the relevant rows. "It compiles" is not a visual
   check.
4. For motion, input, timing or anything interactive, say plainly that it needs a live
   run in a real terminal, and ask the user to do it.

## Golden tests

`cargo test -p terminal_state --test golden_campaign` compares a shade digest and a
geom digest for seven campaign levels against `tests/golden/campaign_fb_hashes.txt`.

- Shade moved and geom did not: a look change. Both moved: treat it as a regression
  until proven otherwise. A new engine tag can move hashes too.
- **Never re-bless (`TS_BLESS_GOLDEN=1`) without first showing the user before/after
  `--dump` frames for every moved level and getting agreement.** Never bless to silence
  an unexpected failure, a missing level or a missing hash.
- When a bless is agreed, commit the hash file with the change and add a dated entry to
  `tests/golden/README.md`.

## Working rules

- **Bug fixes: write the failing test first**, confirm it fails, then fix.
- **No `unsafe`.** `unsafe_code = "forbid"` for the workspace.
- **Clippy `all` is deny.** Do not add `#[allow]` to hide a real finding.
- **Match the surrounding code style.** Do not run `cargo fmt` on whole files. Keep the
  `//!` module headers true when you change behaviour; comments say why, not what.
- **Determinism:** seeded paths stay seeded. No wall-clock time or unseeded randomness
  in anything a test or golden hash pins.
- **New player-facing text goes in `strings.rs`** under a symbolic key. The game is
  English-only and has no locale layer yet. Do not add translations.
- **No downloaded assets.** Do not fetch or add images, models, fonts or audio from the
  web, even for tests. Every file under `levels/`, `assets/` and `docs/lore/` needs a
  `PROVENANCE.toml` entry.
- **No new dependencies without a stated reason.** They must be headless and pass
  `deny.toml`.
- **Do not weaken tests to make them pass.** If a test's premise is wrong, explain why
  and change it deliberately.
- **Levels must pass `lvlcheck`.** Do not edit `levels/campaign.toml` unless asked.
- **Privacy:** the fourth-wall reads in `story/interp.rs` are an allowlist that the
  README documents one by one. Do not add a new read without updating the README's
  privacy section, and keep the `lore_fourth_wall = false` opt-out performing no reads
  at all.
- `docs/lore/` contains puzzle solutions and story spoilers. Do not paste them into
  code comments, commit messages or player-facing text unless the task requires it.
- Commits are signed off (`git commit -s`); the project uses the DCO.
- Licences: the game's code is GPL-3.0-or-later (`LICENSE`), content is CC BY-SA 4.0
  (`LICENSE-CONTENT`), and the terminal3d engine is MIT OR Apache-2.0. A new content
  file needs a `PROVENANCE.toml` entry.
- Tags in comments and docs use a closed set: `@todo`, `@q(...)`, `@wip`,
  `@blocked(...)`, `@dropped(...)`, `@decision(...)`, `@entry`, `@canon(...)`. Do not
  invent new ones.

## Docs to read per area

| Working on | Read first |
|---|---|
| Building, running | [INSTALL.md](INSTALL.md) |
| Engine and game together | [docs/DEVELOPING_WITH_LOCAL_ENGINE.md](docs/DEVELOPING_WITH_LOCAL_ENGINE.md) |
| Levels | [docs/LEVEL_DESIGN.md](docs/LEVEL_DESIGN.md), then the terminal3d level-format docs |
| Story, campaign | [docs/lore/STORY_BIBLE.md](docs/lore/STORY_BIBLE.md) (spoilers) |
| Golden tests | [tests/golden/README.md](tests/golden/README.md) |
| What is planned | [ROADMAP.md](ROADMAP.md) |
