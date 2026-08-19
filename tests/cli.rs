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

#[test]
fn shell_chain_keeps_directory_and_binds_middle_placeholder() {
    let temp = TempDir::new().unwrap();
    let work = temp.path().join("shell work");
    std::fs::create_dir_all(&work).unwrap();
    let argument = "safe & name.txt";

    let mut definition = vec!["create".into(), "shell-download".into(), "--shell".into()];
    #[cfg(windows)]
    definition.push("powershell".into());
    #[cfg(not(windows))]
    definition.push("sh".into());
    definition.extend(["--chain".into(), "--".into(), "cd".into()]);
    definition.push(work.as_os_str().to_owned());
    definition.push("--then".into());
    #[cfg(windows)]
    definition.extend(["New-Item", "-ItemType", "File", "-Name", "{1}"].map(Into::into));
    #[cfg(not(windows))]
    definition.extend(["touch", "{1}"].map(Into::into));
    definition.extend(["--then".into(), "ls".into()]);

    assert!(
        transfigure(&temp)
            .args(definition)
            .status()
            .unwrap()
            .success()
    );
    let run = transfigure(&temp)
        .args(["run", "shell-download", "--", argument])
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(work.join(argument).exists());
}

#[test]
fn reports_missing_and_unused_placeholder_arguments() {
    let temp = TempDir::new().unwrap();
    assert!(
        transfigure(&temp)
            .args(["create", "templated", "--", "unused-tool", "{1}"])
            .status()
            .unwrap()
            .success()
    );
    let missing = transfigure(&temp)
        .args(["run", "templated"])
        .output()
        .unwrap();
    assert!(!missing.status.success());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("requires invocation argument 1"));

    let extra = transfigure(&temp)
        .args(["run", "templated", "first", "second"])
        .output()
        .unwrap();
    assert!(!extra.status.success());
    assert!(String::from_utf8_lossy(&extra.stderr).contains("argument 2 is unused"));
}

#[test]
fn shell_chain_stops_before_later_steps() {
    let temp = TempDir::new().unwrap();
    let marker = temp.path().join("should-not-exist");
    let mut definition = vec!["create", "stop", "--shell"];
    #[cfg(windows)]
    definition.extend(["powershell", "--chain", "--", "cmd", "/C", "exit", "7"]);
    #[cfg(not(windows))]
    definition.extend(["sh", "--chain", "--", "sh", "-c", "exit 7"]);
    definition.push("--then");
    #[cfg(windows)]
    definition.extend(["New-Item", "-ItemType", "File", "-Path"]);
    #[cfg(not(windows))]
    definition.push("touch");

    let mut command = transfigure(&temp);
    command.args(definition).arg(&marker);
    assert!(command.status().unwrap().success());
    let run = transfigure(&temp).args(["run", "stop"]).status().unwrap();
    assert_eq!(run.code(), Some(7));
    assert!(!marker.exists());
}

#[test]
fn accepts_a_definition_without_the_separator() {
    let temp = TempDir::new().unwrap();
    assert!(
        transfigure(&temp)
            .args(["create", "compact", "example-program", "fixed-argument"])
            .status()
            .unwrap()
            .success()
    );
    let shown = transfigure(&temp)
        .args(["show", "compact"])
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&shown.stdout).contains("example-program fixed-argument"));
}

#[test]
fn expands_all_placeholder_arguments_in_direct_mode() {
    let temp = TempDir::new().unwrap();
    #[cfg(windows)]
    let definition = ["create", "all-args", "--", "cmd", "/D", "/C", "echo", "{*}"];
    #[cfg(not(windows))]
    let definition = ["create", "all-args", "--", "printf", "<%s>\\n", "{*}"];
    assert!(
        transfigure(&temp)
            .args(definition)
            .status()
            .unwrap()
            .success()
    );
    let run = transfigure(&temp)
        .args(["run", "all-args", "alpha", "two words"])
        .output()
        .unwrap();
    assert!(run.status.success());
    let output = String::from_utf8_lossy(&run.stdout);
    assert!(output.contains("alpha"));
    assert!(output.contains("two words"));
}

fn launcher_path(root: &Path, name: &str) -> std::path::PathBuf {
    #[cfg(windows)]
    return root.join("bin").join(format!("{name}.cmd"));
    #[cfg(not(windows))]
    return root.join("bin").join(name);
}
