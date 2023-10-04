// adapted from
// https://github.com/gokberkkocak/rust_glucose/blob/master/build.rs
// and
// https://rust-lang.github.io/rust-bindgen/non-system-libraries.html

use std::process::Command;
use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rustc-rerun-if-changed=vendor");
    println!("cargo:rerun-if-changed=build.rs");

    // must be ./ to be recognised as relative path
    // from project root
    println!("cargo:rustc-link-search=all=./solvers/chuffed/vendor/build/");
    println!("cargo:rustc-link-lib=static=chuffed");
    println!("cargo:rustc-link-lib=static=chuffed_fzn");

    // also need to (dynamically) link to c++ stdlib
    // https://flames-of-code.netlify.app/blog/rust-and-cmake-cplusplus/
    let target = env::var("TARGET").unwrap();
    if target.contains("apple") {
        println!("cargo:rustc-link-lib=dylib=c++");
    } else if target.contains("linux") {
        println!("cargo:rustc-link-lib=dylib=stdc++");
    } else {
        unimplemented!();
    }
    build();
    bind();
    
}
fn build() {
    let output = Command::new("bash")
        .args(["build.sh"])
        .output()
        .expect("Failed to run build.sh");

    /*
    do cargo build -vv to see
    */
    println!("stdout");
    println!("{}", String::from_utf8_lossy(&output.stdout));
    println!("stderr");
    println!("{}", String::from_utf8_lossy(&output.stderr));

    if !output.status.success() {
        panic!("build.sh has non zero exit status")
    }
}

fn bind() {
    // The bindgen::Builder is the main entry point
    // to bindgen, and lets you build up options for
    // the resulting bindings.
    let bindings = bindgen::Builder::default()
        // The input header we would like to generate
        // bindings for.
        .header("wrapper.h")
        // Tell cargo to invalidate the built crate whenever any of the
        // included header files changed.
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        // Must manually give allow list to stop bindgen accidentally binding something complicated
        // in C++ stdlib that will make it crash.
        .allowlist_function("createVars")
        .allowlist_function("createVar")
        .clang_arg("-Ivendor/build") // generated from configure.py
        .clang_arg("-Ivendor")
        .clang_arg(r"--std=gnu++11")
        .clang_arg(r"-xc++")
        // Finish the builder and generate the bindings.
        .generate()
        // Unwrap the Result and panic on failure.
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("chuffed_bindings.rs"))
        .expect("Couldn't write bindings to file!");
}
