use itertools::Itertools;

//use super::prelude::*;
// https://conjure-cp.github.io/conjure-oxide/docs/conjure_core/representation/trait.Representation.html
use conjure_core::{
    ast::{
        matrix, AbstractLiteral, Atom, Declaration, Domain, Expression, Literal, Name, Range,
        RecordEntry, SymbolTable,
    },
    bug, into_matrix,
    metadata::Metadata,
    register_representation,
    representation::{get_repr_rule, Representation},
    rule_engine::{ApplicationError, ApplicationError::RuleNotApplicable, ApplicationResult},
};

register_representation!(IntToAtom, "int_to_atom");

#[derive(Clone, Debug)]
pub struct IntToAtom {
    src_var: Name,
}

impl IntToAtom {
    fn names(&self) -> impl Iterator<Item = Name> + '_ {
        (0..32).map(move |i| Name::from(format!("{}_{}", self.src_var, i)))
    }
}

impl Representation for IntToAtom {
    fn init(name: &Name, symtab: &SymbolTable) -> Option<Self> {
        let domain = symtab.resolve_domain(name)?;

        if !domain.is_finite().expect("should be finite?") {
            return None;
        }

        let Domain::IntDomain(ranges) = domain else {
            return None;
        };

        // TODO: Change this to allow for all range types
        if !ranges
            .iter()
            .all(|x| matches!(x, Range::Bounded(_, _)) || matches!(x, Range::Single(_)))
        {
            return None;
        }

        Some(IntToAtom {
            src_var: name.clone(),
        })
    }

    fn variable_name(&self) -> &Name {
        &self.src_var
    }

    fn value_down(
        &self,
        value: Literal,
    ) -> Result<std::collections::BTreeMap<Name, Literal>, ApplicationError> {
        let Literal::Int(value_i32) = value else {
            return Err(ApplicationError::RuleNotApplicable);
        };

        let mut result = std::collections::BTreeMap::new();

        // name_0 is the least significant bit, name_31 is the sign bit
        for i in 0..32 {
            let name = format!("{}_{}", self.src_var, i);
            result.insert(Name::from(name), Literal::Bool((value_i32 & 1) != 0));
            value_i32 >> 1;
        }

        Ok(result)
    }

    fn value_up(
        &self,
        values: &std::collections::BTreeMap<Name, Literal>,
    ) -> Result<Literal, ApplicationError> {
        let mut out: i32 = 0;
        let mut power: i32 = 1;

        for i in 0..32 {
            let name = Name::from(format!("{}_{}", self.src_var, i));

            let value = values
                .get(&name)
                .ok_or(ApplicationError::RuleNotApplicable)?;

            if let Literal::Bool(value) = value {
                if *value {
                    out += power;
                }
                power << 1;
            } else {
                return Err(ApplicationError::RuleNotApplicable);
            }
        }

        Ok(Literal::Int(out))
    }

    fn expression_down(
        &self,
        _: &SymbolTable,
    ) -> Result<std::collections::BTreeMap<Name, Expression>, ApplicationError> {
        //TODO create boolean expressions to desribe ranges for each
        // Each variable is dependent on other variables
        // Ranges need to be combined, work out union
        // Account for negative values
        Ok(self
            .names()
            .map(|name| {
                (
                    name.clone(),
                    Expression::Atomic(Metadata::new(), Atom::Reference(name)),
                )
            })
            .collect())
    }

    fn declaration_down(&self) -> Result<Vec<Declaration>, ApplicationError> {
        // TODO: work out what these are
        Ok(self
            .names()
            .map(|name| Declaration::new_var(name, Domain::BoolDomain))
            .collect())
    }

    fn repr_name(&self) -> &str {
        "int_to_atom"
    }

    fn box_clone(&self) -> Box<dyn Representation> {
        Box::new(self.clone()) as _
    }
}
