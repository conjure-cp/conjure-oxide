#[test]
fn derive_tests() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/derive-fail/*.rs");
    t.pass("tests/derive-pass/*.rs");
    t.pass("examples/*.rs");
}
