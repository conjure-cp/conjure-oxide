use std::path::PathBuf;

fn main() {
    // paths to search for Z3 dylib 
    let candidates: Vec<PathBuf> = [
        // macOS
        "/opt/homebrew/lib",
        "/usr/local/lib",
        // linux
        "/usr/lib",
        "/usr/lib64",
        "/lib",
        "/lib64",
    ].into_iter().map(PathBuf::from).collect();

    let mut valid_paths = Vec::new();

    // check possible lib names
    for dir in candidates {
        if dir.join("libz3.dylib").exists()
            || dir.join("libz3.so").exists()
            || dir.join("libz3.a").exists() {
                valid_paths.push(dir);
        }
    }
    // if all else failed, maybe it is local?
    valid_paths.push(PathBuf::from("@executable_path/../lib"));

    // do some horrible linker things
    for path in &valid_paths {
        if let Some(p) = path.to_str() {
            println!("cargo:rustc-link-search=native={}", p);
        }
    }

}

