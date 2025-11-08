// https://conjure-cp.github.io/conjure-oxide/docs/conjure_core/representation/trait.Representation.html
use conjure_cp::{
    ast::{Atom, DeclarationPtr, Domain, Expression, Literal, Metadata, Name, Range, SymbolTable}, bug, register_representation, representation::Representation, rule_engine::ApplicationError
};

register_representation!(SATOrderInt, "sat_order_int");

// The number of bits used to represent the integer.
// This is a fixed value for the representation, but could be made dynamic if needed.
const BITS: i32 = 8;

#[derive(Clone, Debug)]
pub struct SATOrderInt {
    src_var: Name,
    // ranges: Vec<Range<i32>>,
    domain: Domain,
}

impl SATOrderInt {
    /// Returns the names of the representation variable
    fn names(&self) -> impl Iterator<Item = Name> + '_ {
        (0..BITS).map(move |index| self.index_to_name(index))
    }

    /// Gets the representation variable name for a specific index.
    fn index_to_name(&self, index: i32) -> Name {
        Name::Represented(Box::new((
            self.src_var.clone(),
            self.repr_name().into(),
            format!("{index:02}").into(), // stored as _00, _01, ...
        )))
    }
}

impl Representation for SATOrderInt {
    fn init(name: &Name, symtab: &SymbolTable) -> Option<Self> {
        let domain = symtab.resolve_domain(name)?;

        if !domain.is_finite().expect("should be finite?") {
            // Domain not finite => return None
            return None;
        }

        let Domain::Int(ranges) = domain.clone() else {
            // Not integer domain => return None
            return None;
        };

        // Essence only supports decision variables with finite domains
        if !ranges
            .iter()
            .all(|x| matches!(x, Range::Bounded(_, _)) || matches!(x, Range::Single(_)))
        {
            return None;
        }

        Some(SATOrderInt {
            src_var: name.clone(),
            domain: domain,
        })
    }

    fn variable_name(&self) -> &Name {
        &self.src_var
    }

    
    fn value_down(
        &self,
        value: Literal,
    ) -> Result<std::collections::BTreeMap<Name, Literal>, ApplicationError> {

        // FOR Order: Given an int 'i' in domain (n..m):
        // {n : 1, n+1 : 1, ..., i : 1, i+1 : 0,.. , m-1 : 0}

        // If not integer - throw ApplicationError
        let Literal::Int(value_i32) = value else {
            return Err(ApplicationError::RuleNotApplicable);
        };

        let mut result = std::collections::BTreeMap::new();

        let idxs = match self.domain.clone().values_i32() {
            Ok(vec_i32) => vec_i32,
            Err(_) => panic!("oh naaauurrrrr"),
        };

        for idx in idxs {
            if idx <= value_i32 { 
                // 1
                let name = Name::Machine(idx);
                let val = Literal::Bool(true);

                result.insert(name, val);
            } else {
                // 0
                let name = Name::Machine(idx);
                let val = Literal::Bool(false);

                result.insert(name, val);
            }
        }

        Ok(result)
    }


    /// The keys are expected to be of the `Name::Machine(i)` variant, and the function will return `i + 1`.
    ///
    /// # Arguments
    ///
    /// * `values`: A reference to a `BTreeMap<Name, Literal>` where `Literal`s are expected to be `Literal::Bool`.
    fn value_up(
        &self,
        values: &std::collections::BTreeMap<Name, Literal>,
    ) -> Result<Literal, ApplicationError> {
        // FOR Order: expect pattern {n : 1, n+1 : 1, ..., i : 1, i+1 : 0,.. , m-1 : 0}
        // return i
        
        let value = values
            .iter()
            .rev()
            .find(|(_, literal)| {
                match literal {
                    Literal::Int(a) => *a == 1,
                    _ => false
                }
            });
        
        print!("{:?}", value);
        let top_index = match value {
            Some(x) => {match x.0 {
                Name::Machine(index) => *index + 1,
                _ => bug!("We expect only machine names in here got {:?} instead", x.0)
            }},
            None => -1,
        };
        // if align_of_val(val) {
            
        // }
        let out = Literal::from(top_index);
        Ok(out)

    }

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

    fn declaration_down(&self) -> Result<Vec<DeclarationPtr>, ApplicationError> {
        Ok(self
            .names()
            .map(|name| DeclarationPtr::new_var(name, Domain::Bool))
            .collect())
    }

    fn repr_name(&self) -> &str {
        "sat_order_int"
    }

    fn box_clone(&self) -> Box<dyn Representation> {
        Box::new(self.clone()) as _
    }
}
