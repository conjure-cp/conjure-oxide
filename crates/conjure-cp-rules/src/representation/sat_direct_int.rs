use conjure_cp::{
    ast::{Atom, DeclarationPtr, Expression, Literal, Metadata, Name, Range, SymbolTable},
    register_representation,
    representation::Representation,
    rule_engine::ApplicationError,
};

register_representation!(SATDirectInt, "sat_direct_int");

#[derive(Clone, Debug)]
pub struct SATDirectInt {
    src_var: Name,
    domain: Domain,
}

impl SATDirectInt {
    /// Returns the names of the representation variable
    fn names(&self) -> Impl Iterator<Item = Name> + '_ {
        let name_vec = self
            .domain
            .values_i32()
            .into_iter()
            .flatten()
            .map(move |index| self.index_to_name(index));
        println!("names: {:?}", name_vec);
        name_vec.clone().for_each(|i| println!("{:?}!", i));
        name_vec
    }

    /// Get the representation variable name for a specific index
    fn index_to_name(&self, index: i32) -> Name {
        Name::Represented(Box::new((
            self.src_var.clone(),
            self.repr_name().into(),
            format!("{index:02}").into(),
        )))
    }    
}

impl Representation for SATDirectInt {
    fn init(name: &Name, symtab: &SymbolTable) -> Option<Self> {
        let domain = symtab.resolve_domain(name)?;

        // domain should be finite
        if !domain.is_finite().expect("Domain should be finite") {
            return None;
        }

        // ensure integer domain
        let Domain::Int(ranges) = domain.clone() else {
            return None;
        };

        // Essence only supports decision variables with finite domains
        if !ranges
            .iter()
            .all(|i| matches!(i, Range::Bounded(_, _)) || matches!(i, Range::Single(_)))
        {
            return None;
        }

        Some(SATDirectInt {
            src_var: name.clone(),
            domain,
        })
    }

    fn variable_name(&self) -> &Name {
        &self.src_var
    }

    // convert integer value to its Boolean encoding
    fn value_down(
        &self,
        value: Literal,
    ) -> Result<std::collections::BTreeMap<Name, Literal>, ApplicationError> {
        // for direct encoding: given int 'i' in domain (n..m):
        // i : 1, all other j in (n..m), j != i : 0

        // ensure value is an integer
        let Literal::Int(value_i32) = value else {
            return Err(ApplicationError::RuleNotApplicable);
        }

        let mut result = std::collections::BTreeMap::new();

        // get the list of integers in the domain
        let idxs = match self.domain.clone().values_i32() {
            Ok(vec_i32) => vec_i32,
            Err(_) => panic!("Error"),
        };

        for idx in idxs {
            println!("{:?}", idx);
            let name = self.index_to_name(idx);
            let val = Literal::Bool(idx == value_i32);  // only the bit equal to value is true

            result.insert(name, val);
        }

        Ok(result);
    }

    fn value_up(
        &self,
        values: &std::collections::BTreeMap<Name, Literal>,
    ) -> Result<Literal, ApplicationError> {
        // find the Boolean var that is true (x_i = 1)
        let value = values.iter().find_map(|(name, val)| {
            if *val == Literal::Bool(true) {
                // extract index from the variable name
                if let Name::Represented(boxed) = name {
                    let (_, _, index_str) = &**boxed;
                    index_str.parse::<i32>().ok()
                } else {
                    None
                }
            }
        });

        match value {
            Some(idx) => Ok(Literal::Int(idx)),
            None => Err(ApplicationError::RuleNotApplicable),
        }
    }

    fn expression_down(
        &self,
        symtab: &SymbolTable,
    ) -> Result<std::collections::BTreeMap<Name, Expression>, ApplicationError> {
        let e = self
            .names()
            .map(|name| {
                let decl = symtab.lookup(&name).unwrap();
                (
                    name,
                    Expression::Atomic(
                        Metadata::new(),
                        Atom::Reference(conjure_cp::ast::Reference {
                            ptr: decl
                        }),
                    ),
                )
            })
            .collect();

        Ok(e)
    }

    fn declaration_down(&self) -> Result<Vec<DeclarationPtr>, ApplicationError> {
        let e = self
            .names()
            .map(|name| DeclarationPtr::new_var(name, Domain::Bool))
            .collect();
        Ok(e)
    }

    fn repr_name(&self) -> &str {
        "sat_direct_int"
    }

    fn box_clone(&self) -> Box<dyn Representation> {
        Box::new(self.clone()) as _
    }
}
