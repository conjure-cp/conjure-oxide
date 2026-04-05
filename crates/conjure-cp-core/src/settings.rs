use std::{cell::Cell, fmt::Display, str::FromStr};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum_macros::{Display as StrumDisplay, EnumIter};

use crate::bug;

use crate::solver::adaptors::smt::{IntTheory, MatrixTheory, TheoryConfig};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Parser {
    #[default]
    TreeSitter,
    ViaConjure,
}

impl Display for Parser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Parser::TreeSitter => write!(f, "tree-sitter"),
            Parser::ViaConjure => write!(f, "via-conjure"),
        }
    }
}

impl FromStr for Parser {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "tree-sitter" => Ok(Parser::TreeSitter),
            "via-conjure" => Ok(Parser::ViaConjure),
            other => Err(format!(
                "unknown parser: {other}; expected one of: tree-sitter, via-conjure"
            )),
        }
    }
}

thread_local! {
    /// Thread-local setting for which parser is currently active.
    ///
    /// Must be explicitly set before use.
    static CURRENT_PARSER: Cell<Option<Parser>> = const { Cell::new(None) };
}

pub fn set_current_parser(parser: Parser) {
    CURRENT_PARSER.with(|current| current.set(Some(parser)));
}

pub fn current_parser() -> Parser {
    CURRENT_PARSER.with(|current| {
        current.get().unwrap_or_else(|| {
            // loud failure on purpose, so we don't end up using the default
            bug!("current parser not set for this thread; call set_current_parser first")
        })
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MorphConfig {
    pub cache: MorphCachingStrategy,
    pub prefilter: bool,
    /// Use naive (no-levels) traversal (`morph_naive`). Enabled with `levelsoff`, disabled with `levelson`.
    pub naive: bool,
    pub fixedpoint: bool,
}

impl Default for MorphConfig {
    fn default() -> Self {
        Self {
            cache: MorphCachingStrategy::default(),
            prefilter: true,
            naive: false,
            fixedpoint: false,
        }
    }
}

impl Display for MorphConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "morph")?;
        write!(f, "-{}", if self.naive { "levelsoff" } else { "levelson" })?;
        write!(f, "-{}", self.cache)?;
        write!(
            f,
            "-{}",
            if self.prefilter {
                "prefilteron"
            } else {
                "prefilteroff"
            }
        )?;
        if self.fixedpoint {
            write!(f, "-fixedpoint")?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum MorphCachingStrategy {
    NoCache,
    Cache,
    #[default]
    IncrementalCache,
}

impl FromStr for MorphCachingStrategy {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "no-cache" => Ok(Self::NoCache),
            "cache" => Ok(Self::Cache),
            "inc-cache" => Ok(Self::IncrementalCache),
            other => Err(format!(
                "unknown cache strategy: {other}; expected one of: no-cache, cahce, inc-cache"
            )),
        }
    }
}

impl Display for MorphCachingStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MorphCachingStrategy::NoCache => write!(f, "nocache"),
            MorphCachingStrategy::Cache => write!(f, "cache"),
            MorphCachingStrategy::IncrementalCache => write!(f, "inccache"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Rewriter {
    Naive,
    Morph(MorphConfig),
}

thread_local! {
    /// Thread-local setting for which rewriter is currently active.
    ///
    /// Must be explicitly set before use.
    static CURRENT_REWRITER: Cell<Option<Rewriter>> = const { Cell::new(None) };
}

impl Display for Rewriter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Rewriter::Naive => write!(f, "naive"),
            Rewriter::Morph(config) => write!(f, "{config}"),
        }
    }
}

