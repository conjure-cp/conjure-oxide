use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use conjure_cp_core::ast::OptimiseDirection;
use conjure_cp_core::context::Context;
use conjure_cp_essence_parser::parse_essence_file_native;

fn workspace_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

#[test]
fn parse_savilerow_objective_models() {
    let context = Arc::new(RwLock::new(Context::default()));
    let cases = [
        (
            workspace_path("test-suite/tests/integration/savilerow/diet/input.essence"),
            OptimiseDirection::Minimising,
        ),
        (
            workspace_path("test-suite/tests/integration/savilerow/knapsack/input.essence"),
            OptimiseDirection::Maximising,
        ),
        (
            workspace_path("test-suite/tests/integration/savilerow/graphColouring/input.essence"),
            OptimiseDirection::Minimising,
        ),
    ];

    for (path, expected_direction) in cases {
        let path = path.to_str().unwrap();
        let model = parse_essence_file_native(path, Arc::clone(&context))
            .unwrap_or_else(|err| panic!("failed to parse {path}: {err:?}"));
        let objective = model
            .objective
            .as_ref()
            .unwrap_or_else(|| panic!("missing objective in {path}"));
        assert_eq!(objective.direction, expected_direction, "{path}");
    }
}
