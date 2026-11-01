//! Compact's structural work estimate for Minion's collection representations.
//!
//! One primitive variable and one estimated expanded structural AST node each cost one unit.
//! Domain bits break ties. These are work counts, not fitted coefficients or predicted seconds.
//! Matrix/tuple children use their ordinary component layouts; unknown layouts, large probes
//! and unsupported domains decline the entire comparison, falling back to domain cardinality.
//! User constraints and search behaviour are deliberately not predicted by this estimate.

use conjure_cp::ast::comprehension::ComprehensionQualifier;
use conjure_cp::ast::{DeclarationPtr, DomainPtr, Expression, GroundDomain, Name};
use conjure_cp::representation::{ReprRulePtr, get_repr_rules};
use conjure_cp::settings::{Channelling, SolverFamily, channelling, try_current_solver_family};
use std::collections::HashMap;
use uniplate::Uniplate;

// Resource bounds, not scoring weights. Refuse lookahead rather than partially rank candidates.
const MAX_EXTENT: u64 = 256;
const MAX_DEPTH: usize = 8;
const MAX_VISITS: usize = 16_384;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Cost {
    variables: u64,
    structural_nodes: u64,
    domain_bits: u64,
}

impl Cost {
    fn add(self, other: Self) -> Option<Self> {
        Some(Self {
            variables: self.variables.checked_add(other.variables)?,
            structural_nodes: self.structural_nodes.checked_add(other.structural_nodes)?,
            domain_bits: self.domain_bits.checked_add(other.domain_bits)?,
        })
    }

    fn times(self, count: u64) -> Option<Self> {
        Some(Self {
            variables: self.variables.checked_mul(count)?,
            structural_nodes: self.structural_nodes.checked_mul(count)?,
            domain_bits: self.domain_bits.checked_mul(count)?,
        })
    }

    fn key(self) -> (u64, u64) {
        (
            self.variables.saturating_add(self.structural_nodes),
            self.domain_bits,
        )
    }
}

pub(super) fn choose(
    declaration: &DeclarationPtr,
    candidates: &[(ReprRulePtr, usize)],
) -> Option<ReprRulePtr> {
    if try_current_solver_family() != Some(SolverFamily::Minion) || channelling() != Channelling::No
    {
        return None;
    }
    let domain = declaration.domain()?;
    if !(declaration.as_find().is_some() || declaration.is_find_auxiliary())
        || !is_collection(&domain)
    {
        return None;
    }
    let mut estimates = Vec::new();
    for &(rule, old_score) in candidates {
        let mut estimator = Estimator::default();
        let Some(cost) = estimator.representation(declaration, rule, 0) else {
            tracing::info!(declaration = %declaration.name(), representation = rule.name(),
                "compact structural cost unavailable; using domain cardinality for this choice");
            return None;
        };
        tracing::info!(declaration = %declaration.name(), representation = rule.name(),
            variables = cost.variables, structural_nodes = cost.structural_nodes,
            domain_bits = cost.domain_bits, units = cost.key().0, domain_score = old_score,
            "compact representation cost");
        estimates.push((rule, cost, old_score));
    }
    let &(selected, _, _) = estimates
        .iter()
        .min_by_key(|(rule, cost, old_score)| (cost.key(), *old_score, rule.name()))?;
    tracing::info!(declaration = %declaration.name(), representation = selected.name(),
        "compact selected representation");
    Some(selected)
}

fn is_collection(domain: &DomainPtr) -> bool {
    domain.resolve().is_ok_and(|domain| {
        matches!(
            domain.as_ref(),
            GroundDomain::Function(..) | GroundDomain::Set(..) | GroundDomain::Relation(..)
        )
    })
}

struct Estimator {
    remaining: usize,
    domains: HashMap<DomainPtr, Option<Cost>>,
}

impl Default for Estimator {
    fn default() -> Self {
        Self {
            remaining: MAX_VISITS,
            domains: HashMap::new(),
        }
    }
}

impl Estimator {
    fn visit(&mut self, depth: usize) -> Option<()> {
        if depth > MAX_DEPTH {
            return None;
        }
        self.remaining = self.remaining.checked_sub(1)?;
        Some(())
    }

    fn representation(
        &mut self,
        decl: &DeclarationPtr,
        rule: ReprRulePtr,
        depth: usize,
    ) -> Option<Cost> {
        self.visit(depth)?;
        let domain = decl.domain()?.resolve().ok()?;
        // Check physical extents before asking initialisers to allocate variables or constraints.
        bounded_extent(domain.as_ref())?;
        let probe = rule.probe_structure_for(decl).ok()?;
        let mut cost = Cost::default();
        for constraint in &probe.constraints {
            cost.structural_nodes = cost
                .structural_nodes
                .checked_add(self.expression(constraint)?)?;
        }
        for child in probe.domains {
            cost = cost.add(self.domain(&child, depth + 1)?)?;
        }
        Some(cost)
    }

