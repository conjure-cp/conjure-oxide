// cpu_time_tests.rs
// @author Pedro Gronda Garrigues

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};

// struct for CPU time data
#[derive(Serialize, Deserialize, Debug)]
struct TestCPUTime {
    name: String,
    cpu_time: Duration,
}

// function to run test & measure CPU time
fn run_test_cpu_time<F: FnOnce()>(name: &str, test_fn: F) -> TestCPUTime {
    let start_time = Instant::now();
    test_fn();
    let end_time = Instant::now();
    let cpu_time = end_time - start_time;
    TestCPUTime {
        name: name.to_string(),
        cpu_time,
    }
}

// function to write CPU tiem data to file
fn write_cpu_time_data(data: HashMap<String, Vec<TestCPUTime>) -> Result<(), Box<dyn std::error::Error>> {
    let file = std::fs::File::create("cpu_time_data.json")?;
    serde_json::to_writer(file, &data)?;
    Ok(())
}

fn write_tests() {
    // assume import of tests (Work in Progress...)
    //

    let mut cpu_time_data: HashMap<String, Vec<TestCPUTime>> = HashMap::new();
    let tests = vec![
        // format: ("name", tests::test_<...>)
    ];

    for (name, test_fn) in tests {
        let test_cpu_time = run_test_cpu_time(name, test_fn);
        let category = "Testing".to_string(); // assume test category (to be changed)
        cpu_time_data.entry(name).or_insert_with(Vec::new).push(test_cpu_time);
    }

    if let Err(err) = write_cpu_time_data(cpu_time_data) {
        eprintln!("Error writing CPU time data: {}", err);
    }
}
