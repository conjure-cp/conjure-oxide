#![no_main]

use conjure_cp_essence_parser::util::get_tree;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // checks that the data is actually valid strings
    if let Ok(s) = std::str::from_utf8(data) {
        
        // checks if the strings are valid essence or nah
        if let Some(cst) = get_tree(&s) {

            // use a func to run the input thru conjure and conjure oxide and capture the result

            // 
        } 
    }
});
