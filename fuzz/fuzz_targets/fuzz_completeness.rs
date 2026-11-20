#![no_main]

// use conjure_cp_essence_parser::util::get_tree;
use libfuzzer_sys::fuzz_target;
use conjure_cp::{pipeline::{Config, Pipeline}};
use conjure_cp::settings::QuantifiedExpander;
use conjure_cp::solutions::SolutionOptions;


fuzz_target!(|data: &[u8]| {
    if let Ok(source) = std::str::from_utf8(data) {

        let pipeline = Pipeline::new(Config { 
            comprehension_expander: QuantifiedExpander::Native, 
            ..Default::default()
        }).expect("the default rule sets should resolve");

        if let Ok(model) = pipeline.compile(source, None) {
           let _ = pipeline.solve(model, &SolutionOptions::default());
        };
    }
});
