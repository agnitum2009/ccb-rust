use ccb_runtime_pid_cleanup::version;

#[test]
fn crate_compiles_and_runs() {
    assert_eq!(version(), env!("CARGO_PKG_VERSION"));
}
