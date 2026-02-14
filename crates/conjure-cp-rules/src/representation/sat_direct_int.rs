// https://conjure-cp.github.io/conjure-oxide/docs/conjure_core/representation/trait.Representation.html
use conjure_cp::ast::GroundDomain;
use conjure_cp::bug;
use conjure_cp::{
    ast::{Atom, DeclarationPtr, Domain, Expression, Literal, Metadata, Name, SymbolTable},
    register_representation,
    representation::Representation,
    rule_engine::ApplicationError,
};

register_representation!(SatDirectInt, "sat_direct_int");

#[derive(Clone, Debug)]
pub struct SatDirectInt {
    src_var: Name,
    upper_bound: i32,
    lower_bound: i32,
}

impl SatDirectInt {
    /// Returns the names of the boolean variables used in the direct encoding.
    fn names(&self) -> impl Iterator<Item = Name> + '_ {
        (self.lower_bound..=self.upper_bound).map(|index| self.index_to_name(index))
    }

    /// Gets the representation variable name corresponding to a concrete integer value.
    fn index_to_name(&self, index: i32) -> Name {
        Name::Represented(Box::new((
            self.src_var.clone(),
            self.repr_name().into(),
            format!("{index}").into(), // stored as _00, _01, ...
        )))
    }
}

impl Representation for SatDirectInt {
    /// Creates a direct int representation object for the given name.
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

        Some(SatDirectInt {
            src_var: name.clone(),
            lower_bound: min,
            upper_bound: max,
        })
    }

    /// The variable being represented.
    fn variable_name(&self) -> &Name {
        &self.src_var
    }

    fn value_down(
        &self,
        _value: Literal,
    ) -> Result<std::collections::BTreeMap<Name, Literal>, ApplicationError> {
        // NOTE: It's unclear where and when `value_down` would be called for
        // direct encoding. This is also never called in log encoding, so we
        // deliberately fail here to surface unexpected usage.
        bug!("value_down is not implemented for direct encoding and should not be called")
    }

    /// Given the values for its boolean representation variables, creates an assignment for `self` - the integer form.
    fn value_up(
        &self,
        values: &std::collections::BTreeMap<Name, Literal>,
    ) -> Result<Literal, ApplicationError> {
        let mut found_value: Option<i32> = None;

        for value_candidate in self.lower_bound..=self.upper_bound {
            let name = self.index_to_name(value_candidate);
            let value_literal = values
                .get(&name)
                .ok_or(ApplicationError::RuleNotApplicable)?;

            let is_true = match value_literal {
                Literal::Int(1) | Literal::Bool(true) => true,
                Literal::Int(0) | Literal::Bool(false) => false,
                _ => return Err(ApplicationError::RuleNotApplicable),
            };

            if is_true {
                if found_value.is_some() {
                    // More than one variable is true, which is an error for direct encoding
                    return Err(ApplicationError::RuleNotApplicable);
                }
                found_value = Some(value_candidate);
            }
        }

        found_value
            .map(Literal::Int)
            .ok_or(ApplicationError::RuleNotApplicable)
    }

    /// Returns [`Expression`]s representing each boolean representation variable.
    fn expression_down(
        &self,
        st: &SymbolTable,
    ) -> Result<std::collections::BTreeMap<Name, Expression>, ApplicationError> {
        Ok(self
            .names()
            .enumerate()
            .map(|(index, name)| {
                let decl = st.lookup(&name).unwrap();
                (
                    // Machine names are used so that the derived ordering matches the correct ordering of the representation variables
                    Name::Machine(index as i32),
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
        let temp_a = self
            .names()
            .map(|name| DeclarationPtr::new_var(name, Domain::bool()))
            .collect();

        Ok(temp_a)
    }

    /// The rule name for this representation.
    fn repr_name(&self) -> &str {
        "sat_direct_int"
    }

    /// Makes a clone of `self` into a `Representation` trait object.
    fn box_clone(&self) -> Box<dyn Representation> {
        Box::new(self.clone()) as _
    }
}
