use super::{AtomKind, RewriteError, RulePrefilter, RuleSet, resolve_rules::RuleData};
use crate::{
    Model,
    ast::{
        Atom, Expression as Expr, ExpressionArena, ExpressionNodeId, Metadata, Moo, Name,
        discriminant_from_value, normalise_evaluator_local,
    },
    bug,
    objective::introduce_objective_auxiliary,
    rule_engine::{
        get_rules_grouped,
        rewriter_common::{
            RuleResult, VariableDeclarationSnapshot, log_rule_application,
            snapshot_symbols_after_effect, snapshot_variable_declarations,
            try_rewrite_value_letting_once,
        },
    },
    settings::{
        RewriteConfig, Rewriter, default_rule_trace_enabled, rule_trace_enabled,
        rule_trace_verbose_enabled, set_current_rewriter,
    },
    stats::RewriterStats,
};

use itertools::Itertools;
use std::{
    cmp::Ordering,
    collections::{BTreeMap, BinaryHeap, HashMap, HashSet},
    fmt::Write as FmtWrite,
    fs::{self, OpenOptions},
    hash::{DefaultHasher, Hash, Hasher},
    io::Write as IoWrite,
    path::PathBuf,
    time::Instant,
};
use tracing::trace;
use uniplate::{Biplate, Uniplate};

// Rewriter selection invariant:
//
// 1. Higher rule priority wins.
// 2. Within one priority, an expression is tried before its descendants.
//
// Consequently, if A contains B and both are rewriteable at the same priority, A must be selected
// first. If A rewrites, old descendants such as B are not considered; descendants are only
// scheduled when no enclosing expression rewrites first. The rewriter does not require old full-scan
// preorder between unrelated sibling subtrees.
// This is a semantic requirement of the rewriter, not merely a scheduling optimisation.
//
// Rule side effects:
//
// A selected rule always replaces the focused expression. It may also append top-level constraints,
// append CNF clauses, add symbol declarations, or change existing symbol declarations (for example
// by tightening a domain). Rewriting a value-letting surface writes the replacement expression back
// to that symbol declaration. Symbol changes invalidate rule-application context caches; changed
// declarations and CNF effects may require rebuilding rewrite surfaces from the model.

// debug imports
#[cfg(debug_assertions)]
use {
    crate::ast::assertions::debug_assert_model_well_formed,
    tracing::{Level, span},
};

type ApplicableRule<'a, CtxFnType> = (
    RuleResult<'a>,
    usize,
    Expr,
    CtxFnType,
    Option<VariableDeclarationSnapshot>,
);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ScheduledMode {
    /// Check only this node at the scheduled rule level.
    CheckNode,
    /// Traverse this subtree at the scheduled level and carry it forward if it survives.
    TraverseSubtreeRoot,
    /// Traverse this descendant at the current level only; the root carries future levels.
    TraverseSubtreeDescendant,
}

impl ScheduledMode {
    fn includes(self, other: Self) -> bool {
        matches!(
            (self, other),
            (ScheduledMode::TraverseSubtreeRoot, ScheduledMode::CheckNode)
                | (
                    ScheduledMode::TraverseSubtreeRoot,
                    ScheduledMode::TraverseSubtreeRoot
                )
                | (
                    ScheduledMode::TraverseSubtreeRoot,
                    ScheduledMode::TraverseSubtreeDescendant
                )
                | (
                    ScheduledMode::TraverseSubtreeDescendant,
                    ScheduledMode::CheckNode
                )
                | (
                    ScheduledMode::TraverseSubtreeDescendant,
                    ScheduledMode::TraverseSubtreeDescendant
                )
                | (ScheduledMode::CheckNode, ScheduledMode::CheckNode)
        )
    }

    fn descends_on_failure(self) -> bool {
        matches!(
            self,
            ScheduledMode::TraverseSubtreeRoot | ScheduledMode::TraverseSubtreeDescendant
        )
    }

    fn next_self_mode(self) -> Option<Self> {
        match self {
            ScheduledMode::CheckNode => Some(ScheduledMode::CheckNode),
            ScheduledMode::TraverseSubtreeRoot => Some(ScheduledMode::TraverseSubtreeRoot),
            ScheduledMode::TraverseSubtreeDescendant => None,
        }
    }

    fn advances_as_subtree(self) -> bool {
        matches!(self, ScheduledMode::TraverseSubtreeRoot)
    }
}

#[derive(Default, Debug)]
struct WorklistModeCounts {
    check_node: usize,
    traverse_subtree: usize,
}

impl WorklistModeCounts {
    fn increment(&mut self, mode: ScheduledMode) {
        self.add(mode, 1);
    }

    fn add(&mut self, mode: ScheduledMode, value: usize) {
        match mode {
            ScheduledMode::CheckNode => self.check_node += value,
            ScheduledMode::TraverseSubtreeRoot | ScheduledMode::TraverseSubtreeDescendant => {
                self.traverse_subtree += value
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum WorklistStaleReason {
    ModeMismatch,
    MissingSurface,
    InactiveSurface,
    UnreachableNode,
    GenerationMismatch,
}

#[derive(Default, Debug)]
struct WorklistStaleReasonCounts {
    mode_mismatch: WorklistModeCounts,
    missing_surface: WorklistModeCounts,
    inactive_surface: WorklistModeCounts,
    unreachable_node: WorklistModeCounts,
    generation_mismatch: WorklistModeCounts,
}

impl WorklistStaleReasonCounts {
    fn increment(&mut self, reason: WorklistStaleReason, mode: ScheduledMode) {
        match reason {
            WorklistStaleReason::ModeMismatch => self.mode_mismatch.increment(mode),
            WorklistStaleReason::MissingSurface => self.missing_surface.increment(mode),
            WorklistStaleReason::InactiveSurface => self.inactive_surface.increment(mode),
            WorklistStaleReason::UnreachableNode => self.unreachable_node.increment(mode),
            WorklistStaleReason::GenerationMismatch => self.generation_mismatch.increment(mode),
        }
    }
}

#[derive(Default)]
struct DirtyTrace {
    enabled: bool,
    destination: DirtyTraceDestination,
    passes: usize,
    priority_scans: usize,
    expression_visits: usize,
    dirty_hits: usize,
    clean_marks: usize,
    attempted_expressions: usize,
    rule_attempts: usize,
    rewrites: usize,
    value_letting_rewrites: usize,
    whole_model_clears_after_value_letting: usize,
    whole_model_clears_after_side_effects: usize,
    side_effect_arena_reimports: usize,
    side_effects_kept_in_arena: usize,
    replacement_subtree_clears: usize,
    ancestor_clears: usize,
    cache_hits: usize,
    cache_misses: usize,
    cache_terminal_hits: usize,
    cache_rewrite_hits: usize,
    cache_inserts: usize,
    cache_ancestor_mappings: usize,
    cache_resets: usize,
    arena_content_hash_requests: usize,
    arena_content_hash_hits: usize,
    arena_content_hash_misses: usize,
    ancestor_hash_capture_runs: usize,
    ancestor_hash_captured_nodes: usize,
    candidate_index_scans: usize,
    candidate_index_full_scans: usize,
    candidate_index_filtered_scans: usize,
    candidate_index_skipped_nodes: usize,
    rule_memo_hits: usize,
    worklist_enqueues: usize,
    worklist_pops: usize,
    worklist_stale_pops: usize,
    worklist_enqueues_by_mode: WorklistModeCounts,
    worklist_pops_by_mode: WorklistModeCounts,
    worklist_stale_pops_by_mode: WorklistModeCounts,
    worklist_stale_pops_by_reason: WorklistStaleReasonCounts,
    worklist_no_candidate_pops_by_mode: WorklistModeCounts,
    worklist_rule_attempt_pops_by_mode: WorklistModeCounts,
    worklist_child_descents_by_mode: WorklistModeCounts,
    dirty_hits_by_priority: BTreeMap<u16, usize>,
    clean_marks_by_priority: BTreeMap<u16, usize>,
    rule_attempts_by_priority: BTreeMap<u16, usize>,
    rule_attempts_by_rule: BTreeMap<String, usize>,
    rewrites_by_rule: BTreeMap<String, usize>,
    side_effect_rewrites_by_rule: BTreeMap<String, usize>,
    whole_model_clears_by_rule: BTreeMap<String, usize>,
}

#[derive(Default, Debug, PartialEq, Eq)]
enum DirtyTraceDestination {
    #[default]
    Stderr,
    File(PathBuf),
    Directory(PathBuf),
}

impl DirtyTrace {
    fn from_env() -> Self {
        let Some(destination) = std::env::var_os("CONJURE_DIRTY_TRACE") else {
            return Self::default();
        };

        Self {
            enabled: true,
            destination: dirty_trace_destination_from_env_value(destination),
            ..Self::default()
        }
    }

    fn record_dirty_hit(&mut self, priority: u16) {
        if !self.enabled {
            return;
        }
        self.dirty_hits += 1;
        *self.dirty_hits_by_priority.entry(priority).or_default() += 1;
    }

    fn record_clean_mark(&mut self, priority: u16) {
        if !self.enabled {
            return;
        }
        self.clean_marks += 1;
        *self.clean_marks_by_priority.entry(priority).or_default() += 1;
    }

    fn record_rewrite(&mut self, rule_name: &str, side_effects: bool) {
        if !self.enabled {
            return;
        }
        self.rewrites += 1;
        *self
            .rewrites_by_rule
            .entry(rule_name.to_owned())
            .or_default() += 1;
        if side_effects {
            *self
                .side_effect_rewrites_by_rule
                .entry(rule_name.to_owned())
                .or_default() += 1;
        }
    }

    fn record_whole_model_clear(&mut self, rule_name: &str) {
        if !self.enabled {
            return;
        }
        self.whole_model_clears_after_side_effects += 1;
        *self
            .whole_model_clears_by_rule
            .entry(rule_name.to_owned())
            .or_default() += 1;
    }

    fn record_side_effect_arena_reimport(&mut self) {
        if !self.enabled {
            return;
        }
        self.side_effect_arena_reimports += 1;
    }

    fn record_side_effect_kept_in_arena(&mut self) {
        if !self.enabled {
            return;
        }
        self.side_effects_kept_in_arena += 1;
    }

    fn record_arena_content_hash(&mut self, hit: bool) {
        if !self.enabled {
            return;
        }
        self.arena_content_hash_requests += 1;
        if hit {
            self.arena_content_hash_hits += 1;
        } else {
            self.arena_content_hash_misses += 1;
        }
    }

    fn record_candidate_index_scan(&mut self, total_nodes: usize, scanned_nodes: usize) {
        if !self.enabled {
            return;
        }
        self.candidate_index_scans += 1;
        if scanned_nodes == total_nodes {
            self.candidate_index_full_scans += 1;
        } else {
            self.candidate_index_filtered_scans += 1;
            self.candidate_index_skipped_nodes += total_nodes - scanned_nodes;
        }
    }

    fn record_rule_memo_hit(&mut self) {
        if !self.enabled {
            return;
        }
        self.rule_memo_hits += 1;
    }

    fn record_rule_attempt(&mut self, priority: u16, rule_name: &str) {
        self.rule_attempts += 1;
        if !self.enabled {
            return;
        }
        *self.rule_attempts_by_priority.entry(priority).or_default() += 1;
        *self
            .rule_attempts_by_rule
            .entry(rule_name.to_owned())
            .or_default() += 1;
    }

    fn record_worklist_enqueue(&mut self, mode: ScheduledMode) {
        if !self.enabled {
            return;
        }
        self.worklist_enqueues += 1;
        self.worklist_enqueues_by_mode.increment(mode);
    }

    fn record_worklist_pop(&mut self, mode: ScheduledMode) {
        if !self.enabled {
            return;
        }
        self.worklist_pops += 1;
        self.worklist_pops_by_mode.increment(mode);
    }

    fn record_worklist_stale_pop(&mut self, mode: ScheduledMode, reason: WorklistStaleReason) {
        if !self.enabled {
            return;
        }
        self.worklist_stale_pops += 1;
        self.worklist_stale_pops_by_mode.increment(mode);
        self.worklist_stale_pops_by_reason.increment(reason, mode);
    }

    fn record_worklist_no_candidate_pop(&mut self, mode: ScheduledMode) {
        if !self.enabled {
            return;
        }
        self.worklist_no_candidate_pops_by_mode.increment(mode);
    }

    fn record_worklist_rule_attempt_pop(&mut self, mode: ScheduledMode) {
        if !self.enabled {
            return;
        }
        self.worklist_rule_attempt_pops_by_mode.increment(mode);
    }

    fn record_worklist_child_descent(&mut self, mode: ScheduledMode, children: usize) {
        if !self.enabled {
            return;
        }
        self.worklist_child_descents_by_mode.add(mode, children);
    }

    fn finish(&self, stats: &RewriterStats) {
        if !self.enabled {
            return;
        }

        let mut output = String::new();
        writeln!(output, "[dirty-trace] passes={}", self.passes).unwrap();
        writeln!(
            output,
            "[dirty-trace] priority_scans={}",
            self.priority_scans
        )
        .unwrap();
        writeln!(
            output,
            "[dirty-trace] expression_visits={}",
            self.expression_visits
        )
        .unwrap();
        writeln!(
            output,
            "[dirty-trace] attempted_expressions={}",
            self.attempted_expressions
        )
        .unwrap();
        writeln!(
            output,
            "[dirty-trace] rule_attempts_counted={}",
            self.rule_attempts
        )
        .unwrap();
        writeln!(
            output,
            "[dirty-trace] rule_memo_hits={}",
            self.rule_memo_hits
        )
        .unwrap();
        writeln!(
            output,
            "[dirty-trace] stats_rule_attempts={}",
            stats.rewriter_rule_application_attempts.unwrap_or(0)
        )
        .unwrap();
        writeln!(output, "[dirty-trace] clean_marks={}", self.clean_marks).unwrap();
        writeln!(output, "[dirty-trace] dirty_hits={}", self.dirty_hits).unwrap();
        writeln!(output, "[dirty-trace] rewrites={}", self.rewrites).unwrap();
        writeln!(
            output,
            "[dirty-trace] value_letting_rewrites={}",
            self.value_letting_rewrites
        )
        .unwrap();
        writeln!(
            output,
            "[dirty-trace] whole_model_clears_after_value_letting={}",
            self.whole_model_clears_after_value_letting
        )
        .unwrap();
        writeln!(
            output,
            "[dirty-trace] whole_model_clears_after_side_effects={}",
            self.whole_model_clears_after_side_effects
        )
        .unwrap();
        writeln!(
            output,
            "[dirty-trace] side_effect_arena_reimports={}",
            self.side_effect_arena_reimports
        )
        .unwrap();
        writeln!(
            output,
            "[dirty-trace] side_effects_kept_in_arena={}",
            self.side_effects_kept_in_arena
        )
        .unwrap();
        writeln!(
            output,
            "[dirty-trace] replacement_subtree_clears={}",
            self.replacement_subtree_clears
        )
        .unwrap();
        writeln!(
            output,
            "[dirty-trace] ancestor_clears={}",
            self.ancestor_clears
        )
        .unwrap();
        writeln!(
            output,
            "[dirty-trace] dirty_hits_by_priority={:?}",
            self.dirty_hits_by_priority
        )
        .unwrap();
        writeln!(
            output,
            "[dirty-trace] clean_marks_by_priority={:?}",
            self.clean_marks_by_priority
        )
        .unwrap();
        writeln!(
            output,
            "[dirty-trace] rule_attempts_by_priority={:?}",
            self.rule_attempts_by_priority
        )
        .unwrap();
        writeln!(
            output,
            "[dirty-trace] rule_attempts_by_rule={:?}",
            self.rule_attempts_by_rule
        )
        .unwrap();
        writeln!(
            output,
            "[dirty-trace] rewrites_by_rule={:?}",
            self.rewrites_by_rule
        )
        .unwrap();
        writeln!(
            output,
            "[dirty-trace] side_effect_rewrites_by_rule={:?}",
            self.side_effect_rewrites_by_rule
        )
        .unwrap();
        writeln!(
            output,
            "[dirty-trace] whole_model_clears_by_rule={:?}",
            self.whole_model_clears_by_rule
        )
        .unwrap();
        writeln!(output, "[dirty-trace] cache_hits={}", self.cache_hits).unwrap();
        writeln!(output, "[dirty-trace] cache_misses={}", self.cache_misses).unwrap();
        writeln!(
            output,
            "[dirty-trace] cache_terminal_hits={}",
            self.cache_terminal_hits
        )
        .unwrap();
        writeln!(
            output,
            "[dirty-trace] cache_rewrite_hits={}",
            self.cache_rewrite_hits
        )
        .unwrap();
        writeln!(output, "[dirty-trace] cache_inserts={}", self.cache_inserts).unwrap();
        writeln!(
            output,
            "[dirty-trace] cache_ancestor_mappings={}",
            self.cache_ancestor_mappings
        )
        .unwrap();
        writeln!(output, "[dirty-trace] cache_resets={}", self.cache_resets).unwrap();
        writeln!(
            output,
            "[dirty-trace] arena_content_hash_requests={}",
            self.arena_content_hash_requests
        )
        .unwrap();
        writeln!(
            output,
            "[dirty-trace] arena_content_hash_hits={}",
            self.arena_content_hash_hits
        )
        .unwrap();
        writeln!(
            output,
            "[dirty-trace] arena_content_hash_misses={}",
            self.arena_content_hash_misses
        )
        .unwrap();
        writeln!(
            output,
            "[dirty-trace] ancestor_hash_capture_runs={}",
            self.ancestor_hash_capture_runs
        )
        .unwrap();
        writeln!(
            output,
            "[dirty-trace] ancestor_hash_captured_nodes={}",
            self.ancestor_hash_captured_nodes
        )
        .unwrap();
        writeln!(
            output,
            "[dirty-trace] candidate_index_scans={}",
            self.candidate_index_scans
        )
        .unwrap();
        writeln!(
            output,
            "[dirty-trace] candidate_index_full_scans={}",
            self.candidate_index_full_scans
        )
        .unwrap();
        writeln!(
            output,
            "[dirty-trace] candidate_index_filtered_scans={}",
            self.candidate_index_filtered_scans
        )
        .unwrap();
        writeln!(
            output,
            "[dirty-trace] candidate_index_skipped_nodes={}",
            self.candidate_index_skipped_nodes
        )
        .unwrap();
        writeln!(
            output,
            "[dirty-trace] worklist_enqueues={}",
            self.worklist_enqueues
        )
        .unwrap();
        writeln!(
            output,
            "[dirty-trace] worklist_enqueues_by_mode={:?}",
            self.worklist_enqueues_by_mode
        )
        .unwrap();
        writeln!(output, "[dirty-trace] worklist_pops={}", self.worklist_pops).unwrap();
        writeln!(
            output,
            "[dirty-trace] worklist_pops_by_mode={:?}",
            self.worklist_pops_by_mode
        )
        .unwrap();
        writeln!(
            output,
            "[dirty-trace] worklist_stale_pops={}",
            self.worklist_stale_pops
        )
        .unwrap();
        writeln!(
            output,
            "[dirty-trace] worklist_stale_pops_by_mode={:?}",
            self.worklist_stale_pops_by_mode
        )
        .unwrap();
        writeln!(
            output,
            "[dirty-trace] worklist_stale_pops_by_reason={:?}",
            self.worklist_stale_pops_by_reason
        )
        .unwrap();
        writeln!(
            output,
            "[dirty-trace] worklist_no_candidate_pops_by_mode={:?}",
            self.worklist_no_candidate_pops_by_mode
        )
        .unwrap();
        writeln!(
            output,
            "[dirty-trace] worklist_rule_attempt_pops_by_mode={:?}",
            self.worklist_rule_attempt_pops_by_mode
        )
        .unwrap();
        writeln!(
            output,
            "[dirty-trace] worklist_child_descents_by_mode={:?}",
            self.worklist_child_descents_by_mode
        )
        .unwrap();

        self.write_output(&output);
    }

    fn write_output(&self, output: &str) {
        let path = match &self.destination {
            DirtyTraceDestination::Stderr => {
                eprint!("{output}");
                return;
            }
            DirtyTraceDestination::File(path) => path.clone(),
            DirtyTraceDestination::Directory(directory) => directory.join(format!(
                "dirty-trace-{}.txt",
                current_test_name_for_dirty_trace()
            )),
        };

        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            if let Err(error) = fs::create_dir_all(parent) {
                eprintln!(
                    "[dirty-trace] failed to create trace directory {}: {error}",
                    parent.display()
                );
                eprint!("{output}");
                return;
            }
        }

        match OpenOptions::new().create(true).append(true).open(&path) {
            Ok(mut file) => {
                if let Err(error) = file.write_all(output.as_bytes()) {
                    eprintln!(
                        "[dirty-trace] failed to write trace file {}: {error}",
                        path.display()
                    );
                    eprint!("{output}");
                }
            }
            Err(error) => {
                eprintln!(
                    "[dirty-trace] failed to open trace file {}: {error}",
                    path.display()
                );
                eprint!("{output}");
            }
        }
    }
}

fn dirty_trace_destination_from_env_value(
    destination: std::ffi::OsString,
) -> DirtyTraceDestination {
    if destination.is_empty()
        || destination == "1"
        || destination
            .to_str()
            .is_some_and(|value| value.eq_ignore_ascii_case("true"))
    {
        return DirtyTraceDestination::Stderr;
    }

    let path = PathBuf::from(destination);
    if path.is_file() {
        return DirtyTraceDestination::File(path);
    }

    // Treat bare paths as directories to support a single `nextest` run with
    // `CONJURE_DIRTY_TRACE=/abs/trace-dir`; each test then writes its own file.
    // Explicit file output remains available for paths that look like filenames.
    if path.is_dir() || path.extension().is_none() {
        DirtyTraceDestination::Directory(path)
    } else {
        DirtyTraceDestination::File(path)
    }
}

fn current_test_name_for_dirty_trace() -> String {
    let current_thread = std::thread::current();
    let name = current_thread
        .name()
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("pid-{}", std::process::id()));
    sanitize_dirty_trace_filename(&name)
}

fn sanitize_dirty_trace_filename(name: &str) -> String {
    let sanitized = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();

    let sanitized = sanitized.trim_matches('-');
    if sanitized.is_empty() {
        "unknown".to_string()
    } else {
        sanitized.to_string()
    }
}

/// Result of looking up an expression at a rewrite rule-group level.
enum CacheResult {
    /// The expression has not been cached at this level.
    Unknown,
    /// The expression is known not to rewrite through this maximum cached level.
    Terminal(usize),
    /// The expression rewrites to a cached replacement.
    Rewrite(CachedRewrite),
}

/// Cached rewrite target for a semantic expression/context key.
#[derive(Clone)]
struct CachedRewrite {
    /// Replacement expression to splice into the tree on a cache hit.
    expr: Expr,
    /// Earliest rule-group index at which this rewrite chain is known valid.
    valid_from_rule_group_index: usize,
}

/// Rewrite cache keyed by expression content hash and symbol context hash.
///
/// Rewrite entries are transitively resolved: inserting `A -> B`, `B -> C`, then `C -> D`
/// updates the observable cache result for `A`, `B`, and `C` to `D`.
#[derive(Default)]
struct RewriteCache {
    /// Rewrite map. Each entry records the earliest rule-group index where the chain is valid.
    rewrites: HashMap<u64, CachedRewrite>,
    /// Reverse edges used to update earlier mappings when a target later rewrites again.
    predecessors: HashMap<u64, Vec<u64>>,
    /// Context-qualified terminal shortcut: a subtree clean through rule group N is clean for <= N.
    clean_levels: HashMap<u64, usize>,
}

impl RewriteCache {
    /// Returns the level-independent content hash for an expression.
    ///
    /// Symbol-sensitive correctness is provided by mixing `symbol_context_hash` into
    /// [`Self::combine`], not by hashing declaration values into every node key.
    fn expression_content_hash(expr: &Expr, _symbol_context_hash: u64) -> u64 {
        expr.cached_content_hash()
    }

    /// Combines an expression hash and symbol context hash.
    fn combine(expression_content_hash: u64, symbol_context_hash: u64) -> u64 {
        let mut hasher = DefaultHasher::new();
        expression_content_hash.hash(&mut hasher);
        symbol_context_hash.hash(&mut hasher);
        hasher.finish()
    }

    /// Returns the context-qualified cache key for an expression.
    fn key(expr: &Expr, symbol_context_hash: u64) -> u64 {
        Self::combine(
            Self::expression_content_hash(expr, symbol_context_hash),
            symbol_context_hash,
        )
    }

    /// Removes terminal evidence for a source when stronger rewrite evidence is installed.
    ///
    /// This is needed for ancestor mappings: changing a child proves that the old ancestor maps to
    /// a rebuilt ancestor even if that old shape was previously cached as terminal.
    fn clear_terminal_fact(&mut self, expression_content_hash: u64, symbol_context_hash: u64) {
        let clean_key = Self::combine(expression_content_hash, symbol_context_hash);
        self.clean_levels.remove(&clean_key);
    }

    /// Looks up a subtree at a rule-group level.
    #[cfg(test)]
    fn get(&self, subtree: &Expr, level: usize, symbol_context_hash: u64) -> CacheResult {
        let expression_content_hash = Self::expression_content_hash(subtree, symbol_context_hash);
        self.get_from_hash(expression_content_hash, level, symbol_context_hash)
    }

    /// Looks up a subtree by precomputed content hash at a rule-group level.
    fn get_from_hash(
        &self,
        expression_content_hash: u64,
        level: usize,
        symbol_context_hash: u64,
    ) -> CacheResult {
        let clean_key = Self::combine(expression_content_hash, symbol_context_hash);
        if let Some(&max_clean) = self.clean_levels.get(&clean_key)
            && max_clean >= level
        {
            return CacheResult::Terminal(max_clean);
        }

        match self
            .rewrites
            .get(&Self::combine(expression_content_hash, symbol_context_hash))
        {
            Some(rewrite) if rewrite.valid_from_rule_group_index <= level => {
                CacheResult::Rewrite(rewrite.clone())
            }
            Some(_) | None => CacheResult::Unknown,
        }
    }

    /// Inserts either a terminal result or a rewrite result for `from`.
    #[cfg(test)]
    fn insert(&mut self, from: &Expr, to: Option<Expr>, level: usize, symbol_context_hash: u64) {
        self.insert_from_hash(
            Self::expression_content_hash(from, symbol_context_hash),
            to,
            level,
            symbol_context_hash,
        );
    }

    /// Inserts using a pre-replacement source hash.
    ///
    /// This is used for ancestor mappings, where the old expression no longer exists after the
    /// zipper has rebuilt an ancestor with the replacement child.
    fn insert_from_hash(
        &mut self,
        from_content_hash: u64,
        to: Option<Expr>,
        level: usize,
        symbol_context_hash: u64,
    ) {
        let from_key = Self::combine(from_content_hash, symbol_context_hash);

        let Some(to_expr) = to else {
            let clean_key = Self::combine(from_content_hash, symbol_context_hash);
            self.clean_levels
                .entry(clean_key)
                .and_modify(|l| *l = (*l).max(level))
                .or_insert(level);
            return;
        };

        let to_key = Self::key(&to_expr, symbol_context_hash);
        if from_key == to_key {
            return;
        }

        let valid_from_rule_group_index = self.rewrites.get(&from_key).map_or(level, |rewrite| {
            rewrite.valid_from_rule_group_index.min(level)
        });
        self.clear_terminal_fact(from_content_hash, symbol_context_hash);

        let resolved = match self.rewrites.get(&to_key) {
            Some(rewrite) => CachedRewrite {
                expr: rewrite.expr.clone(),
                valid_from_rule_group_index,
            },
            None => CachedRewrite {
                expr: to_expr,
                valid_from_rule_group_index,
            },
        };

        let resolved_key = Self::key(&resolved.expr, symbol_context_hash);
        self.rewrites.insert(from_key, resolved.clone());

        if let Some(mut predecessors) = self.predecessors.remove(&from_key) {
            for &predecessor in &predecessors {
                if let Some(predecessor_rewrite) = self.rewrites.get(&predecessor).cloned() {
                    self.rewrites.insert(
                        predecessor,
                        CachedRewrite {
                            expr: resolved.expr.clone(),
                            valid_from_rule_group_index: predecessor_rewrite
                                .valid_from_rule_group_index,
                        },
                    );
                }
            }

            self.predecessors
                .entry(resolved_key)
                .or_default()
                .append(&mut predecessors);
        }

        self.predecessors
            .entry(resolved_key)
            .or_default()
            .push(from_key);
    }
}

#[derive(Clone)]
struct RuleGroup<'a> {
    priority: u16,
    rules: Vec<RuleData<'a>>,
    /// Indexed by discriminant id for O(1) lookup of simple variant prefilters.
    rules_by_discriminant: Vec<Option<Vec<RuleData<'a>>>>,
    universal_rules: Vec<RuleData<'a>>,
    has_non_discriminant_filters: bool,
    target_discriminants: Vec<usize>,
    target_discriminant_mask: Vec<bool>,
}

enum CandidateRules<'group, 'rules> {
    Slice(std::slice::Iter<'group, RuleData<'rules>>),
    Filtered {
        iter: std::slice::Iter<'group, RuleData<'rules>>,
        expr: &'group Expr,
        include_universal: bool,
    },
}

impl<'group, 'rules> Iterator for CandidateRules<'group, 'rules> {
    type Item = &'group RuleData<'rules>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            CandidateRules::Slice(iter) => iter.next(),
            CandidateRules::Filtered {
                iter,
                expr,
                include_universal,
            } => loop {
                let rule_data = iter.next()?;
                if rule_matches_specific_prefilter(rule_data, expr)
                    || (*include_universal && rule_is_universal(rule_data))
                {
                    return Some(rule_data);
                }
            },
        }
    }
}

