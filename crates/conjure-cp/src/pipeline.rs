//! Run the modelling pipeline without command-line arguments or global initialisation.

use std::cell::Cell;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use anyhow::{Context as _, Result, ensure};

use crate::Model;
use crate::ast::sat_decision::{
    AllDifferentEncoding, AmoEncoding, CardinalityEncoding, ElementEncoding, PbEncoding,
    TableEncoding,
};
use crate::ast::{Literal, Name};
use crate::context::Context;
use crate::defaults::DEFAULT_RULE_SETS;
use crate::instantiate::{instantiate_model, validate_instantiation_conditions};
use crate::parse::tree_sitter::errors::ParseErrorCollection;
use crate::rule_engine::{get_rules, resolve_rule_sets, rewrite_model};
use crate::settings::{
    self, Channelling, Heuristic, Parser, QuantifiedExpander, Rewriter, SolverFamily,
};
use crate::solutions::{SolutionCollectionTimings, SolutionOptions, get_solutions_with_timings};
use crate::solver::Solver;
use crate::solver::adaptors::*;

thread_local! {
    static ACTIVE_PIPELINE: Cell<*const Pipeline> = const { Cell::new(std::ptr::null()) };
}

struct ActivePipelineGuard(*const Pipeline);

impl Drop for ActivePipelineGuard {
    fn drop(&mut self) {
        ACTIVE_PIPELINE.with(|active| active.set(self.0));
    }
}

/// Modelling and solver options shared by library callers and the CLI.
#[derive(Clone, Debug)]
pub struct Config {
    /// Input parser; the native parser needs no external executable.
    pub parser: Parser,
    /// Rewrite engine configuration.
    pub rewriter: Rewriter,
    /// Strategy for expanding comprehensions.
    pub comprehension_expander: QuantifiedExpander,
    /// Target solver family.
    pub solver: SolverFamily,
    /// Additional registered rule sets, alongside the defaults and solver rules.
    pub extra_rule_sets: Vec<String>,
    /// Strategy for modelling decisions.
    pub heuristic: Heuristic,
    /// Random modelling seed, reset for each operation.
    pub seed: u64,
    /// Recorded 1-based interactive answers, reset for each operation.
    pub responses: Vec<usize>,
    /// Representation channelling policy.
    pub channelling: Channelling,
    /// Optional SAT encoder overrides.
    pub amo_encoding: Option<AmoEncoding>,
    /// Optional SAT cardinality encoder override.
    pub cardinality_encoding: Option<CardinalityEncoding>,
    /// Optional SAT pseudo-Boolean encoder override.
    pub pb_encoding: Option<PbEncoding>,
    /// Optional SAT element-constraint encoder override.
    pub element_encoding: Option<ElementEncoding>,
    /// Optional SAT table-constraint encoder override.
    pub table_encoding: Option<TableEncoding>,
    /// Optional SAT allDifferent encoder override.
    pub alldifferent_encoding: Option<AllDifferentEncoding>,
    /// Optional Minion discrete-domain span threshold.
    pub minion_discrete_threshold: Option<usize>,
    /// Optional Minion search orders.
    pub minion_varorder: Option<MinionVariableOrder>,
    /// Optional Minion value ordering.
    pub minion_valorder: Option<MinionValueOrder>,
    /// Compiled SAT implementation to use.
    #[cfg(any(
        feature = "sat-batsat",
        all(feature = "sat-cadical", not(target_family = "wasm"))
    ))]
    pub sat_backend: SatBackend,
    /// Backend solver seed.
    pub solver_seed: u32,
    /// Cumulative wall-clock solver timeout.
    pub solver_timeout: Option<Duration>,
    /// Trace events to emit to the caller's tracing subscriber.
    pub trace: TraceConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            parser: Parser::default(),
            rewriter: Rewriter::default(),
            comprehension_expander: QuantifiedExpander::Auto,
            solver: SolverFamily::Minion,
            extra_rule_sets: Vec::new(),
            heuristic: Heuristic::default(),
            seed: settings::DEFAULT_HEURISTIC_SEED,
            responses: Vec::new(),
            channelling: Channelling::default(),
            amo_encoding: None,
            cardinality_encoding: None,
            pb_encoding: None,
            element_encoding: None,
            table_encoding: None,
            alldifferent_encoding: None,
            minion_discrete_threshold: None,
            minion_varorder: None,
            minion_valorder: None,
            #[cfg(any(
                feature = "sat-batsat",
                all(feature = "sat-cadical", not(target_family = "wasm"))
            ))]
            sat_backend: SatBackend::default(),
            solver_seed: 0,
            solver_timeout: None,
            trace: TraceConfig::default(),
        }
    }
}

