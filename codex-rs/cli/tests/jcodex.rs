use anyhow::Result;
use codex_utils_cargo_bin::cargo_bin;
use predicates::str::contains;
use pretty_assertions::assert_eq;

#[test]
fn shared_config_cannot_disable_monitors_and_is_not_rewritten() -> Result<()> {
    let home = tempfile::tempdir()?;
    let config = "[features]\nmonitor = false\nunified_exec = false\n";
    std::fs::write(home.path().join("config.toml"), config)?;
    let output = assert_cmd::Command::new(cargo_bin("codex")?)
        .env("CODEX_HOME", home.path())
        .args(["--disable", "monitor", "features", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let output = String::from_utf8(output)?;
    assert_eq!(
        output
            .lines()
            .map(|line| line.split_whitespace().collect::<Vec<_>>())
            .find(|fields| fields.first() == Some(&"monitor")),
        Some(vec!["monitor", "stable", "true"])
    );
    assert_eq!(
        std::fs::read_to_string(home.path().join("config.toml"))?,
        config
    );
    Ok(())
}

#[test]
fn fork_does_not_start_or_modify_the_shared_daemon() -> Result<()> {
    let home = tempfile::tempdir()?;
    for args in [
        vec!["app-server", "daemon", "start"],
        vec!["remote-control", "start"],
    ] {
        assert_cmd::Command::new(cargo_bin("codex")?)
            .env("CODEX_HOME", home.path())
            .args(args)
            .assert()
            .failure()
            .stderr(contains("jcodex does not manage the shared Codex daemon"));
    }
    Ok(())
}
