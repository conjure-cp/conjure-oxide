use std::collections::{BTreeMap, BTreeSet};

use conjure_cp::ast::sat_decision::AmoEncoding;
use conjure_cp::ast::{AbstractLiteral, Literal, Name};
use conjure_cp::pipeline::{Config, Pipeline};
use conjure_cp::settings::{
    self, Channelling, Heuristic, Parser, QuantifiedExpander, SolverFamily,
};
use conjure_cp::solutions::SolutionOptions;
use conjure_cp_rules as _;

const PARAMETERISED: &str =
    "language Essence 1.3\ngiven n : int\nfind x : int(1..3)\nsuch that x <= n";

fn all_solutions() -> SolutionOptions {
    SolutionOptions {
        number_of_solutions: 0,
        ..Default::default()
    }
}

fn solve_parameterised(solver: SolverFamily) {
    let pipeline = Pipeline::new(Config {
        solver,
        ..Default::default()
    })
    .unwrap();
    let model = pipeline
        .compile(PARAMETERISED, Some("language Essence 1.3\nletting n be 2"))
        .unwrap();
    let solutions = pipeline.solve(model, &all_solutions()).unwrap();
    let values: BTreeSet<_> = solutions
        .iter()
        .map(|solution| match solution.get(&Name::user("x")).unwrap() {
            Literal::Int(value) => *value,
            value => panic!("expected integer x, got {value}"),
        })
        .collect();
    assert_eq!(values, BTreeSet::from([1, 2]));
    assert_eq!(solutions.len(), 2);
}

#[test]
fn minion_compiles_and_solves_source_and_parameters_in_memory() {
    solve_parameterised(SolverFamily::Minion);
}

fn check_constant_models(solver: SolverFamily) {
    let pipeline = Pipeline::new(Config {
        solver,
        ..Default::default()
    })
    .unwrap();
    let satisfiable = pipeline.compile("such that true", None).unwrap();
    assert_eq!(
        pipeline.solve(satisfiable, &all_solutions()).unwrap(),
        vec![BTreeMap::new()]
    );
    let unsatisfiable = pipeline.compile("such that false", None).unwrap();
    assert!(
        pipeline
            .solve(unsatisfiable, &all_solutions())
            .unwrap()
            .is_empty()
    );
}

#[test]
fn minion_distinguishes_empty_assignments_from_unsatisfiable_constant_models() {
    check_constant_models(SolverFamily::Minion);
}

#[cfg(any(feature = "sat-batsat", feature = "sat-cadical"))]
#[test]
fn sat_distinguishes_empty_assignments_from_unsatisfiable_constant_models() {
    check_constant_models(SolverFamily::Sat);
}

#[test]
fn solutions_reconstruct_structured_values_and_remove_auxiliaries() {
    let pipeline = Pipeline::new(Config::default()).unwrap();
    let model = pipeline
        .compile("find s : set (size 1) of int(1..2)", None)
        .unwrap();
    let solutions = pipeline.solve(model, &all_solutions()).unwrap();
    assert_eq!(solutions.len(), 2);
    for solution in &solutions {
        assert_eq!(solution.len(), 1);
        assert!(matches!(
            solution.get(&Name::user("s")),
            Some(Literal::AbstractLiteral(AbstractLiteral::Set(_)))
        ));
    }
    for value in [1, 2] {
        assert!(
            solutions
                .iter()
                .any(|solution| solution.get(&Name::user("s"))
                    == Some(&Literal::AbstractLiteral(AbstractLiteral::Set(vec![
                        Literal::Int(value)
                    ]))))
        );
    }
}

#[cfg(any(feature = "sat-batsat", feature = "sat-cadical"))]
#[test]
fn sat_compiles_and_solves_source_and_parameters_in_memory() {
    solve_parameterised(SolverFamily::Sat);
}

#[test]
fn invalid_rules_and_input_return_errors_and_allow_the_next_run() {
    let error = Pipeline::new(Config {
        extra_rule_sets: vec!["nonexistent-fuzzer-rules".into()],
        ..Default::default()
    })
    .err()
    .unwrap();
    assert!(error.to_string().contains("could not resolve rule sets"));

    let pipeline = Pipeline::new(Config::default()).unwrap();
    assert!(pipeline.parse("find x : int(").is_err());
    assert!(pipeline.compile(PARAMETERISED, None).is_err());
    let model = pipeline.compile("find x : bool", None).unwrap();
    assert_eq!(pipeline.solve(model, &all_solutions()).unwrap().len(), 2);
}

