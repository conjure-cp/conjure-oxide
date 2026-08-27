use super::{AtomKind, RewriteError, RulePrefilter, RuleSet, resolve_rules::RuleData};
use crate::{
    Model,
    ast::{
        Atom, Expression as Expr, ExpressionArena, ExpressionNodeId, Metadata, Name,
        discriminant_from_value, finish_root_evaluator_normalisation, normalise_evaluator_local,
        normalise_root_constraint_deep,
    },
    bug,
    objective::introduce_objective_auxiliary,
    rule_engine::{
        get_rules_grouped,
        rewriter_common::{
            RuleResult, VariableDeclarationSnapshot, choose_rule_result_index,
            log_rule_application, root_variable_snapshot_for_default_trace,
            snapshot_symbols_after_effect, try_rewrite_value_letting_once,
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
    io::Write as IoWrite,
    path::PathBuf,
    time::Instant,
};
use tracing::trace;
use uniplate::Uniplate;

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
    attempted_expressions: usize,
    rule_attempts: usize,
    rewrites: usize,
    value_letting_rewrites: usize,
    side_effects_kept_in_arena: usize,
    replacement_subtree_clears: usize,
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
    rule_attempts_by_priority: BTreeMap<u16, usize>,
    rule_attempts_by_rule: BTreeMap<String, usize>,
    rewrites_by_rule: BTreeMap<String, usize>,
    side_effect_rewrites_by_rule: BTreeMap<String, usize>,
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

    fn record_side_effect_kept_in_arena(&mut self) {
        if !self.enabled {
            return;
        }
        self.side_effects_kept_in_arena += 1;
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
            "[dirty-trace] stats_rule_attempts={}",
            stats.rewriter_rule_application_attempts.unwrap_or(0)
        )
        .unwrap();
        writeln!(output, "[dirty-trace] rewrites={}", self.rewrites).unwrap();
        writeln!(
            output,
            "[dirty-trace] value_letting_rewrites={}",
            self.value_letting_rewrites
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
            && let Err(error) = fs::create_dir_all(parent)
        {
            eprintln!(
                "[dirty-trace] failed to create trace directory {}: {error}",
                parent.display()
            );
            eprint!("{output}");
            return;
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

#[derive(Clone)]
struct RuleGroup<'a> {
    priority: u16,
    rules: Vec<RuleData<'a>>,
    /// Indexed by discriminant id for O(1) lookup of simple variant prefilters.
    rules_by_discriminant: Vec<Option<Vec<RuleData<'a>>>>,
    universal_rules: Vec<RuleData<'a>>,
    has_non_discriminant_filters: bool,
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
                    .filter(|rd| {
                        rule_is_universal(rd) || rule_matches_self_discriminant(rd, discriminant)
                    })
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
        Self {
            priority,
            rules,
            rules_by_discriminant,
            universal_rules,
            has_non_discriminant_filters,
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
            // Always keep universal rules eligible. A matching child/atom
            // prefilter on a sibling rule does not mean universals are
            // inapplicable — suppressing them here previously skipped
            // `matrix_to_list` on `SafeIndex` when `matrix_ref_to_atom` matched
            // `* / Atomic` (permMultElementId).
            return CandidateRules::Filtered {
                iter: self.rules.iter(),
                expr,
                include_universal: true,
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

#[derive(Clone, Copy)]
struct WorklistSchedulingContext<'arena, 'groups, 'rules> {
    arena: &'arena ExpressionArena,
    surface: usize,
    rule_groups: &'groups [RuleGroup<'rules>],
    config: RewriteConfig,
}

impl<'arena, 'groups, 'rules> WorklistSchedulingContext<'arena, 'groups, 'rules> {
    fn new(
        arena: &'arena ExpressionArena,
        surface: usize,
        rule_groups: &'groups [RuleGroup<'rules>],
        config: RewriteConfig,
    ) -> Self {
        Self {
            arena,
            surface,
            rule_groups,
            config,
        }
    }
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
        dirty_trace: Option<&mut DirtyTrace>,
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
            dirty_trace,
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
        context: WorklistSchedulingContext<'_, '_, '_>,
        node_id: ExpressionNodeId,
        level: usize,
        mut dirty_trace: Option<&mut DirtyTrace>,
    ) -> usize {
        if matches!(context.arena.expression(node_id), Expr::Comprehension(_, _)) {
            return 0;
        }

        let mut child_count = 0;
        for &child_id in context.arena.children(node_id) {
            if !self.subtree_has_candidates_at_level(
                context.arena,
                context.surface,
                child_id,
                level,
                context.rule_groups,
                context.config,
            ) {
                continue;
            }
            child_count += 1;
            self.enqueue_node_at_level(
                context.arena,
                context.surface,
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
                    if let Some(trace) = dirty_trace {
                        trace.record_worklist_enqueue(mode);
                    }
                }
            }
        }
    }

    fn enqueue_after_no_rewrite(
        &mut self,
        context: WorklistSchedulingContext<'_, '_, '_>,
        node_id: ExpressionNodeId,
        level: usize,
        next_self_level: usize,
        mode: ScheduledMode,
        mut dirty_trace: Option<&mut DirtyTrace>,
    ) {
        if mode.descends_on_failure() {
            let child_count =
                self.enqueue_children_at_level(context, node_id, level, dirty_trace.as_deref_mut());
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
                context.arena,
                context.surface,
                node_id,
                next_self_level,
                context.rule_groups,
                context.config,
            )
        } else {
            next_worklist_candidate_level(
                context.arena,
                node_id,
                next_self_level,
                context.rule_groups,
                context.config,
            )
        };
        self.enqueue_node_at_level(
            context.arena,
            context.surface,
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
        let mut changed_names: Vec<_> = effect
            .changed_symbols(symbols)
            .into_iter()
            .map(|(name, _, _)| name)
            .collect();
        for name in effect.updated_declaration_names() {
            if !changed_names.contains(&name) {
                changed_names.push(name);
            }
        }
        Self {
            added_names: effect.added_symbols(symbols).into_iter().collect(),
            changed_names,
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
}

struct RewritePassContext<'ctx, 'rules> {
    rules_grouped: &'ctx Vec<(u16, Vec<RuleData<'rules>>)>,
    bucketed_rules: &'ctx Vec<RuleGroup<'rules>>,
    prop_multiple_equally_applicable: bool,
    stats: &'ctx mut RewriterStats,
    dirty_trace: &'ctx mut DirtyTrace,
    config: RewriteConfig,
    #[cfg(debug_assertions)]
    run_start: &'ctx Instant,
}

/// True when a domain still needs Essence→Essence' abstract representation
/// (set/tuple/record/…); concrete bool/int and matrices of those do not.
fn domain_needs_abstract_repr(domain: &crate::ast::DomainPtr) -> bool {
    domain_needs_abstract_repr_at(domain, true)
}

/// `is_value` is false for a matrix's index domains, which describe which entries exist rather
/// than a value the solver assigns, and so are never encoded.
fn domain_needs_abstract_repr_at(domain: &crate::ast::DomainPtr, is_value: bool) -> bool {
    use crate::ast::{Domain, GroundDomain, UnresolvedDomain};
    let int_is_abstract = is_value && crate::settings::ints_need_representation();
    match domain.as_ref() {
        Domain::Ground(gd) => match gd.as_ref() {
            GroundDomain::Empty(..) | GroundDomain::Bool => false,
            GroundDomain::Int(_) => int_is_abstract,
            // Every matrix has a layout to choose between, so representation selection has work
            // to do whatever the elements are.
            GroundDomain::Matrix(..) => true,
            _ => true,
        },
        Domain::Unresolved(ud) => match ud.as_ref() {
            UnresolvedDomain::Int(..) => int_is_abstract,
            UnresolvedDomain::Matrix(..) => true,
            UnresolvedDomain::Reference(re) => re
                .domain()
                .is_some_and(|d| domain_needs_abstract_repr_at(&d, is_value)),
            _ => true,
        },
    }
}

/// Whether `ReprGeneral` / `ReprTuplePacked` can do useful work on this model.
///
/// Models that only use bools, ints, and matrices of those (e.g. solitaire_battleship)
/// still paid hundreds of thousands of failed attempts on record/tuple/set rules.
fn model_needs_abstract_repr_rules(model: &Model) -> bool {
    use crate::ast::{AbstractLiteral, Atom, Expression as Expr, Literal};

    for (_, decl) in model.symbols().iter_local() {
        if let Some(domain) = decl.domain()
            && domain_needs_abstract_repr(&domain)
        {
            return true;
        }
    }

    // Record/tuple/set *literals* also need ReprGeneral even without abstract finds.
    // Matrix literals are handled by `ReprMatrixComponents`, not these rule sets.
    for expr in model.root().universe() {
        match expr {
            Expr::AbstractLiteral(_, abs) if !matches!(abs, AbstractLiteral::Matrix(..)) => {
                return true;
            }
            Expr::Atomic(_, Atom::Literal(Literal::AbstractLiteral(abs)))
                if !matches!(abs, AbstractLiteral::Matrix(..)) =>
            {
                return true;
            }
            _ => {}
        }
    }

    false
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

    let needs_abstract_repr = model_needs_abstract_repr_rules(model);
    let filtered_rule_sets: Vec<&'a RuleSet<'a>> = if needs_abstract_repr {
        rule_sets.clone()
    } else {
        rule_sets
            .iter()
            .copied()
            .filter(|rs| rs.name != "ReprGeneral" && rs.name != "ReprTuplePacked")
            .collect()
    };

    let rules_grouped = get_rules_grouped(&filtered_rule_sets)
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

    // Flatten top-level `and` into the root constraint list (and strip `true` / propagate
    // `false`) after rewriting. Must not run mid-loop: early flattening of large expanded
    // conjunctions explodes worklist rule attempts. Solver adaptors expect one flat constraint
    // per root entry.
    if let Some(normalised_root) = finish_root_evaluator_normalisation(model.root()) {
        model.replace_root(normalised_root);
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
    if !ctx.config.worklist
        && try_rewrite_value_letting_once(
            submodel,
            ctx.rules_grouped,
            ctx.prop_multiple_equally_applicable,
        )
        .is_some()
    {
        ctx.dirty_trace.value_letting_rewrites += 1;
        increment_counter(&mut ctx.stats.rewriter_value_letting_rewrites);
        return Some(());
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
        // Iterate over rules by priority in descending order.
        'top: for (level, rule_group) in ctx.bucketed_rules.iter().enumerate() {
            ctx.dirty_trace.priority_scans += 1;
            for &node_id in &preorder_ids {
                ctx.dirty_trace.expression_visits += 1;
                let mut attempted_rule = false;
                {
                    let expr = arena.expression(node_id);
                    for rd in rule_group.candidates(ctx.config, expr) {
                        attempted_rule = true;
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
                                    root_variable_snapshot_for_default_trace(
                                        expr,
                                        &submodel.symbols(),
                                    ),
                                ));
                            }
                            Err(_) => {
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

        if !results.is_empty() {
            if ctx.prop_multiple_equally_applicable {
                assert_no_multiple_equally_applicable_rules(&results, ctx.rules_grouped);
            }
            let selected =
                choose_rule_result_index(results.iter().map(|(result, _, _, _, _)| result));
            results.swap(0, selected);
        }

        match results.as_slice() {
            [] => {
                submodel.replace_root(arena.into_root_expression());
                break;
            }
            [
                (result, _level, expr, node_id, variable_snapshot_before),
                ..,
            ] => {
                let effect = result.effect.materialise(&submodel.symbols());
                let variable_snapshots = variable_snapshot_before.clone().map(|before| {
                    let after = snapshot_symbols_after_effect(&submodel.symbols(), &effect);
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
                let rule_name = result.rule_data.rule.name;
                let RuleResult { effect, .. } = result;
                let crate::rule_engine::rule::RuleEffect {
                    new_expression,
                    new_top,
                    symbols,
                    new_clauses,
                    declaration_updates,
                    ..
                } = effect;
                // Replace expr with new_expression
                replace_focus_and_sync_ancestors(&mut arena, *node_id, new_expression);

                // Apply new symbols and top level
                ctx.dirty_trace
                    .record_rewrite(rule_name, has_model_side_effects);
                for update in declaration_updates {
                    update.apply();
                }
                submodel.symbols_mut().extend(symbols);
                if effect_impact.has_new_top {
                    arena.add_root_children(new_top);
                }
                submodel.add_clauses(new_clauses);
                let _ =
                    normalise_evaluators_from_node_to_root(&mut arena, *node_id, ctx.dirty_trace);
                if has_model_side_effects {
                    ctx.dirty_trace.record_side_effect_kept_in_arena();
                }

                #[cfg(debug_assertions)]
                {
                    // Check well-formedness without rebuilding the live arena: a rebuild would
                    // renumber nodes and can change subsequent full-scan order vs release builds.
                    submodel.replace_root(arena.clone().into_root_expression());
                    let assertion_context = format!("rewriter after applying rule '{rule_name}'");
                    debug_assert_model_well_formed(submodel, &assertion_context);
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
        if !rule_group.has_candidates(
            ctx.config,
            surfaces[surface_index].arena.expression(node_id),
        ) {
            ctx.dirty_trace
                .record_worklist_no_candidate_pop(scheduled_mode);
            scheduler.enqueue_after_no_rewrite(
                WorklistSchedulingContext::new(
                    &surfaces[surface_index].arena,
                    surface_index,
                    ctx.bucketed_rules,
                    ctx.config,
                ),
                node_id,
                level,
                level + 1,
                scheduled_mode,
                Some(ctx.dirty_trace),
            );
            continue;
        }

        let mut results: Vec<ApplicableRule<'_, ExpressionNodeId>> = vec![];
        let mut attempted_rule = false;
        {
            let expr = surfaces[surface_index].arena.expression(node_id);
            for rd in rule_group.candidates(ctx.config, expr) {
                attempted_rule = true;
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
                            root_variable_snapshot_for_default_trace(expr, &submodel.symbols()),
                        ));
                    }
                    Err(_) =>
                    {
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
        }

        if attempted_rule {
            ctx.dirty_trace.attempted_expressions += 1;
            ctx.dirty_trace
                .record_worklist_rule_attempt_pop(scheduled_mode);
        }

        if results.is_empty() {
            scheduler.enqueue_after_no_rewrite(
                WorklistSchedulingContext::new(
                    &surfaces[surface_index].arena,
                    surface_index,
                    ctx.bucketed_rules,
                    ctx.config,
                ),
                node_id,
                level,
                level + 1,
                scheduled_mode,
                Some(ctx.dirty_trace),
            );
            continue;
        }

        if ctx.prop_multiple_equally_applicable {
            assert_no_multiple_equally_applicable_rules(&results, ctx.rules_grouped);
        }

        let selected = choose_rule_result_index(results.iter().map(|(result, _, _, _, _)| result));
        results.swap(0, selected);

        let [
            (result, _level, expr, node_id, variable_snapshot_before),
            ..,
        ] = results.as_slice()
        else {
            unreachable!("checked non-empty results above")
        };

        let effect = result.effect.materialise(&submodel.symbols());
        let variable_snapshots = variable_snapshot_before.clone().map(|before| {
            let after = snapshot_symbols_after_effect(&submodel.symbols(), &effect);
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
        let rule_name = result.rule_data.rule.name;
        let RuleResult { effect, .. } = result;
        let crate::rule_engine::rule::RuleEffect {
            new_expression,
            new_top,
            symbols,
            new_clauses,
            declaration_updates,
            ..
        } = effect;
        {
            let arena = &mut surfaces[surface_index].arena;
            replace_focus_and_sync_ancestors(arena, *node_id, new_expression);
        }

        ctx.dirty_trace
            .record_rewrite(rule_name, has_model_side_effects);
        for update in declaration_updates {
            update.apply();
        }
        submodel.symbols_mut().extend(symbols);
        let new_top_node_ids = if effect_impact.has_new_top {
            surfaces[root_surface].arena.add_root_children(new_top)
        } else {
            Vec::new()
        };
        submodel.add_clauses(new_clauses);
        let (rewrite_impact_node_id, _) = {
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
        {
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
            // Well-formedness only: do not rebuild surfaces/scheduler here. A full rebuild after
            // every rule changes same-priority sibling order vs release (incremental enqueue).
            write_worklist_surfaces_to_model(submodel, &surfaces);
            let assertion_context = format!("rewriter after applying rule '{rule_name}'");
            debug_assert_model_well_formed(submodel, &assertion_context);
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
    normalise_evaluators_subtree_bottom_up(arena, arena.root(), dirty_trace);
}

fn normalise_evaluators_subtree_bottom_up(
    arena: &mut ExpressionArena,
    subtree_root: ExpressionNodeId,
    dirty_trace: &mut DirtyTrace,
) -> bool {
    let nodes = rewriter_reachable_subtree_ids(arena, subtree_root);
    let mut changed = false;

    for node_id in nodes.into_iter().rev() {
        if !arena.is_reachable(node_id) {
            continue;
        }
        changed |= normalise_evaluator_node_to_fixpoint(arena, node_id, dirty_trace);
    }

    changed
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
    // Ordinary rewrites can introduce fresh nested arithmetic whose children were not previously
    // scheduled. Normalise the replacement subtree bottom-up first, then walk upward so evaluator
    // simplification remains privileged without running ordinary rules to a fixpoint.
    let subtree_changed = normalise_evaluators_subtree_bottom_up(arena, node_id, dirty_trace);
    let mut came_from = node_id;
    let mut current = arena.parent(node_id);
    let mut highest_rewritten = node_id;
    let mut changed = subtree_changed;

    while let Some(current_id) = current {
        if !arena.is_reachable(current_id) {
            break;
        }

        let rewritten = if current_id == arena.root() {
            // Only deep-normalise the root child that contains the rewrite. Full-root selective
            // deep on every upward walk re-traverses every non-flat sibling and dominates CPU on
            // large models (lee-distance profile).
            normalise_root_evaluator_for_child(arena, came_from, dirty_trace)
        } else {
            normalise_evaluator_node_to_fixpoint(arena, current_id, dirty_trace)
        };
        if rewritten {
            highest_rewritten = current_id;
            changed = true;
        }
        came_from = current_id;
        current = arena.parent(current_id);
    }

    (highest_rewritten, changed)
}

/// Deep-normalises only the root constraint that `child_id` belongs to.
///
/// `child_id` must be a direct child of the arena root. Sibling constraints are left untouched.
///
/// The replacement is written to `child_id` rather than to the root. Rebuilding the root here
/// would clone every sibling constraint and re-import the whole model into the arena, which this
/// hook cannot afford: it runs after every rewrite.
fn normalise_root_evaluator_for_child(
    arena: &mut ExpressionArena,
    child_id: ExpressionNodeId,
    dirty_trace: &mut DirtyTrace,
) -> bool {
    let root_id = arena.root();
    if !arena.children(root_id).contains(&child_id) {
        return normalise_evaluator_node_to_fixpoint(arena, root_id, dirty_trace);
    }

    let Some(replacement) = normalise_root_constraint_deep(arena.expression(child_id)) else {
        return false;
    };

    dirty_trace.replacement_subtree_clears += 1;
    arena.replace_subtree(child_id, replacement);
    dirty_trace.record_rewrite("evaluator_normalisation_hook", false);
    sync_ancestor_payloads(arena, child_id);
    true
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
        arena.replace_subtree(node_id, replacement);
        dirty_trace.record_rewrite("evaluator_normalisation_hook", false);
        changed = true;
    }

    if changed {
        sync_ancestor_payloads(arena, node_id);
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
        let letting_expr = decl.as_value_letting().map(|expr| expr.clone());
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
    declaration.as_value_letting().map(|expr| expr.clone())
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

    let mut wrote_value_letting = false;
    for surface in surfaces.iter().skip(1) {
        if !surface.active {
            continue;
        }
        let Some(name) = value_letting_surface_name(&surface.kind) else {
            continue;
        };
        wrote_value_letting |=
            write_value_letting_surface_to_model_without_refresh(submodel, name, &surface.arena);
    }
    if wrote_value_letting {
        submodel.symbols_mut().refresh_local_binding_hashes();
    }
}

fn write_value_letting_surface_to_model(
    submodel: &mut Model,
    name: &Name,
    arena: &ExpressionArena,
) -> bool {
    let written = write_value_letting_surface_to_model_without_refresh(submodel, name, arena);
    if written {
        submodel.symbols_mut().refresh_local_binding_hashes();
    }
    written
}

fn write_value_letting_surface_to_model_without_refresh(
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

fn increment_counter(counter: &mut Option<usize>) {
    *counter = Some(counter.unwrap_or(0) + 1);
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

/// Replaces the focused expression and updates ancestor payloads to match it.
fn replace_focus_and_sync_ancestors(
    arena: &mut ExpressionArena,
    node_id: ExpressionNodeId,
    new_focus: Expr,
) {
    arena.replace_subtree(node_id, new_focus);
    sync_ancestor_payloads(arena, node_id);
}

fn sync_ancestor_payloads(arena: &mut ExpressionArena, node_id: ExpressionNodeId) {
    let mut child_id = node_id;
    let mut ancestor = arena.parent(node_id);
    while let Some(ancestor_id) = ancestor {
        // Same-arity parent update: clone only the changed child into the parent payload.
        arena.sync_payload_for_changed_child(ancestor_id, child_id);
        child_id = ancestor_id;
        ancestor = arena.parent(ancestor_id);
    }
}

fn take_model_root(model: &mut Model) -> Expr {
    model.replace_root(Expr::Root(Metadata::new(), Vec::new()))
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
    fn rule_group_includes_universal_rules_in_variant_buckets() {
        let lex_discriminant = discriminant_from_value(&Expr::LexLt(
            Metadata::new(),
            Moo::new(int_lit(1)),
            Moo::new(int_lit(2)),
        ));
        let lex_prefilters: &'static [RulePrefilter] =
            Box::leak(Box::new([RulePrefilter::Variant(lex_discriminant)]));
        let variant_rule: &'static crate::rule_engine::Rule<'static> =
            Box::leak(Box::new(crate::rule_engine::Rule {
                name: "variant-specific-test-rule",
                application: never_apply_test_rule,
                rule_sets: &[("test-rule-set", 1)],
                prefilters: Some(lex_prefilters),
            }));
        let universal_rule: &'static crate::rule_engine::Rule<'static> =
            Box::leak(Box::new(crate::rule_engine::Rule {
                name: "universal-test-rule",
                application: never_apply_test_rule,
                rule_sets: &[("test-rule-set", 1)],
                prefilters: None,
            }));
        let rule_group = RuleGroup::new(
            1,
            vec![
                crate::rule_engine::RuleData {
                    rule: variant_rule,
                    priority: 1,
                    rule_set: &TEST_RULE_SET,
                },
                crate::rule_engine::RuleData {
                    rule: universal_rule,
                    priority: 1,
                    rule_set: &TEST_RULE_SET,
                },
            ],
        );
        let config = RewriteConfig::optimised();
        let lex = Expr::LexLt(Metadata::new(), Moo::new(int_lit(1)), Moo::new(int_lit(2)));

        assert!(rule_group.has_candidates(config, &lex));
        assert_eq!(
            rule_group
                .candidates(config, &lex)
                .map(|rule_data| rule_data.rule.name)
                .collect_vec(),
            vec!["variant-specific-test-rule", "universal-test-rule"]
        );
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
            WorklistSchedulingContext::new(arena, 0, &rule_groups, RewriteConfig::optimised()),
            root_id,
            0,
            1,
            ScheduledMode::TraverseSubtreeRoot,
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
            WorklistSchedulingContext::new(arena, 0, &rule_groups, RewriteConfig::optimised()),
            eq_id,
            0,
            1,
            ScheduledMode::TraverseSubtreeDescendant,
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
            WorklistSchedulingContext::new(arena, 0, &rule_groups, RewriteConfig::optimised()),
            root_id,
            1,
            2,
            ScheduledMode::TraverseSubtreeRoot,
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
            WorklistSchedulingContext::new(
                &surfaces[0].arena,
                0,
                &rule_groups,
                RewriteConfig::optimised(),
            ),
            root_id,
            0,
            1,
            ScheduledMode::TraverseSubtreeRoot,
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
        replace_focus_and_sync_ancestors(&mut surfaces[0].arena, eq_id, int_lit(10));
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
                    WorklistSchedulingContext::new(
                        &surfaces[0].arena,
                        0,
                        &rule_groups,
                        RewriteConfig::optimised(),
                    ),
                    root_id,
                    1,
                    2,
                    ScheduledMode::TraverseSubtreeRoot,
                    Some(&mut dirty_trace),
                );
                break;
            }
            scheduler.enqueue_after_no_rewrite(
                WorklistSchedulingContext::new(
                    &surfaces[surface].arena,
                    surface,
                    &rule_groups,
                    RewriteConfig::optimised(),
                ),
                node_id,
                level,
                level + 1,
                mode,
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
            WorklistSchedulingContext::new(arena, 0, &rule_groups, RewriteConfig::optimised()),
            root_id,
            0,
            1,
            ScheduledMode::TraverseSubtreeRoot,
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
            WorklistSchedulingContext::new(arena, 0, &rule_groups, RewriteConfig::optimised()),
            root_id,
            0,
            1,
            ScheduledMode::TraverseSubtreeRoot,
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
}
