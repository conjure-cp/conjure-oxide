// https://conjure-cp.github.io/conjure-oxide/docs/conjure_core/representation/trait.Representation.html
use conjure_cp::ast::GroundDomain;
use conjure_cp::bug;
use conjure_cp::{
    ast::{Atom, DeclarationPtr, Domain, Expression, Literal, Metadata, Name, SymbolTable},
    register_representation,
    representation::Representation,
    rule_engine::ApplicationError,
};

register_representation!(SATLogInt, "sat_log_int");

#[derive(Clone, Debug)]
pub struct SATLogInt {
    src_var: Name,
    bits: u32,
}

impl SATLogInt {
    /// Returns the names of the representation variable
    fn names(&self) -> impl Iterator<Item = Name> + '_ {
        (0..self.bits).map(move |index| self.index_to_name(index))
    }

    /// Gets the representation variable name for a specific index.
    fn index_to_name(&self, index: u32) -> Name {
        Name::Represented(Box::new((
            self.src_var.clone(),
            self.repr_name().into(),
            format!("{index:02}").into(), // stored as _00, _01, ...
        )))
    }
}

impl Representation for SATLogInt {
    /// Creates a log int representation object for the given name.
    fn init(name: &Name, symtab: &SymbolTable) -> Option<Self> {
        let domain = symtab.resolve_domain(name)?;

        if !domain.is_finite() {
            return None;
        }

        let GroundDomain::Int(ranges) = domain.as_ref() else {
            return None;
        };

        // Determine min/max and return None if range is unbounded
        let (min, max) =
            ranges
                .iter()
                .try_fold((i32::MAX, i32::MIN), |(min_a, max_b), range| {
                    let lb = range.low()?;
                    let ub = range.high()?;
                    Some((min_a.min(*lb), max_b.max(*ub)))
                })?;

        // calculate the bits needed to represent the integer
        let bit_count = (1..=32)
            .find(|&bits| {
                let min_possible = -(1i64 << (bits - 1));
                let max_possible = (1i64 << (bits - 1)) - 1;
                (min as i64) >= min_possible && (max as i64) <= max_possible
            })
            .unwrap_or_else(|| bug!("Should never be reached: i32 integer should always be with storable with 32 bits.")); // safe unwrap as i32 fits in 32 bits

        Some(SATLogInt {
            src_var: name.clone(),
            bits: bit_count,
        })
    }

    /// The variable being represented.
    fn variable_name(&self) -> &Name {
        &self.src_var
    }

    /// Given the integer assignment for `self`, creates assignments for its representation variables.
    fn value_down(
        &self,
        value: Literal,
    ) -> Result<std::collections::BTreeMap<Name, Literal>, ApplicationError> {
        let Literal::Int(mut value_i32) = value else {
            return Err(ApplicationError::RuleNotApplicable);
        };

        let mut result = std::collections::BTreeMap::new();

        // name_0 is the least significant bit, name_<final> is the sign bit
        for name in self.names() {
            result.insert(name, Literal::Bool((value_i32 & 1) != 0));
            value_i32 >>= 1;
        }

        Ok(result)
    }

    /// Given the values for its boolean representation variables, creates an assignment for `self` - the integer form.
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

        let sign_bit = 1 << (self.bits - 1);
        // Mask to `BITS` bits
        out &= (sign_bit << 1) - 1;

        // If the sign bit is set, convert to negative using two's complement
        if out & sign_bit != 0 {
            out -= sign_bit << 1;
        }

        Ok(Literal::Int(out))
    }

    /// Returns [`Expression`]s representing each boolean representation variable.
    fn expression_down(
        &self,
        st: &SymbolTable,
    ) -> Result<std::collections::BTreeMap<Name, Expression>, ApplicationError> {
        Ok(self
            .names()
            .map(|name| {
                let decl = st.lookup(&name).unwrap();
                (
                    name,
                    Expression::Atomic(
                        Metadata::new(),
                        Atom::Reference(conjure_cp::ast::Reference { ptr: decl }),
                    ),
                )
            })
            .collect())
    }

    /// Creates declarations for the boolean representation variables of `self`.
    fn declaration_down(&self) -> Result<Vec<DeclarationPtr>, ApplicationError> {
        Ok(self
            .names()
            .map(|name| DeclarationPtr::new_var(name, Domain::bool()))
            .collect())
    }

    /// The rule name for this representaion.
    fn repr_name(&self) -> &str {
        "sat_log_int"
    }

    /// Makes a clone of `self` into a `Representation` trait object.
    fn box_clone(&self) -> Box<dyn Representation> {
        Box::new(self.clone()) as _
    }
}