impl FromStr for Rewriter {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "naive" => Ok(Rewriter::Naive),
            "morph" => Ok(Rewriter::Morph(MorphConfig::default())),
            other => {
                if !other.starts_with("morph-") {
                    return Err(format!(
                        "unknown rewriter: {other}; expected one of: naive, morph, morph-[levelson|levelsoff]-[nocache|cache|inccache]-[prefilteron|prefilteroff]-[fixedpoint]"
                    ));
                }

                let parts = other.split('-').skip(1);
                let mut config = MorphConfig::default();
                let mut cache_set = false;
                let mut levels_set = false;
                let mut prefilter_set = false;
                for token in parts {
                    match token {
                        "" => (),
                        "levelson" => {
                            if levels_set {
                                return Err("conflicting levels options: only one of levelson|levelsoff is allowed".to_string());
                            }
                            config.naive = false;
                            levels_set = true;
                        }
                        "levelsoff" => {
                            if levels_set {
                                return Err("conflicting levels options: only one of levelson|levelsoff is allowed".to_string());
                            }
                            config.naive = true;
                            levels_set = true;
                        }
                        "nocache" | "cache" | "inccache" => {
                            if cache_set {
                                return Err("conflicting cache options: only one of nocache|cache|inccache is allowed".to_string());
                            }
                            config.cache = match token {
                                "nocache" => MorphCachingStrategy::NoCache,
                                "cache" => MorphCachingStrategy::Cache,
                                "inccache" => MorphCachingStrategy::IncrementalCache,
                                _ => unreachable!(),
                            };
                            cache_set = true;
                        }
                        "prefilteron" => {
                            if prefilter_set {
                                return Err("conflicting prefilter options: only one of prefilteron|prefilteroff is allowed".to_string());
                            }
                            config.prefilter = true;
                            prefilter_set = true;
                        }
                        "prefilteroff" => {
                            if prefilter_set {
                                return Err("conflicting prefilter options: only one of prefilteron|prefilteroff is allowed".to_string());
                            }
                            config.prefilter = false;
                            prefilter_set = true;
                        }
                        "fixedpoint" => {
                            config.fixedpoint = true;
                        }
                        other_token => {
                            return Err(format!(
                                "unknown morph option '{other_token}', must be one of levelson|levelsoff|nocache|cache|inccache|prefilteron|prefilteroff|fixedpoint"
                            ));
                        }
                    }
                }

                Ok(Rewriter::Morph(config))
            }
        }
    }
}

pub fn set_current_rewriter(rewriter: Rewriter) {
    CURRENT_REWRITER.with(|current| current.set(Some(rewriter)));
}

pub fn current_rewriter() -> Rewriter {
    CURRENT_REWRITER.with(|current| {
        current.get().unwrap_or_else(|| {
            // loud failure on purpose, so we don't end up using the default
            bug!("current rewriter not set for this thread; call set_current_rewriter first")
        })
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum QuantifiedExpander {
    Native,
    ViaSolver,
    ViaSolverAc,
}

impl Display for QuantifiedExpander {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QuantifiedExpander::Native => write!(f, "native"),
            QuantifiedExpander::ViaSolver => write!(f, "via-solver"),
            QuantifiedExpander::ViaSolverAc => write!(f, "via-solver-ac"),
        }
    }
}

impl FromStr for QuantifiedExpander {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "native" => Ok(QuantifiedExpander::Native),
            "via-solver" => Ok(QuantifiedExpander::ViaSolver),
            "via-solver-ac" => Ok(QuantifiedExpander::ViaSolverAc),
            _ => Err(format!(
                "unknown comprehension expander: {s}; expected one of: \
                 native, via-solver, via-solver-ac"
            )),
        }
    }
}

thread_local! {
    /// Thread-local setting for which comprehension expansion strategy is currently active.
    ///
    /// Must be explicitly set before use.
    static COMPREHENSION_EXPANDER: Cell<Option<QuantifiedExpander>> = const { Cell::new(None) };
}

pub fn set_comprehension_expander(expander: QuantifiedExpander) {
    COMPREHENSION_EXPANDER.with(|current| current.set(Some(expander)));
}

