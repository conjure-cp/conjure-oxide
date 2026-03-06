#![no_main]

use libfuzzer_sys::fuzz_target;
use conjure_cp_essence_parser::detect_errors;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = detect_errors(s);
    }
});
