use conjure_oxide::parse::parse_json;

fn integration_test(path: &str) {
    let mut cmd = std::process::Command::new("conjure");
    cmd.arg("pretty --output-format=astjson").arg(format!(
        "{path}/input.essence > {path}/input.astjson.json",
        path = path
    ));
    cmd.output().unwrap();
}

include!(concat!(env!("OUT_DIR"), "/gen_tests.rs"));
