use itertools::Itertools;

pub use super::prelude::*;

register_represention!(MatrixToAtom, "matrix_to_atom");

#[derive(Clone, Debug)]
pub struct MatrixToAtom {
    src_var: Name,

    // all the possible indices in this matrix, in order.
    indices: Vec<Vec<Literal>>,

    // the element domain for the matrix.
    elem_domain: Domain,
}

impl MatrixToAtom {
    /// Returns the names of the representation variables, in the same order as the indices.
    fn names(&self) -> Vec<Name> {
        self.indices
            .iter()
            .map(|x| self.indices_to_name(x))
            .collect_vec()
    }

    /// Gets the representation variable name for a specific set of indices.
    fn indices_to_name(&self, indices: &[Literal]) -> Name {
        Name::RepresentationOf(
            Box::new(self.src_var.clone()),
            self.repr_name().to_string(),
            indices.iter().join("_"),
        )
    }

    /// Panics if name is invalid.
    fn name_to_indices(&self, name: &Name) -> Vec<Literal> {
        let Name::RepresentationOf(src_var, rule_string, suffix) = name else {
            bug!("representation name should be Name::RepresentationOf");
        };

        assert_eq!(
            src_var.as_ref(),
            self.variable_name(),
            "name should have the same source var as self"
        );
        assert_eq!(
            rule_string,
            self.repr_name(),
            "name should have the same repr_name as self"
        );

        // FIXME: call the parser here to parse the literals properly; support more literal kinds
        // ~niklasdewally
        let indices = suffix.split("_").collect_vec();
        assert_eq!(
            indices.len(),
            self.indices[0].len(),
            "name should have same number of indices as self"
        );

        let parsed_indices = indices
            .into_iter()
            .map(|x| match x {
                "true" => Literal::Bool(true),
                "false" => Literal::Bool(false),
                x if x.parse::<i32>().is_ok() => {
                    let i: i32 = x
                        .parse()
                        .expect("already checked whether this parses into an int");
                    Literal::Int(i)
                }

                x => bug!("{x} should be a string that can parse into a valid Literal"),
            })
            .collect_vec();

        assert!(
            self.indices.contains(&parsed_indices),
            "indices parsed from the representation name should be valid indices for this variable"
        );

        parsed_indices
    }
}

impl Representation for MatrixToAtom {
    fn init(name: &Name, symtab: &SymbolTable) -> Option<Self> {
        let domain = symtab.resolve_domain(name)?;

        if !domain
            .is_finite()
            .expect("domain was resolved earlier, so should be ground here")
        {
            return None;
        }

        let Domain::DomainMatrix(elem_domain, index_domains) = domain else {
            return None;
        };

        let indices = index_domains
            .iter()
            .map(|domain| {
                domain.values().expect(
                    "as this is an index domain, it should be finite and we should be able to enumerable over its values",
                )
            })
            .multi_cartesian_product()
            .collect_vec();

        Some(MatrixToAtom {
            src_var: name.clone(),
            indices,
            elem_domain: *elem_domain,
        })
    }

    fn variable_name(&self) -> &Name {
        &self.src_var
    }

    fn value_down(
        &self,
        value: Literal,
    ) -> Result<std::collections::BTreeMap<Name, Literal>, ApplicationError> {
        todo!()
    }

    fn value_up(
        &self,
        values: std::collections::BTreeMap<Name, Literal>,
    ) -> Result<Literal, ApplicationError> {
        todo!()
    }

    fn expression_down(&self, _: &SymbolTable) -> Result<Vec<Expression>, ApplicationError> {
        Ok(self
            .names()
            .into_iter()
            .map(|name| Expression::Atomic(Metadata::new(), Atom::Reference(name)))
            .collect_vec())
    }

    fn declaration_down(&self) -> Result<Vec<Declaration>, ApplicationError> {
        Ok(self
            .names()
            .into_iter()
            .map(|name| Declaration::new_var(name, self.elem_domain.clone()))
            .collect_vec())
    }

    fn repr_name(&self) -> &str {
        "matrix_to_atom"
    }

    fn box_clone(&self) -> Box<dyn Representation> {
        Box::new(self.clone()) as _
    }
}
