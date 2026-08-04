/// Runs every UI fixture against the lint library built for this test process.
#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