#[test]
fn native_file_parsing_and_io_errors() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("model.essence");
    std::fs::write(&path, "find x : bool").unwrap();
    let pipeline = Pipeline::new(Config::default()).unwrap();
    let model = pipeline.parse_file(&path).unwrap();
    let model = pipeline.rewrite(model).unwrap();
    assert_eq!(pipeline.solve(model, &all_solutions()).unwrap().len(), 2);
    assert!(
        pipeline
            .parse_file(directory.path().join("missing.essence"))
            .is_err()
    );
}

#[cfg(unix)]
#[test]
fn non_utf8_paths_return_io_errors_without_panicking() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let directory = tempfile::tempdir().unwrap();
    let path = directory
        .path()
        .join(OsString::from_vec(b"missing-\xff.essence".to_vec()));
    let pipeline = Pipeline::new(Config::default()).unwrap();
    assert!(pipeline.parse_file(path).is_err());
}

#[test]
fn models_from_another_pipeline_are_rejected() {
    let first = Pipeline::new(Config::default()).unwrap();
    let second = Pipeline::new(Config::default()).unwrap();
    let model = first.parse("find x : bool").unwrap();
    let error = second.rewrite(model).err().unwrap();
    assert!(error.to_string().contains("different pipeline context"));

    let model = first.compile("find x : bool", None).unwrap();
    let error = second.solve(model, &all_solutions()).err().unwrap();
    assert!(error.to_string().contains("different pipeline context"));
}

#[test]
fn repeated_seeded_runs_restore_the_callers_settings_and_random_stream() {
    settings::with_saved_settings(|| {
        settings::set_current_parser(Parser::ViaConjure);
        settings::set_current_solver_family(SolverFamily::Z3);
        settings::set_comprehension_expander(QuantifiedExpander::ViaSolverAc);
        settings::set_heuristic(Heuristic::Interactive);
        settings::set_channelling(Channelling::Uniform);
        settings::set_amo_encoding(Some(AmoEncoding::Pairwise));
        settings::set_heuristic_responses(vec![2]);
        settings::set_heuristic_seed(99);
        let expected_next_random = settings::next_heuristic_random_index(997);
        settings::set_heuristic_seed(99);

        let random = Pipeline::new(Config {
            heuristic: Heuristic::Random,
            seed: 42,
            ..Default::default()
        })
        .unwrap();
        let compact = Pipeline::new(Config::default()).unwrap();
        let source = "find x, y : int(0..3)\nsuch that x + y = 3";
        let first = random.compile(source, None).unwrap();
        let first_text = first.to_string();
        let first_solutions = random.solve(first, &all_solutions()).unwrap();
        let other = compact.compile(source, None).unwrap();
        assert_eq!(compact.solve(other, &all_solutions()).unwrap().len(), 4);
        let repeated = random.compile(source, None).unwrap();
        assert_eq!(repeated.to_string(), first_text);
        let repeated_solutions = random.solve(repeated, &all_solutions()).unwrap();
        assert_eq!(repeated_solutions, first_solutions);

        assert_eq!(settings::current_parser(), Parser::ViaConjure);
        assert_eq!(settings::current_solver_family(), SolverFamily::Z3);
        assert_eq!(
            settings::comprehension_expander(),
            QuantifiedExpander::ViaSolverAc
        );
        assert_eq!(settings::heuristic(), Heuristic::Interactive);
        assert_eq!(settings::channelling(), Channelling::Uniform);
        assert_eq!(settings::amo_encoding(), Some(AmoEncoding::Pairwise));
        assert_eq!(
            settings::next_heuristic_random_index(997),
            expected_next_random
        );
        assert_eq!(settings::next_heuristic_interactive_index(&["a", "b"]), 1);
    });
}