impl<'a> RuleGroup<'a> {
    fn new(priority: u16, rules: Vec<RuleData<'a>>) -> Self {
        let discriminants = rules
            .iter()
            .filter_map(|rd| rd.rule.prefilters)
            .flatten()
            .filter_map(|prefilter| match prefilter {
                RulePrefilter::Variant(discriminant) => Some(*discriminant),
                RulePrefilter::Child { .. }
                | RulePrefilter::VariantChild { .. }
                | RulePrefilter::Atom(_) => None,
            })
            .collect_vec();

        let mut rules_by_discriminant = Vec::new();
        if let Some(max_discriminant) = discriminants.iter().copied().max() {
            rules_by_discriminant.resize_with(max_discriminant + 1, || None);
        }

        let target_discriminants = discriminants.into_iter().unique().collect_vec();

        for &discriminant in &target_discriminants {
            rules_by_discriminant[discriminant] = Some(
                rules
                    .iter()
                    .filter(|rd| rule_matches_self_discriminant(rd, discriminant))
                    .cloned()
                    .collect(),
            );
        }

        let universal_rules: Vec<RuleData<'a>> = rules
            .iter()
            .filter(|rd| rule_is_universal(rd))
            .cloned()
            .collect();
        let has_non_discriminant_filters = rules.iter().any(|rd| {
            rd.rule.prefilters.is_some_and(|prefilters| {
                prefilters
                    .iter()
                    .any(|prefilter| !matches!(prefilter, RulePrefilter::Variant(_)))
            })
        });
        // Child and atom filters depend on more than this node's expression variant, so the
        // root-discriminant candidate index cannot safely skip them until it carries those
        // summaries as well.
        let target_discriminants = if universal_rules.is_empty() && !has_non_discriminant_filters {
            target_discriminants
        } else {
            Vec::new()
        };
        let mut target_discriminant_mask = Vec::new();
        if let Some(max_discriminant) = target_discriminants.iter().copied().max() {
            target_discriminant_mask.resize(max_discriminant + 1, false);
            for &discriminant in &target_discriminants {
                target_discriminant_mask[discriminant] = true;
            }
        }

        Self {
            priority,
            rules,
            rules_by_discriminant,
            universal_rules,
            has_non_discriminant_filters,
            target_discriminants,
            target_discriminant_mask,
        }
    }

    fn candidates<'group>(
        &'group self,
        config: RewriteConfig,
        expr: &'group Expr,
    ) -> CandidateRules<'group, 'a> {
        if !config.prefilter {
            return CandidateRules::Slice(self.rules.iter());
        }

        if self.has_non_discriminant_filters {
            let has_specific_match = self
                .rules
                .iter()
                .any(|rule_data| rule_matches_specific_prefilter(rule_data, expr));
            return CandidateRules::Filtered {
                iter: self.rules.iter(),
                expr,
                include_universal: !has_specific_match,
            };
        }

        let discriminant = discriminant_from_value(expr);
        CandidateRules::Slice(
            self.rules_by_discriminant
                .get(discriminant)
                .and_then(Option::as_deref)
                .unwrap_or(&self.universal_rules)
                .iter(),
        )
    }

    fn has_candidates(&self, config: RewriteConfig, expr: &Expr) -> bool {
        if !config.prefilter {
            return !self.rules.is_empty();
        }

        if self.has_non_discriminant_filters {
            return !self.universal_rules.is_empty()
                || self
                    .rules
                    .iter()
                    .any(|rule_data| rule_matches_specific_prefilter(rule_data, expr));
        }

        let discriminant = discriminant_from_value(expr);
        !self
            .rules_by_discriminant
            .get(discriminant)
            .and_then(Option::as_deref)
            .unwrap_or(&self.universal_rules)
            .is_empty()
    }
}

struct CandidateNodeIndex {
    preorder_discriminants: Vec<usize>,
    node_counts_by_discriminant: Vec<usize>,
}

impl CandidateNodeIndex {
    fn new(arena: &ExpressionArena, preorder_ids: &[ExpressionNodeId]) -> Self {
        let mut preorder_discriminants = Vec::with_capacity(preorder_ids.len());
        let mut node_counts_by_discriminant = Vec::new();

        for &node_id in preorder_ids {
            let discriminant = discriminant_from_value(arena.expression(node_id));
            preorder_discriminants.push(discriminant);
            if discriminant >= node_counts_by_discriminant.len() {
                node_counts_by_discriminant.resize(discriminant + 1, 0);
            }
            node_counts_by_discriminant[discriminant] += 1;
        }

        Self {
            preorder_discriminants,
            node_counts_by_discriminant,
        }
    }

    fn should_scan_position(&self, rule_group: &RuleGroup<'_>, preorder_position: usize) -> bool {
        if rule_group.target_discriminants.is_empty() {
            return true;
        }

        let discriminant = self.preorder_discriminants[preorder_position];
        rule_group
            .target_discriminant_mask
            .get(discriminant)
            .copied()
            .unwrap_or(false)
    }

    fn scan_count_for_rule_group(&self, rule_group: &RuleGroup<'_>, total_nodes: usize) -> usize {
        if rule_group.target_discriminants.is_empty() {
            return total_nodes;
        }

        rule_group
            .target_discriminants
            .iter()
            .filter_map(|discriminant| self.node_counts_by_discriminant.get(*discriminant))
            .sum()
    }
}

struct DirtyNodeQueues {
    nodes_by_level: Vec<Vec<(usize, ExpressionNodeId)>>,
}

impl DirtyNodeQueues {
    fn new(
        arena: &ExpressionArena,
        preorder_ids: &[ExpressionNodeId],
        rule_groups: &[RuleGroup<'_>],
    ) -> Option<Self> {
        if preorder_ids.is_empty() || rule_groups.is_empty() {
            return None;
        }

        let mut first_dirty_levels = Vec::with_capacity(preorder_ids.len());
        let mut queued_nodes = 0usize;
        for &node_id in preorder_ids {
            let clean_priority = arena.clean_rule_priority(node_id);
            let first_dirty_level =
                rule_groups.partition_point(|rule_group| rule_group.priority >= clean_priority);
            first_dirty_levels.push(first_dirty_level);
            queued_nodes += rule_groups.len() - first_dirty_level;
        }

        let full_scan_nodes = preorder_ids.len() * rule_groups.len();
        if queued_nodes >= full_scan_nodes {
            return None;
        }

        let mut nodes_by_level = vec![Vec::new(); rule_groups.len()];
        for (preorder_position, (&node_id, first_dirty_level)) in preorder_ids
            .iter()
            .zip(first_dirty_levels.iter().copied())
            .enumerate()
        {
            for nodes in &mut nodes_by_level[first_dirty_level..] {
                nodes.push((preorder_position, node_id));
            }
        }

        Some(Self { nodes_by_level })
    }

    fn nodes_for_level(&self, level: usize) -> &[(usize, ExpressionNodeId)] {
        &self.nodes_by_level[level]
    }
}

#[derive(Clone, Debug)]
enum RewriteSurfaceKind {
    Root,
    ValueLetting { name: Name },
}

struct RewriteSurface {
    kind: RewriteSurfaceKind,
    arena: ExpressionArena,
    active: bool,
}

impl RewriteSurface {
    fn root(arena: ExpressionArena) -> Self {
        Self {
            kind: RewriteSurfaceKind::Root,
            arena,
            active: true,
        }
    }

