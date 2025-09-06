use conjure_cp::ast::{DeclarationPtr, Domain};
use conjure_cp::parse::tree_sitter::parse_literal;
use itertools::{Itertools, izip};
use std::collections::BTreeMap;

use super::prelude::*;

register_representation!(MatrixToAtom, "matrix_to_atom");

#[derive(Clone, Debug)]
pub struct MatrixToAtom {
    src_var: Name,

    // all the possible indices in this matrix, in order.
    indices: Vec<Vec<Literal>>,

    // the element domain for the matrix.
    elem_domain: Domain,

    // the index domains for the matrix.
    index_domains: Vec<Domain>,
}

impl MatrixToAtom {
    /// Returns the names of the representation variables, in the same order as the indices.
    fn names(&self) -> impl Iterator<Item = Name> + '_ {
        self.indices.iter().map(|x| self.indices_to_name(x))
    }

    /// Gets the representation variable name for a specific set of indices.
    fn indices_to_name(&self, indices: &[Literal]) -> Name {
        Name::Represented(Box::new((
            self.src_var.clone(),
            self.repr_name().into(),
            indices.iter().join("_").into(),
        )))
    }

    /// Panics if name is invalid.
    #[allow(dead_code)]
    fn name_to_indices(&self, name: &Name) -> Vec<Literal> {
        let Name::Represented(fields) = name else {
            bug!("representation name should be Name::RepresentationOf");
        };

        let (src_var, rule_string, suffix) = fields.as_ref();

        assert_eq!(
            src_var,
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
            .map(|x| match parse_literal(x) {
                Ok(literal) => literal,
                Err(_) => bug!("{x} should be a string that can parse into a valid Literal"),
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

        let Domain::Matrix(elem_domain, index_domains) = domain else {
            return None;
        };

        let indices = matrix::enumerate_indices(index_domains.clone()).collect_vec();

        Some(MatrixToAtom {
            src_var: name.clone(),
            indices,
            elem_domain: *elem_domain,
            index_domains,
        })
    }

    fn variable_name(&self) -> &Name {
        &self.src_var
    }

    fn value_down(&self, value: Literal) -> Result<BTreeMap<Name, Literal>, ApplicationError> {
        let Literal::AbstractLiteral(matrix) = value else {
            return Err(RuleNotApplicable);
        };

        let AbstractLiteral::Matrix(_, ref index_domain) = matrix else {
            return Err(RuleNotApplicable);
        };

        if index_domain.as_ref() != &self.index_domains[0] {
            return Err(RuleNotApplicable);
        }

        Ok(izip!(self.names(), matrix::flatten(matrix)).collect())
    }

    fn value_up(&self, values: &BTreeMap<Name, Literal>) -> Result<Literal, ApplicationError> {
        // TODO: this has no error checking or failures that don't panic...

        let n_dims = self.index_domains.len();
        fn inner(
            current_index: Vec<Literal>,
            current_dim: usize,
            self1: &MatrixToAtom,
            values: &BTreeMap<Name, Literal>,
            n_dims: usize,
        ) -> Literal {
            if current_dim < n_dims {
                Literal::AbstractLiteral(into_matrix![
                    self1.index_domains[current_dim]
                        .values()
                        .unwrap()
                        .into_iter()
                        .map(|i| {
                            let mut current_index_1 = current_index.clone();
                            current_index_1.push(i);
                            inner(current_index_1, current_dim + 1, self1, values, n_dims)
                        })
                        .collect_vec()
                ])
            } else {
                values
                    .get(&self1.indices_to_name(&current_index))
                    .unwrap()
                    .clone()
            }
        }

        Ok(inner(vec![], 0, self, values, n_dims))
    }

    fn expression_down(
        &self,
        symtab: &SymbolTable,
    ) -> Result<BTreeMap<Name, Expression>, ApplicationError> {
        Ok(self
            .names()
            .map(|name| {
                let declaration = symtab.lookup(&name).expect("declarations of the representation variables should exist in the symbol table before expression_down is called");
                (name, declaration)
            })
            .map(|(name, decl)| (name, Expression::Atomic(Metadata::new(), Atom::Reference(decl))))
            .collect())
    }

    fn declaration_down(&self) -> Result<Vec<DeclarationPtr>, ApplicationError> {
        Ok(self
            .names()
            .map(|name| DeclarationPtr::new_var(name, self.elem_domain.clone()))
            .collect_vec())
    }

    fn repr_name(&self) -> &str {
        "matrix_to_atom"
    }

    fn box_clone(&self) -> Box<dyn Representation> {
        Box::new(self.clone()) as _
    }
}
