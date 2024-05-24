use rustc_version::{version, version_meta, Channel, Version};

fn main() {
    // Set cfg flags depending on release channel
    match version_meta().unwrap().channel {
        Channel::Nightly => {
            // required for the linkme feature use_linker enabled by --features nightly
            println!("cargo:rustc-flags=-Zused_with_args");
        }
        _ => {}
    }
}
