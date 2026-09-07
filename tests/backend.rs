#[cfg(feature = "embedded-git")]
#[test]
fn missing_git_uses_embedded_and_overrides_are_authoritative() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_synchrogit"))
        .arg("backend")
        .env("PATH", "")
        .env("SYNCHROGIT_GIT_BACKEND", "embedded")
        .env_remove("SYNCHROGIT_GIT")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("embedded (libgit2)"));

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_synchrogit"))
        .arg("backend")
        .env("SYNCHROGIT_GIT_BACKEND", "auto")
        .env("SYNCHROGIT_GIT", "/explicit/git/path")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("external (/explicit/git/path)"));
}
