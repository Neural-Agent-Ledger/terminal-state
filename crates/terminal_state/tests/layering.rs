//! Guard: `terminal_state`'s dependency direction must point one way — game to
//! engine — and the game must resolve exactly one `bevy_ecs`.
//!
//! - the game's **normal** dependency tree may contain `engine/*` packages
//!   freely, but **no `editor/*` package** — `editor/re_voximport`,
//!   `editor/re_lvlcheck` and `editor/re_golden` are dev-dependencies only
//!   (see this crate's `Cargo.toml`, and the `terminal3d` side's own
//!   `cargo xtask layers` for the matching engine-repo guard);
//! - the whole resolved graph (normal + dev + build) must contain **exactly
//!   one** version of `bevy_ecs`. Two would mean the game repo's
//!   `[workspace.dependencies]` pinned a `bevy` that resolved a different
//!   `bevy_ecs` than the one `terminal3d`'s engine crates bring in — a silent
//!   version fork between the two repos' lockfiles that `cargo build` alone
//!   would not surface as an error (Cargo is happy to compile a crate graph
//!   with two versions of the same crate; it would only show up as
//!   mysterious `expected re_core::Framebuffer, found re_core::Framebuffer`
//!   type-mismatch errors much later, or not at all if the two `bevy_ecs`
//!   types never meet directly).
//!
//! ## Mechanism
//!
//! Same approach as the sibling guard `no_net_in_game_build.rs`: run
//! `cargo metadata --format-version 1 --locked`, parse the JSON with
//! `serde_json` (already a dev-dependency here, for that same sibling test —
//! this file adds no new dependency), and walk `resolve.nodes[].deps` by
//! `dep_kinds`.
//!
//! An `engine/*` vs `editor/*` package is told apart by whether **any path
//! component** of its `manifest_path` is literally `engine` or `editor`. That
//! works unchanged whether the dependency resolves to a local path (the
//! private working tree's `../terminal3d/engine/re_core`) or to a `git`
//! checkout of a tagged release (`~/.cargo/git/checkouts/terminal3d-*/<rev>/
//! engine/re_core`), because both preserve `terminal3d`'s own
//! `engine/`/`editor/` top-level layout.

use std::{
    collections::{HashMap, HashSet, VecDeque},
    process::Command,
};

use serde_json::Value;

/// Returns the workspace root directory (two levels above this test's
/// manifest directory, `crates/terminal_state`) — the same helper as
/// `no_net_in_game_build.rs`, duplicated rather than shared because test
/// binaries in this crate do not share a common support module.
fn workspace_root() -> std::path::PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    std::path::Path::new(manifest_dir)
        .parent()
        .expect("crates/terminal_state has no parent")
        .parent()
        .expect("crates/ has no parent")
        .to_path_buf()
}

/// Run `cargo metadata --format-version 1 --locked` and parse it as JSON.
///
/// `--locked` reads the committed `Cargo.lock` as-is (no network, no
/// re-resolve) — required here in particular, since a re-resolve against the
/// private working tree's path deps could silently paper over a version fork
/// that only shows up once the `# BEGIN engine-deps` block is rewritten to
/// git deps at export.
fn cargo_metadata() -> Value {
    let output = Command::new(env!("CARGO"))
        .args(["metadata", "--format-version", "1", "--locked"])
        .current_dir(workspace_root())
        .output()
        .expect("failed to run `cargo metadata` — is cargo on PATH?");

    assert!(
        output.status.success(),
        "`cargo metadata` exited non-zero:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    serde_json::from_slice(&output.stdout).expect("cargo metadata produced invalid JSON")
}

/// Does any path component of `manifest_path` equal `segment` exactly?
///
/// Splits on both a forward slash and a backslash so this reads correctly
/// regardless of the host OS's path separator or which separator the
/// checked-out source happens to use.
fn path_has_segment(manifest_path: &str, segment: &str) -> bool {
    manifest_path
        .split(['/', '\\'])
        .any(|component| component == segment)
}

/// The one package in `packages` named `terminal_state`'s `id` field.
fn find_package_id<'a>(packages: &'a [Value], name: &str) -> &'a str {
    packages
        .iter()
        .find(|p| p["name"].as_str() == Some(name))
        .and_then(|p| p["id"].as_str())
        .unwrap_or_else(|| panic!("`{name}` package not found in `cargo metadata` output"))
}

