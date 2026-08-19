use std::path::Path;
use std::process::Command;

use tempfile::TempDir;

fn transfigure(temp: &TempDir) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_transfigure"));
    command
        .env("TRANSFIGURE_CONFIG_DIR", temp.path().join("config"))
        .env("TRANSFIGURE_BIN_DIR", temp.path().join("bin"));
    command
}

#[test]
fn creates_lists_and_removes_a_shortcut() {
    let temp = TempDir::new().unwrap();
    let create = transfigure(&temp)
        .args(["create", "download", "--", "yt-dlp", "-f", "best video"])
        .status()
        .unwrap();
    assert!(create.success());
    let launcher = launcher_path(temp.path(), "download");
    assert!(launcher.exists());

    let list = transfigure(&temp).arg("list").output().unwrap();
    assert!(String::from_utf8_lossy(&list.stdout).contains("download"));
    assert!(
        transfigure(&temp)
            .args(["remove", "download"])
            .status()
            .unwrap()
            .success()
    );
    assert!(!launcher.exists());
}

#[test]
fn propagates_a_child_exit_code() {
    let temp = TempDir::new().unwrap();
    #[cfg(windows)]
    let definition = ["create", "fail", "--", "cmd", "/C", "exit", "7"];
    #[cfg(not(windows))]
    let definition = ["create", "fail", "--", "sh", "-c", "exit 7"];
    assert!(
        transfigure(&temp)
            .args(definition)
            .status()
            .unwrap()
            .success()
    );
    let run = transfigure(&temp).args(["run", "fail"]).status().unwrap();
    assert_eq!(run.code(), Some(7));
}

#[test]
fn rejects_case_insensitive_duplicates() {
    let temp = TempDir::new().unwrap();
    assert!(
        transfigure(&temp)
            .args(["create", "build", "--", "tool"])
            .status()
            .unwrap()
            .success()
    );
    let duplicate = transfigure(&temp)
        .args(["create", "BUILD", "--", "other"])
        .output()
        .unwrap();
    assert!(!duplicate.status.success());
    assert!(String::from_utf8_lossy(&duplicate.stderr).contains("already exists"));
}

fn launcher_path(root: &Path, name: &str) -> std::path::PathBuf {
    #[cfg(windows)]
    return root.join("bin").join(format!("{name}.cmd"));
    #[cfg(not(windows))]
    return root.join("bin").join(name);
}