    fn value_letting(name: Name, expr: Expr) -> Self {
        Self {
            kind: RewriteSurfaceKind::ValueLetting { name },
            arena: ExpressionArena::from_root(expr),
            active: true,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct ScheduledNode {
    surface: usize,
    node_id: ExpressionNodeId,
    generation: u32,
    mode: ScheduledMode,
    depth: usize,
    sequence: u64,
}

impl PartialEq for ScheduledNode {
    fn eq(&self, other: &Self) -> bool {
        self.depth == other.depth && self.sequence == other.sequence
    }
}

impl Eq for ScheduledNode {}

impl PartialOrd for ScheduledNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ScheduledNode {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .depth
            .cmp(&self.depth)
            .then_with(|| other.sequence.cmp(&self.sequence))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct ScheduledKey {
    level: usize,
    surface: usize,
    node_id: ExpressionNodeId,
    generation: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct SubtreeCandidateKey {
    level: usize,
    surface: usize,
    node_id: ExpressionNodeId,
    generation: u32,
}

struct WorklistScheduler {
    // Candidate-level skipping can enqueue a descendant into a future rule level before an
    // ancestor reaches that same level. Each level is therefore ordered by depth, then insertion
    // sequence, so the semantic invariant "try an expression before its descendants at the same
    // priority" does not depend on enqueue timing.
    queues_by_level: Vec<BinaryHeap<ScheduledNode>>,
    scheduled: HashMap<ScheduledKey, ScheduledMode>,
    subtree_candidate_cache: HashMap<SubtreeCandidateKey, bool>,
    next_sequence: u64,
}

impl WorklistScheduler {
    fn empty(rule_groups: &[RuleGroup<'_>]) -> Self {
        Self {
            queues_by_level: vec![BinaryHeap::new(); rule_groups.len()],
            scheduled: HashMap::new(),
            subtree_candidate_cache: HashMap::new(),
            next_sequence: 0,
        }
    }

    fn new(
        surfaces: &[RewriteSurface],
        rule_groups: &[RuleGroup<'_>],
        config: RewriteConfig,
    ) -> Self {
        let mut scheduler = Self::empty(rule_groups);
        for surface in 0..surfaces.len() {
            scheduler.enqueue_surface(surfaces, surface, rule_groups, config, None);
        }
        scheduler
    }

    fn enqueue_surface(
        &mut self,
        surfaces: &[RewriteSurface],
        surface: usize,
        _rule_groups: &[RuleGroup<'_>],
        _config: RewriteConfig,
        mut dirty_trace: Option<&mut DirtyTrace>,
    ) {
        let Some(rewrite_surface) = surfaces.get(surface) else {
            return;
        };
        if !rewrite_surface.active {
            return;
        }

        self.enqueue_subtree(
            &rewrite_surface.arena,
            surface,
            rewrite_surface.arena.root(),
            dirty_trace.as_deref_mut(),
        );
    }

    fn enqueue_subtree(
        &mut self,
        arena: &ExpressionArena,
        surface: usize,
        node_id: ExpressionNodeId,
        dirty_trace: Option<&mut DirtyTrace>,
    ) {
        self.enqueue_node_at_level(
            arena,
            surface,
            node_id,
            0,
            ScheduledMode::TraverseSubtreeRoot,
            dirty_trace,
        );
    }

    fn enqueue_node_and_ancestors(
        &mut self,
        arena: &ExpressionArena,
        surface: usize,
        node_id: ExpressionNodeId,
        dirty_trace: &mut DirtyTrace,
    ) {
        let mut chain = Vec::new();
        let mut current = Some(node_id);
        while let Some(current_id) = current {
            chain.push(current_id);
            current = arena.parent(current_id);
        }

        for current_id in chain.into_iter().rev() {
            self.enqueue_node_at_level(
                arena,
                surface,
                current_id,
                0,
                ScheduledMode::CheckNode,
                Some(dirty_trace),
            );
        }
    }

    fn enqueue_children_at_level(
        &mut self,
        arena: &ExpressionArena,
        surface: usize,
        node_id: ExpressionNodeId,
        level: usize,
        rule_groups: &[RuleGroup<'_>],
        config: RewriteConfig,
        mut dirty_trace: Option<&mut DirtyTrace>,
    ) -> usize {
        if matches!(arena.expression(node_id), Expr::Comprehension(_, _)) {
            return 0;
        }

        let mut child_count = 0;
        for &child_id in arena.children(node_id) {
            if !self.subtree_has_candidates_at_level(
                arena,
                surface,
                child_id,
                level,
                rule_groups,
                config,
            ) {
                continue;
            }
            child_count += 1;
            self.enqueue_node_at_level(
                arena,
                surface,
                child_id,
                level,
                ScheduledMode::TraverseSubtreeDescendant,
                dirty_trace.as_deref_mut(),
            );
        }
        child_count
    }

    fn enqueue_node_at_level(
        &mut self,
        arena: &ExpressionArena,
        surface: usize,
        node_id: ExpressionNodeId,
        level: usize,
        mode: ScheduledMode,
        mut dirty_trace: Option<&mut DirtyTrace>,
    ) {
        if level >= self.queues_by_level.len() {
            return;
        }
        if !arena.is_reachable(node_id) {
            return;
        }

        let scheduled = ScheduledNode {
            surface,
            node_id,
            generation: arena.generation(node_id),
            mode,
            depth: arena.preorder_path(node_id).len(),
            sequence: self.next_sequence,
        };
        self.next_sequence += 1;
        let key = ScheduledKey {
            level,
            surface,
            node_id,
            generation: scheduled.generation,
        };
        match self.scheduled.entry(key) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(mode);
                self.queues_by_level[level].push(scheduled);
                if let Some(trace) = dirty_trace.as_deref_mut() {
                    trace.record_worklist_enqueue(mode);
                }
            }
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                if !entry.get().includes(mode) {
                    entry.insert(mode);
                    self.queues_by_level[level].push(scheduled);
                    if let Some(trace) = dirty_trace.as_deref_mut() {
                        trace.record_worklist_enqueue(mode);
                    }
                }
            }
        }
    }

    fn enqueue_after_no_rewrite(
        &mut self,
        arena: &ExpressionArena,
        surface: usize,
        node_id: ExpressionNodeId,
        level: usize,
        next_self_level: usize,
        mode: ScheduledMode,
        rule_groups: &[RuleGroup<'_>],
        config: RewriteConfig,
        mut dirty_trace: Option<&mut DirtyTrace>,
    ) {
        if mode.descends_on_failure() {
            let child_count = self.enqueue_children_at_level(
                arena,
                surface,
                node_id,
                level,
                rule_groups,
                config,
                dirty_trace.as_deref_mut(),
            );
            if let Some(trace) = dirty_trace.as_deref_mut() {
                trace.record_worklist_child_descent(mode, child_count);
            }
        }

        let Some(next_self_mode) = mode.next_self_mode() else {
            return;
        };
        // Descendants reached while traversing the current level do not eagerly advance to
        // future levels. The traversal root advances and rediscovers descendants only after it
        // survives that future level, avoiding stale descendant work when an ancestor rewrites.
        let next_self_level = if mode.advances_as_subtree() {
            self.next_subtree_candidate_level(
                arena,
                surface,
                node_id,
                next_self_level,
                rule_groups,
                config,
            )
        } else {
            next_worklist_candidate_level(arena, node_id, next_self_level, rule_groups, config)
        };
        self.enqueue_node_at_level(
            arena,
            surface,
            node_id,
            next_self_level,
            next_self_mode,
            dirty_trace,
        );
    }

    fn pop_next(
        &mut self,
        surfaces: &[RewriteSurface],
        rule_groups: &[RuleGroup<'_>],
        config: RewriteConfig,
        dirty_trace: &mut DirtyTrace,
    ) -> Option<(usize, usize, ExpressionNodeId, ScheduledMode)> {
        for level in 0..self.queues_by_level.len() {
            while let Some(scheduled) = self.queues_by_level[level].pop() {
                let key = ScheduledKey {
                    level,
                    surface: scheduled.surface,
                    node_id: scheduled.node_id,
                    generation: scheduled.generation,
                };
                if self.scheduled.get(&key).copied() != Some(scheduled.mode) {
                    dirty_trace.record_worklist_stale_pop(
                        scheduled.mode,
                        WorklistStaleReason::ModeMismatch,
                    );
                    continue;
                }
                self.scheduled.remove(&key);

                let Some(surface) = surfaces.get(scheduled.surface) else {
                    dirty_trace.record_worklist_stale_pop(
                        scheduled.mode,
                        WorklistStaleReason::MissingSurface,
                    );
                    continue;
                };
                if !surface.active {
                    dirty_trace.record_worklist_stale_pop(
                        scheduled.mode,
                        WorklistStaleReason::InactiveSurface,
                    );
                    continue;
                }
                let arena = &surface.arena;
                if !arena.is_reachable(scheduled.node_id) {
                    dirty_trace.record_worklist_stale_pop(
                        scheduled.mode,
                        WorklistStaleReason::UnreachableNode,
                    );
                    continue;
                }
                if arena.generation(scheduled.node_id) != scheduled.generation {
                    dirty_trace.record_worklist_stale_pop(
                        scheduled.mode,
                        WorklistStaleReason::GenerationMismatch,
                    );
                    if scheduled.mode.advances_as_subtree() {
                        // A descendant rewrite updates ancestor generations. Refreshing the
                        // traversal root preserves its job of carrying still-unseen sibling
                        // descendants to later rule levels.
                        let refresh_level = self.next_subtree_candidate_level(
                            arena,
                            scheduled.surface,
                            scheduled.node_id,
                            level,
                            rule_groups,
                            config,
                        );
                        self.enqueue_node_at_level(
                            arena,
                            scheduled.surface,
                            scheduled.node_id,
                            refresh_level,
                            scheduled.mode,
                            Some(dirty_trace),
                        );
                    }
                    continue;
                }

                dirty_trace.record_worklist_pop(scheduled.mode);
                return Some((level, scheduled.surface, scheduled.node_id, scheduled.mode));
            }
        }

        None
    }

    fn next_subtree_candidate_level(
        &mut self,
        arena: &ExpressionArena,
        surface: usize,
        node_id: ExpressionNodeId,
        start_level: usize,
        rule_groups: &[RuleGroup<'_>],
        config: RewriteConfig,
    ) -> usize {
        if start_level >= rule_groups.len() || !arena.is_reachable(node_id) {
            return rule_groups.len();
        }

        (start_level..rule_groups.len())
            .find(|&level| {
                self.subtree_has_candidates_at_level(
                    arena,
                    surface,
                    node_id,
                    level,
                    rule_groups,
                    config,
                )
            })
            .unwrap_or(rule_groups.len())
    }

    fn subtree_has_candidates_at_level(
        &mut self,
        arena: &ExpressionArena,
        surface: usize,
        node_id: ExpressionNodeId,
        level: usize,
        rule_groups: &[RuleGroup<'_>],
        config: RewriteConfig,
    ) -> bool {
        if level >= rule_groups.len() || !arena.is_reachable(node_id) {
            return false;
        }

        let key = SubtreeCandidateKey {
            level,
            surface,
            node_id,
            generation: arena.generation(node_id),
        };
        if let Some(&has_candidates) = self.subtree_candidate_cache.get(&key) {
            return has_candidates;
        }

        let rule_group = &rule_groups[level];
        let has_candidates = rule_group.has_candidates(config, arena.expression(node_id))
            || (!matches!(arena.expression(node_id), Expr::Comprehension(_, _))
                && arena.children(node_id).iter().any(|&child_id| {
                    self.subtree_has_candidates_at_level(
                        arena,
                        surface,
                        child_id,
                        level,
                        rule_groups,
                        config,
                    )
                }));

        self.subtree_candidate_cache.insert(key, has_candidates);
        has_candidates
    }
}

fn next_worklist_candidate_level(
    arena: &ExpressionArena,
    node_id: ExpressionNodeId,
    start_level: usize,
    rule_groups: &[RuleGroup<'_>],
    config: RewriteConfig,
) -> usize {
    if start_level >= rule_groups.len() || !arena.is_reachable(node_id) {
        return rule_groups.len();
    }

    let expr = arena.expression(node_id);
    rule_groups
        .iter()
        .enumerate()
        .skip(start_level)
        .find_map(|(level, rule_group)| rule_group.has_candidates(config, expr).then_some(level))
        .unwrap_or(rule_groups.len())
}

#[derive(Default)]
struct RuleApplicabilityMemo {
    failures: HashSet<(usize, usize, u64, u32, u32)>,
}

impl RuleApplicabilityMemo {
    fn rule_name_hash(rule_name: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        rule_name.hash(&mut hasher);
        hasher.finish()
    }

    fn is_known_failure(
        &self,
        surface: usize,
        node_id: ExpressionNodeId,
        rule_name: &str,
        node_generation: u32,
        symbol_generation: u32,
    ) -> bool {
        self.failures.contains(&(
            surface,
            node_id.index(),
            Self::rule_name_hash(rule_name),
            node_generation,
            symbol_generation,
        ))
    }

    fn record_failure(
        &mut self,
        surface: usize,
        node_id: ExpressionNodeId,
        rule_name: &str,
        node_generation: u32,
        symbol_generation: u32,
    ) {
        self.failures.insert((
            surface,
            node_id.index(),
            Self::rule_name_hash(rule_name),
            node_generation,
            symbol_generation,
        ));
    }

    fn clear(&mut self) {
        self.failures.clear();
    }
}

struct RuleEffectImpact {
    added_names: Vec<Name>,
    changed_names: Vec<Name>,
    has_new_top: bool,
    has_new_clauses: bool,
}

impl RuleEffectImpact {
    fn new(
        effect: &crate::rule_engine::rule::RuleEffect,
        symbols: &crate::ast::SymbolTable,
    ) -> Self {
        Self {
            added_names: effect.added_symbols(symbols).into_iter().collect(),
            changed_names: effect
                .changed_symbols(symbols)
                .into_iter()
                .map(|(name, _, _)| name)
                .collect(),
            has_new_top: !effect.new_top.is_empty(),
            has_new_clauses: !effect.new_clauses.is_empty(),
        }
    }

    fn has_model_side_effects(&self) -> bool {
        self.has_new_top
            || self.has_new_clauses
            || !self.added_names.is_empty()
            || !self.changed_names.is_empty()
    }

    fn changes_symbol_context(&self) -> bool {
        !self.added_names.is_empty() || !self.changed_names.is_empty()
    }

    fn requires_arena_reimport_for_invalidation(&self) -> bool {
        self.has_new_clauses || !self.changed_names.is_empty()
    }
}

struct RewritePassContext<'ctx, 'rules> {
    rules_grouped: &'ctx Vec<(u16, Vec<RuleData<'rules>>)>,
    bucketed_rules: &'ctx Vec<RuleGroup<'rules>>,
    prop_multiple_equally_applicable: bool,
    stats: &'ctx mut RewriterStats,
    dirty_trace: &'ctx mut DirtyTrace,
    cache: Option<RewriteCache>,
    symbol_context_hash: Option<u64>,
    symbol_generation: u32,
    rule_applicability_memo: Option<RuleApplicabilityMemo>,
    config: RewriteConfig,
    #[cfg(debug_assertions)]
    run_start: &'ctx Instant,
}

/// Rewrites a model by applying rules in priority order, trying enclosing expressions before their
/// descendants at each priority.
pub fn rewrite_model<'a>(
    model: &Model,
    rule_sets: &Vec<&'a RuleSet<'a>>,
    prop_multiple_equally_applicable: bool,
    config: RewriteConfig,
) -> Result<Model, RewriteError> {
    set_current_rewriter(Rewriter::Rewrite(config));

    let rules_grouped = get_rules_grouped(rule_sets)
        .unwrap_or_else(|_| bug!("get_rule_priorities() failed!"))
        .into_iter()
        .collect_vec();
    let bucketed_rules = rules_grouped
        .iter()
        .map(|(priority, rules)| RuleGroup::new(*priority, rules.clone()))
        .collect_vec();

    let mut model = introduce_objective_auxiliary(model.clone());
    let mut rewriter_stats = RewriterStats::new();
    rewriter_stats.is_optimisation_enabled = Some(!config.is_baseline());
    let mut dirty_trace = DirtyTrace::from_env();
    let run_start = Instant::now();

    if rule_trace_enabled() && default_rule_trace_enabled() {
        trace!(
            target: "rule_engine_rule_trace",
            "Model before rewriting:\n\n{}\n--\n",
            model
        );
    }
    if rule_trace_enabled() && rule_trace_verbose_enabled() {
        trace!(
            target: "rule_engine_rule_trace_verbose",
            "elapsed_s,rule_level,rule_name,rule_set,status,expression"
        );
    }

    // Rewrite until there are no more rules left to apply.
    {
        let mut pass_ctx = RewritePassContext {
            rules_grouped: &rules_grouped,
            bucketed_rules: &bucketed_rules,
            prop_multiple_equally_applicable,
            stats: &mut rewriter_stats,
            dirty_trace: &mut dirty_trace,
            cache: config.cache.then(RewriteCache::default),
            symbol_context_hash: None,
            symbol_generation: 0,
            rule_applicability_memo: config.rule_memo.then(RuleApplicabilityMemo::default),
            config,
            #[cfg(debug_assertions)]
            run_start: &run_start,
        };
        if config.worklist {
            let _ = try_rewrite_model(&mut model, &mut pass_ctx);
        } else {
            let mut done_something = true;
            while done_something {
                done_something = try_rewrite_model(&mut model, &mut pass_ctx).is_some();
            }
        }
    }

    let run_end = Instant::now();
    rewriter_stats.rewriter_run_time = Some(run_end - run_start);

    model
        .context
        .write()
        .unwrap()
        .stats
        .add_rewriter_run(rewriter_stats);
    dirty_trace.finish(
        model
            .context
            .read()
            .unwrap()
            .stats
            .rewriter_runs
            .last()
            .expect("rewriter stats were just added"),
    );

    if rule_trace_enabled() && default_rule_trace_enabled() {
        trace!(
            target: "rule_engine_rule_trace",
            "Final model:\n\n{}",
            model
        );
    }
    Ok(model)
}

// Tries to rewrite the model until a full scan finds no applicable rules.
//
// Returns None if no change was made.
fn try_rewrite_model<'ctx, 'rules>(
    submodel: &mut Model,
    ctx: &mut RewritePassContext<'ctx, 'rules>,
) -> Option<()> {
    ctx.dirty_trace.passes += 1;
    if !ctx.config.worklist {
        if let Some(letting_name) = try_rewrite_value_letting_once(
            submodel,
            ctx.rules_grouped,
            ctx.prop_multiple_equally_applicable,
        ) {
            ctx.dirty_trace.value_letting_rewrites += 1;
            increment_counter(&mut ctx.stats.rewriter_value_letting_rewrites);
            invalidate_symbol_context_caches(submodel, ctx);
            if ctx.config.dirty {
                ctx.dirty_trace.whole_model_clears_after_value_letting += 1;
                clear_clean_rule_metadata_for_name(submodel, &letting_name);
            }
            return Some(());
        }
    }

    let mut did_rewrite = false;
    let mut arena = ExpressionArena::from_root(take_model_root(submodel));
    normalise_evaluators_bottom_up(&mut arena, ctx.dirty_trace);

    if ctx.config.worklist {
        return try_rewrite_model_with_worklist(submodel, ctx, arena);
    }

    'rewrite_loop: loop {
        let mut results: Vec<ApplicableRule<'_, ExpressionNodeId>> = vec![];
        let preorder_ids = rewriter_preorder_ids(&arena);
        let candidate_node_index = candidate_node_index_enabled(ctx.config)
            .then(|| CandidateNodeIndex::new(&arena, &preorder_ids));
        let dirty_node_queues = dirty_node_queues_enabled(ctx.config)
            .then(|| DirtyNodeQueues::new(&arena, &preorder_ids, ctx.bucketed_rules))
            .flatten();

        // Iterate over rules by priority in descending order.
        'top: for (level, rule_group) in ctx.bucketed_rules.iter().enumerate() {
            ctx.dirty_trace.priority_scans += 1;
            let full_scan_count = dirty_node_queues
                .as_ref()
                .map(|queues| queues.nodes_for_level(level).len())
                .unwrap_or(preorder_ids.len());
            let candidate_scan_count = candidate_node_index
                .as_ref()
                .map(|index| index.scan_count_for_rule_group(rule_group, full_scan_count))
                .unwrap_or(full_scan_count);
            ctx.dirty_trace
                .record_candidate_index_scan(full_scan_count, candidate_scan_count);
            let scan_symbol_context_hash = ctx
                .cache
                .is_some()
                .then(|| current_symbol_context_hash(submodel, ctx));
            let queued_nodes = dirty_node_queues
                .as_ref()
                .map(|queues| queues.nodes_for_level(level));
            let node_scan = queued_nodes.into_iter().flatten().copied().chain(
                dirty_node_queues
                    .is_none()
                    .then(|| preorder_ids.iter().copied().enumerate())
                    .into_iter()
                    .flatten(),
            );
            for (preorder_position, node_id) in node_scan {
                if let Some(index) = candidate_node_index.as_ref()
                    && !index.should_scan_position(rule_group, preorder_position)
                {
                    continue;
                }

                ctx.dirty_trace.expression_visits += 1;
                if ctx.config.dirty
                    && arena.is_clean_for_rule_priority(node_id, rule_group.priority)
                {
                    ctx.dirty_trace.record_dirty_hit(rule_group.priority);
                    continue;
                }

                let mut node_content_hash = None;
                if let Some(symbol_context_hash) = scan_symbol_context_hash {
                    let cache_result = {
                        let expression_content_hash = *node_content_hash.get_or_insert_with(|| {
                            traced_arena_content_hash(&mut arena, node_id, ctx.dirty_trace)
                        });
                        let cache = ctx.cache.as_mut().expect("checked above");
                        cache.get_from_hash(expression_content_hash, level, symbol_context_hash)
                    };
                    match cache_result {
                        CacheResult::Terminal(clean_level) => {
                            ctx.dirty_trace.cache_hits += 1;
                            ctx.dirty_trace.cache_terminal_hits += 1;
                            trace!(target: "rule_engine", clean_level, "Rewrite cache terminal hit");
                            if ctx.config.dirty {
                                arena.mark_clean_for_rule_priority(node_id, rule_group.priority);
                            }
                            continue;
                        }
                        CacheResult::Rewrite(cached) => {
                            ctx.dirty_trace.cache_hits += 1;
                            ctx.dirty_trace.cache_rewrite_hits += 1;
                            let mappings = replace_focus_and_dirty_ancestors(
                                &mut arena,
                                node_id,
                                cached.expr,
                                ctx.dirty_trace,
                                Some(symbol_context_hash),
                            );
                            let (_, evaluator_changed) = normalise_evaluators_from_node_to_root(
                                &mut arena,
                                node_id,
                                ctx.dirty_trace,
                            );
                            let cache = ctx.cache.as_mut().expect("cache enabled");
                            // TODO: if cache becomes part of the optimised profile again, preserve
                            // ancestor mappings through evaluator normalisation instead of dropping
                            // this evidence when the hook changes an ancestor.
                            if !evaluator_changed {
                                let mapping_count = mappings.len();
                                insert_ancestor_mappings(
                                    cache,
                                    mappings,
                                    level,
                                    symbol_context_hash,
                                );
                                ctx.dirty_trace.cache_ancestor_mappings += mapping_count;
                            }
                            did_rewrite = true;
                            continue 'rewrite_loop;
                        }
                        CacheResult::Unknown => {
                            ctx.dirty_trace.cache_misses += 1;
                        }
                    }
                }

                let mut attempted_rule = false;
                let results_before_expr = results.len();
                let node_generation = arena.generation(node_id);
                let symbol_generation = ctx.symbol_generation;
                {
                    let expr = arena.expression(node_id);
                    for rd in rule_group.candidates(ctx.config, expr) {
                        attempted_rule = true;
                        if ctx.config.rule_memo
                            && ctx.rule_applicability_memo.as_ref().is_some_and(|memo| {
                                memo.is_known_failure(
                                    0,
                                    node_id,
                                    rd.rule.name,
                                    node_generation,
                                    symbol_generation,
                                )
                            })
                        {
                            ctx.dirty_trace.record_rule_memo_hit();
                            continue;
                        }

                        ctx.dirty_trace
                            .record_rule_attempt(rule_group.priority, rd.rule.name);
                        // Count rule application attempts
                        ctx.stats.rewriter_rule_application_attempts =
                            Some(ctx.stats.rewriter_rule_application_attempts.unwrap_or(0) + 1);

                        #[cfg(debug_assertions)]
                        let span = span!(Level::TRACE,"trying_rule_application",rule_name=rd.rule.name,rule_target_expression=%expr);

                        #[cfg(debug_assertions)]
                        let _guard = span.enter();

                        #[cfg(debug_assertions)]
                        tracing::trace!(rule_name = rd.rule.name, "Trying rule");

                        let variable_snapshot_before = matches!(expr, Expr::Root(_, _))
                            .then(|| snapshot_variable_declarations(&submodel.symbols()));

                        match (rd.rule.application)(expr, &submodel.symbols()) {
                            Ok(red) => {
                                // when called a lot, this becomes very expensive!
                                #[cfg(debug_assertions)]
                                if rule_trace_enabled() && rule_trace_verbose_enabled() {
                                    log_verbose_rule_attempt(
                                        ctx.run_start,
                                        &rule_group.priority,
                                        rd.rule.name,
                                        rd.rule_set.name,
                                        "success",
                                        expr,
                                    );
                                }

                                // Count successful rule applications
                                ctx.stats.rewriter_rule_applications =
                                    Some(ctx.stats.rewriter_rule_applications.unwrap_or(0) + 1);

                                // Collect applicable rules
                                results.push((
                                    RuleResult {
                                        rule_data: rd.clone(),
                                        effect: red,
                                    },
                                    level,
                                    expr.clone(),
                                    node_id,
                                    variable_snapshot_before,
                                ));
                            }
                            Err(_) => {
                                if ctx.config.rule_memo
                                    && let Some(memo) = ctx.rule_applicability_memo.as_mut()
                                {
                                    memo.record_failure(
                                        0,
                                        node_id,
                                        rd.rule.name,
                                        node_generation,
                                        symbol_generation,
                                    );
                                }
                                // when called a lot, this becomes very expensive!
                                #[cfg(debug_assertions)]
                                if rule_trace_enabled() && rule_trace_verbose_enabled() {
                                    log_verbose_rule_attempt(
                                        ctx.run_start,
                                        &rule_group.priority,
                                        rd.rule.name,
                                        rd.rule_set.name,
                                        "fail",
                                        expr,
                                    );
                                }
                            }
                        }
                    }
                    if ctx.config.cache
                        && results.len() == results_before_expr
                        && let Some(symbol_context_hash) = scan_symbol_context_hash
                        && let Some(cache) = ctx.cache.as_mut()
                    {
                        let hash = node_content_hash
                            .expect("cache lookup computed the arena node content hash");
                        cache.insert_from_hash(hash, None, level, symbol_context_hash);
                        ctx.dirty_trace.cache_inserts += 1;
                    }
                }
                if ctx.config.dirty && attempted_rule && results.len() == results_before_expr {
                    ctx.dirty_trace.record_clean_mark(rule_group.priority);
                    arena.mark_clean_for_rule_priority(node_id, rule_group.priority);
                }
                if attempted_rule {
                    ctx.dirty_trace.attempted_expressions += 1;
                }
                // This expression has the highest rule priority so far, so this is what we want to
                // rewrite.
                if !results.is_empty() {
                    break 'top;
                }
            }
        }

        match results.as_slice() {
            [] => {
                submodel.replace_root(arena.into_root_expression());
                break;
            }
            [(result, level, expr, node_id, variable_snapshot_before), ..] => {
                if ctx.prop_multiple_equally_applicable {
                    assert_no_multiple_equally_applicable_rules(&results, ctx.rules_grouped);
                }

                let effect = result.effect.materialise(&submodel.symbols());
                let variable_snapshots = variable_snapshot_before.clone().map(|before| {
                    let after = snapshot_symbols_after_effect(&submodel.symbols(), &effect.symbols);
                    (before, after)
                });
                let result = RuleResult {
                    rule_data: result.rule_data.clone(),
                    effect,
                };

                // Extract the single applicable rule and apply it
                log_rule_application(
                    &result,
                    expr,
                    &submodel.symbols(),
                    variable_snapshots
                        .as_ref()
                        .map(|(before, after)| (before, after)),
                );

                let effect_impact = RuleEffectImpact::new(&result.effect, &submodel.symbols());
                let has_model_side_effects = effect_impact.has_model_side_effects();
                let changes_symbol_context = effect_impact.changes_symbol_context();
                let rule_name = result.rule_data.rule.name;
                let RuleResult { effect, .. } = result;
                let crate::rule_engine::rule::RuleEffect {
                    new_expression,
                    new_top,
                    symbols,
                    new_clauses,
                    ..
                } = effect;
                let replacement = clear_expr_clean_rule_metadata(new_expression);
                let pre_effect_symbol_context_hash = ctx
                    .cache
                    .is_some()
                    .then(|| current_symbol_context_hash(submodel, ctx));

                // Replace expr with new_expression
                let cache_mapping_context = ctx
                    .config
                    .cache
                    .then_some(pre_effect_symbol_context_hash)
                    .flatten();
                let mappings = replace_focus_and_dirty_ancestors(
                    &mut arena,
                    *node_id,
                    replacement.clone(),
                    ctx.dirty_trace,
                    cache_mapping_context,
                );

                // Apply new symbols and top level
                ctx.dirty_trace
                    .record_rewrite(rule_name, has_model_side_effects);
                submodel.symbols_mut().extend(symbols);
                if effect_impact.has_new_top {
                    arena.add_root_children(new_top);
                }
                submodel.add_clauses(new_clauses);
                if changes_symbol_context {
                    invalidate_symbol_context_caches(submodel, ctx);
                }
                let (_, evaluator_changed) =
                    normalise_evaluators_from_node_to_root(&mut arena, *node_id, ctx.dirty_trace);
                if let Some(pre_effect_symbol_context_hash) = pre_effect_symbol_context_hash {
                    let cache_symbol_context_hash = if changes_symbol_context {
                        current_symbol_context_hash(submodel, ctx)
                    } else {
                        pre_effect_symbol_context_hash
                    };
                    let expr_hash =
                        RewriteCache::expression_content_hash(expr, cache_symbol_context_hash);
                    if let Some(cache) = ctx.cache.as_mut() {
                        if arena.is_reachable(*node_id) {
                            cache.insert_from_hash(
                                expr_hash,
                                Some(arena.expression(*node_id).clone()),
                                *level,
                                cache_symbol_context_hash,
                            );
                            ctx.dirty_trace.cache_inserts += 1;
                        }
                        if !evaluator_changed {
                            // TODO: thread old ancestor hashes through evaluator normalisation so
                            // cache can keep these mappings even when the hook changes an ancestor.
                            let mapping_count = mappings.len();
                            insert_ancestor_mappings(
                                cache,
                                mappings,
                                *level,
                                cache_symbol_context_hash,
                            );
                            ctx.dirty_trace.cache_ancestor_mappings += mapping_count;
                        }
                    }
                }
                if effect_impact.requires_arena_reimport_for_invalidation()
                    && (ctx.config.dirty || ctx.config.cache)
                {
                    ctx.dirty_trace.record_side_effect_arena_reimport();
                    submodel.replace_root(arena.into_root_expression());
                    let mut targeted = false;
                    if !effect_impact.changed_names.is_empty() {
                        clear_clean_rule_metadata_for_names(submodel, &effect_impact.changed_names);
                        targeted = true;
                    }
                    if effect_impact.has_new_top {
                        clear_root_clean_rule_metadata(submodel);
                        targeted = true;
                    }
                    if !targeted {
                        ctx.dirty_trace.record_whole_model_clear(rule_name);
                        clear_model_clean_rule_metadata(submodel);
                    }
                    arena = ExpressionArena::from_root(take_model_root(submodel));
                    reset_rule_applicability_memo(ctx);
                } else if has_model_side_effects {
                    ctx.dirty_trace.record_side_effect_kept_in_arena();
                }

                #[cfg(debug_assertions)]
                {
                    submodel.replace_root(arena.clone().into_root_expression());
                    let assertion_context = format!("rewriter after applying rule '{rule_name}'");
                    debug_assert_model_well_formed(submodel, &assertion_context);
                    arena = ExpressionArena::from_root(take_model_root(submodel));
                    reset_rule_applicability_memo(ctx);
                }

                did_rewrite = true;
                continue 'rewrite_loop;
            }
        }
    }

    did_rewrite.then_some(())
}

fn try_rewrite_model_with_worklist<'ctx, 'rules>(
    submodel: &mut Model,
    ctx: &mut RewritePassContext<'ctx, 'rules>,
    arena: ExpressionArena,
) -> Option<()> {
    let mut did_rewrite = false;
    let root_surface = 0usize;
    let (mut surfaces, mut value_letting_surfaces) = build_worklist_surfaces(submodel, arena);
    for surface in &mut surfaces {
        if surface.active {
            normalise_evaluators_bottom_up(&mut surface.arena, ctx.dirty_trace);
        }
    }
    write_worklist_surfaces_to_model(submodel, &surfaces);
    let mut scheduler = WorklistScheduler::new(&surfaces, ctx.bucketed_rules, ctx.config);

    while let Some((level, surface_index, node_id, scheduled_mode)) =
        scheduler.pop_next(&surfaces, ctx.bucketed_rules, ctx.config, ctx.dirty_trace)
    {
        ctx.dirty_trace.priority_scans += 1;
        ctx.dirty_trace.expression_visits += 1;

        let rule_group = &ctx.bucketed_rules[level];
        if ctx.config.dirty
            && surfaces[surface_index]
                .arena
                .is_clean_for_rule_priority(node_id, rule_group.priority)
        {
            ctx.dirty_trace.record_dirty_hit(rule_group.priority);
            scheduler.enqueue_after_no_rewrite(
                &surfaces[surface_index].arena,
                surface_index,
                node_id,
                level,
                level + 1,
                scheduled_mode,
                ctx.bucketed_rules,
                ctx.config,
                Some(ctx.dirty_trace),
            );
            continue;
        }

        if !rule_group.has_candidates(
            ctx.config,
            surfaces[surface_index].arena.expression(node_id),
        ) {
            ctx.dirty_trace
                .record_worklist_no_candidate_pop(scheduled_mode);
            scheduler.enqueue_after_no_rewrite(
                &surfaces[surface_index].arena,
                surface_index,
                node_id,
                level,
                level + 1,
                scheduled_mode,
                ctx.bucketed_rules,
                ctx.config,
                Some(ctx.dirty_trace),
            );
            continue;
        }

        let mut node_content_hash = None;
        let scan_symbol_context_hash = ctx
            .cache
            .is_some()
            .then(|| current_symbol_context_hash(submodel, ctx));

        if let Some(symbol_context_hash) = scan_symbol_context_hash {
            let cache_result = {
                let expression_content_hash = *node_content_hash.get_or_insert_with(|| {
                    traced_arena_content_hash(
                        &mut surfaces[surface_index].arena,
                        node_id,
                        ctx.dirty_trace,
                    )
                });
                let cache = ctx.cache.as_mut().expect("checked above");
                cache.get_from_hash(expression_content_hash, level, symbol_context_hash)
            };
            match cache_result {
                CacheResult::Terminal(clean_level) => {
                    ctx.dirty_trace.cache_hits += 1;
                    ctx.dirty_trace.cache_terminal_hits += 1;
                    trace!(target: "rule_engine", clean_level, "Rewrite cache terminal hit");
                    if ctx.config.dirty {
                        surfaces[surface_index]
                            .arena
                            .mark_clean_for_rule_priority(node_id, rule_group.priority);
                    }
                    scheduler.enqueue_after_no_rewrite(
                        &surfaces[surface_index].arena,
                        surface_index,
                        node_id,
                        level,
                        clean_level + 1,
                        scheduled_mode,
                        ctx.bucketed_rules,
                        ctx.config,
                        Some(ctx.dirty_trace),
                    );
                    continue;
                }
                CacheResult::Rewrite(cached) => {
                    ctx.dirty_trace.cache_hits += 1;
                    ctx.dirty_trace.cache_rewrite_hits += 1;
                    let rewritten_value_letting_name =
                        value_letting_surface_name(&surfaces[surface_index].kind).cloned();
                    let mappings = {
                        let arena = &mut surfaces[surface_index].arena;
                        replace_focus_and_dirty_ancestors(
                            arena,
                            node_id,
                            cached.expr,
                            ctx.dirty_trace,
                            Some(symbol_context_hash),
                        )
                    };
                    let (rewrite_impact_node_id, evaluator_changed) = {
                        let arena = &mut surfaces[surface_index].arena;
                        normalise_evaluators_from_node_to_root(arena, node_id, ctx.dirty_trace)
                    };
                    let cache = ctx.cache.as_mut().expect("cache enabled");
                    // TODO: if cache becomes part of the optimised profile again, preserve
                    // ancestor mappings through evaluator normalisation instead of dropping
                    // this evidence when the hook changes an ancestor.
                    if !evaluator_changed {
                        let mapping_count = mappings.len();
                        insert_ancestor_mappings(cache, mappings, level, symbol_context_hash);
                        ctx.dirty_trace.cache_ancestor_mappings += mapping_count;
                    }
                    enqueue_worklist_rewrite_impact(
                        &mut scheduler,
                        &surfaces[surface_index].arena,
                        surface_index,
                        rewrite_impact_node_id,
                        ctx.dirty_trace,
                    );
                    if let Some(name) = rewritten_value_letting_name {
                        write_value_letting_surface_to_model(
                            submodel,
                            &name,
                            &surfaces[surface_index].arena,
                        );
                        ctx.dirty_trace.value_letting_rewrites += 1;
                        increment_counter(&mut ctx.stats.rewriter_value_letting_rewrites);
                        invalidate_symbol_context_caches(submodel, ctx);
                        enqueue_worklist_nodes_referencing_names(
                            &mut scheduler,
                            &surfaces,
                            std::slice::from_ref(&name),
                            ctx.dirty_trace,
                        );
                    }
                    did_rewrite = true;
                    continue;
                }
                CacheResult::Unknown => {
                    ctx.dirty_trace.cache_misses += 1;
                }
            }
        }

        let mut results: Vec<ApplicableRule<'_, ExpressionNodeId>> = vec![];
        let mut attempted_rule = false;
        let mut actual_rule_attempted = false;
        let node_generation = surfaces[surface_index].arena.generation(node_id);
        let symbol_generation = ctx.symbol_generation;
        {
            let expr = surfaces[surface_index].arena.expression(node_id);
            for rd in rule_group.candidates(ctx.config, expr) {
                attempted_rule = true;
                if ctx.config.rule_memo
                    && ctx.rule_applicability_memo.as_ref().is_some_and(|memo| {
                        memo.is_known_failure(
                            surface_index,
                            node_id,
                            rd.rule.name,
                            node_generation,
                            symbol_generation,
                        )
                    })
                {
                    ctx.dirty_trace.record_rule_memo_hit();
                    continue;
                }

                actual_rule_attempted = true;
                ctx.dirty_trace
                    .record_rule_attempt(rule_group.priority, rd.rule.name);
                ctx.stats.rewriter_rule_application_attempts =
                    Some(ctx.stats.rewriter_rule_application_attempts.unwrap_or(0) + 1);

                #[cfg(debug_assertions)]
                let span = span!(Level::TRACE,"trying_rule_application",rule_name=rd.rule.name,rule_target_expression=%expr);

                #[cfg(debug_assertions)]
                let _guard = span.enter();

                #[cfg(debug_assertions)]
                tracing::trace!(rule_name = rd.rule.name, "Trying rule");

                let variable_snapshot_before = matches!(expr, Expr::Root(_, _))
                    .then(|| snapshot_variable_declarations(&submodel.symbols()));

                match (rd.rule.application)(expr, &submodel.symbols()) {
                    Ok(red) => {
                        #[cfg(debug_assertions)]
                        if rule_trace_enabled() && rule_trace_verbose_enabled() {
                            log_verbose_rule_attempt(
                                ctx.run_start,
                                &rule_group.priority,
                                rd.rule.name,
                                rd.rule_set.name,
                                "success",
                                expr,
                            );
                        }

                        ctx.stats.rewriter_rule_applications =
                            Some(ctx.stats.rewriter_rule_applications.unwrap_or(0) + 1);

                        results.push((
                            RuleResult {
                                rule_data: rd.clone(),
                                effect: red,
                            },
                            level,
                            expr.clone(),
                            node_id,
                            variable_snapshot_before,
                        ));
                    }
                    Err(_) => {
                        if ctx.config.rule_memo
                            && let Some(memo) = ctx.rule_applicability_memo.as_mut()
                        {
                            memo.record_failure(
                                surface_index,
                                node_id,
                                rd.rule.name,
                                node_generation,
                                symbol_generation,
                            );
                        }
                        #[cfg(debug_assertions)]
                        if rule_trace_enabled() && rule_trace_verbose_enabled() {
                            log_verbose_rule_attempt(
                                ctx.run_start,
                                &rule_group.priority,
                                rd.rule.name,
                                rd.rule_set.name,
                                "fail",
                                expr,
                            );
                        }
                    }
                }
            }

            if ctx.config.cache
                && results.is_empty()
                && let Some(symbol_context_hash) = scan_symbol_context_hash
                && let Some(cache) = ctx.cache.as_mut()
            {
                let hash =
                    node_content_hash.expect("cache lookup computed the arena node content hash");
                cache.insert_from_hash(hash, None, level, symbol_context_hash);
                ctx.dirty_trace.cache_inserts += 1;
            }
        }

        if ctx.config.dirty && attempted_rule && results.is_empty() {
            ctx.dirty_trace.record_clean_mark(rule_group.priority);
            surfaces[surface_index]
                .arena
                .mark_clean_for_rule_priority(node_id, rule_group.priority);
        }
        if attempted_rule {
            ctx.dirty_trace.attempted_expressions += 1;
        }
        if actual_rule_attempted {
            ctx.dirty_trace
                .record_worklist_rule_attempt_pop(scheduled_mode);
        }

        if results.is_empty() {
            scheduler.enqueue_after_no_rewrite(
                &surfaces[surface_index].arena,
                surface_index,
                node_id,
                level,
                level + 1,
                scheduled_mode,
                ctx.bucketed_rules,
                ctx.config,
                Some(ctx.dirty_trace),
            );
            continue;
        }

        if ctx.prop_multiple_equally_applicable {
            assert_no_multiple_equally_applicable_rules(&results, ctx.rules_grouped);
        }

        let [(result, level, expr, node_id, variable_snapshot_before), ..] = results.as_slice()
        else {
            unreachable!("checked non-empty results above")
        };

        let effect = result.effect.materialise(&submodel.symbols());
        let variable_snapshots = variable_snapshot_before.clone().map(|before| {
            let after = snapshot_symbols_after_effect(&submodel.symbols(), &effect.symbols);
            (before, after)
        });
        let result = RuleResult {
            rule_data: result.rule_data.clone(),
            effect,
        };

        log_rule_application(
            &result,
            expr,
            &submodel.symbols(),
            variable_snapshots
                .as_ref()
                .map(|(before, after)| (before, after)),
        );

        let effect_impact = RuleEffectImpact::new(&result.effect, &submodel.symbols());
        let has_model_side_effects = effect_impact.has_model_side_effects();
        let rewritten_value_letting_name =
            value_letting_surface_name(&surfaces[surface_index].kind).cloned();
        let value_letting_rewrite = rewritten_value_letting_name.is_some();
        let changes_symbol_context =
            effect_impact.changes_symbol_context() || value_letting_rewrite;
        let rule_name = result.rule_data.rule.name;
        let RuleResult { effect, .. } = result;
        let crate::rule_engine::rule::RuleEffect {
            new_expression,
            new_top,
            symbols,
            new_clauses,
            ..
        } = effect;
        let replacement = clear_expr_clean_rule_metadata(new_expression);
        let pre_effect_symbol_context_hash = ctx
            .cache
            .is_some()
            .then(|| current_symbol_context_hash(submodel, ctx));

        let cache_mapping_context = ctx
            .config
            .cache
            .then_some(pre_effect_symbol_context_hash)
            .flatten();
        let mappings = {
            let arena = &mut surfaces[surface_index].arena;
            replace_focus_and_dirty_ancestors(
                arena,
                *node_id,
                replacement.clone(),
                ctx.dirty_trace,
                cache_mapping_context,
            )
        };

        ctx.dirty_trace
            .record_rewrite(rule_name, has_model_side_effects);
        submodel.symbols_mut().extend(symbols);
        let new_top_node_ids = if effect_impact.has_new_top {
            surfaces[root_surface].arena.add_root_children(new_top)
        } else {
            Vec::new()
        };
        submodel.add_clauses(new_clauses);
        let (rewrite_impact_node_id, evaluator_changed) = {
            let arena = &mut surfaces[surface_index].arena;
            normalise_evaluators_from_node_to_root(arena, *node_id, ctx.dirty_trace)
        };
        for &new_top_node_id in &new_top_node_ids {
            if surfaces[root_surface].arena.is_reachable(new_top_node_id) {
                normalise_evaluators_from_node_to_root(
                    &mut surfaces[root_surface].arena,
                    new_top_node_id,
                    ctx.dirty_trace,
                );
            }
        }
        if let Some(name) = rewritten_value_letting_name.as_ref() {
            write_value_letting_surface_to_model(submodel, name, &surfaces[surface_index].arena);
            ctx.dirty_trace.value_letting_rewrites += 1;
            increment_counter(&mut ctx.stats.rewriter_value_letting_rewrites);
        }
        let mut affected_names = effect_impact.changed_names.clone();
        if let Some(name) = rewritten_value_letting_name.as_ref()
            && !affected_names.contains(name)
        {
            affected_names.push(name.clone());
        }
        let mut symbol_surface_names = effect_impact.added_names.clone();
        symbol_surface_names.extend(effect_impact.changed_names.iter().cloned());
        if let Some(name) = rewritten_value_letting_name.as_ref() {
            symbol_surface_names.retain(|candidate| candidate != name);
        }
        let synced_surfaces = sync_value_letting_surfaces(
            submodel,
            &mut surfaces,
            &mut value_letting_surfaces,
            &symbol_surface_names,
        );
        if changes_symbol_context {
            invalidate_symbol_context_caches(submodel, ctx);
        }
        if let Some(pre_effect_symbol_context_hash) = pre_effect_symbol_context_hash {
            let cache_symbol_context_hash = if changes_symbol_context {
                current_symbol_context_hash(submodel, ctx)
            } else {
                pre_effect_symbol_context_hash
            };
            let expr_hash = RewriteCache::expression_content_hash(expr, cache_symbol_context_hash);
            if let Some(cache) = ctx.cache.as_mut() {
                if surfaces[surface_index].arena.is_reachable(*node_id) {
                    cache.insert_from_hash(
                        expr_hash,
                        Some(surfaces[surface_index].arena.expression(*node_id).clone()),
                        *level,
                        cache_symbol_context_hash,
                    );
                    ctx.dirty_trace.cache_inserts += 1;
                }
                if !evaluator_changed {
                    // TODO: thread old ancestor hashes through evaluator normalisation so cache
                    // can keep these mappings even when the hook changes an ancestor.
                    let mapping_count = mappings.len();
                    insert_ancestor_mappings(cache, mappings, *level, cache_symbol_context_hash);
                    ctx.dirty_trace.cache_ancestor_mappings += mapping_count;
                }
            }
        }
        if effect_impact.requires_arena_reimport_for_invalidation()
            && (ctx.config.dirty || ctx.config.cache)
        {
            ctx.dirty_trace.record_side_effect_arena_reimport();
            write_worklist_surfaces_to_model(submodel, &surfaces);
            let mut targeted = false;
            if !effect_impact.changed_names.is_empty() {
                clear_clean_rule_metadata_for_names(submodel, &effect_impact.changed_names);
                targeted = true;
            }
            if effect_impact.has_new_top {
                clear_root_clean_rule_metadata(submodel);
                targeted = true;
            }
            if !targeted {
                ctx.dirty_trace.record_whole_model_clear(rule_name);
                clear_model_clean_rule_metadata(submodel);
            }
            let rebuilt_root = ExpressionArena::from_root(take_model_root(submodel));
            let (rebuilt_surfaces, rebuilt_value_letting_surfaces) =
                build_worklist_surfaces(submodel, rebuilt_root);
            surfaces = rebuilt_surfaces;
            #[cfg(not(debug_assertions))]
            {
                value_letting_surfaces = rebuilt_value_letting_surfaces;
                scheduler = WorklistScheduler::new(&surfaces, ctx.bucketed_rules, ctx.config);
            }
            #[cfg(debug_assertions)]
            {
                let _ = rebuilt_value_letting_surfaces;
            }
            reset_rule_applicability_memo(ctx);
        } else {
            if has_model_side_effects {
                ctx.dirty_trace.record_side_effect_kept_in_arena();
            }
            enqueue_worklist_rewrite_impact(
                &mut scheduler,
                &surfaces[surface_index].arena,
                surface_index,
                rewrite_impact_node_id,
                ctx.dirty_trace,
            );
            for new_top_node_id in new_top_node_ids {
                scheduler.enqueue_subtree(
                    &surfaces[root_surface].arena,
                    root_surface,
                    new_top_node_id,
                    Some(ctx.dirty_trace),
                );
            }
            for synced_surface in synced_surfaces {
                scheduler.enqueue_surface(
                    &surfaces,
                    synced_surface,
                    ctx.bucketed_rules,
                    ctx.config,
                    Some(ctx.dirty_trace),
                );
            }
            enqueue_worklist_nodes_referencing_names(
                &mut scheduler,
                &surfaces,
                &affected_names,
                ctx.dirty_trace,
            );
        }

        #[cfg(debug_assertions)]
        {
            write_worklist_surfaces_to_model(submodel, &surfaces);
            let assertion_context = format!("rewriter after applying rule '{rule_name}'");
            debug_assert_model_well_formed(submodel, &assertion_context);
            let rebuilt_root = ExpressionArena::from_root(take_model_root(submodel));
            (surfaces, value_letting_surfaces) = build_worklist_surfaces(submodel, rebuilt_root);
            scheduler = WorklistScheduler::new(&surfaces, ctx.bucketed_rules, ctx.config);
            reset_rule_applicability_memo(ctx);
        }

        did_rewrite = true;
    }

    write_worklist_surfaces_to_model(submodel, &surfaces);
    did_rewrite.then_some(())
}

fn enqueue_worklist_rewrite_impact(
    scheduler: &mut WorklistScheduler,
    arena: &ExpressionArena,
    surface: usize,
    node_id: ExpressionNodeId,
    dirty_trace: &mut DirtyTrace,
) {
    scheduler.enqueue_node_and_ancestors(arena, surface, node_id, dirty_trace);
    scheduler.enqueue_subtree(arena, surface, node_id, Some(dirty_trace));
}

/// Applies evaluator normalisation throughout an existing arena surface.
///
/// This is the initial half of the evaluator normalisation hook. Evaluation is a privileged pure
/// simplification rather than an ordinary rule: before normal rule scheduling starts, each surface
/// is normalised bottom-up so parent evaluators can assume already-normal children. Comprehensions
/// remain atomic here for the same scoped-rewrite reason as normal scheduler traversal.
fn normalise_evaluators_bottom_up(arena: &mut ExpressionArena, dirty_trace: &mut DirtyTrace) {
    let nodes = rewriter_reachable_subtree_ids(arena, arena.root());
    for node_id in nodes.into_iter().rev() {
        if !arena.is_reachable(node_id) {
            continue;
        }
        normalise_evaluator_node_to_fixpoint(arena, node_id, dirty_trace);
    }
}

/// Applies the post-rewrite evaluator hook from `node_id` up to the root.
///
/// Ordinary rules still obey the rewriter invariant that higher priority wins and, within a
/// priority, enclosing expressions are tried before descendants. Evaluators are the explicit
/// exception: after an ordinary rule changes a subtree, local full/partial evaluation is allowed to
/// fire immediately at that node and then at each ancestor because such simplification is always
/// preferable to scheduling another ordinary rule on the unevaluated expression.
fn normalise_evaluators_from_node_to_root(
    arena: &mut ExpressionArena,
    node_id: ExpressionNodeId,
    dirty_trace: &mut DirtyTrace,
) -> (ExpressionNodeId, bool) {
    let mut current = Some(node_id);
    let mut highest_rewritten = node_id;
    let mut changed = false;

    while let Some(current_id) = current {
        if !arena.is_reachable(current_id) {
            break;
        }

        if normalise_evaluator_node_to_fixpoint(arena, current_id, dirty_trace) {
            highest_rewritten = current_id;
            changed = true;
        }
        current = arena.parent(current_id);
    }

    (highest_rewritten, changed)
}

fn normalise_evaluator_node_to_fixpoint(
    arena: &mut ExpressionArena,
    node_id: ExpressionNodeId,
    dirty_trace: &mut DirtyTrace,
) -> bool {
    let mut changed = false;

    while arena.is_reachable(node_id) {
        let expr = arena.expression(node_id);
        let Some(replacement) = normalise_evaluator_local(expr) else {
            break;
        };

        dirty_trace.replacement_subtree_clears += 1;
        arena.replace_subtree(node_id, clear_expr_clean_rule_metadata(replacement));
        dirty_trace.record_rewrite("evaluator_normalisation_hook", false);
        changed = true;
    }

    if changed {
        dirty_ancestors_after_focus_change(arena, node_id, dirty_trace, None);
    }

    changed
}

fn build_worklist_surfaces(
    submodel: &Model,
    root_arena: ExpressionArena,
) -> (Vec<RewriteSurface>, HashMap<Name, usize>) {
    let mut surfaces = vec![RewriteSurface::root(root_arena)];
    let mut value_letting_surfaces = HashMap::new();

    for (name, decl) in submodel.symbols().clone().into_iter_local() {
        let letting_expr = decl
            .as_value_letting()
            .map(|expr| clear_expr_clean_rule_metadata(expr.clone()));
        if let Some(expr) = letting_expr {
            let surface = surfaces.len();
            surfaces.push(RewriteSurface::value_letting(name.clone(), expr));
            value_letting_surfaces.insert(name, surface);
        }
    }

    (surfaces, value_letting_surfaces)
}

fn value_letting_surface_name(kind: &RewriteSurfaceKind) -> Option<&Name> {
    match kind {
        RewriteSurfaceKind::Root => None,
        RewriteSurfaceKind::ValueLetting { name } => Some(name),
    }
}

fn current_value_letting_expression(submodel: &Model, name: &Name) -> Option<Expr> {
    let declaration = {
        let symbols = submodel.symbols();
        symbols.lookup_local(name)
    }?;
    declaration
        .as_value_letting()
        .map(|expr| clear_expr_clean_rule_metadata(expr.clone()))
}

fn sync_value_letting_surfaces(
    submodel: &Model,
    surfaces: &mut Vec<RewriteSurface>,
    value_letting_surfaces: &mut HashMap<Name, usize>,
    names: &[Name],
) -> Vec<usize> {
    let mut synced_surfaces = Vec::new();
    let mut seen = HashSet::new();

    for name in names {
        if !seen.insert(name.clone()) {
            continue;
        }

        if let Some(expr) = current_value_letting_expression(submodel, name) {
            if let Some(old_surface) = value_letting_surfaces.get(name).copied()
                && let Some(surface) = surfaces.get_mut(old_surface)
            {
                surface.active = false;
            }
            let new_surface = surfaces.len();
            surfaces.push(RewriteSurface::value_letting(name.clone(), expr));
            value_letting_surfaces.insert(name.clone(), new_surface);
            synced_surfaces.push(new_surface);
        } else if let Some(old_surface) = value_letting_surfaces.remove(name)
            && let Some(surface) = surfaces.get_mut(old_surface)
        {
            surface.active = false;
        }
    }

    synced_surfaces
}

fn write_worklist_surfaces_to_model(submodel: &mut Model, surfaces: &[RewriteSurface]) {
    let root = surfaces[0].arena.expression_from(surfaces[0].arena.root());
    submodel.replace_root(root);

    for surface in surfaces.iter().skip(1) {
        if !surface.active {
            continue;
        }
        let Some(name) = value_letting_surface_name(&surface.kind) else {
            continue;
        };
        write_value_letting_surface_to_model(submodel, name, &surface.arena);
    }
}

fn write_value_letting_surface_to_model(
    submodel: &mut Model,
    name: &Name,
    arena: &ExpressionArena,
) -> bool {
    let declaration = {
        let symbols = submodel.symbols();
        symbols.lookup_local(name)
    };
    let Some(mut declaration) = declaration else {
        return false;
    };
    {
        let Some(mut letting) = declaration.as_value_letting_mut() else {
            return false;
        };

        *letting = arena.expression_from(arena.root());
    }
    submodel.symbols_mut().refresh_local_binding_hashes();
    true
}

fn enqueue_worklist_nodes_referencing_names(
    scheduler: &mut WorklistScheduler,
    surfaces: &[RewriteSurface],
    names: &[Name],
    dirty_trace: &mut DirtyTrace,
) {
    if names.is_empty() {
        return;
    }

    for (surface_index, surface) in surfaces.iter().enumerate() {
        if !surface.active {
            continue;
        }

        let mut affected_nodes = Vec::new();
        collect_worklist_nodes_referencing_names(
            &surface.arena,
            surface.arena.root(),
            names,
            &mut affected_nodes,
        );
        for node_id in affected_nodes.into_iter().rev() {
            scheduler.enqueue_node_at_level(
                &surface.arena,
                surface_index,
                node_id,
                0,
                ScheduledMode::CheckNode,
                Some(dirty_trace),
            );
        }
    }
}

fn collect_worklist_nodes_referencing_names(
    arena: &ExpressionArena,
    node_id: ExpressionNodeId,
    names: &[Name],
    affected_nodes: &mut Vec<ExpressionNodeId>,
) -> bool {
    if !arena.is_reachable(node_id) {
        return false;
    }

    let mut references_changed_name =
        expression_directly_references_any(arena.expression(node_id), names);
    if !matches!(arena.expression(node_id), Expr::Comprehension(_, _)) {
        for &child_id in arena.children(node_id) {
            references_changed_name |=
                collect_worklist_nodes_referencing_names(arena, child_id, names, affected_nodes);
        }
    }

    if references_changed_name {
        affected_nodes.push(node_id);
    }
    references_changed_name
}

fn expression_directly_references_any(expr: &Expr, names: &[Name]) -> bool {
    let Expr::Atomic(_, Atom::Reference(reference)) = expr else {
        return false;
    };
    names.iter().any(|name| &*reference.name() == name)
}

/// Returns a cached hash of the symbol values visible to rule applications.
fn current_symbol_context_hash<'ctx, 'rules>(
    submodel: &Model,
    ctx: &mut RewritePassContext<'ctx, 'rules>,
) -> u64 {
    if let Some(hash) = ctx.symbol_context_hash {
        return hash;
    }

    let hash = submodel.symbols().context_hash();
    ctx.symbol_context_hash = Some(hash);
    hash
}

fn invalidate_symbol_context_caches<'ctx, 'rules>(
    submodel: &mut Model,
    ctx: &mut RewritePassContext<'ctx, 'rules>,
) {
    ctx.symbol_context_hash = None;
    ctx.symbol_generation = ctx.symbol_generation.wrapping_add(1);
    submodel.symbols_mut().invalidate_context_hash_cache();
}

fn reset_rule_applicability_memo<'ctx, 'rules>(ctx: &mut RewritePassContext<'ctx, 'rules>) {
    if let Some(memo) = ctx.rule_applicability_memo.as_mut() {
        memo.clear();
    }
}

fn candidate_node_index_enabled(config: RewriteConfig) -> bool {
    config.prefilter && config.candidate_index
}

fn dirty_node_queues_enabled(config: RewriteConfig) -> bool {
    config.dirty && config.dirty_node_queues
}

fn increment_counter(counter: &mut Option<usize>) {
    *counter = Some(counter.unwrap_or(0) + 1);
}

fn traced_arena_content_hash(
    arena: &mut ExpressionArena,
    node_id: ExpressionNodeId,
    dirty_trace: &mut DirtyTrace,
) -> u64 {
    let (hash, hit) = arena.content_hash_with_cache_status(node_id);
    dirty_trace.record_arena_content_hash(hit);
    hash
}

/// Returns expression node ids in rewriter preorder, without entering comprehensions.
fn rewriter_preorder_ids(arena: &ExpressionArena) -> Vec<ExpressionNodeId> {
    rewriter_reachable_subtree_ids(arena, arena.root())
}

/// Returns reachable expression node ids under `node_id` in rewriter preorder.
///
/// The rewrite pass treats comprehensions as atomic here; comprehension expansion is responsible
/// for rewriting their bodies in the right scoped context.
fn rewriter_reachable_subtree_ids(
    arena: &ExpressionArena,
    node_id: ExpressionNodeId,
) -> Vec<ExpressionNodeId> {
    fn collect(
        arena: &ExpressionArena,
        node_id: ExpressionNodeId,
        nodes: &mut Vec<ExpressionNodeId>,
    ) {
        if !arena.is_reachable(node_id) {
            return;
        }
        nodes.push(node_id);
        if matches!(arena.expression(node_id), Expr::Comprehension(_, _)) {
            return;
        }

        for child in arena.children(node_id) {
            collect(arena, *child, nodes);
        }
    }

    let mut nodes = Vec::new();
    collect(arena, node_id, &mut nodes);
    nodes
}

type AncestorCacheMappings = Vec<(u64, Expr)>;

/// Replaces the focused expression and clears rewrite metadata on the changed path to root.
///
/// When `cache_mapping_context` is `Some`, this also returns each old ancestor hash with its
/// rebuilt ancestor so future duplicate enclosing subtrees can jump directly to the rewritten form.
fn replace_focus_and_dirty_ancestors(
    arena: &mut ExpressionArena,
    node_id: ExpressionNodeId,
    new_focus: Expr,
    dirty_trace: &mut DirtyTrace,
    cache_mapping_context: Option<u64>,
) -> AncestorCacheMappings {
    let old_ancestor_content_hashes =
        cache_mapping_context.map(|_| ancestor_content_hashes_to_root(arena, node_id, dirty_trace));
    dirty_trace.replacement_subtree_clears += 1;
    arena.replace_subtree(node_id, new_focus);

    dirty_ancestors_after_focus_change(arena, node_id, dirty_trace, old_ancestor_content_hashes)
}

fn dirty_ancestors_after_focus_change(
    arena: &mut ExpressionArena,
    node_id: ExpressionNodeId,
    dirty_trace: &mut DirtyTrace,
    old_ancestor_content_hashes: Option<Vec<u64>>,
) -> AncestorCacheMappings {
    let mut ancestor_mappings = Vec::new();
    let mut ancestor_index = 0;
    let mut ancestor = arena.parent(node_id);
    while let Some(ancestor_id) = ancestor {
        dirty_trace.ancestor_clears += 1;
        arena.clear_clean_rule_priority(ancestor_id);
        arena.rebuild_payload_from_children(ancestor_id);
        if let Some(hashes) = old_ancestor_content_hashes.as_ref()
            && let Some(&old_hash) = hashes.get(ancestor_index)
        {
            ancestor_mappings.push((old_hash, arena.expression(ancestor_id).clone()));
        }
        ancestor = arena.parent(ancestor_id);
        ancestor_index += 1;
    }

    ancestor_mappings
}

/// Captures ancestor content hashes before replacing the focused subtree.
fn ancestor_content_hashes_to_root(
    arena: &mut ExpressionArena,
    node_id: ExpressionNodeId,
    dirty_trace: &mut DirtyTrace,
) -> Vec<u64> {
    dirty_trace.ancestor_hash_capture_runs += 1;
    let mut hashes = Vec::new();
    let mut ancestor = arena.parent(node_id);
    while let Some(ancestor_id) = ancestor {
        hashes.push(traced_arena_content_hash(arena, ancestor_id, dirty_trace));
        dirty_trace.ancestor_hash_captured_nodes += 1;
        ancestor = arena.parent(ancestor_id);
    }
    hashes
}

/// Inserts old-ancestor-hash to rebuilt-ancestor mappings under one symbol context.
fn insert_ancestor_mappings(
    cache: &mut RewriteCache,
    mappings: AncestorCacheMappings,
    level: usize,
    symbol_context_hash: u64,
) {
    for (old_hash, new_ancestor) in mappings {
        cache.insert_from_hash(old_hash, Some(new_ancestor), level, symbol_context_hash);
    }
}

/// Clears rewrite metadata from every expression in a subtree.
fn clear_expr_clean_rule_metadata(expr: Expr) -> Expr {
    let expr = expr.transform_bi(&|metadata: Metadata| {
        metadata.clear_clean_rule_priority();
        metadata
    });
    expr.invalidate_cached_content_hash_recursive();
    expr
}

/// Clears clean-rule metadata from the model root expression tree.
fn clear_model_clean_rule_metadata(model: &mut Model) {
    let root = take_model_root(model);
    model.replace_root(clear_expr_clean_rule_metadata(root));
}

/// Clears clean-rule metadata only in subtrees that reference a changed letting.
fn clear_clean_rule_metadata_for_name(model: &mut Model, name: &Name) {
    let root = take_model_root(model);
    model.replace_root(clear_expr_clean_rule_metadata_for_name(root, name));
}

/// Clears clean-rule metadata only in subtrees that reference one of the given symbols.
fn clear_clean_rule_metadata_for_names(model: &mut Model, names: &[Name]) {
    if names.is_empty() {
        return;
    }
    let root = take_model_root(model);
    model.replace_root(clear_expr_clean_rule_metadata_for_names(root, names));
}

fn clear_root_clean_rule_metadata(model: &mut Model) {
    let root = take_model_root(model);
    let cleared = match root {
        Expr::Root(metadata, constraints) => {
            metadata.clear_clean_rule_priority();
            let root = Expr::Root(metadata, constraints);
            root.invalidate_cached_content_hash();
            root
        }
        other => other,
    };
    model.replace_root(cleared);
}

fn take_model_root(model: &mut Model) -> Expr {
    model.replace_root(Expr::Root(Metadata::new(), Vec::new()))
}

fn clear_expr_clean_rule_metadata_for_name(expr: Expr, name: &Name) -> Expr {
    clear_expr_clean_rule_metadata_for_names(expr, std::slice::from_ref(name))
}

fn clear_expr_clean_rule_metadata_for_names(expr: Expr, names: &[Name]) -> Expr {
    if !subtree_references_any(&expr, names) {
        return expr;
    }

    match expr {
        Expr::Root(metadata, constraints) => {
            metadata.clear_clean_rule_priority();
            let constraints = constraints
                .into_iter()
                .map(|child| clear_expr_clean_rule_metadata_for_names(child, names))
                .collect();
            let root = Expr::Root(metadata, constraints);
            root.invalidate_cached_content_hash();
            root
        }
        Expr::Eq(metadata, left, right) => {
            metadata.clear_clean_rule_priority();
            let left = clear_expr_clean_rule_metadata_for_names(left.as_ref().clone(), names);
            let right = clear_expr_clean_rule_metadata_for_names(right.as_ref().clone(), names);
            let eq = Expr::Eq(metadata, Moo::new(left), Moo::new(right));
            eq.invalidate_cached_content_hash();
            eq
        }
        Expr::Sum(metadata, matrix) => {
            metadata.clear_clean_rule_priority();
            let matrix = clear_expr_clean_rule_metadata_for_names(matrix.as_ref().clone(), names);
            let sum = Expr::Sum(metadata, Moo::new(matrix));
            sum.invalidate_cached_content_hash();
            sum
        }
        other => clear_expr_clean_rule_metadata(other),
    }
}

fn subtree_references_any(expr: &Expr, names: &[Name]) -> bool {
    names.iter().any(|name| subtree_references_name(expr, name))
}

fn subtree_references_name(expr: &Expr, name: &Name) -> bool {
    expr.universe().into_iter().any(|subexpr| {
        matches!(
            subexpr,
            Expr::Atomic(_, Atom::Reference(reference)) if &*reference.name() == name
        )
    })
}

fn rule_is_universal(rule_data: &RuleData<'_>) -> bool {
    rule_data.rule.prefilters.is_none()
}

fn rule_matches_self_discriminant(rule_data: &RuleData<'_>, expr_discriminant: usize) -> bool {
    rule_data
        .rule
        .prefilters
        .is_some_and(|prefilters| {
            prefilters.iter().any(|prefilter| {
                matches!(prefilter, RulePrefilter::Variant(discriminant) if *discriminant == expr_discriminant)
            })
        })
}

fn rule_matches_specific_prefilter(rule_data: &RuleData<'_>, expr: &Expr) -> bool {
    if rule_is_universal(rule_data) {
        return false;
    }

    let expr_discriminant = discriminant_from_value(expr);
    rule_data.rule.prefilters.is_some_and(|prefilters| {
        prefilters.iter().any(|prefilter| match prefilter {
            RulePrefilter::Variant(discriminant) => *discriminant == expr_discriminant,
            RulePrefilter::Child { child } => expr_has_direct_child_discriminant(expr, &[*child]),
            RulePrefilter::VariantChild { variant, child } => {
                *variant == expr_discriminant && expr_has_direct_child_discriminant(expr, &[*child])
            }
            RulePrefilter::Atom(atom_kind) => expr_atom_kind(expr) == Some(*atom_kind),
        })
    })
}

fn expr_atom_kind(expr: &Expr) -> Option<AtomKind> {
    match expr {
        Expr::Atomic(_, Atom::Literal(_)) => Some(AtomKind::Literal),
        Expr::Atomic(_, Atom::Reference(_)) => Some(AtomKind::Reference),
        _ => None,
    }
}

fn expr_has_direct_child_discriminant(expr: &Expr, target_discriminants: &[usize]) -> bool {
    if target_discriminants.is_empty() {
        return false;
    }

    let mut found = false;
    expr.for_each_expr_child(&mut |child| {
        if !found && target_discriminants.contains(&discriminant_from_value(child)) {
            found = true;
        }
    });
    found
}

#[cfg(debug_assertions)]
fn csv_escape(field: &str) -> String {
    if field.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

#[cfg(debug_assertions)]
fn log_verbose_rule_attempt(
    run_start: &Instant,
    priority: &u16,
    rule_name: &str,
    rule_set_name: &str,
    status: &str,
    expr: &Expr,
) {
    let elapsed_seconds = run_start.elapsed().as_secs_f64();
    let expr_str = expr.to_string();
    trace!(
        target: "rule_engine_rule_trace_verbose",
        "{:.3},{},{},{},{},{}",
        elapsed_seconds,
        priority,
        csv_escape(rule_name),
        csv_escape(rule_set_name),
        status,
        csv_escape(&expr_str)
    );
}

// Exits with a bug if there are multiple equally applicable rules for an expression.
fn assert_no_multiple_equally_applicable_rules<CtxFnType>(
    results: &Vec<ApplicableRule<'_, CtxFnType>>,
    rules_grouped: &Vec<(u16, Vec<RuleData<'_>>)>,
) {
    if results.len() <= 1 {
        return;
    }

    let names: Vec<_> = results
        .iter()
        .map(|(result, _, _, _, _)| result.rule_data.rule.name)
        .collect();

    // Extract the expression from the first result
    let expr = results[0].2.clone();

    // Construct a single string to display the names of the rules grouped by priority
    let mut rules_by_priority_string = String::new();
    rules_by_priority_string.push_str("Rules grouped by priority:\n");
    for (priority, rules) in rules_grouped.iter() {
        rules_by_priority_string.push_str(&format!("Priority {priority}:\n"));
        for rd in rules {
            rules_by_priority_string.push_str(&format!(
                "  - {} (from {})\n",
                rd.rule.name, rd.rule_set.name
            ));
        }
    }
    bug!("Multiple equally applicable rules for {expr}: {names:#?}\n\n{rules_by_priority_string}");
}

#[cfg(test)]
mod tests {
    use crate::ast::comprehension::ComprehensionBuilder;
    use crate::ast::{Atom, DeclarationPtr, ExpressionArena, Literal, Moo, SymbolTablePtr};
    use crate::matrix_expr;

    use super::*;

    fn int_lit(value: i32) -> Expr {
        Expr::Atomic(Metadata::new(), Atom::Literal(Literal::Int(value)))
    }

    fn bool_lit(value: bool) -> Expr {
        Expr::Atomic(Metadata::new(), Atom::Literal(Literal::Bool(value)))
    }

    fn root(exprs: Vec<Expr>) -> Expr {
        Expr::Root(Metadata::new(), exprs)
    }

    fn comprehension(return_expression: Expr, guards: Vec<Expr>) -> Expr {
        let mut builder = ComprehensionBuilder::new(SymbolTablePtr::new());
        for guard in guards {
            builder = builder.guard(guard);
        }
        Expr::Comprehension(
            Metadata::new(),
            Moo::new(builder.with_return_value(return_expression)),
        )
    }

    #[test]
    fn dirty_trace_filename_replaces_path_separators_and_module_delimiters() {
        assert_eq!(
            sanitize_dirty_trace_filename("generated_tests::savilerow/quasiGroup6"),
            "generated_tests--savilerow-quasiGroup6"
        );
    }

    #[test]
    fn dirty_trace_bare_path_is_directory_destination() {
        assert_eq!(
            dirty_trace_destination_from_env_value("trace-dir".into()),
            DirtyTraceDestination::Directory(PathBuf::from("trace-dir"))
        );
    }

    #[test]
    fn dirty_trace_txt_path_is_file_destination() {
        assert_eq!(
            dirty_trace_destination_from_env_value("trace.txt".into()),
            DirtyTraceDestination::File(PathBuf::from("trace.txt"))
        );
    }

    #[test]
    fn rewrite_cache_resolves_transitive_rewrites() {
        let a = int_lit(1);
        let b = int_lit(2);
        let c = int_lit(3);
        let d = int_lit(4);
        let mut cache = RewriteCache::default();
        let context = 10;

        cache.insert(&a, Some(b.clone()), 0, context);
        cache.insert(&b, Some(c.clone()), 0, context);
        cache.insert(&c, Some(d.clone()), 0, context);

        for expr in [&a, &b, &c] {
            match cache.get(expr, 0, context) {
                CacheResult::Rewrite(rewritten) => assert_eq!(rewritten.expr, d),
                CacheResult::Unknown | CacheResult::Terminal(_) => {
                    panic!("expected transitive rewrite cache hit")
                }
            }
        }
    }

    #[test]
    fn rewrite_cache_does_not_rewrite_before_proven_rule_group() {
        let a = int_lit(1);
        let b = int_lit(2);
        let mut cache = RewriteCache::default();
        let context = 10;

        cache.insert(&a, Some(b.clone()), 2, context);

        assert!(matches!(cache.get(&a, 1, context), CacheResult::Unknown));
        match cache.get(&a, 2, context) {
            CacheResult::Rewrite(rewritten) => assert_eq!(rewritten.expr, b),
            CacheResult::Unknown | CacheResult::Terminal(_) => {
                panic!("expected rewrite at proven rule group")
            }
        }
    }

    #[test]
    fn rewrite_cache_compresses_transitive_rewrites_across_rule_groups() {
        let a = int_lit(1);
        let b = int_lit(2);
        let c = int_lit(3);
        let d = int_lit(4);
        let mut cache = RewriteCache::default();
        let context = 10;

        cache.insert(&a, Some(b.clone()), 0, context);
        cache.insert(&b, Some(c.clone()), 1, context);
        cache.insert(&c, Some(d.clone()), 2, context);

        match cache.get(&a, 0, context) {
            CacheResult::Rewrite(rewritten) => assert_eq!(rewritten.expr, d),
            CacheResult::Unknown | CacheResult::Terminal(_) => {
                panic!("expected compressed rewrite chain")
            }
        }
        assert!(matches!(cache.get(&b, 0, context), CacheResult::Unknown));
        match cache.get(&b, 1, context) {
            CacheResult::Rewrite(rewritten) => assert_eq!(rewritten.expr, d),
            CacheResult::Unknown | CacheResult::Terminal(_) => {
                panic!("expected compressed suffix chain")
            }
        }
    }

    #[test]
    fn rewrite_cache_preserves_rewrite_to_terminal_target() {
        let a = int_lit(1);
        let b = int_lit(2);
        let mut cache = RewriteCache::default();
        let context = 10;

        cache.insert(&b, None, 0, context);
        cache.insert(&a, Some(b.clone()), 0, context);

        match cache.get(&a, 0, context) {
            CacheResult::Rewrite(rewritten) => assert_eq!(rewritten.expr, b),
            CacheResult::Unknown | CacheResult::Terminal(_) => {
                panic!("terminal target must not make its source terminal")
            }
        }
        assert!(matches!(
            cache.get(&b, 0, context),
            CacheResult::Terminal(0)
        ));
    }

    #[test]
    fn rewrite_cache_terminal_target_does_not_terminalise_existing_predecessor() {
        let a = int_lit(1);
        let b = int_lit(2);
        let mut cache = RewriteCache::default();
        let context = 10;

        cache.insert(&a, Some(b.clone()), 0, context);
        cache.insert(&b, None, 0, context);

        match cache.get(&a, 0, context) {
            CacheResult::Rewrite(rewritten) => assert_eq!(rewritten.expr, b),
            CacheResult::Unknown | CacheResult::Terminal(_) => {
                panic!("terminal target must not make its predecessor terminal")
            }
        }
    }

    #[test]
    fn rewrite_cache_compresses_chain_ending_at_terminal_target() {
        let a = int_lit(1);
        let b = int_lit(2);
        let c = int_lit(3);
        let mut cache = RewriteCache::default();
        let context = 10;

        cache.insert(&c, None, 0, context);
        cache.insert(&b, Some(c.clone()), 0, context);
        cache.insert(&a, Some(b.clone()), 0, context);

        for expr in [&a, &b] {
            match cache.get(expr, 0, context) {
                CacheResult::Rewrite(rewritten) => assert_eq!(rewritten.expr, c),
                CacheResult::Unknown | CacheResult::Terminal(_) => {
                    panic!("expected rewrite to terminal target")
                }
            }
        }
        assert!(matches!(
            cache.get(&c, 0, context),
            CacheResult::Terminal(0)
        ));
    }

    #[test]
    fn rewrite_cache_rewrite_overrides_stale_terminal_fact() {
        let a = int_lit(1);
        let b = int_lit(2);
        let mut cache = RewriteCache::default();
        let context = 10;

        cache.insert(&a, None, 0, context);
        cache.insert(&a, Some(b.clone()), 0, context);

        match cache.get(&a, 0, context) {
            CacheResult::Rewrite(rewritten) => assert_eq!(rewritten.expr, b),
            CacheResult::Unknown | CacheResult::Terminal(_) => {
                panic!("new rewrite evidence must override a stale terminal fact")
            }
        }
    }

    #[test]
    fn rewrite_cache_tracks_terminal_levels() {
        let a = int_lit(1);
        let mut cache = RewriteCache::default();
        let context = 10;

        cache.insert(&a, None, 0, context);
        cache.insert(&a, None, 1, context);

        match cache.get(&a, 0, context) {
            CacheResult::Terminal(level) => assert_eq!(level, 1),
            CacheResult::Unknown | CacheResult::Rewrite(_) => panic!("expected terminal hit"),
        }
        match cache.get(&a, 1, context) {
            CacheResult::Terminal(level) => assert_eq!(level, 1),
            CacheResult::Unknown | CacheResult::Rewrite(_) => panic!("expected terminal hit"),
        }
        assert!(matches!(cache.get(&a, 2, context), CacheResult::Unknown));
    }

    #[test]
    fn rewrite_cache_resolves_ancestor_mappings_transitively() {
        let old_parent = root(vec![int_lit(1)]);
        let mid_parent = root(vec![int_lit(2)]);
        let final_parent = root(vec![int_lit(3)]);
        let mut cache = RewriteCache::default();
        let context = 10;
        let old_parent_hash = RewriteCache::expression_content_hash(&old_parent, context);

        cache.insert_from_hash(old_parent_hash, Some(mid_parent.clone()), 0, context);
        cache.insert(&mid_parent, Some(final_parent.clone()), 0, context);

        match cache.get(&old_parent, 0, context) {
            CacheResult::Rewrite(rewritten) => assert_eq!(rewritten.expr, final_parent),
            CacheResult::Unknown | CacheResult::Terminal(_) => {
                panic!("expected ancestor rewrite cache hit")
            }
        }
    }

    #[test]
    fn rewrite_cache_separates_symbol_contexts() {
        let from = int_lit(1);
        let to = int_lit(2);
        let terminal = int_lit(3);
        let mut cache = RewriteCache::default();
        let old_context = 10;
        let new_context = 20;

        cache.insert(&from, Some(to.clone()), 0, old_context);
        cache.insert(&terminal, None, 0, old_context);

        match cache.get(&from, 0, old_context) {
            CacheResult::Rewrite(rewritten) => assert_eq!(rewritten.expr, to),
            CacheResult::Unknown | CacheResult::Terminal(_) => {
                panic!("expected rewrite hit in original context")
            }
        }
        assert!(matches!(
            cache.get(&from, 0, new_context),
            CacheResult::Unknown
        ));
        assert!(matches!(
            cache.get(&terminal, 0, new_context),
            CacheResult::Unknown
        ));
    }

    #[test]
    fn rewrite_cache_resolves_chains_within_one_symbol_context() {
        let a = int_lit(1);
        let b = int_lit(2);
        let c = int_lit(3);
        let mut cache = RewriteCache::default();
        let old_context = 10;
        let new_context = 20;

        cache.insert(&a, Some(b.clone()), 0, old_context);
        cache.insert(&b, Some(c.clone()), 0, old_context);

        match cache.get(&a, 0, old_context) {
            CacheResult::Rewrite(rewritten) => assert_eq!(rewritten.expr, c),
            CacheResult::Unknown | CacheResult::Terminal(_) => {
                panic!("expected rewrite hit in original context")
            }
        }
        assert!(matches!(
            cache.get(&a, 0, new_context),
            CacheResult::Unknown
        ));
    }

    fn assert_clean_at(expr: &Expr, priority: u16) {
        assert!(
            expr.meta_ref().is_clean_for_rule_priority(priority),
            "expected expression {expr} to remain clean at priority {priority}"
        );
    }

    fn assert_not_clean_at(expr: &Expr, priority: u16) {
        assert!(
            !expr.meta_ref().is_clean_for_rule_priority(priority),
            "expected expression {expr} to require re-check from the top at priority {priority}"
        );
    }

    fn arena_with_preorder_focus(tree: Expr, index: usize) -> (ExpressionArena, ExpressionNodeId) {
        let arena = ExpressionArena::from_root(tree);
        let node_id = rewriter_preorder_ids(&arena)[index];
        (arena, node_id)
    }

    fn reference_expr(name: &Name) -> Expr {
        use crate::ast::{Domain, Range, Reference};

        Expr::Atomic(
            Metadata::new(),
            Atom::Reference(Reference::new(DeclarationPtr::new_find(
                name.clone(),
                Domain::int(vec![Range::Bounded(1, 3)]),
            ))),
        )
    }

    fn test_rule_set_applies(_: &crate::settings::SolverFamily) -> bool {
        true
    }

    fn never_apply_test_rule(
        _: &Expr,
        _: &crate::ast::SymbolTable,
    ) -> crate::rule_engine::ApplicationResult {
        Err(crate::rule_engine::ApplicationError::RuleNotApplicable)
    }

    static TEST_RULE_SET: RuleSet<'static> =
        RuleSet::new("test-rule-set", &[], test_rule_set_applies);
    static TEST_RULE: crate::rule_engine::Rule<'static> = crate::rule_engine::Rule::new(
        "never-apply-test-rule",
        never_apply_test_rule,
        &[("test-rule-set", 1)],
    );
    static TEST_NO_TARGET_RULE: crate::rule_engine::Rule<'static> = crate::rule_engine::Rule {
        name: "no-target-test-rule",
        application: never_apply_test_rule,
        rule_sets: &[("test-rule-set", 1)],
        prefilters: Some(&[]),
    };
    fn test_rule_groups_at_priorities(priorities: &[u16]) -> Vec<RuleGroup<'static>> {
        priorities
            .iter()
            .map(|&priority| {
                RuleGroup::new(
                    priority,
                    vec![crate::rule_engine::RuleData {
                        rule: &TEST_RULE,
                        priority,
                        rule_set: &TEST_RULE_SET,
                    }],
                )
            })
            .collect()
    }

    fn test_rule_groups() -> Vec<RuleGroup<'static>> {
        test_rule_groups_at_priorities(&[1])
    }

    fn test_rule_groups_with_two_levels() -> Vec<RuleGroup<'static>> {
        test_rule_groups_at_priorities(&[1, 2])
    }

    fn test_rule_groups_with_no_candidate_middle_level() -> Vec<RuleGroup<'static>> {
        vec![
            RuleGroup::new(
                1,
                vec![crate::rule_engine::RuleData {
                    rule: &TEST_RULE,
                    priority: 1,
                    rule_set: &TEST_RULE_SET,
                }],
            ),
            RuleGroup::new(
                2,
                vec![crate::rule_engine::RuleData {
                    rule: &TEST_NO_TARGET_RULE,
                    priority: 2,
                    rule_set: &TEST_RULE_SET,
                }],
            ),
            RuleGroup::new(
                3,
                vec![crate::rule_engine::RuleData {
                    rule: &TEST_RULE,
                    priority: 3,
                    rule_set: &TEST_RULE_SET,
                }],
            ),
        ]
    }

    fn test_rule_groups_targeting_expr(expr: &Expr) -> Vec<RuleGroup<'static>> {
        let discriminant = discriminant_from_value(expr);
        let mut rules_by_discriminant = Vec::new();
        rules_by_discriminant.resize_with(discriminant + 1, || None);
        rules_by_discriminant[discriminant] = Some(vec![crate::rule_engine::RuleData {
            rule: &TEST_RULE,
            priority: 1,
            rule_set: &TEST_RULE_SET,
        }]);

        vec![RuleGroup {
            priority: 1,
            rules: vec![crate::rule_engine::RuleData {
                rule: &TEST_RULE,
                priority: 1,
                rule_set: &TEST_RULE_SET,
            }],
            rules_by_discriminant,
            universal_rules: Vec::new(),
            has_non_discriminant_filters: false,
            target_discriminants: vec![discriminant],
            target_discriminant_mask: {
                let mut mask = vec![false; discriminant + 1];
                mask[discriminant] = true;
                mask
            },
        }]
    }

    #[test]
    fn rule_group_child_filter_matches_immediate_child_kind() {
        let bubble_discriminant = discriminant_from_value(&Expr::Bubble(
            Metadata::new(),
            Moo::new(int_lit(0)),
            Moo::new(bool_lit(true)),
        ));
        let child_prefilters: &'static [RulePrefilter] = Box::leak(
            vec![RulePrefilter::Child {
                child: bubble_discriminant,
            }]
            .into_boxed_slice(),
        );
        let child_bubble_rule: &'static crate::rule_engine::Rule<'static> =
            Box::leak(Box::new(crate::rule_engine::Rule {
                name: "child-bubble-test-rule",
                application: never_apply_test_rule,
                rule_sets: &[("test-rule-set", 1)],
                prefilters: Some(child_prefilters),
            }));
        let rule_group = RuleGroup::new(
            1,
            vec![crate::rule_engine::RuleData {
                rule: child_bubble_rule,
                priority: 1,
                rule_set: &TEST_RULE_SET,
            }],
        );
        let config = RewriteConfig::optimised();

        let expr_with_bubble_child = Expr::Eq(
            Metadata::new(),
            Moo::new(Expr::Bubble(
                Metadata::new(),
                Moo::new(int_lit(1)),
                Moo::new(bool_lit(true)),
            )),
            Moo::new(int_lit(2)),
        );
        let expr_without_bubble_child =
            Expr::Eq(Metadata::new(), Moo::new(int_lit(1)), Moo::new(int_lit(2)));

        assert!(rule_group.has_candidates(config, &expr_with_bubble_child));
        assert_eq!(
            rule_group
                .candidates(config, &expr_with_bubble_child)
                .map(|rule_data| rule_data.rule.name)
                .collect_vec(),
            vec!["child-bubble-test-rule"]
        );
        assert!(!rule_group.has_candidates(config, &expr_without_bubble_child));
    }

    #[test]
    fn rule_group_atom_filter_matches_atomic_reference() {
        let atom_reference_rule: &'static crate::rule_engine::Rule<'static> =
            Box::leak(Box::new(crate::rule_engine::Rule {
                name: "atom-reference-test-rule",
                application: never_apply_test_rule,
                rule_sets: &[("test-rule-set", 1)],
                prefilters: Some(&[RulePrefilter::Atom(AtomKind::Reference)]),
            }));
        let rule_group = RuleGroup::new(
            1,
            vec![crate::rule_engine::RuleData {
                rule: atom_reference_rule,
                priority: 1,
                rule_set: &TEST_RULE_SET,
            }],
        );
        let config = RewriteConfig::optimised();
        let reference = reference_expr(&Name::user("x"));
        let literal = int_lit(1);
        let composite = Expr::Eq(Metadata::new(), Moo::new(int_lit(1)), Moo::new(int_lit(2)));

        assert!(rule_group.has_candidates(config, &reference));
        assert_eq!(
            rule_group
                .candidates(config, &reference)
                .map(|rule_data| rule_data.rule.name)
                .collect_vec(),
            vec!["atom-reference-test-rule"]
        );
        assert!(!rule_group.has_candidates(config, &literal));
        assert!(!rule_group.has_candidates(config, &composite));
    }

    #[test]
    fn rule_group_variant_child_filter_does_not_cross_product_alternatives() {
        let and_discriminant = discriminant_from_value(&Expr::And(
            Metadata::new(),
            Moo::new(matrix_expr![bool_lit(true)]),
        ));
        let or_discriminant = discriminant_from_value(&Expr::Or(
            Metadata::new(),
            Moo::new(matrix_expr![bool_lit(true)]),
        ));
        let comprehension_discriminant =
            discriminant_from_value(&comprehension(bool_lit(true), vec![]));
        let atomic_discriminant = discriminant_from_value(&bool_lit(true));
        let paired_prefilters: &'static [RulePrefilter] = Box::leak(
            vec![
                RulePrefilter::VariantChild {
                    variant: and_discriminant,
                    child: comprehension_discriminant,
                },
                RulePrefilter::VariantChild {
                    variant: or_discriminant,
                    child: atomic_discriminant,
                },
            ]
            .into_boxed_slice(),
        );
        let paired_rule: &'static crate::rule_engine::Rule<'static> =
            Box::leak(Box::new(crate::rule_engine::Rule {
                name: "paired-prefilter-test-rule",
                application: never_apply_test_rule,
                rule_sets: &[("test-rule-set", 1)],
                prefilters: Some(paired_prefilters),
            }));
        let rule_group = RuleGroup::new(
            1,
            vec![crate::rule_engine::RuleData {
                rule: paired_rule,
                priority: 1,
                rule_set: &TEST_RULE_SET,
            }],
        );
        let config = RewriteConfig::optimised();
        let and_with_comprehension = Expr::And(
            Metadata::new(),
            Moo::new(comprehension(bool_lit(true), vec![])),
        );
        let or_with_atomic = Expr::Or(Metadata::new(), Moo::new(bool_lit(true)));
        let and_with_atomic = Expr::And(Metadata::new(), Moo::new(bool_lit(true)));
        let or_with_comprehension = Expr::Or(
            Metadata::new(),
            Moo::new(comprehension(bool_lit(true), vec![])),
        );

        assert!(rule_group.has_candidates(config, &and_with_comprehension));
        assert!(rule_group.has_candidates(config, &or_with_atomic));
        assert!(!rule_group.has_candidates(config, &and_with_atomic));
        assert!(!rule_group.has_candidates(config, &or_with_comprehension));
    }

