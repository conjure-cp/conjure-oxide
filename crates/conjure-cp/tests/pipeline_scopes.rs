use conjure_cp::pipeline::{Config, Pipeline, TraceConfig};
use conjure_cp::settings::{self, Channelling, Heuristic, Parser};
use conjure_cp::solutions::SolutionOptions;
use conjure_cp_rules as _;

fn random_sequence(seed: u64) -> [usize; 3] {
    settings::with_saved_settings(|| {
        settings::set_heuristic_seed(seed);
        std::array::from_fn(|_| settings::next_heuristic_random_index(997))
    })
}

#[test]
fn interactive_answers_continue_from_rewriting_into_solver_loading() {
    let pipeline = Pipeline::new(Config {
        heuristic: Heuristic::Interactive,
        responses: vec![1, 2, 3],
        ..Default::default()
    })
    .unwrap();
    let solver_input = tempfile::NamedTempFile::new().unwrap();

    pipeline.with_settings(|| {
        // The first answer selects a set representation; the second selects all BOUND integers.
        let model = pipeline
            .compile(
                "find s : set (size 1) of int(1..3)\nfind x : int(1..3)",
                None,
            )
            .unwrap();
        let solutions = pipeline
            .solve(
                model,
                &SolutionOptions {
                    solver_input_file: Some(solver_input.path().to_path_buf()),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(solutions.len(), 1);
        assert_eq!(
            settings::next_heuristic_interactive_index(&["a", "b", "c"]),
            2
        );
    });

    let input = std::fs::read_to_string(solver_input.path()).unwrap();
    assert!(
        input.lines().any(|line| line == "BOUND x {1..3}"),
        "the second answer should select BOUND for x:\n{input}"
    );
}

#[test]
fn nested_pipelines_restore_each_stream_and_the_caller_after_panic() {
    let first = Pipeline::new(Config {
        heuristic: Heuristic::Random,
        seed: 11,
        responses: vec![1, 2, 3],
        channelling: Channelling::Uniform,
        ..Default::default()
    })
    .unwrap();
    let second = Pipeline::new(Config {
        parser: Parser::ViaConjure,
        heuristic: Heuristic::Compact,
        seed: 29,
        responses: vec![2, 1],
        ..Default::default()
    })
    .unwrap();
    let first_random = random_sequence(11);
    let second_random = random_sequence(29);
    let caller_random = random_sequence(101);
    let options = ["a", "b", "c"];

    settings::with_saved_settings(|| {
        settings::set_current_parser(Parser::TreeSitter);
        settings::set_heuristic(Heuristic::First);
        settings::set_channelling(Channelling::Yes);
        settings::set_heuristic_seed(101);
        settings::set_heuristic_responses(vec![3]);

        first.with_settings(|| {
            assert_eq!(settings::heuristic(), Heuristic::Random);
            assert_eq!(settings::channelling(), Channelling::Uniform);
            assert_eq!(settings::next_heuristic_random_index(997), first_random[0]);
            assert_eq!(settings::next_heuristic_interactive_index(&options), 0);

            second.with_settings(|| {
                assert_eq!(settings::current_parser(), Parser::ViaConjure);
                assert_eq!(settings::heuristic(), Heuristic::Compact);
                assert_eq!(settings::next_heuristic_random_index(997), second_random[0]);
                assert_eq!(settings::next_heuristic_interactive_index(&options), 1);

                let interrupted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    first.with_settings(|| {
                        assert_eq!(settings::current_parser(), Parser::TreeSitter);
                        assert_eq!(settings::heuristic(), Heuristic::Random);
                        assert_eq!(settings::channelling(), Channelling::Uniform);
                        assert_eq!(settings::next_heuristic_random_index(997), first_random[0]);
                        assert_eq!(settings::next_heuristic_interactive_index(&options), 0);
                        panic!("interrupted nested pipeline");
                    });
                }));
                assert!(interrupted.is_err());
                assert_eq!(settings::current_parser(), Parser::ViaConjure);
                assert_eq!(settings::heuristic(), Heuristic::Compact);
                assert_eq!(settings::channelling(), Channelling::No);
                assert_eq!(settings::next_heuristic_random_index(997), second_random[1]);
                assert_eq!(settings::next_heuristic_interactive_index(&options), 0);
            });

            assert_eq!(settings::current_parser(), Parser::TreeSitter);
            assert_eq!(settings::heuristic(), Heuristic::Random);
            assert_eq!(settings::channelling(), Channelling::Uniform);
            assert_eq!(settings::next_heuristic_random_index(997), first_random[1]);
            assert_eq!(settings::next_heuristic_interactive_index(&options), 1);
            first.with_settings(|| {
                assert_eq!(settings::next_heuristic_random_index(997), first_random[2]);
                assert_eq!(settings::next_heuristic_interactive_index(&options), 2);
            });
        });

        assert_eq!(settings::current_parser(), Parser::TreeSitter);
        assert_eq!(settings::heuristic(), Heuristic::First);
        assert_eq!(settings::channelling(), Channelling::Yes);
        assert_eq!(settings::next_heuristic_random_index(997), caller_random[0]);
        assert_eq!(settings::next_heuristic_interactive_index(&options), 2);
    });
}

#[test]
fn solver_search_restores_rewrite_tracing_for_successive_operations() {
    let pipeline = Pipeline::new(Config {
        trace: TraceConfig {
            rules: true,
            ..Default::default()
        },
        ..Default::default()
    })
    .unwrap();

    pipeline.with_settings(|| {
        let first = pipeline.compile("find x : bool", None).unwrap();
        assert!(settings::rule_trace_enabled());
        assert_eq!(
            pipeline
                .solve(first, &SolutionOptions::default())
                .unwrap()
                .len(),
            1
        );
        assert!(settings::rule_trace_enabled());

        let second = pipeline
            .compile("find x : int(1..3)\nminimising x", None)
            .unwrap();
        assert!(settings::rule_trace_enabled());
        let error = pipeline
            .solve(
                second,
                &SolutionOptions {
                    number_of_solutions: 0,
                    ..Default::default()
                },
            )
            .unwrap_err();
        assert!(error.to_string().contains("optimisation problems"));
        assert!(settings::rule_trace_enabled());
    });
}
