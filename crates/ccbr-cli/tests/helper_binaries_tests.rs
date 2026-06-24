use std::process::Command;

fn helper_binary(name: &str) -> std::path::PathBuf {
    let mut exe = std::env::current_exe().unwrap();
    if let Ok(resolved) = std::fs::canonicalize(&exe) {
        exe = resolved;
    }
    // current_exe is under target/debug/deps/; helper binaries live in target/debug/.
    exe.parent().unwrap().parent().unwrap().join(name)
}

#[test]
fn ask_version_introspection() {
    let output = Command::new(helper_binary("ask"))
        .arg("--version")
        .output()
        .expect("ask binary should run");
    assert!(output.status.success(), "ask --version should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("ccbr 7.5.2"),
        "unexpected version output: {stdout}"
    );
}

#[test]
fn autonew_version_introspection() {
    let output = Command::new(helper_binary("autonew"))
        .arg("--version")
        .output()
        .expect("autonew binary should run");
    assert!(output.status.success(), "autonew --version should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("ccbr 7.5.2"),
        "unexpected version output: {stdout}"
    );
}

#[test]
fn ctx_transfer_version_introspection() {
    let output = Command::new(helper_binary("ctx-transfer"))
        .arg("--version")
        .output()
        .expect("ctx-transfer binary should run");
    assert!(
        output.status.success(),
        "ctx-transfer --version should succeed"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("ccbr 7.5.2"),
        "unexpected version output: {stdout}"
    );
}

#[test]
fn ask_help_introspection() {
    let output = Command::new(helper_binary("ask"))
        .arg("--help")
        .output()
        .expect("ask binary should run");
    assert!(output.status.success(), "ask --help should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Usage:") && stdout.contains("ask ") && stdout.contains("ccbr ask"),
        "unexpected help output: {stdout}"
    );
}

#[test]
fn autonew_help_introspection() {
    let output = Command::new(helper_binary("autonew"))
        .arg("--help")
        .output()
        .expect("autonew binary should run");
    assert!(output.status.success(), "autonew --help should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Usage:") && stdout.contains("autonew ") && stdout.contains("claude"),
        "unexpected help output: {stdout}"
    );
}

#[test]
fn ctx_transfer_help_introspection() {
    let output = Command::new(helper_binary("ctx-transfer"))
        .arg("--help")
        .output()
        .expect("ctx-transfer binary should run");
    assert!(
        output.status.success(),
        "ctx-transfer --help should succeed"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Usage:") && stdout.contains("ctx-transfer ") && stdout.contains("--send"),
        "unexpected help output: {stdout}"
    );
}

#[test]
fn helper_binaries_refuse_to_run_without_runtime_ok() {
    // Running `ask <target> <msg>` from the source checkout without an allowed
    // project should hit the source-runtime guard.
    let output = Command::new(helper_binary("ask"))
        .args(["claude", "hello"])
        .output()
        .expect("ask binary should run");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Refusing to run"),
        "expected source-runtime guard denial, got: {stderr}"
    );
}