    #[test]
    fn rewriter_subtree_preorder_does_not_enter_comprehensions() {
        let tree = root(vec![
            comprehension(int_lit(1), vec![int_lit(2)]),
            int_lit(3),
        ]);
        let arena = ExpressionArena::from_root(tree);
        let root_ids = rewriter_preorder_ids(&arena);
        let comp_id = arena.children(arena.root())[0];

        assert_eq!(root_ids.len(), 3);
        assert_eq!(
            rewriter_reachable_subtree_ids(&arena, comp_id),
            vec![comp_id]
        );
    }

    #[test]
    fn worklist_ancestor_enqueue_checks_enclosing_nodes_first() {
        let tree = root(vec![Expr::Eq(
            Metadata::new(),
            Moo::new(int_lit(1)),
            Moo::new(int_lit(2)),
        )]);
        let surfaces = vec![RewriteSurface::root(ExpressionArena::from_root(tree))];
        let arena = &surfaces[0].arena;
        let ids = rewriter_preorder_ids(arena);
        let root_id = ids[0];
        let eq_id = ids[1];
        let left_leaf_id = ids[2];

        let rule_groups = test_rule_groups();
        let mut scheduler = WorklistScheduler::empty(&rule_groups);
        let mut dirty_trace = DirtyTrace::default();

        scheduler.enqueue_node_and_ancestors(arena, 0, left_leaf_id, &mut dirty_trace);

        assert_eq!(
            scheduler.pop_next(
                &surfaces,
                &rule_groups,
                RewriteConfig::optimised(),
                &mut dirty_trace
            ),
            Some((0, 0, root_id, ScheduledMode::CheckNode))
        );
        assert_eq!(
            scheduler.pop_next(
                &surfaces,
                &rule_groups,
                RewriteConfig::optimised(),
                &mut dirty_trace
            ),
            Some((0, 0, eq_id, ScheduledMode::CheckNode))
        );
        assert_eq!(
            scheduler.pop_next(
                &surfaces,
                &rule_groups,
                RewriteConfig::optimised(),
                &mut dirty_trace
            ),
            Some((0, 0, left_leaf_id, ScheduledMode::CheckNode))
        );
        assert_eq!(
            scheduler.pop_next(
                &surfaces,
                &rule_groups,
                RewriteConfig::optimised(),
                &mut dirty_trace
            ),
            None
        );
    }

