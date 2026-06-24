use ccbr_message_bureau::version;

#[test]
fn crate_compiles_and_runs() {
    assert_eq!(version(), env!("CARGO_PKG_VERSION"));
}