pub fn comprehension_expander() -> QuantifiedExpander {
    COMPREHENSION_EXPANDER.with(|current| {
        current.get().unwrap_or_else(|| {
            // loud failure on purpose, so we don't end up using the default
            bug!(
                "comprehension expander not set for this thread; call set_comprehension_expander first"
            )
        })
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize, JsonSchema)]
pub enum SatEncoding {
    #[default]
    Log,
    Direct,
    Order,
}

impl SatEncoding {
    pub const fn as_str(self) -> &'static str {
        match self {
            SatEncoding::Log => "log",
            SatEncoding::Direct => "direct",
            SatEncoding::Order => "order",
        }
    }

    pub const fn as_rule_set(self) -> &'static str {
        match self {
            SatEncoding::Log => "SAT_Log",
            SatEncoding::Direct => "SAT_Direct",
            SatEncoding::Order => "SAT_Order",
        }
    }
}

impl Display for SatEncoding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SatEncoding::Log => write!(f, "log"),
            SatEncoding::Direct => write!(f, "direct"),
            SatEncoding::Order => write!(f, "order"),
        }
    }
}

impl FromStr for SatEncoding {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "log" => Ok(SatEncoding::Log),
            "direct" => Ok(SatEncoding::Direct),
            "order" => Ok(SatEncoding::Order),
            other => Err(format!(
                "unknown sat-encoding: {other}; expected one of: log, direct, order"
            )),
        }
    }
}

#[derive(
    Debug,
    EnumIter,
    StrumDisplay,
    PartialEq,
    Eq,
    Hash,
    Clone,
    Copy,
    Serialize,
    Deserialize,
    JsonSchema,
)]
pub enum SolverFamily {
    Minion,
    Sat(SatEncoding),
    Smt(TheoryConfig),
    SavileRow,
}

thread_local! {
    /// Thread-local setting for which solver family is currently active.
    ///
    /// Must be explicitly set before use.
    static CURRENT_SOLVER_FAMILY: Cell<Option<SolverFamily>> = const { Cell::new(None) };
}

pub const DEFAULT_MINION_DISCRETE_THRESHOLD: usize = 10;

thread_local! {
    /// Thread-local setting controlling when Minion int domains are emitted as `DISCRETE`.
    ///
    /// If an int domain size is <= this threshold, the Minion adaptor uses `DISCRETE`; otherwise
    /// it uses `BOUND`, unless another constraint requires `DISCRETE`.
    static MINION_DISCRETE_THRESHOLD: Cell<usize> =
        const { Cell::new(DEFAULT_MINION_DISCRETE_THRESHOLD) };

    /// Thread-local setting controlling whether rule-trace outputs are active in this phase.
    ///
    /// This is intentionally off by default and can be disabled before solver-time rewrites so
    /// follow-up dominance-blocking rewrites do not pollute the initial rewrite trace.
    static RULE_TRACE_ENABLED: Cell<bool> = const { Cell::new(false) };

    /// Thread-local setting controlling whether default rule traces are configured.
    static DEFAULT_RULE_TRACE_ENABLED: Cell<bool> = const { Cell::new(false) };

    /// Thread-local setting controlling whether verbose rule-attempt traces are configured.
    static RULE_TRACE_VERBOSE_ENABLED: Cell<bool> = const { Cell::new(false) };

    /// Thread-local setting controlling whether aggregate rule-application traces are configured.
    static RULE_TRACE_AGGREGATES_ENABLED: Cell<bool> = const { Cell::new(false) };
}

pub fn set_current_solver_family(solver_family: SolverFamily) {
    CURRENT_SOLVER_FAMILY.with(|current| current.set(Some(solver_family)));
}

pub fn current_solver_family() -> SolverFamily {
    CURRENT_SOLVER_FAMILY.with(|current| {
        current.get().unwrap_or_else(|| {
            // loud failure on purpose, so we don't end up using the default
            bug!(
                "current solver family not set for this thread; call set_current_solver_family first"
            )
        })
    })
}

pub fn set_minion_discrete_threshold(threshold: usize) {
    MINION_DISCRETE_THRESHOLD.with(|current| current.set(threshold));
}

