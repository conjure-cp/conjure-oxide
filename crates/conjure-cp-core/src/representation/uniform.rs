//! Model-scoped representation choices for `--channelling uniform`.
//!
//! Choices concern the outer type family, ignoring domain bounds and element types. Integer
//! encoding is separate from matrix layout. State is scoped to a rewrite, including nested
//! rewrites, so enumerated models and subsequent solves never inherit another model's choices.
use super::{ReprInitError, ReprRulePtr};
use crate::ast::{DomainPtr, ReturnType, Typeable};
use crate::settings::{Channelling, channelling};
use std::cell::RefCell;
use std::collections::HashMap;
use std::mem::{Discriminant, discriminant};

pub(crate) type Choices = HashMap<Discriminant<ReturnType>, ReprRulePtr>;
thread_local! {
    static CHOICES: RefCell<Choices> = RefCell::new(HashMap::new());
}

fn family(dom: &DomainPtr, rule: ReprRulePtr) -> Discriminant<ReturnType> {
    discriminant(&if rule.is_integer_encoding() {
        ReturnType::Int
    } else {
        dom.return_type()
    })
}

pub fn selected(dom: &DomainPtr, integer_encoding: bool) -> Option<ReprRulePtr> {
    if channelling() != Channelling::Uniform {
        return None;
    }
    let key = discriminant(&if integer_encoding {
        ReturnType::Int
    } else {
        dom.return_type()
    });
    CHOICES.with(|choices| choices.borrow().get(&key).copied())
}

pub(crate) fn check_choice(dom: &DomainPtr, rule: ReprRulePtr) -> Result<(), ReprInitError> {
    if channelling() == Channelling::Uniform
        && let Some(existing) =
            CHOICES.with(|choices| choices.borrow().get(&family(dom, rule)).copied())
        && existing.id() != rule.id()
    {
        return Err(ReprInitError::UniformConflict {
            domain: dom.clone(),
            existing: existing.name(),
            requested: rule.name(),
        });
    }
    Ok(())
}

pub(crate) fn reset() {
    CHOICES.with(|choices| choices.borrow_mut().clear());
}

pub(crate) fn record_choice(dom: &DomainPtr, rule: ReprRulePtr) {
    if channelling() == Channelling::Uniform {
        CHOICES.with(|choices| {
            choices.borrow_mut().insert(family(dom, rule), rule);
        });
    }
}

/// Restore the enclosing rewrite's choices even on early return or panic.
pub(crate) struct Scope(Choices);
impl Scope {
    pub(crate) fn new() -> Self {
        Self(CHOICES.with(|choices| std::mem::take(&mut *choices.borrow_mut())))
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        CHOICES.with(|choices| *choices.borrow_mut() = std::mem::take(&mut self.0));
    }
}

/// Representation probes instantiate a detached declaration. They must not select a family.
pub(crate) fn probe<T>(f: impl FnOnce() -> T) -> T {
    if channelling() != Channelling::Uniform {
        return f();
    }
    let _restore = Scope(CHOICES.with(|choices| choices.borrow().clone()));
    f()
}

/// Rule applicability is speculative. Commit choices only for the selected effect.
pub(crate) fn attempt(
    f: impl FnOnce() -> crate::rule_engine::ApplicationResult,
) -> crate::rule_engine::ApplicationResult {
    if channelling() != Channelling::Uniform {
        return f();
    }
    let previous = CHOICES.with(|choices| choices.borrow().clone());
    let restore = Scope(previous.clone());
    let result = f().map(|mut effect| {
        effect.uniform_choices = CHOICES.with(|choices| {
            choices
                .borrow()
                .iter()
                .filter(|(key, _)| !previous.contains_key(*key))
                .map(|(key, rule)| (*key, *rule))
                .collect()
        });
        effect
    });
    drop(restore);
    result
}

pub(crate) fn commit(choices: Choices) {
    if channelling() == Channelling::Uniform {
        CHOICES.with(|current| current.borrow_mut().extend(choices));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::DeclarationPtr;
    use crate::range;
    use crate::representation::types::ReprGetOrInitResult;
    use crate::representation::{ReprId, ReprResult, ReprRuleStored, ReprStateStored};
    use crate::rule_engine::{ApplicationError, RuleEffect};
    use crate::settings::{SolverFamily, set_channelling};
    use parking_lot::MappedRwLockReadGuard;

    struct TestRule;
    static RULE: TestRule = TestRule;
    impl crate::representation::ReprRuleStored for TestRule {
        fn name(&self) -> &'static str {
            "UniformTest"
        }
        fn short_name(&self) -> &'static str {
            "uniform_test"
        }
        fn id(&self) -> ReprId {
            ReprId::new(self.name(), self.short_name())
        }
        fn applies_to(&self, _: SolverFamily) -> bool {
            true
        }
        fn is_integer_encoding(&self) -> bool {
            true
        }
        fn init_for(&self, _: &mut DeclarationPtr) -> ReprResult {
            unreachable!()
        }
        fn init_for_if_not_exists(&self, _: &mut DeclarationPtr) -> ReprResult {
            unreachable!()
        }
        fn probe_for(&self, _: &DeclarationPtr) -> Result<usize, super::super::ReprError> {
            unreachable!()
        }
        fn get_or_init_for<'a>(
            &self,
            _: &'a mut DeclarationPtr,
        ) -> ReprGetOrInitResult<'a, dyn ReprStateStored, super::super::ReprError> {
            unreachable!()
        }
        fn get_for<'a>(
            &self,
            _: &'a DeclarationPtr,
        ) -> Option<MappedRwLockReadGuard<'a, dyn ReprStateStored>> {
            unreachable!()
        }
        fn deserialize_state(
            &self,
            _: serde_json::Value,
        ) -> Result<Box<dyn ReprStateStored>, serde_json::Error> {
            unreachable!()
        }
    }

    #[test]
    fn speculative_choices_commit_only_with_the_selected_effect() {
        set_channelling(Channelling::Uniform);
        let dom = crate::domain_int!(0..3);
        assert!(
            attempt(|| {
                record_choice(&dom, &RULE);
                Err(ApplicationError::RuleNotApplicable)
            })
            .is_err()
        );
        assert!(selected(&dom, true).is_none());
        let effect = attempt(|| {
            record_choice(&dom, &RULE);
            Ok(RuleEffect::pure(true.into()))
        })
        .unwrap();
        assert!(selected(&dom, true).is_none());
        let _ = effect.materialise(&crate::ast::SymbolTable::new());
        assert_eq!(selected(&dom, true).unwrap().id(), RULE.id());
        set_channelling(Channelling::No);
    }

    #[test]
    fn probing_a_representation_does_not_select_it() {
        set_channelling(Channelling::Uniform);
        let dom = crate::domain_int!(0..3);
        probe(|| record_choice(&dom, &RULE));
        assert!(selected(&dom, true).is_none());
        set_channelling(Channelling::No);
    }

    #[test]
    fn nested_scopes_restore_choices_and_do_not_leak_to_the_next_model() {
        set_channelling(Channelling::Uniform);
        let dom = crate::domain_int!(-1..1);
        {
            let _outer = Scope::new();
            record_choice(&dom, &RULE);
            {
                let _inner = Scope::new();
                assert!(selected(&dom, true).is_none());
            }
            assert_eq!(selected(&dom, true).unwrap().id(), RULE.id());
        }
        assert!(selected(&dom, true).is_none());
        set_channelling(Channelling::No);
    }
}