    fn domain(&mut self, domain: &DomainPtr, depth: usize) -> Option<Cost> {
        self.visit(depth)?;
        if let Some(cost) = self.domains.get(domain) {
            return *cost;
        }
        // Also stops a representation that accidentally introduces its own domain again.
        self.domains.insert(domain.clone(), None);
        let ground = domain.resolve().ok()?;
        let cost = match ground.as_ref() {
            GroundDomain::Bool | GroundDomain::Int(..) => {
                let cardinality = ground.length().ok()?;
                Cost {
                    variables: 1,
                    structural_nodes: 0,
                    domain_bits: u64::from(
                        u64::BITS - cardinality.saturating_sub(1).leading_zeros(),
                    ),
                }
            }
            GroundDomain::Matrix(inner, indices, preference) => {
                if preference.as_deref().is_some_and(|p| p != "components") {
                    return None;
                }
                let cells = product_lengths(indices.iter().map(|d| d.as_ref()))?;
                if cells > MAX_EXTENT {
                    return None;
                }
                self.domain(&inner.clone().into(), depth + 1)?
                    .times(cells)?
            }
            GroundDomain::Tuple(fields, preference) => {
                if preference.as_deref().is_some_and(|p| p != "components") {
                    return None;
                }
                let mut cost = Cost::default();
                for field in fields {
                    cost = cost.add(self.domain(&field.clone().into(), depth + 1)?)?;
                }
                cost
            }
            _ if is_collection(domain) => {
                let decl =
                    DeclarationPtr::new_find(Name::user("__compact_cost_probe"), domain.clone());
                let preference = domain.representation_preference();
                let mut rules: Vec<_> = get_repr_rules()
                    .filter(|rule| {
                        !rule.is_integer_encoding()
                            && rule.applies_to(SolverFamily::Minion)
                            && preference.is_none_or(|pref| rule.short_name() == pref)
                    })
                    .collect();
                rules.sort_by_key(|rule| rule.name());
                let mut best: Option<Cost> = None;
                for rule in rules {
                    if rule.probe_for(&decl).is_err() {
                        continue;
                    }
                    // Do not silently treat an applicable but unestimated layout as expensive.
                    let candidate = self.representation(&decl, rule, depth + 1)?;
                    if best.is_none_or(|cost| candidate.key() < cost.key()) {
                        best = Some(candidate);
                    }
                }
                best?
            }
            _ => return None,
        };
        self.domains.insert(domain.clone(), Some(cost));
        Some(cost)
    }

    fn expression(&mut self, expression: &Expression) -> Option<u64> {
        self.remaining = self.remaining.checked_sub(1)?;
        if let Expression::Comprehension(_, comprehension) = expression {
            let mut iterations = 1u64;
            let mut body = self.expression(&comprehension.return_expression)?;
            for qualifier in &comprehension.qualifiers {
                match qualifier {
                    ComprehensionQualifier::Generator { ptr } => {
                        iterations = iterations.checked_mul(ptr.domain()?.length().ok()?)?;
                    }
                    ComprehensionQualifier::ExpressionGenerator { ptr } => {
                        let source = ptr.as_quantified_expr()?;
                        let ground = source.domain_of()?.resolve().ok()?;
                        iterations = iterations.checked_mul(collection_extent(ground.as_ref())?)?;
                    }
                    ComprehensionQualifier::Condition(condition) => {
                        body = body.checked_add(self.expression(condition)?)?;
                    }
                }
            }
            // Guards may prune iterations; this deliberately uses the full upper bound.
            return 1u64.checked_add(iterations.checked_mul(body)?);
        }
        expression.children().iter().try_fold(1u64, |nodes, child| {
            nodes.checked_add(self.expression(child)?)
        })
    }
}

fn product_lengths<'a>(mut domains: impl Iterator<Item = &'a GroundDomain>) -> Option<u64> {
    domains.try_fold(1u64, |product, domain| {
        product.checked_mul(domain.length().ok()?)
    })
}

fn collection_extent(domain: &GroundDomain) -> Option<u64> {
    let extent = match domain {
        GroundDomain::Matrix(_, indices, _) => product_lengths(indices.iter().map(|d| d.as_ref()))?,
        GroundDomain::Set(attributes, inner) => attributes
            .size
            .high()
            .copied()
            .map(u64::try_from)
            .transpose()
            .ok()?
            .unwrap_or(inner.length().ok()?)
            .min(inner.length().ok()?),
        GroundDomain::Relation(attributes, fields) => {
            let universe = product_lengths(fields.iter().map(|d| d.as_ref()))?;
            attributes
                .size
                .high()
                .copied()
                .map(u64::try_from)
                .transpose()
                .ok()?
                .unwrap_or(universe)
                .min(universe)
        }
        _ => return None,
    };
    (extent <= MAX_EXTENT).then_some(extent)
}

fn bounded_extent(domain: &GroundDomain) -> Option<()> {
    let extent = match domain {
        GroundDomain::Function(_, keys, values) => {
            keys.length().ok()?.checked_mul(values.length().ok()?)?
        }
        GroundDomain::Set(_, inner) => inner.length().ok()?,
        GroundDomain::Relation(_, fields) => product_lengths(fields.iter().map(|d| d.as_ref()))?,
        _ => return None,
    };
    (extent <= MAX_EXTENT).then_some(())
}
