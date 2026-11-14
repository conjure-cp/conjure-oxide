#![no_main]

use conjure_cp::pipeline::{Config, Pipeline};
use conjure_cp::settings::QuantifiedExpander;
use conjure_cp_rules as _;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(source) = std::str::from_utf8(data) else {
        return;
    };
    let pipeline = Pipeline::new(Config {
        comprehension_expander: QuantifiedExpander::Native,
        ..Default::default()
    })
    .expect("the linked default rule sets should resolve");

    // Invalid models are expected; panics and crashes are fuzzer findings.
    let _ = pipeline.compile(source, None);
});