/// Rule trace events enabled for a pipeline; subscribers and output belong to the caller.
#[derive(Clone, Copy, Debug, Default)]
pub struct TraceConfig {
    /// Emit the default rule trace.
    pub rules: bool,
    /// Emit rule-attempt events.
    pub attempts: bool,
    /// Emit aggregate rule-application events.
    pub aggregates: bool,
}

impl Config {
    fn apply(&self) {
        settings::set_current_parser(self.parser);
        settings::set_current_rewriter(self.rewriter);
        settings::set_comprehension_expander(self.comprehension_expander);
        settings::set_current_solver_family(self.solver);
        settings::set_minion_discrete_threshold(self.minion_discrete_threshold);
        settings::set_heuristic(self.heuristic);
        settings::set_heuristic_seed(self.seed);
        settings::set_heuristic_responses(self.responses.clone());
        settings::begin_heuristic_all_choices(Vec::new());
        settings::set_channelling(self.channelling);
        settings::set_amo_encoding(self.amo_encoding);
        settings::set_cardinality_encoding(self.cardinality_encoding);
        settings::set_pb_encoding(self.pb_encoding);
        settings::set_element_encoding(self.element_encoding);
        settings::set_table_encoding(self.table_encoding);
        settings::set_alldifferent_encoding(self.alldifferent_encoding);
        settings::set_default_rule_trace_enabled(self.trace.rules);
        settings::set_rule_attempt_trace_enabled(self.trace.attempts);
        settings::set_rule_trace_aggregates_enabled(self.trace.aggregates);
        settings::set_rule_trace_enabled(
            self.trace.rules || self.trace.attempts || self.trace.aggregates,
        );
    }
}

/// A configured pipeline with a shared model context and accumulated statistics.
///
/// Link a rule provider (for example `use conjure_cp_rules as _;`) before constructing one.
/// Each operation installs its settings on the current thread and restores the caller's settings,
/// including on panic. A pipeline can be reused for successive fuzzer inputs; construct a new one
/// when each run needs separate statistics.
pub struct Pipeline {
    config: Config,
    context: Arc<RwLock<Context<'static>>>,
}