pub fn minion_discrete_threshold() -> usize {
    MINION_DISCRETE_THRESHOLD.with(|current| current.get())
}

pub fn set_rule_trace_enabled(enabled: bool) {
    RULE_TRACE_ENABLED.with(|current| current.set(enabled));
}

pub fn rule_trace_enabled() -> bool {
    RULE_TRACE_ENABLED.with(|current| current.get())
}

pub fn set_default_rule_trace_enabled(enabled: bool) {
    DEFAULT_RULE_TRACE_ENABLED.with(|current| current.set(enabled));
}

pub fn default_rule_trace_enabled() -> bool {
    DEFAULT_RULE_TRACE_ENABLED.with(|current| current.get())
}

pub fn set_rule_trace_verbose_enabled(enabled: bool) {
    RULE_TRACE_VERBOSE_ENABLED.with(|current| current.set(enabled));
}

pub fn rule_trace_verbose_enabled() -> bool {
    RULE_TRACE_VERBOSE_ENABLED.with(|current| current.get())
}

pub fn set_rule_trace_aggregates_enabled(enabled: bool) {
    RULE_TRACE_AGGREGATES_ENABLED.with(|current| current.set(enabled));
}

pub fn rule_trace_aggregates_enabled() -> bool {
    RULE_TRACE_AGGREGATES_ENABLED.with(|current| current.get())
}

pub fn configured_rule_trace_enabled() -> bool {
    default_rule_trace_enabled() || rule_trace_verbose_enabled() || rule_trace_aggregates_enabled()
}

impl FromStr for SolverFamily {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim().to_ascii_lowercase();

        match s.as_str() {
            "minion" => Ok(SolverFamily::Minion),
            "savilerow" => Ok(SolverFamily::SavileRow),
            "sat" | "sat-log" => Ok(SolverFamily::Sat(SatEncoding::Log)),
            "sat-direct" => Ok(SolverFamily::Sat(SatEncoding::Direct)),
            "sat-order" => Ok(SolverFamily::Sat(SatEncoding::Order)),
            "smt" => Ok(SolverFamily::Smt(TheoryConfig::default())),
            other => {
                // allow forms like `smt-bv-atomic` or `smt-lia-arrays`
                if other.starts_with("smt-") {
                    let parts = other.split('-').skip(1);
                    let mut ints = IntTheory::default();
                    let mut matrices = MatrixTheory::default();
                    let mut unwrap_alldiff = false;

                    for token in parts {
                        match token {
                            "" => {}
                            "lia" => ints = IntTheory::Lia,
                            "bv" => ints = IntTheory::Bv,
                            "arrays" => matrices = MatrixTheory::Arrays,
                            "atomic" => matrices = MatrixTheory::Atomic,
                            "nodiscrete" => unwrap_alldiff = true,
                            other_token => {
                                return Err(format!(
                                    "unknown SMT theory option '{other_token}', must be one of bv|lia|arrays|atomic|nodiscrete"
                                ));
                            }
                        }
                    }

                    return Ok(SolverFamily::Smt(TheoryConfig {
                        ints,
                        matrices,
                        unwrap_alldiff,
                    }));
                }
                Err(format!(
                    "unknown solver family '{other}', expected one of: minion, savilerow, savile-row, sat-log, sat-direct, sat-order, smt[(bv|lia)-(arrays|atomic)][-nodiscrete]"
                ))
            }
        }
    }
}

impl SolverFamily {
    pub fn as_str(&self) -> String {
        match self {
            SolverFamily::Minion => "minion".to_owned(),
            SolverFamily::SavileRow => "savile-row".to_owned(),
            SolverFamily::Sat(encoding) => format!("sat-{}", encoding.as_str()),
            SolverFamily::Smt(theory_config) => format!("smt-{}", theory_config.as_str()),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SolverArgs {
    pub timeout_ms: Option<u64>,
}
