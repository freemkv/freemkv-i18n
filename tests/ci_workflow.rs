// ST-I0 (stop-design-v5.md §5.7), IT0: ci/pick-libfreemkv-ref.sh must not compare
// against `github.ref_name`, which is `<n>/merge` on a `pull_request` run. Hermetic: a
// fake `git` first on PATH stands in for the real `git ls-remote` network call.

use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

/// A fake `git` that only understands `ls-remote --exit-code --heads <url> <ref>` and
/// exits with `ls_remote_exit` every time, regardless of the ref asked about. Returns
/// the directory holding it, to be put first on PATH.
fn fake_git(dir: &std::path::Path, ls_remote_exit: i32) {
    let path = dir.join("git");
    let mut f = std::fs::File::create(&path).unwrap();
    writeln!(
        f,
        "#!/bin/sh\nif [ \"$1\" = ls-remote ]; then exit {ls_remote_exit}; fi\necho \"fake git: unsupported: $*\" >&2\nexit 99"
    )
    .unwrap();
    drop(f);
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

struct Run {
    status: std::process::ExitStatus,
    stderr: String,
    output: Option<String>,
}

fn run_picker(ls_remote_exit: i32, env: &[(&str, &str)]) -> Run {
    // A timestamp alone can collide between threads running concurrently (`cargo test`'s
    // default); an atomic counter guarantees each call gets its own directory.
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = std::env::temp_dir().join(format!(
        "pick-libfreemkv-ref-test-{}-{n}",
        std::process::id()
    ));
    std::fs::create_dir_all(&tmp).unwrap();
    fake_git(&tmp, ls_remote_exit);
    let out_file = tmp.join("github_output");
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ci/pick-libfreemkv-ref.sh");

    let real_path = std::env::var("PATH").unwrap_or_default();
    let mut cmd = Command::new(&script);
    cmd.env("PATH", format!("{}:{real_path}", tmp.display()));
    for var in ["GITHUB_HEAD_REF", "GITHUB_BASE_REF", "GITHUB_REF_NAME"] {
        if !env.iter().any(|(k, _)| *k == var) {
            cmd.env_remove(var);
        }
    }
    cmd.env("GITHUB_OUTPUT", &out_file);
    for (k, v) in env {
        cmd.env(k, v);
    }
    let out = cmd.output().expect("run ci/pick-libfreemkv-ref.sh");

    let output = std::fs::File::open(&out_file).ok().map(|mut f| {
        let mut s = String::new();
        f.read_to_string(&mut s).unwrap();
        s
    });
    std::fs::remove_dir_all(&tmp).ok();
    Run {
        status: out.status,
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        output,
    }
}

/// `ls-remote --exit-code` exits 0: the branch exists on libfreemkv, so a PR whose
/// head branch is paired with one there uses that branch.
#[test]
fn a_pr_with_a_paired_libfreemkv_branch_uses_it() {
    let run = run_picker(
        0,
        &[
            ("GITHUB_HEAD_REF", "feature-x"),
            ("GITHUB_BASE_REF", "qa"),
            ("GITHUB_REF_NAME", "5/merge"),
        ],
    );
    assert!(run.status.success(), "{}", run.stderr);
    assert_eq!(run.output.as_deref(), Some("ref=feature-x\n"));
}

/// `ls-remote --exit-code` exits 2: no such branch on libfreemkv (J-5.5-4). A PR falls
/// back to its base branch, never the pull_request `<n>/merge` ref (GITHUB_REF_NAME).
#[test]
fn a_pr_with_no_pair_falls_back_to_its_base_branch() {
    let run = run_picker(
        2,
        &[
            ("GITHUB_HEAD_REF", "feature-x"),
            ("GITHUB_BASE_REF", "qa"),
            ("GITHUB_REF_NAME", "5/merge"),
        ],
    );
    assert!(run.status.success(), "{}", run.stderr);
    assert_eq!(run.output.as_deref(), Some("ref=qa\n"));
}

/// A push run has no GITHUB_HEAD_REF/GITHUB_BASE_REF; `want` is GITHUB_REF_NAME itself.
/// No pair on libfreemkv falls back to the plain `dev` default.
#[test]
fn a_push_with_no_pair_falls_back_to_dev() {
    let run = run_picker(2, &[("GITHUB_REF_NAME", "some-feature")]);
    assert!(run.status.success(), "{}", run.stderr);
    assert_eq!(run.output.as_deref(), Some("ref=dev\n"));
}

/// Only exit 2 means "no such branch". Any other `ls-remote` failure (network down,
/// rate-limited, ...) must fail the job loudly, not silently fall back to dev/base.
#[test]
fn a_real_ls_remote_failure_fails_loudly_instead_of_falling_back() {
    let run = run_picker(
        128,
        &[("GITHUB_HEAD_REF", "feature-x"), ("GITHUB_BASE_REF", "qa")],
    );
    assert!(
        !run.status.success(),
        "a real ls-remote failure must not exit 0"
    );
    assert!(
        run.stderr.contains("::error::") && run.stderr.contains("128"),
        "expected a loud, explicit error message; got: {:?}",
        run.stderr
    );
    assert!(
        run.output.is_none_or(|o| !o.contains("ref=")),
        "must not silently write a fallback ref"
    );
}
