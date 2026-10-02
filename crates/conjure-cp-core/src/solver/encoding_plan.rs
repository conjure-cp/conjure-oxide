//! SAT encoding decisions before representation materialisation or literal allocation.
//!
//! Declaration object IDs identify mathematical variables. References identify individual uses;
//! each use can request its own representation kind, while materialisation will share instances
//! by `(SemanticVarId, RepresentationKind)`. This initial IR deliberately has no SAT literals or
//! external encoder types. It is not yet used by the production SAT translation pipeline.
use crate::ast::serde::{HasId, ObjId};
use crate::ast::{DeclarationPtr, GroundDomain, Range};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SemanticVarId(usize);
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VarRefId(usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RepresentationKind {
    Boolean,
    Direct,
    Order,
    BinaryValue,
    BinaryOffset,
    BinaryRank,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VariableOrigin {
    User,
    RewriteAuxiliary,
    EncodingAuxiliary,
}

/// Canonical finite domains without enumerating large intervals.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SemanticDomain {
    Boolean,
    Integer { ranges: Vec<(i32, i32)> },
}
impl SemanticDomain {
    pub fn integer(mut ranges: Vec<(i32, i32)>) -> Self {
        ranges.retain(|(low, high)| low <= high);
        ranges.sort_unstable();
        let mut canonical: Vec<(i32, i32)> = Vec::new();
        for (low, high) in ranges {
            if let Some(last) = canonical.last_mut()
                && i64::from(low) <= i64::from(last.1) + 1
            {
                last.1 = last.1.max(high);
            } else {
                canonical.push((low, high));
            }
        }
        Self::Integer { ranges: canonical }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticVariable {
    pub source: ObjId,
    pub name: String,
    pub domain: SemanticDomain,
    pub origin: VariableOrigin,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VariableReference {
    pub variable: SemanticVarId,
    pub context: ReferenceContext,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReferenceContext {
    BooleanOperand,
    EqualityOperand,
    ComparisonOperand,
    LinearTerm { coefficient: i64 },
    AllDifferentMember,
    ObjectiveTerm,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepresentationRequest {
    pub reference: VarRefId,
    pub kind: RepresentationKind,
}

/// Boolean structure retained until the explicitly chosen Tseitin generator runs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BooleanFormula {
    Constant(bool),
    Reference(VarRefId),
    Not(Box<BooleanFormula>),
    And(Vec<BooleanFormula>),
    Or(Vec<BooleanFormula>),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EncodingPlanKind {
    BooleanTseitin { formula: BooleanFormula },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanProvenance {
    DefaultRule { rule: String },
    ExplicitConfiguration,
    Heuristic { heuristic: String },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncodingPlan {
    pub source_constraint: usize,
    pub kind: EncodingPlanKind,
    pub requests: Vec<RepresentationRequest>,
    pub provenance: PlanProvenance,
}

/// Dense IDs and ordered vectors make plan allocation and diagnostics deterministic.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncodingDecision {
    variables: Vec<SemanticVariable>,
    references: Vec<VariableReference>,
    plans: Vec<EncodingPlan>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PlanError {
    #[error("unsupported semantic domain for declaration `{0}`")]
    UnsupportedDomain(String),
    #[error("unknown semantic variable {0}")]
    UnknownVariable(usize),
    #[error("unknown variable reference {0}")]
    UnknownReference(usize),
    #[error("representation {kind:?} is incompatible with reference {reference}")]
    IncompatibleRepresentation {
        reference: usize,
        kind: RepresentationKind,
    },
    #[error("Boolean formula references a non-Boolean semantic variable")]
    NonBooleanOperand,
    #[error("Boolean reference {0} has no explicit Boolean representation request")]
    MissingBooleanRepresentation(usize),
    #[error("semantic declaration identity occurs twice in the variable arena")]
    DuplicateVariable,
    #[error("domain changed after interning declaration `{0}`")]
    ChangedDomain(String),
    #[error("non-canonical integer domain in a serialised plan")]
    NonCanonicalDomain,
}

impl EncodingDecision {
    pub fn variables(&self) -> &[SemanticVariable] {
        &self.variables
    }
    pub fn variable_ids(&self) -> impl Iterator<Item = SemanticVarId> + '_ {
        (0..self.variables.len()).map(SemanticVarId)
    }
    pub fn references(&self) -> &[VariableReference] {
        &self.references
    }
    pub fn plans(&self) -> &[EncodingPlan] {
        &self.plans
    }

    /// Intern by declaration identity, never by name or expression equality.
    pub fn intern(
        &mut self,
        declaration: &DeclarationPtr,
        origin: VariableOrigin,
    ) -> Result<SemanticVarId, PlanError> {
        let name = declaration.name().to_string();
        let ground = declaration
            .domain()
            .and_then(|domain| domain.resolve().ok())
            .ok_or_else(|| PlanError::UnsupportedDomain(name.clone()))?;
        let domain = match ground.as_ref() {
            GroundDomain::Bool => SemanticDomain::Boolean,
            GroundDomain::Int(ranges) => {
                let ranges = ranges
                    .iter()
                    .map(|range| match range {
                        Range::Single(value) => Ok((*value, *value)),
                        Range::Bounded(low, high) => Ok((*low, *high)),
                        _ => Err(PlanError::UnsupportedDomain(name.clone())),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                SemanticDomain::integer(ranges)
            }
            _ => return Err(PlanError::UnsupportedDomain(name)),
        };
        let source = declaration.id();
        if let Some(index) = self
            .variables
            .iter()
            .position(|variable| variable.source == source)
        {
            if self.variables[index].domain != domain {
                return Err(PlanError::ChangedDomain(name));
            }
            return Ok(SemanticVarId(index));
        }
        let id = SemanticVarId(self.variables.len());
        self.variables.push(SemanticVariable {
            source,
            name,
            domain,
            origin,
        });
        Ok(id)
    }

    pub fn new_reference(
        &mut self,
        variable: SemanticVarId,
        context: ReferenceContext,
    ) -> Result<VarRefId, PlanError> {
        self.variable(variable)?;
        let id = VarRefId(self.references.len());
        self.references
            .push(VariableReference { variable, context });
        Ok(id)
    }

    pub fn duplicate_reference(&mut self, reference: VarRefId) -> Result<VarRefId, PlanError> {
        let reference = self.reference(reference)?.clone();
        self.new_reference(reference.variable, reference.context)
    }

    pub fn variable(&self, id: SemanticVarId) -> Result<&SemanticVariable, PlanError> {
        self.variables
            .get(id.0)
            .ok_or(PlanError::UnknownVariable(id.0))
    }
    pub fn reference(&self, id: VarRefId) -> Result<&VariableReference, PlanError> {
        self.references
            .get(id.0)
            .ok_or(PlanError::UnknownReference(id.0))
    }

    pub fn add_plan(&mut self, plan: EncodingPlan) -> Result<(), PlanError> {
        self.validate_plan(&plan)?;
        self.plans.push(plan);
        Ok(())
    }

    /// Validate imported plans before any representation or literal allocation.
    pub fn validate(&self) -> Result<(), PlanError> {
        let mut sources = HashSet::new();
        for variable in &self.variables {
            if !sources.insert(&variable.source) {
                return Err(PlanError::DuplicateVariable);
            }
            if let SemanticDomain::Integer { ranges } = &variable.domain
                && variable.domain != SemanticDomain::integer(ranges.clone())
            {
                return Err(PlanError::NonCanonicalDomain);
            }
        }
        for reference in &self.references {
            self.variable(reference.variable)?;
        }
        for plan in &self.plans {
            self.validate_plan(plan)?;
        }
        Ok(())
    }

    fn validate_plan(&self, plan: &EncodingPlan) -> Result<(), PlanError> {
        for request in &plan.requests {
            let reference = self.reference(request.reference)?;
            let boolean = matches!(
                self.variable(reference.variable)?.domain,
                SemanticDomain::Boolean
            );
            if boolean != (request.kind == RepresentationKind::Boolean) {
                return Err(PlanError::IncompatibleRepresentation {
                    reference: request.reference.0,
                    kind: request.kind,
                });
            }
        }
        match &plan.kind {
            EncodingPlanKind::BooleanTseitin { formula } => {
                self.validate_formula(formula)?;
                self.validate_boolean_requests(formula, &plan.requests)
            }
        }
    }

    fn validate_boolean_requests(
        &self,
        formula: &BooleanFormula,
        requests: &[RepresentationRequest],
    ) -> Result<(), PlanError> {
        match formula {
            BooleanFormula::Reference(reference) => {
                if !requests.iter().any(|request| {
                    request.reference == *reference && request.kind == RepresentationKind::Boolean
                }) {
                    return Err(PlanError::MissingBooleanRepresentation(reference.0));
                }
            }
            BooleanFormula::Not(inner) => self.validate_boolean_requests(inner, requests)?,
            BooleanFormula::And(children) | BooleanFormula::Or(children) => {
                for child in children {
                    self.validate_boolean_requests(child, requests)?;
                }
            }
            BooleanFormula::Constant(_) => {}
        }
        Ok(())
    }

    fn validate_formula(&self, formula: &BooleanFormula) -> Result<(), PlanError> {
        match formula {
            BooleanFormula::Constant(_) => Ok(()),
            BooleanFormula::Reference(id) => {
                let reference = self.reference(*id)?;
                if self.variable(reference.variable)?.domain != SemanticDomain::Boolean {
                    return Err(PlanError::NonBooleanOperand);
                }
                Ok(())
            }
            BooleanFormula::Not(inner) => self.validate_formula(inner),
            BooleanFormula::And(children) | BooleanFormula::Or(children) => {
                for child in children {
                    self.validate_formula(child)?;
                }
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Domain, Name};

    fn declared(name: &str, domain: crate::ast::DomainPtr) -> DeclarationPtr {
        DeclarationPtr::new_find(Name::User(name.into()), domain)
    }

    #[test]
    fn identities_and_uses_are_distinct_even_with_identical_names() {
        let mut decision = EncodingDecision::default();
        let x = declared("x", Domain::int(vec![Range::Bounded(-2, 3)]));
        let a = decision.intern(&x, VariableOrigin::User).unwrap();
        assert_eq!(
            a,
            decision.intern(&x.clone(), VariableOrigin::User).unwrap()
        );
        let b = decision
            .intern(
                &declared("x", Domain::int(vec![Range::Bounded(-2, 3)])),
                VariableOrigin::User,
            )
            .unwrap();
        assert_ne!(a, b);
        let first = decision
            .new_reference(a, ReferenceContext::EqualityOperand)
            .unwrap();
        let second = decision.duplicate_reference(first).unwrap();
        assert_ne!(first, second);
        assert_eq!(
            decision.reference(first).unwrap().variable,
            decision.reference(second).unwrap().variable
        );
        let boolean = decision
            .intern(&declared("p", Domain::bool()), VariableOrigin::User)
            .unwrap();
        let p = decision
            .new_reference(boolean, ReferenceContext::BooleanOperand)
            .unwrap();
        decision
            .add_plan(EncodingPlan {
                source_constraint: 0,
                kind: EncodingPlanKind::BooleanTseitin {
                    formula: BooleanFormula::Reference(p),
                },
                requests: vec![
                    RepresentationRequest {
                        reference: p,
                        kind: RepresentationKind::Boolean,
                    },
                    RepresentationRequest {
                        reference: first,
                        kind: RepresentationKind::Direct,
                    },
                    RepresentationRequest {
                        reference: second,
                        kind: RepresentationKind::BinaryOffset,
                    },
                ],
                provenance: PlanProvenance::ExplicitConfiguration,
            })
            .unwrap();
        let encoded = serde_json::to_string_pretty(&decision).unwrap();
        let restored: EncodingDecision = serde_json::from_str(&encoded).unwrap();
        restored.validate().unwrap();
        assert_eq!(decision, restored);
    }

    #[test]
    fn sparse_domains_are_canonical_without_enumerating_intervals() {
        let domain = SemanticDomain::integer(vec![(9, 9), (4, 4), (1, 1), (4, 4)]);
        assert_eq!(
            domain,
            SemanticDomain::Integer {
                ranges: vec![(1, 1), (4, 4), (9, 9)]
            }
        );
        assert_eq!(
            SemanticDomain::integer(vec![(i32::MIN, 0), (1, i32::MAX)]),
            SemanticDomain::Integer {
                ranges: vec![(i32::MIN, i32::MAX)]
            }
        );
    }

    #[test]
    fn boolean_plans_require_an_explicit_compatible_representation() {
        let mut decision = EncodingDecision::default();
        let p = decision
            .intern(&declared("p", Domain::bool()), VariableOrigin::User)
            .unwrap();
        let reference = decision
            .new_reference(p, ReferenceContext::BooleanOperand)
            .unwrap();
        let mut plan = EncodingPlan {
            source_constraint: 0,
            kind: EncodingPlanKind::BooleanTseitin {
                formula: BooleanFormula::Reference(reference),
            },
            requests: vec![],
            provenance: PlanProvenance::ExplicitConfiguration,
        };
        assert_eq!(
            decision.add_plan(plan.clone()).unwrap_err(),
            PlanError::MissingBooleanRepresentation(reference.0)
        );
        plan.requests.push(RepresentationRequest {
            reference,
            kind: RepresentationKind::BinaryValue,
        });
        assert_eq!(
            decision.add_plan(plan).unwrap_err(),
            PlanError::IncompatibleRepresentation {
                reference: reference.0,
                kind: RepresentationKind::BinaryValue,
            }
        );
        assert!(decision.plans().is_empty());
    }

    #[test]
    fn malformed_imported_plans_are_rejected() {
        let mut decision = EncodingDecision::default();
        let x = decision
            .intern(
                &declared("x", Domain::int(vec![Range::Single(4)])),
                VariableOrigin::User,
            )
            .unwrap();
        let reference = decision
            .new_reference(x, ReferenceContext::BooleanOperand)
            .unwrap();
        let plan = EncodingPlan {
            source_constraint: 0,
            kind: EncodingPlanKind::BooleanTseitin {
                formula: BooleanFormula::Reference(reference),
            },
            requests: vec![],
            provenance: PlanProvenance::ExplicitConfiguration,
        };
        assert_eq!(
            decision.add_plan(plan).unwrap_err(),
            PlanError::NonBooleanOperand
        );
        decision.references[0].variable = SemanticVarId(99);
        assert_eq!(
            decision.validate().unwrap_err(),
            PlanError::UnknownVariable(99)
        );
    }
}
