use std::path::Path;
use std::process::{Command, Stdio};

use tempfile::TempDir;

fn sp_bin() -> std::path::PathBuf {
    let mut path = std::env::current_exe().unwrap();
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    path.join("sp")
}

fn sp(workspace: &Path) -> Command {
    let mut cmd = Command::new(sp_bin());
    cmd.env("XDG_CONFIG_HOME", workspace.join("xdg-config"));
    cmd.env("HOME", workspace);
    cmd.env_remove("SP_PROJECT");
    cmd
}

fn run_in(workspace: &Path, cwd: &Path, args: &[&str]) -> std::process::Output {
    let mut cmd = sp(workspace);
    cmd.current_dir(cwd);
    cmd.args(args);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    cmd.output().expect("failed to run sp")
}

fn git_init(dir: &Path, origin: Option<&str>) {
    Command::new("git")
        .args(["init", "-q"])
        .current_dir(dir)
        .status()
        .unwrap();
    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(dir)
        .status()
        .unwrap();
    Command::new("git")
        .args(["config", "user.name", "test"])
        .current_dir(dir)
        .status()
        .unwrap();
    if let Some(url) = origin {
        Command::new("git")
            .args(["remote", "add", "origin", url])
            .current_dir(dir)
            .status()
            .unwrap();
    }
}

#[test]
fn project_resolves_from_git_remote() {
    let workspace = TempDir::new().unwrap();
    let repo = TempDir::new().unwrap();
    git_init(repo.path(), Some("git@github.com:acme/api.git"));

    let out = run_in(workspace.path(), repo.path(), &["context"]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("acme/api"), "got: {stdout}");
    assert!(stdout.contains("git_remote_origin"), "got: {stdout}");
}

#[test]
fn new_session_lands_under_project_dir() {
    let workspace = TempDir::new().unwrap();
    let repo = TempDir::new().unwrap();
    git_init(repo.path(), Some("https://github.com/acme/api.git"));

    let out = run_in(workspace.path(), repo.path(), &["new", "auth-refactor"]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let path_str = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let expected = workspace
        .path()
        .join(".scratchpad/projects/acme/api/auth-refactor");
    assert_eq!(Path::new(&path_str), expected);
    assert!(expected.join("notes.md").exists());
    assert!(expected.join(".sp/meta.toml").exists());
}

#[test]
fn write_increments_revision_and_returns_path() {
    let workspace = TempDir::new().unwrap();
    let repo = TempDir::new().unwrap();
    git_init(repo.path(), Some("git@github.com:acme/api.git"));

    let new_out = run_in(workspace.path(), repo.path(), &["new", "perf-issue"]);
    assert!(new_out.status.success());

    let mut cmd = sp(workspace.path());
    cmd.current_dir(repo.path());
    cmd.args(["write", "perf-issue/capture.log"]);
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().unwrap();
    {
        use std::io::Write;
        child.stdin.as_mut().unwrap().write_all(b"hello").unwrap();
    }
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let session_dir = workspace
        .path()
        .join(".scratchpad/projects/acme/api/perf-issue");
    let meta = std::fs::read_to_string(session_dir.join(".sp/meta.toml")).unwrap();
    assert!(meta.contains("revision = 2") || meta.contains("revision = 1"));
    let capture = session_dir.join("capture.log");
    assert_eq!(std::fs::read_to_string(&capture).unwrap(), "hello");
}

#[test]
fn revision_conflict_exits_with_code_4() {
    let workspace = TempDir::new().unwrap();
    let repo = TempDir::new().unwrap();
    git_init(repo.path(), Some("git@github.com:acme/api.git"));

    run_in(workspace.path(), repo.path(), &["new", "auth"]);

    let mut cmd = sp(workspace.path());
    cmd.current_dir(repo.path());
    cmd.args(["write", "auth/notes.md", "--expect-revision", "999"]);
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().unwrap();
    {
        use std::io::Write;
        child.stdin.as_mut().unwrap().write_all(b"x").unwrap();
    }
    let out = child.wait_with_output().unwrap();
    assert_eq!(out.status.code(), Some(4));
}

#[test]
fn last_returns_most_recent_artifact() {
    let workspace = TempDir::new().unwrap();
    let repo = TempDir::new().unwrap();
    git_init(repo.path(), Some("git@github.com:acme/api.git"));

    run_in(workspace.path(), repo.path(), &["new", "first"]);
    std::thread::sleep(std::time::Duration::from_millis(1100));
    run_in(workspace.path(), repo.path(), &["new", "second"]);

    let session_dir = workspace
        .path()
        .join(".scratchpad/projects/acme/api/second");
    std::fs::write(session_dir.join("latest.log"), "fresh").unwrap();

    let out = run_in(workspace.path(), repo.path(), &["last", "--path"]);
    assert!(out.status.success());
    let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
    assert!(path.ends_with("second/latest.log"), "got: {path}");
}

#[test]
fn list_json_yields_array() {
    let workspace = TempDir::new().unwrap();
    let repo = TempDir::new().unwrap();
    git_init(repo.path(), Some("git@github.com:acme/api.git"));

    run_in(
        workspace.path(),
        repo.path(),
        &["new", "alpha", "--tag", "bug"],
    );

    let out = run_in(workspace.path(), repo.path(), &["list", "--json"]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let arr = v.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["slug"], "alpha");
    assert_eq!(arr[0]["project"], "acme/api");
    assert_eq!(arr[0]["tags"][0], "bug");
}

#[test]
fn shared_project_when_no_git() {
    let workspace = TempDir::new().unwrap();
    let dir = TempDir::new().unwrap();

    let out = run_in(workspace.path(), dir.path(), &["context"]);
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("shared"), "got: {stdout}");
    let expected = workspace.path().join(".scratchpad/shared");
    assert!(stdout.contains(&expected.display().to_string()));
}

