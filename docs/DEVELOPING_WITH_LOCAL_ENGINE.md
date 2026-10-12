# Developing with a local engine

By default this repository builds against a **tagged release** of the engine,
[terminal3d](https://github.com/Neural-Agent-Ledger/terminal3d). `Cargo.toml` declares every
engine crate once, as a git dependency, between the `# BEGIN engine-deps` and
`# END engine-deps` markers:

```toml
# BEGIN engine-deps
re_core    = { git = "https://github.com/Neural-Agent-Ledger/terminal3d", tag = "v0.1.0" }
re_present = { git = "https://github.com/Neural-Agent-Ledger/terminal3d", tag = "v0.1.0", default-features = false }
# … one line per engine crate
# END engine-deps
```

That is all a player or a game-only contributor needs. When you want to change the
engine and see the effect in the game **before** the engine publishes a new tag, clone
both repositories side by side and tell cargo to use your local engine instead of the
tag. Cargo calls this a `[patch]` override. It changes where the crates come from, and
nothing else: the same crates, the same features.

## 1. Clone side by side

```sh
mkdir ts-dev && cd ts-dev
git clone https://github.com/Neural-Agent-Ledger/terminal3d
git clone https://github.com/Neural-Agent-Ledger/terminal-state
```

```
ts-dev/
├── terminal3d/        engine + editor
└── terminal-state/    this repository
```

## 2. Add the override

Do **not** edit the engine-deps block, and do not put the override in
`terminal-state/.cargo/config.toml`, because that file is tracked (it holds the static
C runtime flag and `TS_CONTENT_ROOT`). Put the override one level **up**, in
`ts-dev/.cargo/config.toml`. Cargo reads every `.cargo/config.toml` from the current
directory upward, so a build inside `terminal-state/` picks it up, and nothing in
either clone changes.

`ts-dev/.cargo/config.toml`:

```toml
[patch."https://github.com/Neural-Agent-Ledger/terminal3d"]
re_core      = { path = "terminal3d/engine/re_core" }
re_world     = { path = "terminal3d/engine/re_world" }
re_level     = { path = "terminal3d/engine/re_level" }
re_puzzle    = { path = "terminal3d/engine/re_puzzle" }
re_render    = { path = "terminal3d/engine/re_render" }
re_present   = { path = "terminal3d/engine/re_present" }
re_audio     = { path = "terminal3d/engine/re_audio" }
re_glitch    = { path = "terminal3d/engine/re_glitch" }
re_postfx    = { path = "terminal3d/engine/re_postfx" }
re_asciifx   = { path = "terminal3d/engine/re_asciifx" }
re_runtime   = { path = "terminal3d/engine/re_runtime" }
# dev-dependencies of the game's tests
re_voximport = { path = "terminal3d/editor/re_voximport" }
re_lvlcheck  = { path = "terminal3d/editor/re_lvlcheck" }
re_golden    = { path = "terminal3d/editor/re_golden" }
```

- The `[patch."…"]` key must be the **exact URL** used in the engine-deps block.
- Relative paths in a config file resolve against the directory that **contains**
  `.cargo/`, here `ts-dev/`. If you keep the config somewhere else, use absolute paths.
- Leave features out of the patch lines. `default-features = false` on `re_present`
  stays in `Cargo.toml` and still applies.
- Patch every engine crate, not just the one you are changing. A partial patch mixes a
  local crate with tagged copies of its dependencies, and cargo then sees two different
  `re_core`s.

If you would rather keep everything inside the clone, you can paste the same table into
the bottom of `terminal-state/Cargo.toml` as a `[patch."https://github.com/Neural-Agent-Ledger/terminal3d"]`
section, with the paths changed to `../terminal3d/engine/…` (relative to
`Cargo.toml`). Do not commit that change.

## 3. Build and check

```sh
cd terminal-state
cargo build
cargo tree -i re_core          # shows re_core from terminal3d/engine/re_core, not a git checkout
```

`Cargo.lock` now records path sources. **Do not commit that `Cargo.lock`.** Restore it
before you open a game PR (`git checkout -- Cargo.lock`).

Cargo may still contact GitHub once to read the tag it is patching. After that, local
builds work offline.

## 4. Getting the change upstream

1. Open the engine PR in terminal3d and get it merged.
2. When a new engine tag contains the change, a game PR bumps the `tag = "…"` in the
   engine-deps block and updates `Cargo.lock` with `cargo update`.
3. Run the whole suite, including `cargo test -p terminal_state --test golden_campaign`.
   An engine render change can move the campaign golden hashes. Re-bless only as
   described in [CONTRIBUTING.md](../CONTRIBUTING.md#the-campaign-golden-test).

To go back to the tagged engine, delete `ts-dev/.cargo/config.toml` (or the `[patch]`
section) and restore `Cargo.lock`.
