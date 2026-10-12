//! Guard: the shipped/default game build must not pull a GPU/windowing stack.
//! `#[ignore]`d because it shells out to cargo — run explicitly in CI / via
//! /platform-check:  `cargo test -p terminal_state --test no_gpu_in_default_build -- --ignored`
use std::process::Command;

#[test]
#[ignore]
fn default_game_tree_has_no_wgpu_or_winit() {
    let out = Command::new(env!("CARGO"))
        .args([
            "tree",
            "-p",
            "terminal_state",
            "--no-default-features",
            "--features",
            "backend-terminal",
            "-e",
            "normal",
        ])
        .output()
        .expect("cargo tree failed to run");
    assert!(out.status.success(), "cargo tree exited non-zero");
    let tree = String::from_utf8_lossy(&out.stdout);
    for forbidden in ["wgpu", "winit", "bevy_render"] {
        assert!(
            !tree.contains(forbidden),
            "default game build must not depend on `{forbidden}`:\n{tree}"
        );
    }
}
