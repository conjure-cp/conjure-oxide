use super::types::{LookupFn, ReprAssignment, ReprDeclLevel, ReprInitResult};
use crate::ast::{DeclarationPtr, Literal};
use crate::representation::ReprRule;
use parking_lot::MappedRwLockReadGuard;
use serde::Deserialize;
use serde_json;
use std::any::Any;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};

pub trait ReprStateStored: Any + Send + Sync + Debug {
    fn rule(&self) -> &'static dyn ReprRuleStored;

    fn up_via(&self, lu: &LookupFn<'_>) -> Result<Literal, String>;

    fn as_any(&self) -> &dyn Any;

    fn as_any_mut(&mut self) -> &mut dyn Any;

    fn clone_box(&self) -> Box<dyn ReprStateStored>;

    fn serialise(&self) -> Result<serde_json::Value, serde_json::Error>;
}

impl<D: ReprDeclLevel> ReprStateStored for D {
    fn rule(&self) -> &'static dyn ReprRuleStored {
        D::RULE
    }

    fn up_via(&self, lu: &LookupFn<'_>) -> Result<Literal, String> {
        let res = self.lookup_via(lu)?;
        Ok(res.up())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn clone_box(&self) -> Box<dyn ReprStateStored> {
        Box::new(self.clone())
    }

    fn serialise(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::to_value(self)
    }
}

pub trait ReprRuleStored: Send + Sync {
    fn name(&self) -> &'static str;

    fn init_for(&self, decl: &mut DeclarationPtr) -> ReprInitResult;

    fn get_for<'a>(
        &self,
        decl: &'a DeclarationPtr,
    ) -> Option<MappedRwLockReadGuard<'a, dyn ReprStateStored>>;

    fn deserialize_state(
        &self,
        val: serde_json::Value,
    ) -> Result<Box<dyn ReprStateStored>, serde_json::Error>;
}

impl<R: ReprRule> ReprRuleStored for R {
    fn name(&self) -> &'static str {
        R::NAME
    }

    fn init_for(&self, decl: &mut DeclarationPtr) -> ReprInitResult {
        R::init_for(decl)
    }

    fn get_for<'a>(
        &self,
        decl: &'a DeclarationPtr,
    ) -> Option<MappedRwLockReadGuard<'a, dyn ReprStateStored>> {
        MappedRwLockReadGuard::try_map(decl.reprs(), |store| {
            store.get::<R>().map(|d| d as &dyn ReprStateStored)
        })
        .ok()
    }

    fn deserialize_state(
        &self,
        val: serde_json::Value,
    ) -> Result<Box<dyn ReprStateStored>, serde_json::Error> {
        Ok(Box::new(R::DeclLevel::deserialize(val)?))
    }
}

impl Debug for dyn ReprRuleStored {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(fmt, "ReprRule({})", self.name())
    }
}

impl PartialEq for dyn ReprRuleStored {
    fn eq(&self, other: &Self) -> bool {
        self.name() == other.name()
    }
}

impl Eq for dyn ReprRuleStored {}

impl PartialOrd for dyn ReprRuleStored {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.name().partial_cmp(other.name())
    }
}

impl Ord for dyn ReprRuleStored {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.name().cmp(other.name())
    }
}

impl Hash for dyn ReprRuleStored {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name().hash(state)
    }
}