    #[test]
    fn worklist_level_order_prefers_ancestors_over_earlier_descendants() {
        let tree = root(vec![Expr::Eq(
            Metadata::new(),
            Moo::new(int_lit(1)),
            Moo::new(int_lit(2)),
        )]);
        let surfaces = vec![RewriteSurface::root(ExpressionArena::from_root(tree))];
        let arena = &surfaces[0].arena;
        let ids = rewriter_preorder_ids(arena);
        let eq_id = ids[1];
        let left_leaf_id = ids[2];

        let rule_groups = test_rule_groups();
        let mut scheduler = WorklistScheduler::empty(&rule_groups);
        let mut dirty_trace = DirtyTrace::default();

        scheduler.enqueue_node_at_level(
            arena,
            0,
            left_leaf_id,
            0,
            ScheduledMode::CheckNode,
            Some(&mut dirty_trace),
        );
        scheduler.enqueue_node_at_level(
            arena,
            0,
            eq_id,
            0,
            ScheduledMode::CheckNode,
            Some(&mut dirty_trace),
        );

        assert_eq!(
            scheduler.pop_next(
                &surfaces,
                &rule_groups,
                RewriteConfig::optimised(),
                &mut dirty_trace
            ),
            Some((0, 0, eq_id, ScheduledMode::CheckNode))
        );
        assert_eq!(
            scheduler.pop_next(
                &surfaces,
                &rule_groups,
                RewriteConfig::optimised(),
                &mut dirty_trace
            ),
            Some((0, 0, left_leaf_id, ScheduledMode::CheckNode))
        );
    }

