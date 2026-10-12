# Contributing to TERMINAL STATE

Thank you for helping. Contributions are merged into this repository and maintained
here, so you never have to carry a fork. This page covers what you need to get a
change in.

- [Which repository?](#which-repository)
- [Picking something to work on](#picking-something-to-work-on)
- [Development setup](#development-setup)
- [Building and running](#building-and-running)
- [Tests](#tests)
- [The campaign golden test](#the-campaign-golden-test)
- [Lints and code rules](#lints-and-code-rules)
- [Submitting a level](#submitting-a-level)
- [Story and text](#story-and-text)
- [Assets and provenance](#assets-and-provenance)
- [Sign-off (DCO)](#sign-off-dco)
- [Pull request checklist](#pull-request-checklist)

---

## Which repository?

TERMINAL STATE is split in two:

| Change | Goes to |
|---|---|
| Campaign and archive levels, story, lore, splash art, menus, guide, settings, save, game flow | **this repository** (`terminal-state`) |
| Renderer, materials, lighting, audio synth, input, the `.lvl` format, the level editor, `lvlcheck` | **[terminal3d](https://github.com/Neural-Agent-Ledger/terminal3d)** |

If a game change needs an engine change, open the engine PR first. The game picks it
up once the engine publishes a tag that contains it. Until then you can develop
against a local engine clone, see
[docs/DEVELOPING_WITH_LOCAL_ENGINE.md](docs/DEVELOPING_WITH_LOCAL_ENGINE.md).

---

## Picking something to work on

1. Look at [ROADMAP.md](ROADMAP.md) and the issue tracker. The **Good first issues**
   list is small and self-contained.
2. Comment on the issue, or open one, before you start anything larger than a small
   fix. A short plan in the issue saves a rewrite in review.
3. Keep one pull request to one topic.

Bug reports help most when they include your OS, the terminal emulator and its size,
the colour mode (`truecolor` or `256`), the level, and the `--dump` output of that
level if the bug is visual.

---

## Development setup

- **Rust:** stable, via [rustup](https://rustup.rs). `rust-toolchain.toml` pins the
  channel and adds `rustfmt` and `clippy`.
- **Windows 10+:** the MSVC build tools ("Desktop development with C++"). The C
  runtime is linked statically (`.cargo/config.toml`). Nothing else is needed.
- **Linux:** `pkg-config`, `libasound2-dev` (audio) and `libudev-dev` (gamepad), or
  your distribution's equivalents:

  ```sh
  sudo apt install pkg-config libasound2-dev libudev-dev      # Debian / Ubuntu
  sudo dnf install pkgconf-pkg-config alsa-lib-devel systemd-devel   # Fedora
  ```

- **Terminal:** Windows Terminal, or any modern Linux terminal with truecolor and
  UTF-8.
- **Git** access to GitHub. The engine crates are git dependencies, so the first
  build fetches [terminal3d](https://github.com/Neural-Agent-Ledger/terminal3d).

The first build compiles Bevy and its dependencies, so it takes a while. Later builds
are incremental.

---

## Building and running

This repository is a cargo workspace with one member, `terminal_state` (binary
`terminal-state`). `.cargo/config.toml` sets `TS_CONTENT_ROOT` to the repository
root, so `cargo run` and `cargo test` find `levels/` and `assets/` from any directory
inside the clone.

```sh
cargo build                                   # debug build
cargo run --release                           # play
cargo run --release -- --help                 # CLI options
cargo run --release -- levels/L4-the-angle.lvl   # start on a given level
cargo build --profile dist -p terminal_state  # the lean shipping binary
```

Useful environment variables:

| Variable | Effect |
|---|---|
| `TS_AUDIO_SILENT=1` | Never open an audio device. Use it for tests, CI and quiet runs. |
| `TS_SAVE_DIR=<dir>` | Put progress and settings in `<dir>`, not in your real save folder. |
| `TS_NO_INTRO=1` | Skip the boot-console intro. |
| `TS_COLS` / `TS_ROWS` | Fix the render grid (same as `--size=COLSxROWS`). |

### Checking a visual change with `--dump`

`--dump` renders one frame to stdout without taking over the terminal:

```sh
cargo run --release -- --dump levels/L8-cold-room.lvl --size 100x32 > before.txt
# ...make your change...
cargo run --release -- --dump levels/L8-cold-room.lvl --size 100x32 > after.txt
diff before.txt after.txt
```

Add `--ansi` to include truecolor. Anything that moves or depends on input still
needs a look in a real terminal.

---

## Tests

Always run the tests with a throwaway save directory and no audio device.
`TS_SAVE_DIR` matters: without it, tests that exercise saving write into your real
progress and settings files.

```sh
# Linux / macOS / Git Bash
export TS_AUDIO_SILENT=1 TS_SAVE_DIR="$(mktemp -d)"
cargo test --workspace --no-fail-fast

# Windows PowerShell
$env:TS_AUDIO_SILENT = "1"
$env:TS_SAVE_DIR = Join-Path $env:TEMP ("ts-test-" + [guid]::NewGuid())
New-Item -ItemType Directory -Force $env:TS_SAVE_DIR | Out-Null
cargo test --workspace --no-fail-fast
```

Pass `--no-fail-fast`, or cargo stops at the first failing test binary and you do not
see the rest.

While you iterate, narrow the run:

```sh
cargo test -p terminal_state --lib                     # unit tests only
cargo test -p terminal_state --test campaign_lint      # one integration test
cargo test -p terminal_state --test campaign_solve_through
```

The integration tests under `crates/terminal_state/tests/` check properties of the
campaign itself: every level parses, lints, is walkable and solvable, declares a
budget, keeps its story canon consistent, and the endings resolve. Two guard tests keep
networking and GPU crates out of the game. The GPU guard shells out to `cargo tree`,
so it is `#[ignore]`d by default:

```sh
cargo test -p terminal_state --test no_gpu_in_default_build -- --ignored
```

**Bug fixes start with a failing test.** Write a test that reproduces the bug, watch
it fail, then fix the code. If a bug cannot be captured in a test, say why in the PR.
Do not "fix" a failing test by weakening its assertion.

---

## The campaign golden test

`crates/terminal_state/tests/golden_campaign.rs` renders seven campaign levels exactly
as `--dump` does, at a fixed 100×32 grid, and compares two SHA-256 digests per level
with [tests/golden/campaign_fb_hashes.txt](tests/golden/campaign_fb_hashes.txt):

- **shade** covers `{glyph, fg, bg}` and answers "did the look change?"
- **geom** covers `{depth, side, surface, material}` and answers "did the cast
  change?"

```sh
cargo test -p terminal_state --test golden_campaign
```

A mismatch has two possible causes: you edited one of the pinned levels, or the engine
tag in `Cargo.toml` moved and the engine renders differently. Read the failure this way:

- **Only shade moved:** a material, palette, glyph or lighting change.
- **Both moved:** the geometry changed. Treat it as a regression until you prove
  otherwise.
- **Only geom moved:** something changed that is not visible. Understand it first.

The suite fails closed. A missing level, a level that no longer parses, a key with no
hash and an orphaned hash each fail it, and blessing fixes none of them.

**Re-blessing** rewrites the stored hashes:

```sh
TS_BLESS_GOLDEN=1 cargo test -p terminal_state --test golden_campaign
```

Only re-bless when the change is **intentional and reviewed**:

1. In the PR, show a before/after `--dump` (with `--ansi` if colour changed) for every
   level whose hash moved.
2. Say which digest moved and why.
3. Commit the updated `campaign_fb_hashes.txt` together with the change, and add a
   dated entry to [tests/golden/README.md](tests/golden/README.md) naming the levels and
   the reason.

A PR that re-blesses without before/after frames will not be merged. Never bless to
make an unexpected failure go away.

---

## Lints and code rules

```sh
cargo clippy -p terminal_state --all-targets
```

- The `clippy::all` group is **deny**: the PR must show no `error:`. `pedantic` and
  `nursery` are advisory, but do not add new warnings in code you touch.
- `float_cmp` and `disallowed_types` are deny. `disallowed_types` (from `clippy.toml`)
  keeps terminal types out of this crate: **only the engine's `re_present` touches the
  terminal**. The game draws into a `Framebuffer` and sends backend-neutral events.
- **No `unsafe`.** `unsafe_code = "forbid"` is set for the workspace.
- Do not hide a real lint with `#[allow(...)]`. If you think a lint is wrong for a
  case, explain it in the PR.
- **Match the surrounding code.** Do not run `cargo fmt` over whole files. Format only
  the lines you write, in the style of the file around them.
- New or moved modules get a `//!` header that explains why the module exists, and
  public items get `///` docs. Comments say why, not what.
- **Determinism matters.** Seeded paths stay seeded. Do not add wall-clock time or
  unseeded randomness to anything a test or the golden corpus pins.
- New dependencies need a reason in the PR. They must be headless (no `wgpu`, `winit`
  or `bevy_render`) and pass `deny.toml`. Networking crates must never reach the game.

---

## Submitting a level

Levels are plain-text `.lvl` files. You author them with the terminal3d editor and
check them with its linter, `lvlcheck`. [docs/LEVEL_DESIGN.md](docs/LEVEL_DESIGN.md)
walks through the whole loop.

To submit one:

1. Put it in `levels/` with a short, descriptive file name.
2. **Run `lvlcheck` and make sure it passes.** It checks elements, solvability,
   reachability, whether the player can get stuck, and sound fields, and it exits
   non-zero on any FAIL. Paste its summary into the PR.
3. Play it through at least once and attach a `--dump` frame.
4. State in the PR that the level is **your own work** and that you license it under
   **CC BY-SA 4.0**, and add an entry for it to [PROVENANCE.toml](PROVENANCE.toml).
5. Say whether it is a standalone room or meant for the campaign.

The campaign order lives in [levels/campaign.toml](levels/campaign.toml). It changes
only after a design discussion in an issue, because it also sets the unlock gating,
the fog curve and the story order. New rooms are welcome outside the campaign first.

---

## Story and text

- Player-facing strings for the guide and the first-steps checklist live in
  `crates/terminal_state/src/strings.rs` under symbolic keys (for example
  `guide.goal.title`). New player-facing text goes there, not inline.
- The game is English-only for now. Please do not send translations yet: the locale
  layer comes first, see [ROADMAP.md](ROADMAP.md).
- `docs/lore/` is the story canon and contains the puzzle solutions. Keep spoilers out
  of code comments, commit messages and in-game text that appears before the relevant
  level.

---

## Assets and provenance

Every file under `levels/`, `assets/` and `docs/lore/` must have a known, compatible
origin, recorded in [PROVENANCE.toml](PROVENANCE.toml). We accept only:

- work you made yourself and license to the project (CC BY-SA 4.0 for content);
- files generated by code in this repository or in terminal3d;
- third-party files under **CC0** or **CC BY**, committed together with their licence
  file and a provenance entry.

We do **not** accept art, models, textures, sounds or fonts downloaded from the web
without a clear licence, fan art, or anything derived from someone else's IP. This
includes files used only by tests. If you are unsure, ask in an issue first.

All game audio is synthesized by the engine. Please propose new sounds as synth
recipes in terminal3d, not as audio files.

---

## Sign-off (DCO)

We use the [Developer Certificate of Origin](DCO), not a CLA. Sign off every commit:

```sh
git commit -s -m "levels: widen the L6 balcony so the seat rows read"
```

That adds a `Signed-off-by: Your Name <you@example.com>` line, which certifies that you
have the right to submit the work under the project's licences. Unsigned commits
cannot be merged. `git rebase --signoff` fixes a branch after the fact.

Contributions are licensed inbound = outbound, under the same terms as the rest of the
repository: code you contribute is licensed under **GPL-3.0-or-later** (see
[LICENSE](LICENSE)), and content (levels, story text, splash art) under
**CC BY-SA 4.0** (see [LICENSE-CONTENT](LICENSE-CONTENT)). There is no copyright
assignment; you keep the copyright in your work.

Commit messages start with the area (`levels:`, `story:`, `menu:`, `guide:`,
`settings:`, `docs:`), then say what changed and why.

---

## Pull request checklist

- [ ] One topic per PR. The description says what changed and why.
- [ ] Every commit is signed off (`git commit -s`).
- [ ] `cargo test --workspace --no-fail-fast` passes, run with `TS_AUDIO_SILENT=1`
      and a fresh `TS_SAVE_DIR`.
- [ ] `cargo clippy -p terminal_state --all-targets` shows no errors and no new
      warnings in code you touched.
- [ ] No new `unsafe`, no terminal types in this crate.
- [ ] Bug fix: includes a test that failed before the fix.
- [ ] Visual change: before/after `--dump` frames are in the description.
- [ ] Golden hashes: unchanged, **or** re-blessed with before/after frames and a
      `tests/golden/README.md` entry.
- [ ] Levels: `lvlcheck` passes, and the level has a `PROVENANCE.toml` entry.
- [ ] Docs updated if behaviour, a setting or a CLI flag changed.

Everyone taking part follows the [Code of Conduct](CODE_OF_CONDUCT.md).