impl Pipeline {
    /// Resolve registered rules and create a context, returning configuration errors to the caller.
    pub fn new(config: Config) -> Result<Self> {
        #[cfg(not(any(
            feature = "sat-batsat",
            all(feature = "sat-cadical", not(target_family = "wasm"))
        )))]
        if config.solver == SolverFamily::Sat {
            anyhow::bail!(
                "SAT solver support was not compiled in (enable `sat-batsat` or `sat-cadical`)."
            );
        }
        #[cfg(not(feature = "z3"))]
        if config.solver == SolverFamily::Z3 {
            anyhow::bail!("Z3 solver support was not compiled in (enable the `z3` feature).");
        }
        let names: Vec<&str> = DEFAULT_RULE_SETS
            .iter()
            .copied()
            .chain(config.extra_rule_sets.iter().map(String::as_str))
            .collect();
        let rule_sets = resolve_rule_sets(config.solver, &names)
            .context("could not resolve rule sets; link a rule provider such as conjure-cp-rules and check extra rule set names")?;
        let rules = get_rules(&rule_sets)?.into_iter().collect();
        tracing::info!(target: "conjure::stage", count = rule_sets.len(), "resolved rule sets");
        let context = Context::new_ptr(
            config.solver,
            names.iter().map(|s| s.to_string()).collect(),
            rules,
            rule_sets,
        );
        Ok(Self { config, context })
    }

    /// Access source metadata, enabled rules and accumulated statistics.
    pub fn context(&self) -> Arc<RwLock<Context<'static>>> {
        Arc::clone(&self.context)
    }

    /// Run lower-level library operations with this pipeline's settings in scope.
    ///
    /// Use this when loading or running a [`Solver`] directly, including solver-time rewrites.
    /// Nested operations on this pipeline share the current random stream and interactive answers.
    /// Keep the closure synchronous; settings are local to its current thread.
    pub fn with_settings<T>(&self, f: impl FnOnce() -> T) -> T {
        let pointer = std::ptr::from_ref(self);
        if ACTIVE_PIPELINE.with(|active| active.get() == pointer) {
            return f();
        }
        settings::with_saved_settings(|| {
            let previous = ACTIVE_PIPELINE.with(|active| active.replace(pointer));
            let _active = ActivePipelineGuard(previous);
            self.config.apply();
            f()
        })
    }

    /// Parse Essence source in memory using the configured parser.
    pub fn parse(&self, source: &str) -> Result<Model> {
        self.with_settings(|| match self.config.parser {
            Parser::TreeSitter => self.parse_native(source, None),
        })
    }

    /// Parse a file using the configured parser without requiring a UTF-8 path.
    pub fn parse_file(&self, path: impl AsRef<Path>) -> Result<Model> {
        let path = path.as_ref();
        tracing::info!(target: "conjure::stage", path = %path.display(), "parsing input model");
        self.with_settings(|| match self.config.parser {
            Parser::TreeSitter => {
                let source = std::fs::read_to_string(path)
                    .with_context(|| format!("could not read {}", path.display()))?;
                self.parse_native(&source, Some(path.to_string_lossy().into_owned()))
            }
        })
    }

    fn parse_native(&self, source: &str, filename: Option<String>) -> Result<Model> {
        let mut errors = Vec::new();
        match crate::parse::tree_sitter::parse_essence_with_context(
            source,
            self.context(),
            &mut errors,
        ) {
            Ok(Some(model)) => {
                crate::ast::assertions::debug_assert_model_well_formed(&model, "tree-sitter");
                Ok(model)
            }
            Ok(None) => {
                Err(ParseErrorCollection::multiple(errors, Some(source.into()), filename).into())
            }
            Err(error) => Err(ParseErrorCollection::fatal(error).into()),
        }
    }

    /// Parse, instantiate optional Essence parameters, and rewrite an in-memory problem.
    pub fn compile(&self, source: &str, parameters: Option<&str>) -> Result<Model> {
        self.with_settings(|| {
            let problem = self.parse(source)?;
            let model = match parameters {
                Some(parameters) => instantiate_model(problem, self.parse(parameters)?)?,
                None => {
                    let mut problem = problem;
                    validate_instantiation_conditions(&mut problem)?;
                    problem
                }
            };
            self.rewrite(model)
        })
    }

    /// Rewrite a model parsed with this pipeline, resetting modelling choices for the operation.
    pub fn rewrite(&self, model: Model) -> Result<Model> {
        self.check_context(&model)?;
        for (name, declaration) in model.symbols().iter_local() {
            ensure!(
                declaration.as_given().is_none(),
                "given `{name}` needs a parameter value before rewriting"
            );
        }
        self.with_settings(|| {
            tracing::debug!(model = %model, "initial model");
            let Rewriter::Rewrite(config) = self.config.rewriter;
            let rule_sets = self.context.read().unwrap().rule_sets.clone();
            tracing::info!(target: "conjure::stage", %config, "rewriting model");
            let model = rewrite_model(&model, &rule_sets, config)?;
            tracing::debug!(model = %model, "rewritten model");
            Ok(model)
        })
    }

    /// Create a configured solver; use [`Self::with_settings`] when running it directly.
    pub fn solver(&self) -> Result<Solver> {
        let config = &self.config;
        Ok(match config.solver {
            SolverFamily::Minion => Solver::new(
                Minion::with_search_orders(config.minion_varorder, config.minion_valorder)
                    .with_solver_seed(config.solver_seed)
                    .with_timeout(config.solver_timeout),
            ),
            #[cfg(any(
                feature = "sat-batsat",
                all(feature = "sat-cadical", not(target_family = "wasm"))
            ))]
            SolverFamily::Sat => Solver::new(
                Sat::new(config.sat_backend)
                    .with_solver_seed(config.solver_seed)
                    .with_timeout(config.solver_timeout),
            ),
            #[cfg(not(any(
                feature = "sat-batsat",
                all(feature = "sat-cadical", not(target_family = "wasm"))
            )))]
            SolverFamily::Sat => anyhow::bail!(
                "SAT solver support was not compiled in (enable `sat-batsat` or `sat-cadical`)."
            ),
            #[cfg(feature = "z3")]
            SolverFamily::Z3 => {
                Solver::new(Smt::new(config.solver_timeout).with_solver_seed(config.solver_seed))
            }
            #[cfg(not(feature = "z3"))]
            SolverFamily::Z3 => {
                anyhow::bail!("Z3 solver support was not compiled in (enable the `z3` feature).")
            }
        })
    }

    /// Solve a rewritten model and reconstruct its high-level Essence values.
    pub fn solve(
        &self,
        model: Model,
        options: &SolutionOptions,
    ) -> Result<Vec<BTreeMap<Name, Literal>>> {
        self.solve_with_timings(model, options)
            .map(|(solutions, _)| solutions)
    }

    /// Collect reconstructed solutions and backend, search and reconstruction timings.
    pub fn solve_with_timings(
        &self,
        model: Model,
        options: &SolutionOptions,
    ) -> Result<(Vec<BTreeMap<Name, Literal>>, SolutionCollectionTimings)> {
        self.check_context(&model)?;
        ensure!(
            options.number_of_solutions >= 0,
            "number of solutions must be non-negative (zero requests all)"
        );
        self.with_settings(|| {
            get_solutions_with_timings(
                self.solver()?,
                model,
                options.number_of_solutions,
                options.keep_intermediate_solutions,
                &options.solver_input_file,
                options.rule_trace_cdp,
            )
        })
    }

    fn check_context(&self, model: &Model) -> Result<()> {
        ensure!(
            Arc::ptr_eq(&model.context, &self.context),
            "model belongs to a different pipeline context"
        );
        Ok(())
    }
}
