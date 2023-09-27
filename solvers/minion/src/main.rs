#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use minion_rs::bindings::MinionVersion;
use std::ffi::c_char;
use std::ffi::CString;

pub fn main() {
    unsafe {
        let minionVersion: &str = std::str::from_utf8_unchecked(MinionVersion);
        println!("{:?}", minionVersion);
    }

    // https://stackoverflow.com/questions/34379641/how-do-i-convert-rust-args-into-the-argc-and-argv-c-equivalents
    let args = std::env::args()
        .map(|arg| CString::new(arg).unwrap())
        .collect::<Vec<CString>>();

    let c_args = args
        .iter()
        .map(|arg| arg.as_ptr())
        .collect::<Vec<*const c_char>>();

    unsafe {
        minion_rs::bindings::minion_main(c_args.len() as i32, c_args.as_ptr() as *mut *mut i8);
    }
}
