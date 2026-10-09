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

/// Return the uniform representation selected for this type in the current model.
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