#[cfg(any(feature = "sat-batsat", feature = "sat-cadical"))]
#[test]
fn interleaved_solver_families_use_their_own_configuration() {
    let minion = Pipeline::new(Config::default()).unwrap();
    let sat = Pipeline::new(Config {
        solver: SolverFamily::Sat,
        ..Default::default()
    })
    .unwrap();
    let source = "find x : bool";
    let minion_model = minion.compile(source, None).unwrap();
    let sat_model = sat.compile(source, None).unwrap();
    let minion_solutions = minion.solve(minion_model, &all_solutions()).unwrap();
    let sat_solutions = sat.solve(sat_model, &all_solutions()).unwrap();
    assert_eq!(minion_solutions.len(), 2);
    assert_eq!(sat_solutions.len(), 2);
    for value in [false, true] {
        let expected = std::collections::BTreeMap::from([(Name::user("x"), Literal::Bool(value))]);
        assert!(minion_solutions.contains(&expected));
        assert!(sat_solutions.contains(&expected));
    }
    assert!(
        minion_solutions
            .iter()
            .all(|solution| sat_solutions.contains(solution))
    );
}

#[test]
fn optimisation_retains_the_optimum() {
    let pipeline = Pipeline::new(Config::default()).unwrap();
    let model = pipeline
        .compile("find x : int(0..5)\nmaximising x", None)
        .unwrap();
    let solutions = pipeline.solve(model, &SolutionOptions::default()).unwrap();
    assert_eq!(solutions.len(), 1);
    assert_eq!(solutions[0].get(&Name::user("x")), Some(&Literal::Int(5)));
}

#[test]
fn dominance_rewrites_use_the_pipeline_settings_during_solving() {
    settings::with_saved_settings(|| {
        settings::set_current_solver_family(SolverFamily::Z3);
        settings::set_current_rewriter(settings::Rewriter::Rewrite(
            settings::RewriteConfig::baseline(),
        ));
        let pipeline = Pipeline::new(Config::default()).unwrap();
        let model = pipeline
            .compile(
                "find x : bool\ndominance relation (!x) /\\ fromSolution(x)",
                None,
            )
            .unwrap();
        let solutions = pipeline.solve(model, &all_solutions()).unwrap();
        assert_eq!(solutions.len(), 1);
        assert_eq!(
            solutions[0].get(&Name::user("x")),
            Some(&Literal::Bool(false))
        );
        assert_eq!(settings::current_solver_family(), SolverFamily::Z3);
        assert_eq!(
            settings::current_rewriter(),
            settings::Rewriter::Rewrite(settings::RewriteConfig::baseline())
        );
    });
}

#[test]
fn solver_backed_comprehensions_work_on_a_fresh_thread() {
    std::thread::spawn(|| {
        assert_eq!(settings::try_current_solver_family(), None);
        let pipeline = Pipeline::new(Config {
            comprehension_expander: QuantifiedExpander::ViaSolver,
            ..Default::default()
        })
        .unwrap();
        let model = pipeline
            .compile(
                "find x : int(1..3)\nsuch that x = sum([i | i : int(1..2)])",
                None,
            )
            .unwrap();
        let solutions = pipeline.solve(model, &all_solutions()).unwrap();
        assert_eq!(solutions.len(), 1);
        assert_eq!(solutions[0].get(&Name::user("x")), Some(&Literal::Int(3)));
        assert_eq!(settings::try_current_solver_family(), None);
    })
    .join()
    .unwrap();
}

#[cfg(not(feature = "z3"))]
#[test]
fn selecting_z3_without_the_feature_returns_an_error() {
    let error = Pipeline::new(Config {
        solver: SolverFamily::Z3,
        ..Default::default()
    })
    .err()
    .unwrap();
    assert!(
        error
            .to_string()
            .contains("Z3 solver support was not compiled in")
    );
}

#[cfg(not(any(
    feature = "sat-batsat",
    all(feature = "sat-cadical", not(target_family = "wasm"))
)))]
#[test]
fn selecting_sat_without_a_backend_returns_an_error() {
    let error = Pipeline::new(Config {
        solver: SolverFamily::Sat,
        ..Default::default()
    })
    .err()
    .unwrap();
    assert!(
        error
            .to_string()
            .contains("SAT solver support was not compiled in")
    );
}