    #[test]
    fn worklist_subtree_descends_lazily_in_breadth_first_order() {
        let tree = root(vec![
            Expr::Eq(Metadata::new(), Moo::new(int_lit(1)), Moo::new(int_lit(2))),
            int_lit(3),
        ]);
        let surfaces = vec![RewriteSurface::root(ExpressionArena::from_root(tree))];
        let arena = &surfaces[0].arena;
        let ids = rewriter_preorder_ids(arena);
        let root_id = ids[0];
        let eq_id = ids[1];
        let left_leaf_id = ids[2];
        let right_leaf_id = ids[3];
        let root_sibling_id = ids[4];

        let rule_groups = test_rule_groups_with_two_levels();
        let mut scheduler =
            WorklistScheduler::new(&surfaces, &rule_groups, RewriteConfig::optimised());
        let mut dirty_trace = DirtyTrace::default();

        let root_work = scheduler.pop_next(
            &surfaces,
            &rule_groups,
            RewriteConfig::optimised(),
            &mut dirty_trace,
        );
        assert_eq!(
            root_work,
            Some((0, 0, root_id, ScheduledMode::TraverseSubtreeRoot))
        );
        assert_eq!(
            scheduler.pop_next(
                &surfaces,
                &rule_groups,
                RewriteConfig::optimised(),
                &mut dirty_trace
            ),
            None
        );

        scheduler.enqueue_after_no_rewrite(
            arena,
            0,
            root_id,
            0,
            1,
            ScheduledMode::TraverseSubtreeRoot,
            &rule_groups,
            RewriteConfig::optimised(),
            Some(&mut dirty_trace),
        );
        let eq_work = scheduler.pop_next(
            &surfaces,
            &rule_groups,
            RewriteConfig::optimised(),
            &mut dirty_trace,
        );
        assert_eq!(
            eq_work,
            Some((0, 0, eq_id, ScheduledMode::TraverseSubtreeDescendant))
        );
        scheduler.enqueue_after_no_rewrite(
            arena,
            0,
            eq_id,
            0,
            1,
            ScheduledMode::TraverseSubtreeDescendant,
            &rule_groups,
            RewriteConfig::optimised(),
            Some(&mut dirty_trace),
        );
        assert_eq!(
            scheduler.pop_next(
                &surfaces,
                &rule_groups,
                RewriteConfig::optimised(),
                &mut dirty_trace
            ),
            Some((
                0,
                0,
                root_sibling_id,
                ScheduledMode::TraverseSubtreeDescendant
            ))
        );
        assert_eq!(
            scheduler.pop_next(
                &surfaces,
                &rule_groups,
                RewriteConfig::optimised(),
                &mut dirty_trace
            ),
            Some((0, 0, left_leaf_id, ScheduledMode::TraverseSubtreeDescendant))
        );
        assert_eq!(
            scheduler.pop_next(
                &surfaces,
                &rule_groups,
                RewriteConfig::optimised(),
                &mut dirty_trace
            ),
            Some((
                0,
                0,
                right_leaf_id,
                ScheduledMode::TraverseSubtreeDescendant
            ))
        );
        assert_eq!(
            scheduler.pop_next(
                &surfaces,
                &rule_groups,
                RewriteConfig::optimised(),
                &mut dirty_trace
            ),
            Some((1, 0, root_id, ScheduledMode::TraverseSubtreeRoot))
        );
        assert_eq!(
            scheduler.pop_next(
                &surfaces,
                &rule_groups,
                RewriteConfig::optimised(),
                &mut dirty_trace
            ),
            None
        );

        scheduler.enqueue_after_no_rewrite(
            arena,
            0,
            root_id,
            1,
            2,
            ScheduledMode::TraverseSubtreeRoot,
            &rule_groups,
            RewriteConfig::optimised(),
            Some(&mut dirty_trace),
        );
        assert_eq!(
            scheduler.pop_next(
                &surfaces,
                &rule_groups,
                RewriteConfig::optimised(),
                &mut dirty_trace
            ),
            Some((1, 0, eq_id, ScheduledMode::TraverseSubtreeDescendant))
        );
    }

