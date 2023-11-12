// adapted from
// e https://github.com/gokberkkocak/rust_glucose/blob/master/build.rs
// - https://rust-lang.github.io/rust-bindgen/non-system-libraries.html
// - https://doc.rust-lang.org/cargo/reference/build-scripts.html#rerun-if-changed
//

use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rustc-rerun-if-changed=vendor");
    println!("cargo:rerun-if-changed=build.rs");

    // must be ./ to be recognised as relative path
    // from project root
    println!("cargo:rustc-link-search=all=./solvers/minion/vendor/build/");
    println!("cargo:rustc-link-lib=static=minion");

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
        .header("vendor/minion/libwrapper.h")
        // Tell cargo to invalidate the built crate whenever any of the
        // included header files changed.
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        // Make all templates opaque as reccomended by bindgen
        .opaque_type("std::.*")
        // Manually allow C++ functions to stop bindgen getting confused.
        .allowlist_function("resetMinion")
        .allowlist_function("runMinion")
        .allowlist_function("constantAsVar")
        .allowlist_function("newSearchOptions")
        .allowlist_function("newSearchMethod")
        .allowlist_function("newInstance")
        .allowlist_function("newConstraintBlob")
        .allowlist_function("newSearchOrder")
        .allowlist_function("getVarByName")
        .allowlist_function("searchOptions_free")
        .allowlist_function("searchMethod_free")
        .allowlist_function("instance_free")
        .allowlist_function("constraint_free")
        .allowlist_function("searchOrder_free")
        .allowlist_function("newVar_ffi")
        .allowlist_function("instance_addSearchOrder")
        .allowlist_function("instance_addConstraint")
        .allowlist_function("printMatrix_addVar")
        .allowlist_function("printMatrix_getValue")
        .allowlist_function("constraint_addVarList")
        .allowlist_function("constraint_addConstantList")
        .allowlist_function("vec_var_new")
        .allowlist_function("vec_var_push_back")
        .allowlist_function("vec_var_free")
        .allowlist_function("vec_int_new")
        .allowlist_function("vec_int_push_back")
        .allowlist_function("vec_int_free")
        .clang_arg("-Ivendor/build/src/") // generated from configure.py
        .clang_arg("-Ivendor/minion/")
        .clang_arg("-DLIBMINION")
        .clang_arg(r"--std=gnu++11")
        .clang_arg(r"-xc++")
        // Finish the builder and generate the bindings.
        .generate()
        // Unwrap the Result and panic on failure.
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings to file!");
}
