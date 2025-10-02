use std::collections::BTreeMap;

use core::fmt::Debug;
use linkme::distributed_slice;

//TODO: write good documentation on this! ~niklasdewally

use crate::{
    ast::{DeclarationPtr, Expression, Literal, Name, SymbolTable},
    rule_engine::ApplicationError,
};

#[distributed_slice]
#[doc(hidden)]
pub static REPRESENTATION_RULES: [RepresentationRule];

#[doc(hidden)]
pub struct RepresentationRule {
    pub name: &'static str,
    pub init: fn(&Name, &SymbolTable) -> Option<Box<dyn Representation>>,
}

/// Gets the representation rule named `name`.
#[allow(clippy::type_complexity)]
pub fn get_repr_rule(
    name: &str,
) -> Option<fn(&Name, &SymbolTable) -> Option<Box<dyn Representation>>> {
    REPRESENTATION_RULES
        .iter()
        .find(|x| x.name == name)
        .map(|x| x.init)
}

#[macro_export]
macro_rules! register_representation {
    ($ruleType:ident, $ruleName:literal) => {
        paste::paste!{
        #[linkme::distributed_slice($crate::representation::REPRESENTATION_RULES)]
        pub static [<__ $ruleType:snake:upper _REPRESENTATION_RULE>]: $crate::representation::RepresentationRule = $crate::representation::RepresentationRule {
            name: $ruleName,
            init: [<__create_representation_ $ruleType:snake>]
        };


        fn [<__create_representation_ $ruleType:snake>] (name: &$crate::ast::Name, symtab: &$crate::ast::SymbolTable) -> Option<Box<dyn Representation>>  {
                $ruleType::init(name,symtab).map(|x| Box::new(x) as Box<dyn Representation>)
            }
        }
    };
}

// This alongside Representation::box_clone() allows Representation to be a trait-object but still
// cloneable.
impl Clone for Box<dyn Representation> {
    fn clone(&self) -> Self {
        self.box_clone()
    }
}

pub trait Representation: Send + Sync + Debug {
    /// Creates a representation object for the given name.
    fn init(name: &Name, symtab: &SymbolTable) -> Option<Self>
    where
        Self: Sized;

    /// The variable being represented.
    fn variable_name(&self) -> &Name;

    /// Given an assignment for `self`, creates assignments for its representation variables.
    fn value_down(&self, value: Literal) -> Result<BTreeMap<Name, Literal>, ApplicationError>;

    /// Given assignments for its representation variables, creates an assignment for `self`.
    fn value_up(&self, values: &BTreeMap<Name, Literal>) -> Result<Literal, ApplicationError>;

    /// Returns [`Expression`]s representing each representation variable.
    fn expression_down(
        &self,
        symtab: &SymbolTable,
    ) -> Result<BTreeMap<Name, Expression>, ApplicationError>;

    /// Creates declarations for the representation variables of `self`.
    fn declaration_down(&self) -> Result<Vec<DeclarationPtr>, ApplicationError>;

    /// The rule name for this representaion.
    fn repr_name(&self) -> &str;

    /// Makes a clone of `self` into a `Representation` trait object.
    fn box_clone(&self) -> Box<dyn Representation>;
}
