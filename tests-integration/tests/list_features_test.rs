use std::env;
use std::error::Error;
use std::fs::read_dir;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::RwLock;
use std::thread;

use conjure_cp::context::Context;
use conjure_cp::parse::tree_sitter::parse_essence_file;

// Tests if every Essence file can be successfully parsed by Conjure-Oxide to an AST model
// Erroneous output is printed to screen to guide implementation of Essence features
fn list_features_test() -> Result<(), Box<dyn Error>> {
    let mut total = 0;
    let failing_arc = Arc::new(Mutex::new(0));
    let list_dir = "tests/essence_list";
    let mut handles = vec![];

    // Removes panic hook temporarily to suppress the bug!() output when running unsupported Essence files
    let panic_hook_normal = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_info| {}));

    // Tests all Essence files directly in the directory
    for file in read_dir(list_dir)? {
        let file = file?;
        let path = file.path();

        if path.is_file() && (path.extension().is_some_and(|ext| ext == "essence")) {
            total += 1;

            // Creates a new thread for each file
            let failing_clone = Arc::clone(&failing_arc);
            let handle = thread::spawn(move || {

                let file_name = file.file_name().display().to_string();
                let context: Arc<RwLock<Context<'static>>> = Default::default();

                // Catches any panics, to print out the result
                let res = catch_unwind(AssertUnwindSafe(move || {
                    // Tries to parse each file into a model
                    parse_essence_file(&path.display().to_string(), context.clone())
                }));

                let mut failing = failing_clone.lock().unwrap();
                match res {
                    Ok(parsed) => {
                        // Only intersted in erroneous results
                        if parsed.is_err() {
                            *failing += 1;
                            println!("Failed {}: {}", file_name, parsed.unwrap_err().to_string());
                        }
                    }
                    Err(_) => {
                        *failing += 1;
                        // TODO : Extract specific error output
                        println!("Failed {}: Unknown Object", file_name);
                    }
                }
            });
            handles.push(handle);
        }
    }

    for handle in handles {
        let _ = handle.join();
    }

    // Resets the panic hook now every file in the directory has been tested
    std::panic::set_hook(panic_hook_normal);

    let failing = *failing_arc.lock().unwrap();
    // We want every file to parse correctly into an AST model
    assert_eq!(total - failing, total);

    Ok(())
}

include!(concat!(env!("OUT_DIR"), "/gen_tests_list_features.rs"));
