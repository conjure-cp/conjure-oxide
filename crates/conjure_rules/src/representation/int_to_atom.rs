// https://conjure-cp.github.io/conjure-oxide/docs/conjure_core/representation/trait.Representation.html
use conjure_core::{
    ast::{Atom, Declaration, Domain, Expression, Literal, Name, Range, SymbolTable},
    metadata::Metadata,
    register_representation,
    representation::Representation,
    rule_engine::ApplicationError,
};

register_representation!(IntToAtom, "int_to_atom");

#[derive(Clone, Debug)]
pub struct IntToAtom {
    src_var: Name,
}

impl IntToAtom {
    /// Returns the names of the representation variable
    fn names(&self) -> impl Iterator<Item = Name> + '_ {
        (0..8).map(move |index| self.index_to_name(index)) // BITS
    }

    /// Gets the representation variable name for a specific index.
    fn index_to_name(&self, index: i32) -> Name {
        Name::RepresentedName(
            Box::new(self.src_var.clone()),
            self.repr_name().to_string(),
            format!("{:02}", index), // stored as _00, _01, ..., _31
        )
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
        let Literal::Int(mut value_i32) = value else {
            return Err(ApplicationError::RuleNotApplicable);
        };

        let mut result = std::collections::BTreeMap::new();

        // name_0 is the least significant bit, name_31 is the sign bit
        for name in self.names() {
            result.insert(Name::from(name), Literal::Bool((value_i32 & 1) != 0));
            value_i32 >>= 1;
        }

        Ok(result)
    }

    fn value_up(
        &self,
        values: &std::collections::BTreeMap<Name, Literal>,
    ) -> Result<Literal, ApplicationError> {
        let mut out: i32 = 0;
        let mut power: i32 = 1;

        for name in self.names() {
            let value = values
                .get(&name)
                .ok_or(ApplicationError::RuleNotApplicable)?;

            if let Literal::Int(value) = value {
                out += *value * power;
                power <<= 1;
            } else {
                return Err(ApplicationError::RuleNotApplicable);
            }
        }

        //TODO make the sign bit calculation work for dynamic bit count
        // Mask to 8 bits
        out &= 0xFF; // 0b00111111 BITS

        // If the sign bit (bit 8) is set, convert to negative using two's complement
        if out & 0x80 != 0 {
            // 0b10000000 BITS
            out -= 0x100;
            // BITS
        }

        Ok(Literal::Int(out.into()))
    }

    fn expression_down(
        &self,
        _: &SymbolTable,
    ) -> Result<std::collections::BTreeMap<Name, Expression>, ApplicationError> {
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