    #[test]
    fn worklist_refreshes_stale_subtree_carrier_after_descendant_rewrite() {
        let tree = root(vec![
            Expr::Eq(Metadata::new(), Moo::new(int_lit(1)), Moo::new(int_lit(2))),
            int_lit(3),
        ]);
        let mut surfaces = vec![RewriteSurface::root(ExpressionArena::from_root(tree))];
        let ids = rewriter_preorder_ids(&surfaces[0].arena);
        let root_id = ids[0];
        let eq_id = ids[1];
        let root_sibling_id = ids[4];

        let rule_groups = test_rule_groups_with_two_levels();
        let mut scheduler =
            WorklistScheduler::new(&surfaces, &rule_groups, RewriteConfig::optimised());
        let mut dirty_trace = DirtyTrace::default();

        assert_eq!(
            scheduler.pop_next(
                &surfaces,
                &rule_groups,
                RewriteConfig::optimised(),
                &mut dirty_trace
            ),
            Some((0, 0, root_id, ScheduledMode::TraverseSubtreeRoot))
        );
        scheduler.enqueue_after_no_rewrite(
            &surfaces[0].arena,
            0,
            root_id,
            0,
            1,
            ScheduledMode::TraverseSubtreeRoot,
            &rule_groups,
            RewriteConfig::optimised(),
            Some(&mut dirty_trace),
        );

        assert_eq!(
            scheduler.pop_next(
                &surfaces,
                &rule_groups,
                RewriteConfig::optimised(),
                &mut dirty_trace
            ),
            Some((0, 0, eq_id, ScheduledMode::TraverseSubtreeDescendant))
        );
        replace_focus_and_dirty_ancestors(
            &mut surfaces[0].arena,
            eq_id,
            clear_expr_clean_rule_metadata(int_lit(10)),
            &mut dirty_trace,
            None,
        );
        enqueue_worklist_rewrite_impact(
            &mut scheduler,
            &surfaces[0].arena,
            0,
            eq_id,
            &mut dirty_trace,
        );

        let mut found_refreshed_root = false;
        for _ in 0..32 {
            let Some((level, surface, node_id, mode)) = scheduler.pop_next(
                &surfaces,
                &rule_groups,
                RewriteConfig::optimised(),
                &mut dirty_trace,
            ) else {
                break;
            };
            if (level, surface, node_id, mode)
                == (1, 0, root_id, ScheduledMode::TraverseSubtreeRoot)
            {
                found_refreshed_root = true;
                scheduler.enqueue_after_no_rewrite(
                    &surfaces[0].arena,
                    0,
                    root_id,
                    1,
                    2,
                    ScheduledMode::TraverseSubtreeRoot,
                    &rule_groups,
                    RewriteConfig::optimised(),
                    Some(&mut dirty_trace),
                );
                break;
            }
            scheduler.enqueue_after_no_rewrite(
                &surfaces[surface].arena,
                surface,
                node_id,
                level,
                level + 1,
                mode,
                &rule_groups,
                RewriteConfig::optimised(),
                Some(&mut dirty_trace),
            );
        }
        assert!(found_refreshed_root);

        let mut found_sibling_at_next_level = false;
        for _ in 0..32 {
            let Some((level, _surface, node_id, mode)) = scheduler.pop_next(
                &surfaces,
                &rule_groups,
                RewriteConfig::optimised(),
                &mut dirty_trace,
            ) else {
                break;
            };
            if (level, node_id, mode)
                == (1, root_sibling_id, ScheduledMode::TraverseSubtreeDescendant)
            {
                found_sibling_at_next_level = true;
                break;
            }
        }
        assert!(found_sibling_at_next_level);
    }

