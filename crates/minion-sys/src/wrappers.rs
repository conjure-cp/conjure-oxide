//! Small wrapper functions over FFI things.

use std::ffi::{CStr, CString, c_char};

use crate::ffi;
use libc::free;

/// Gets a given value from Minion's TableOut (where it stores run statistics).
pub fn get_from_table(key: String) -> Option<String> {
    unsafe {
        #[allow(clippy::expect_used)]
        let c_string = CString::new(key).expect("");
        let key_ptr = c_string.into_raw();
        let val_ptr: *mut c_char = ffi::TableOut_get(key_ptr);

        drop(CString::from_raw(key_ptr));

        if val_ptr.is_null() {
            free(val_ptr as _);
            None
        } else {
            #[allow(clippy::unwrap_used)]
            // CStr borrows the string in the ptr.
            // We convert it to &str then clone into a String.
            let res = CStr::from_ptr(val_ptr).to_str().unwrap().to_owned();
            free(val_ptr as _);
            Some(res)
        }
    }
}