/// BFS from `start`, following only edges whose `dep_kinds` contains an entry
/// matching `is_wanted_kind`. Returns every reached package id (`start`
/// itself included).
fn reachable_by_kind<'a>(
    start: &'a str,
    id_to_node: &HashMap<&'a str, &'a Value>,
    is_wanted_kind: impl Fn(&Value) -> bool,
) -> HashSet<&'a str> {
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    visited.insert(start);
    queue.push_back(start);

    while let Some(current) = queue.pop_front() {
        let Some(node) = id_to_node.get(current) else {
            continue;
        };
        let Some(deps) = node["deps"].as_array() else {
            continue;
        };
        for dep in deps {
            let matches = dep["dep_kinds"]
                .as_array()
                .is_some_and(|kinds| kinds.iter().any(&is_wanted_kind));
            if !matches {
                continue;
            }
            let Some(pkg_id) = dep["pkg"].as_str() else {
                continue;
            };
            if visited.insert(pkg_id) {
                queue.push_back(pkg_id);
            }
        }
    }
    visited
}

fn is_normal_kind(k: &Value) -> bool {
    k["kind"].is_null()
}

/// `terminal_state`'s **normal** (shipped) dependency subtree must contain no
/// `editor/*` package — those are dev-only tooling (`re_voximport`,
/// `re_lvlcheck`, `re_golden`), never something the built game links against.
#[test]
fn game_normal_deps_contain_no_editor_package() {
    let metadata = cargo_metadata();
    let packages = metadata["packages"]
        .as_array()
        .expect("metadata.packages must be an array");
    let nodes = metadata["resolve"]["nodes"]
        .as_array()
        .expect("metadata.resolve.nodes must be an array");

    let game_pkg_id = find_package_id(packages, "terminal_state");
    let id_to_node: HashMap<&str, &Value> = nodes
        .iter()
        .filter_map(|n| n["id"].as_str().map(|id| (id, n)))
        .collect();
    let id_to_manifest_path: HashMap<&str, &str> = packages
        .iter()
        .filter_map(|p| {
            let id = p["id"].as_str()?;
            let path = p["manifest_path"].as_str()?;
            Some((id, path))
        })
        .collect();

    let normal_reachable = reachable_by_kind(game_pkg_id, &id_to_node, is_normal_kind);

    let editor_packages: Vec<&str> = normal_reachable
        .iter()
        .filter_map(|&id| {
            let path = id_to_manifest_path.get(id)?;
            path_has_segment(path, "editor").then_some(id)
        })
        .collect();

    assert!(
        editor_packages.is_empty(),
        "LAYERING VIOLATION: `terminal_state`'s NORMAL dependency tree reaches an \
         editor/* package (only allowed as a dev-dependency): {editor_packages:?}\n\
         The game repo's normal dependencies must be engine/* + external only."
    );
}

/// The whole resolved graph (every dependency kind — this crate reaches
/// `bevy_ecs` via both its own normal `bevy` dependency and, through the dev
/// engine crates, indirectly again) must contain exactly one `bevy_ecs`
/// package. `metadata.packages` already lists one entry per distinct
/// (name, version, source) actually present in the lock, so counting matches
/// there is sufficient — no graph walk needed for this one.
#[test]
fn exactly_one_bevy_ecs_in_the_resolve() {
    let metadata = cargo_metadata();
    let packages = metadata["packages"]
        .as_array()
        .expect("metadata.packages must be an array");

    let bevy_ecs_versions: Vec<&str> = packages
        .iter()
        .filter(|p| p["name"].as_str() == Some("bevy_ecs"))
        .filter_map(|p| p["version"].as_str())
        .collect();

    assert_eq!(
        bevy_ecs_versions.len(),
        1,
        "LAYERING VIOLATION: expected exactly one `bevy_ecs` in the resolved graph, \
         found {}: {bevy_ecs_versions:?}\n\
         This means the game repo's `bevy` pin and the engine repo's `bevy` pin \
         resolved to two different `bevy_ecs` versions — align the two \
         [workspace.dependencies] tables and re-run `cargo update` in both repos.",
        bevy_ecs_versions.len()
    );
}
