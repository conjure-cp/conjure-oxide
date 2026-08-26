use std::path::{Component, Path};

pub(crate) fn is_roundtrip_model_input(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == "essence")
        && path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .is_some_and(|stem| !stem.contains(".generated") && !stem.contains(".expected"))
}

/// Returns whether a test-suite file can affect the generated Rust test list or attributes.
pub(crate) fn is_compile_time_test_input(path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix("tests") else {
        return false;
    };
    let mut components = relative.components();
    let Some(Component::Normal(suite_or_file)) = components.next() else {
        return false;
    };
    let file_name = path.file_name().and_then(|name| name.to_str());

    if components.next().is_none() {
        return file_name.is_some_and(|name| name.ends_with("_test_template"));
    }

    match suite_or_file.to_str() {
        Some("integration") => {
            matches!(file_name, Some("config.toml" | "stats.toml"))
                || path.extension().is_some_and(|ext| ext == "essence")
        }
        Some("custom") => matches!(file_name, Some("config.toml" | "run.sh")),
        Some("roundtrip") => file_name == Some("config.toml") || is_roundtrip_model_input(path),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{is_compile_time_test_input, is_roundtrip_model_input};
    use std::path::Path;

    #[test]
    fn watches_files_used_to_generate_tests() {
        for path in [
            "tests/integration/example/input.essence",
            "tests/integration/example/config.toml",
            "tests/integration/example/stats.toml",
            "tests/custom/example/run.sh",
            "tests/custom/example/config.toml",
            "tests/roundtrip/example/input.essence",
            "tests/roundtrip/example/config.toml",
            "tests/integration_test_template",
            "tests/custom_test_template",
            "tests/roundtrip_test_template",
        ] {
            assert!(is_compile_time_test_input(Path::new(path)), "{path}");
        }
    }

    #[test]
    fn ignores_runtime_outputs() {
        for path in [
            "tests/integration/example/input.param",
            "tests/integration/example/model.expected.solutions",
            "tests/integration/example/model-generated-rule-trace.txt",
            "tests/custom/example/expected-output",
            "tests/roundtrip/example/tree-sitter.generated.essence",
            "tests/roundtrip/example/tree-sitter.expected.essence",
            "tests/roundtrip/example/tree-sitter.generated.serialised.json",
            "tests/roundtrip/example/tree-sitter.expected.serialised.json",
        ] {
            assert!(!is_compile_time_test_input(Path::new(path)), "{path}");
        }
    }

    #[test]
    fn selects_only_roundtrip_source_models() {
        assert!(is_roundtrip_model_input(Path::new(
            "tests/roundtrip/example/input.essence"
        )));
        assert!(!is_roundtrip_model_input(Path::new(
            "tests/roundtrip/example/config.toml"
        )));
        assert!(!is_roundtrip_model_input(Path::new(
            "tests/roundtrip/example/tree-sitter.generated.essence"
        )));
        assert!(!is_roundtrip_model_input(Path::new(
            "tests/roundtrip/example/tree-sitter.expected.essence"
        )));
    }
}