    #[test]
    fn worklist_prunes_child_subtrees_without_candidates_at_level() {
        let eq = Expr::Eq(Metadata::new(), Moo::new(int_lit(1)), Moo::new(int_lit(2)));
        let tree = root(vec![eq.clone(), int_lit(3)]);
        let surfaces = vec![RewriteSurface::root(ExpressionArena::from_root(tree))];
        let arena = &surfaces[0].arena;
        let ids = rewriter_preorder_ids(arena);
        let root_id = ids[0];
        let eq_id = ids[1];

        let rule_groups = test_rule_groups_targeting_expr(&eq);
        let mut scheduler =
            WorklistScheduler::new(&surfaces, &rule_groups, RewriteConfig::optimised());
        let mut dirty_trace = DirtyTrace::default();

        assert_eq!(
            scheduler.pop_next(
                &surfaces,
                &rule_groups,
                RewriteConfig::optimised(),
                &mut dirty_trace
            ),
            Some((0, 0, root_id, ScheduledMode::TraverseSubtreeRoot))
        );
        scheduler.enqueue_after_no_rewrite(
            arena,
            0,
            root_id,
            0,
            1,
            ScheduledMode::TraverseSubtreeRoot,
            &rule_groups,
            RewriteConfig::optimised(),
            Some(&mut dirty_trace),
        );

        assert_eq!(
            scheduler.pop_next(
                &surfaces,
                &rule_groups,
                RewriteConfig::optimised(),
                &mut dirty_trace
            ),
            Some((0, 0, eq_id, ScheduledMode::TraverseSubtreeDescendant))
        );
        assert_eq!(
            scheduler.pop_next(
                &surfaces,
                &rule_groups,
                RewriteConfig::optimised(),
                &mut dirty_trace
            ),
            None
        );
    }

    #[test]
    fn worklist_no_rewrite_skips_levels_without_candidates_for_subtree() {
        let tree = root(vec![int_lit(1)]);
        let surfaces = vec![RewriteSurface::root(ExpressionArena::from_root(tree))];
        let arena = &surfaces[0].arena;
        let ids = rewriter_preorder_ids(arena);
        let root_id = ids[0];
        let child_id = ids[1];

        let rule_groups = test_rule_groups_with_no_candidate_middle_level();
        let mut scheduler =
            WorklistScheduler::new(&surfaces, &rule_groups, RewriteConfig::optimised());
        let mut dirty_trace = DirtyTrace::default();

        assert_eq!(
            scheduler.pop_next(
                &surfaces,
                &rule_groups,
                RewriteConfig::optimised(),
                &mut dirty_trace
            ),
            Some((0, 0, root_id, ScheduledMode::TraverseSubtreeRoot))
        );

        scheduler.enqueue_after_no_rewrite(
            arena,
            0,
            root_id,
            0,
            1,
            ScheduledMode::TraverseSubtreeRoot,
            &rule_groups,
            RewriteConfig::optimised(),
            Some(&mut dirty_trace),
        );

        assert_eq!(
            scheduler.pop_next(
                &surfaces,
                &rule_groups,
                RewriteConfig::optimised(),
                &mut dirty_trace
            ),
            Some((0, 0, child_id, ScheduledMode::TraverseSubtreeDescendant))
        );
        assert_eq!(
            scheduler.pop_next(
                &surfaces,
                &rule_groups,
                RewriteConfig::optimised(),
                &mut dirty_trace
            ),
            Some((2, 0, root_id, ScheduledMode::TraverseSubtreeRoot))
        );
    }

    #[test]
    fn worklist_reference_invalidation_schedules_affected_path_once() {
        let x = Name::user("x");
        let tree = root(vec![
            Expr::Eq(
                Metadata::new(),
                Moo::new(reference_expr(&x)),
                Moo::new(int_lit(1)),
            ),
            int_lit(2),
        ]);
        let arena = ExpressionArena::from_root(tree);
        let ids = rewriter_preorder_ids(&arena);
        let root_id = ids[0];
        let eq_id = ids[1];
        let reference_id = ids[2];

        let mut affected_nodes = Vec::new();
        collect_worklist_nodes_referencing_names(
            &arena,
            arena.root(),
            std::slice::from_ref(&x),
            &mut affected_nodes,
        );

        assert_eq!(
            affected_nodes.into_iter().rev().collect_vec(),
            vec![root_id, eq_id, reference_id]
        );
    }

    /// After a rewrite, only the replaced node and its ancestors are invalidated; siblings keep
    /// their clean marks and must not be re-scanned from the top.
    #[test]
    fn rewrite_dirty_invalidation_preserves_root_sibling_clean_marks() {
        let priority = 5u16;

        let sib2 = int_lit(2);
        let sib3 = int_lit(3);
        sib2.meta_ref().mark_clean_for_rule_priority(priority);
        sib3.meta_ref().mark_clean_for_rule_priority(priority);

        let tree = root(vec![int_lit(1), sib2, sib3]);
        let (mut arena, node_id) = arena_with_preorder_focus(tree, 1);

        let mut dirty_trace = DirtyTrace::default();
        replace_focus_and_dirty_ancestors(
            &mut arena,
            node_id,
            clear_expr_clean_rule_metadata(int_lit(10)),
            &mut dirty_trace,
            None,
        );
        let new_root = arena.into_root_expression();

        let Expr::Root(_, constraints) = &new_root else {
            panic!("expected root expression");
        };

        assert_not_clean_at(&new_root, priority);
        assert_not_clean_at(&constraints[0], priority);
        assert_clean_at(&constraints[1], priority);
        assert_clean_at(&constraints[2], priority);
    }

    /// Invalidation walks up the parent chain only; the other side of a binary node is a sibling.
    #[test]
    fn rewrite_dirty_invalidation_preserves_binary_sibling_clean_marks() {
        let priority = 5u16;

        let right = int_lit(2);
        right.meta_ref().mark_clean_for_rule_priority(priority);
        let eq = Expr::Eq(Metadata::new(), Moo::new(int_lit(1)), Moo::new(right));
        let tree = root(vec![eq]);
        let (mut arena, node_id) = arena_with_preorder_focus(tree, 2);

        let mut dirty_trace = DirtyTrace::default();
        replace_focus_and_dirty_ancestors(
            &mut arena,
            node_id,
            clear_expr_clean_rule_metadata(int_lit(10)),
            &mut dirty_trace,
            None,
        );
        let new_root = arena.into_root_expression();

        let Expr::Root(_, constraints) = &new_root else {
            panic!("expected root expression");
        };
        let Expr::Eq(_, _, right) = &constraints[0] else {
            panic!("expected equality at root child");
        };

        assert_not_clean_at(&new_root, priority);
        assert_not_clean_at(&constraints[0], priority);
        assert_clean_at(right.as_ref(), priority);
    }

    /// Cousins inside a shared parent container must also keep their clean marks.
    #[test]
    fn rewrite_dirty_invalidation_preserves_matrix_sibling_clean_marks() {
        let priority = 5u16;

        let sibling = int_lit(2);
        sibling.meta_ref().mark_clean_for_rule_priority(priority);
        let sum = Expr::Sum(Metadata::new(), Moo::new(matrix_expr![int_lit(1), sibling]));
        let tree = root(vec![sum]);
        let (mut arena, node_id) = arena_with_preorder_focus(tree, 3);

        let mut dirty_trace = DirtyTrace::default();
        replace_focus_and_dirty_ancestors(
            &mut arena,
            node_id,
            clear_expr_clean_rule_metadata(int_lit(10)),
            &mut dirty_trace,
            None,
        );
        let new_root = arena.into_root_expression();

        let Expr::Root(_, constraints) = &new_root else {
            panic!("expected root expression");
        };
        let Expr::Sum(_, matrix) = &constraints[0] else {
            panic!("expected sum at root child");
        };
        let Expr::AbstractLiteral(_, matrix_lit) = matrix.as_ref() else {
            panic!("expected matrix literal in sum");
        };
        let crate::ast::AbstractLiteral::Matrix(elements, _) = matrix_lit else {
            panic!("expected matrix literal");
        };

        assert_not_clean_at(&new_root, priority);
        assert_not_clean_at(&constraints[0], priority);
        assert_not_clean_at(&elements[0], priority);
        assert_clean_at(&elements[1], priority);
    }

    #[test]
    fn targeted_symbol_invalidation_preserves_unrelated_sibling_clean_marks() {
        use crate::ast::{Domain, Range, Reference};

        let priority = 5u16;
        let unrelated = int_lit(2);
        unrelated.meta_ref().mark_clean_for_rule_priority(priority);

        let x = Name::user("x");
        let ref_x = Expr::Atomic(
            Metadata::new(),
            Atom::Reference(Reference::new(DeclarationPtr::new_find(
                x.clone(),
                Domain::int(vec![Range::Bounded(1, 3)]),
            ))),
        );
        ref_x.meta_ref().mark_clean_for_rule_priority(priority);

        let tree = root(vec![ref_x, unrelated]);
        let cleared = clear_expr_clean_rule_metadata_for_names(tree, std::slice::from_ref(&x));

        let Expr::Root(_, constraints) = cleared else {
            panic!("expected root expression");
        };

        assert_not_clean_at(&constraints[0], priority);
        assert_clean_at(&constraints[1], priority);
    }

    #[test]
    fn fresh_symbol_effects_do_not_require_arena_reimport() {
        use crate::ast::{Domain, Range, SymbolTable};
        use crate::rule_engine::RuleEffect;

        let symbols = SymbolTable::new();
        let mut effect_symbols = symbols.clone();
        effect_symbols.gen_find(&Domain::int(vec![Range::Bounded(1, 3)]));

        let effect = RuleEffect::new(int_lit(1), vec![int_lit(2)], effect_symbols);
        let impact = RuleEffectImpact::new(&effect, &symbols);

        assert!(impact.has_model_side_effects());
        assert!(impact.changes_symbol_context());
        assert_eq!(impact.added_names.len(), 1);
        assert!(impact.changed_names.is_empty());
        assert!(!impact.requires_arena_reimport_for_invalidation());
    }

    #[test]
    fn changed_symbol_effects_still_require_arena_reimport() {
        use crate::ast::{Domain, Range, SymbolTable};
        use crate::rule_engine::RuleEffect;

        let x = Name::user("x");
        let mut symbols = SymbolTable::new();
        symbols
            .insert(DeclarationPtr::new_find(
                x.clone(),
                Domain::int(vec![Range::Bounded(1, 3)]),
            ))
            .expect("fresh symbol should insert");

        let mut effect_symbols = symbols.clone();
        effect_symbols.update_insert(DeclarationPtr::new_find(
            x.clone(),
            Domain::int(vec![Range::Bounded(1, 4)]),
        ));

        let effect = RuleEffect::with_symbols(int_lit(1), effect_symbols);
        let impact = RuleEffectImpact::new(&effect, &symbols);

        assert!(impact.has_model_side_effects());
        assert!(impact.changes_symbol_context());
        assert!(impact.added_names.is_empty());
        assert_eq!(impact.changed_names, vec![x]);
        assert!(impact.requires_arena_reimport_for_invalidation());
    }

    #[test]
    fn new_top_without_symbol_changes_stays_in_arena() {
        use crate::ast::SymbolTable;
        use crate::rule_engine::RuleEffect;

        let symbols = SymbolTable::new();
        let effect = RuleEffect::with_top(int_lit(1), vec![int_lit(2)]);
        let impact = RuleEffectImpact::new(&effect, &symbols);

        assert!(impact.has_model_side_effects());
        assert!(!impact.changes_symbol_context());
        assert!(impact.added_names.is_empty());
        assert!(impact.changed_names.is_empty());
        assert!(!impact.requires_arena_reimport_for_invalidation());
    }

    #[test]
    fn rule_applicability_memo_is_scoped_to_node_and_context_generations() {
        let mut memo = RuleApplicabilityMemo::default();
        let (arena, node_id) = arena_with_preorder_focus(int_lit(1), 0);
        let generation = arena.generation(node_id);

        assert!(!memo.is_known_failure(0, node_id, "constant_evaluator", generation, 0));
        memo.record_failure(0, node_id, "constant_evaluator", generation, 0);
        assert!(memo.is_known_failure(0, node_id, "constant_evaluator", generation, 0));
        assert!(!memo.is_known_failure(
            0,
            node_id,
            "constant_evaluator",
            generation.wrapping_add(1),
            0
        ));
        assert!(!memo.is_known_failure(0, node_id, "constant_evaluator", generation, 1));
        assert!(!memo.is_known_failure(1, node_id, "constant_evaluator", generation, 0));
    }
}
