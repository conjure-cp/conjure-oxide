// https://conjure-cp.github.io/conjure-oxide/docs/conjure_core/representation/trait.Representation.html
use conjure_core::{
    ast::{Atom, Declaration, Domain, Expression, Literal, Name, Range, SymbolTable},
    metadata::Metadata,
    register_representation,
    representation::Representation,
    rule_engine::ApplicationError,
};

register_representation!(IntToAtom, "int_to_atom");

// The number of bits used to represent the integer.
// This is a fixed value for the representation, but could be made dynamic if needed.
const BITS: i32 = 8;

#[derive(Clone, Debug)]
pub struct IntToAtom {
    src_var: Name,
}

impl IntToAtom {
    /// Returns the names of the representation variable
    fn names(&self) -> impl Iterator<Item = Name> + '_ {
        (0..BITS).map(move |index| self.index_to_name(index))
    }

    /// Gets the representation variable name for a specific index.
    fn index_to_name(&self, index: i32) -> Name {
        Name::RepresentedName(
            Box::new(self.src_var.clone()),
            self.repr_name().to_string(),
            format!("{:02}", index), // stored as _00, _01, ...
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

        // Essence only supports decision variables with finite domains
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

        // name_0 is the least significant bit, name_<BITS-1> is the sign bit
        for name in self.names() {
            result.insert(name, Literal::Bool((value_i32 & 1) != 0));
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

        let sign_bit = 1 << (BITS - 1);
        // Mask to `BITS` bits
        out &= (sign_bit << 1) - 1;

        // If the sign bit is set, convert to negative using two's complement
        if out & sign_bit != 0 {
            out -= sign_bit << 1;
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
