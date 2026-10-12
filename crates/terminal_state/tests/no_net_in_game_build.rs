//! Guard: the shipped/default game build must not pull any networking crates.
//!
//! `bevy_replicon`, `bevy_replicon_renet`, and `renet` belong exclusively to the
//! `ts_net`/`ts_netplay` testbed crates. `terminal_state` (and its `dist` artifact)
//! must have zero dependency on them — networking is out of scope for the
//! shipped game.
//!
//! ## Mechanism
//!
//! The guard calls `cargo metadata --format-version 1` (JSON output) and walks the
//! **resolved dependency graph** starting from the `terminal_state` package node, following
//! only **normal** (non-dev, non-build) edges via `resolve.nodes[].deps[].dep_kinds`.
//! A BFS collects every transitively reachable package ID; the test fails if any of
//! the three forbidden crate names appear in that reachable set.
//!
//! ### Why `cargo metadata` instead of `cargo tree`
//!
//! `cargo tree` acquires an exclusive write lock on the build output directory before
//! it runs, which deadlocks when called from within `cargo test` (the outer test
//! harness holds that same lock for the duration of the test run). `cargo metadata`
//! reads only the manifest files and the existing `Cargo.lock`; it does NOT acquire
//! the build output lock, so it is safe to call from inside `cargo test`.
//!
//! `serde_json` (dev-dep) is used for robust JSON parsing. It is already present in
//! `Cargo.lock` as a transitive dep so this adds no new network download.

use std::{
    collections::{HashSet, VecDeque},
    process::Command,
};

use serde_json::Value;

const FORBIDDEN: &[&str] = &["bevy_replicon", "bevy_replicon_renet", "renet"];

/// Walk `terminal_state`'s normal (non-dev, non-build) dependency subtree in the resolved
/// graph and assert that none of the networking crates are reachable.
#[test]
fn default_game_tree_has_no_networking_crates() {
    // ── 1. Run cargo metadata ────────────────────────────────────────────────
    //
    // `--format-version 1` emits stable JSON on stdout.
    // We pass `--locked` so it uses the committed Cargo.lock without attempting
    // any resolution (avoids network access and is fast).
    // `2>/dev/null` equivalent: we only parse stdout; stderr is ignored.
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

    // ── 2. Parse the JSON ────────────────────────────────────────────────────
    let json: Value =
        serde_json::from_slice(&output.stdout).expect("cargo metadata produced invalid JSON");

    // ── 3. Find terminal_state's package ID ─────────────────────────────────
    let packages = json["packages"]
        .as_array()
        .expect("metadata.packages must be an array");

    let game_pkg_id = packages
        .iter()
        .find(|p| p["name"].as_str() == Some("terminal_state"))
        .and_then(|p| p["id"].as_str())
        .expect("terminal_state package not found in metadata");

    // ── 4. Build a lookup: package-id → node (from resolve) ─────────────────
    let nodes = json["resolve"]["nodes"]
        .as_array()
        .expect("metadata.resolve.nodes must be an array");

    // Map package-id string → index in `nodes` for O(1) lookup.
    let id_to_node: std::collections::HashMap<&str, &Value> = nodes
        .iter()
        .filter_map(|n| n["id"].as_str().map(|id| (id, n)))
        .collect();

    // ── 5. BFS from terminal_state, following normal deps only ───────────────
    //
    // A dep edge is "normal" when at least one entry in `dep_kinds` has
    // `kind == null` (JSON null). Dev-deps have `"dev"`, build-deps have `"build"`.
    let mut visited: HashSet<&str> = HashSet::new();
    let mut queue: VecDeque<&str> = VecDeque::new();

    visited.insert(game_pkg_id);
    queue.push_back(game_pkg_id);

    while let Some(current_id) = queue.pop_front() {
        let node = match id_to_node.get(current_id) {
            Some(n) => n,
            None => continue, // external node not in resolve (shouldn't happen)
        };

        let deps = match node["deps"].as_array() {
            Some(d) => d,
            None => continue,
        };

        for dep in deps {
            // Only follow normal (non-dev, non-build) edges.
            let is_normal = dep["dep_kinds"]
                .as_array()
                .map(|kinds| kinds.iter().any(|k| k["kind"].is_null()))
                .unwrap_or(false);

            if !is_normal {
                continue;
            }

            let pkg_id = match dep["pkg"].as_str() {
                Some(id) => id,
                None => continue,
            };

            if visited.insert(pkg_id) {
                queue.push_back(pkg_id);
            }
        }
    }

    // ── 6. Build a set of reachable package names ────────────────────────────
    //
    // Package IDs look like:
    //   "registry+...#bevy_replicon@0.40.0"
    //   "path+file:///…/crates/terminal_state#terminal_state@0.1.0" (the package
    //   name and the crate directory both read `terminal_state`)
    //
    // Extract the name portion from each ID by looking it up in `packages`.
    let id_to_name: std::collections::HashMap<&str, &str> = packages
        .iter()
        .filter_map(|p| {
            let id = p["id"].as_str()?;
            let name = p["name"].as_str()?;
            Some((id, name))
        })
        .collect();

    let reachable_names: HashSet<&str> = visited
        .iter()
        .filter_map(|id| id_to_name.get(id).copied())
        .collect();

    // ── 7. Assert none of the forbidden crates are reachable ─────────────────
    for &forbidden in FORBIDDEN {
        assert!(
            !reachable_names.contains(forbidden),
            "ISOLATION VIOLATION: `{}` is reachable from `terminal_state`'s normal dependency tree.\n\
             Networking crates must live ONLY in `ts_net`/`ts_netplay` — never in `terminal_state`.\n\
             Reachable packages ({} total): {:?}",
            forbidden,
            reachable_names.len(),
            {
                let mut sorted: Vec<&str> = reachable_names.iter().copied().collect();
                sorted.sort_unstable();
                sorted
            }
        );
    }
}

/// Returns the workspace root directory (two levels above this test's manifest
/// directory, which is `crates/terminal_state`).
///
/// `CARGO_MANIFEST_DIR` is set by cargo to the directory of the crate under test.
fn workspace_root() -> std::path::PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    // crates/terminal_state → ../../ = workspace root
    std::path::Path::new(manifest_dir)
        .parent()
        .expect("crates/terminal_state has no parent")
        .parent()
        .expect("crates/ has no parent")
        .to_path_buf()
}
