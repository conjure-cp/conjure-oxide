#![no_main]

/// fuzz tester for detects_error() in the parser crate, using libFuzzer
///
/// fuzzer takes existing code from /corpus, modifies and removes the bytes to make new essence tests
/// the fuzzer makes sure that the given function can take in all input without panicking or crashing
use conjure_cp_essence_parser::detect_errors;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = detect_errors(s);
    }
});