#[test]
fn archive_and_restore_change_status() {
    let workspace = TempDir::new().unwrap();
    let repo = TempDir::new().unwrap();
    git_init(repo.path(), Some("git@github.com:acme/api.git"));

    run_in(workspace.path(), repo.path(), &["new", "spike"]);
    let out = run_in(workspace.path(), repo.path(), &["archive", "spike"]);
    assert!(out.status.success());

    let json = run_in(
        workspace.path(),
        repo.path(),
        &["list", "--json", "--status", "archived"],
    );
    let arr: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&json.stdout)).unwrap();
    assert_eq!(arr.as_array().unwrap().len(), 1);
    assert_eq!(arr[0]["status"], "archived");

    run_in(workspace.path(), repo.path(), &["restore", "spike"]);
    let json2 = run_in(
        workspace.path(),
        repo.path(),
        &["list", "--json", "--status", "active"],
    );
    let arr2: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&json2.stdout)).unwrap();
    assert_eq!(arr2.as_array().unwrap().len(), 1);
}

#[test]
fn project_link_creates_alias_and_groups_sessions() {
    let workspace = TempDir::new().unwrap();
    let repo_a = TempDir::new().unwrap();
    let repo_b = TempDir::new().unwrap();
    git_init(repo_a.path(), Some("git@github.com:acme/api.git"));
    git_init(repo_b.path(), Some("git@github.com:acme/worker.git"));

    let out = run_in(
        workspace.path(),
        repo_a.path(),
        &["project", "link", "payments"],
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = run_in(
        workspace.path(),
        repo_b.path(),
        &["project", "link", "payments"],
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let ctx_a = run_in(workspace.path(), repo_a.path(), &["context"]);
    assert!(String::from_utf8_lossy(&ctx_a.stdout).contains("payments"));

    run_in(workspace.path(), repo_a.path(), &["new", "shared-feature"]);
    let list = run_in(workspace.path(), repo_b.path(), &["list", "--json"]);
    let arr: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&list.stdout)).unwrap();
    assert_eq!(arr.as_array().unwrap().len(), 1);
    assert_eq!(arr[0]["slug"], "shared-feature");
    assert_eq!(arr[0]["project"], "payments");
}
