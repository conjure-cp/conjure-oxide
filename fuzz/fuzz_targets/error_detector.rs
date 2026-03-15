#![no_main]

use conjure_cp_essence_parser::detect_errors;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = detect_errors(s);
    }
});
