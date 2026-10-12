# TERMINAL STATE — Build and Install

This page covers building the game from source. If you only want to play, the
prebuilt build on itch.io needs no toolchain at all. The source build below gives you
the same game, for free.

## Prerequisites

1. **Rust**, stable, via [rustup](https://rustup.rs):

   ```sh
   # Linux / macOS
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

   # Windows
   winget install Rustlang.Rustup
   ```

   The current stable channel is all you need.

2. **Platform tools:**

   - **Windows 10+:** the MSVC build tools (the "Desktop development with C++" workload,
     or the standalone Build Tools). Nothing else. `.cargo/config.toml` links the C
     runtime statically, so the `.exe` needs no redistributable.
   - **Linux:** `pkg-config` and the ALSA and udev headers (audio and gamepad):

     ```sh
     sudo apt install pkg-config libasound2-dev libudev-dev             # Debian / Ubuntu
     sudo dnf install pkgconf-pkg-config alsa-lib-devel systemd-devel   # Fedora
     ```

3. **Git**, with access to GitHub. The engine,
   [terminal3d](https://github.com/Neural-Agent-Ledger/terminal3d), is a git dependency on a
   tagged release (the `# BEGIN engine-deps` block in `Cargo.toml`). Cargo fetches it
   on the first build; you do not clone it yourself. Every other dependency (Bevy,
   ratatui, crossterm, kira, …) is an ordinary crate pinned by `Cargo.lock`.

4. **A terminal:** Windows Terminal (Windows 10+) or any modern Linux terminal.
   Truecolor and UTF-8 preferred.

---

## Building

```sh
git clone https://github.com/Neural-Agent-Ledger/terminal-state
cd terminal-state
cargo build --release                        # optimized build
cargo build                                  # debug build
cargo build --profile dist -p terminal_state # lean shipping build (stripped, fat LTO)
```

The first build downloads and compiles Bevy and the engine and takes a while. Later
builds are incremental.

To build against a local copy of the engine instead of the tagged release, see
[docs/DEVELOPING_WITH_LOCAL_ENGINE.md](docs/DEVELOPING_WITH_LOCAL_ENGINE.md).

### Build output

| File | What |
|------|------|
| `target/release/terminal-state` (Linux) | game binary |
| `target/release/terminal-state.exe` (Windows) | game binary (static C runtime) |
| `target/dist/terminal-state[.exe]` | the lean shipping binary |
| `target/debug/terminal-state[.exe]` | debug binary |

If you pass a build on to anyone else, the GPL applies: ship `LICENSE` and
`LICENSE-CONTENT` next to the binary and offer the corresponding source (this
repository and terminal3d, at the tags you built from).

---

## Running

```sh
cargo run --release                              # play
cargo run --release -- levels/L1-first-pulse.lvl # start on a given level
cargo run --release -- --help                    # every option
```

The game looks for `levels/` and `assets/` next to the executable, then in its parent
directories, then in the current directory. So a binary under `target/` finds this
repository's content from anywhere, and a packaged build works when `levels/` and
`assets/` sit beside the `.exe` (or one folder up, the layout `TERMINAL-STATE.bat`
expects: `build\terminal-state.exe` next to `levels\` and `assets\`).

### Command-line options

| Option | Effect |
|---|---|
| `[LEVEL]` | Open this level from the catalogue when you choose START. |
| `--dump [LEVEL]` | Render one frame to stdout and exit. No terminal takeover. |
| `--ansi` (or `--dump-color`) | Emit the dump as 24-bit truecolor cells. |
| `--glitch[=N]` | Apply the glitch layer to the dump, intensity 0..1 (default 0.7). |
| `--postfx[=PRESET]` | Post-process the dump: `off`, `clean`, `subtle`, `cinematic`, `toon`, `crt`, `dream`. |
| `--fov=DEG` | Horizontal FOV for the dump, 30..120. |
| `--portal-demo[=N]` | Inject the demo portal pair into the dump; walk N steps through it. |
| `--color=MODE` | `truecolor` or `256`. Saved to the settings. |
| `--size=COLSxROWS` | Fix the render grid (default: your terminal size; 100x32 for `--dump`). |
| `--quality=TIER` | `low`, `medium`, `high` or `ultra`. Saved. |
| `--draw-distance=N` | Draw distance in cells. Saved. |
| `--fps=CAP` | `30`, `60`, `120` or `off`. Saved. |
| `--dump-schedule` | Print the Bevy schedule as GraphViz (needs the `debugdump` feature). |

Every `--flag=VALUE` can also be written `--flag VALUE`.

### Environment variables

| Variable | Effect |
|---|---|
| `TS_SAVE_DIR=<dir>` | Keep progress and settings in `<dir>` instead of the default location. |
| `TS_AUDIO_SILENT=1` | Never open an audio device. |
| `TS_NO_INTRO=1` | Skip the boot-console intro. |
| `TS_COLS` / `TS_ROWS` | Same as `--size`. |

Progress (`progress.toml`) and settings (`settings.toml`) live in
`%APPDATA%\TerminalState\` on Windows and in `$XDG_CONFIG_HOME/TerminalState/` (or
`~/.config/TerminalState/`) on Linux, unless `TS_SAVE_DIR` is set.

---

## Running the tests

Use a throwaway save directory and no audio device, so the tests never touch your real
save:

```sh
export TS_AUDIO_SILENT=1 TS_SAVE_DIR="$(mktemp -d)"
cargo test --workspace --no-fail-fast
```

PowerShell equivalents are in [CONTRIBUTING.md](CONTRIBUTING.md#tests).

This repository's tests cover the game: level catalogue and flow, menus, the guide and
checklist, story text, save and settings, and the campaign (every level parses, lints,
is walkable and solvable, and seven are pinned by the golden framebuffer hashes in
[tests/golden/](tests/golden/README.md)). The raycaster, the puzzle rules, audio and the
level format are tested in the terminal3d repository.

---

## Checking a level file

`--dump` parses a level, builds it and prints a frame. A level that does not parse
reports the error and exits non-zero:

```sh
cargo run --release -- --dump levels/L1-first-pulse.lvl
```

For the full content check (solvability, reachability, getting stuck, sound fields),
use the engine's linter, `lvlcheck`. [docs/LEVEL_DESIGN.md](docs/LEVEL_DESIGN.md) shows
how.

---

## Troubleshooting

**Truecolor not working.** Set `COLORTERM=truecolor` in a truecolor-capable terminal,
or run with `--color=256`. Without truecolor the game falls back to 256 colours by
itself.

**Garbled or boxed glyphs.** Use a UTF-8 terminal and a monospace font with good
Unicode coverage. Windows Terminal with its default font is a good reference.

**No output when piped.** The game expects a real terminal. For CI or piped output,
use `--dump`.

**The game cannot find its levels.** The binary could not find a `levels/` folder beside itself, in a
parent folder or in the current directory. The message lists every place it looked.
Keep `levels/` and `assets/` next to the executable.

**Linux build fails on `alsa` or `libudev`.** Install the headers listed under
Prerequisites.

**Engine fetch fails.** Cargo needs to reach GitHub for the first build. Check your
network or proxy, or use a local engine clone as described in
[docs/DEVELOPING_WITH_LOCAL_ENGINE.md](docs/DEVELOPING_WITH_LOCAL_ENGINE.md).
