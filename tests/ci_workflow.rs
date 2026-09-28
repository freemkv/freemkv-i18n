// ST-I0 (stop-design-v5.md §5.7), IT0: runs ci/pick-libfreemkv-ref.sh (a real
// `git ls-remote` against github.com, so this needs network) under push and PR
// environments and asserts what it writes to $GITHUB_OUTPUT.

use std::io::Read;
use std::process::Command;

fn run_picker(env: &[(&str, &str)]) -> String {
    let out_file = std::env::temp_dir().join(format!(
        "pick-libfreemkv-ref-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ci/pick-libfreemkv-ref.sh");

    let mut cmd = Command::new(&script);
    // Only GITHUB_HEAD_REF/GITHUB_BASE_REF/GITHUB_REF_NAME drive the script; clear
    // whichever of the three this scenario does not set, so a real CI run's own
    // (unrelated) values can't leak in and change the outcome.
    for var in ["GITHUB_HEAD_REF", "GITHUB_BASE_REF", "GITHUB_REF_NAME"] {
        if !env.iter().any(|(k, _)| *k == var) {
            cmd.env_remove(var);
        }
    }
    cmd.env("GITHUB_OUTPUT", &out_file);
    for (k, v) in env {
        cmd.env(k, v);
    }
    let status = cmd.status().expect("run ci/pick-libfreemkv-ref.sh");
    assert!(status.success(), "picker script exited non-zero");

    let mut contents = String::new();
    std::fs::File::open(&out_file)
        .expect("script wrote $GITHUB_OUTPUT")
        .read_to_string(&mut contents)
        .unwrap();
    std::fs::remove_file(&out_file).ok();
    contents
}

/// GITHUB_HEAD_REF is set on `pull_request` runs; a feature branch with no libfreemkv
/// pair must fall back to the PR's base branch (J-5.5-4), never the PR's own
/// `<n>/merge` ref (GITHUB_REF_NAME here, as GitHub sets it on pull_request events).
#[test]
fn drift_job_resolves_a_real_ref_on_push_and_pr() {
    if std::env::var_os("FREEMKV_I18N_SKIP_NETWORK_TESTS").is_some() {
        eprintln!("FREEMKV_I18N_SKIP_NETWORK_TESTS set; skipping (needs github.com)");
        return;
    }

    // Pull request: no `feature-x` branch on freemkv/libfreemkv, so it falls back to
    // GITHUB_BASE_REF, never the pull_request `<n>/merge` value.
    let pr_out = run_picker(&[
        ("GITHUB_HEAD_REF", "feature-x"),
        ("GITHUB_BASE_REF", "dev"),
        ("GITHUB_REF_NAME", "5/merge"),
    ]);
    assert_eq!(pr_out, "ref=dev\n");
    assert!(!pr_out.contains("5/merge"), "{pr_out:?}");

    // Push: GITHUB_HEAD_REF is unset, so `want` is GITHUB_REF_NAME itself; `dev` is a
    // real libfreemkv branch, so it is used directly (the paired-branch case).
    let push_out = run_picker(&[("GITHUB_REF_NAME", "dev")]);
    assert_eq!(push_out, "ref=dev\n");
}
