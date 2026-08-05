use crate::shared::representation_prelude::*;
use conjure_cp::ast::{Domain, GroundDomain, JectivityAttr, Moo, PartialityAttr, Range, Reference};
use conjure_cp::{essence_expr, into_matrix_expr, matrix_expr};

register_representation!(
    FunctionExplicit("explicit")
    struct State<T> {
        /// Codomain values in domain-enumeration order, one per domain element.
        pub values_matrix: T,
        /// Definedness flags in the same order, omitted for total functions.
        pub flags_matrix: Option<T>,
        /// Every domain value, in the same order used by `values_matrix`/`flags_matrix`.
        pub domain_values: Moo<Vec<Literal>>,
        /// Every codomain value, used by the surjective structural constraint.
        pub codomain_values: Moo<Vec<Literal>>,
        /// Canonical value stored for undefined positions (the codomain's first value).
        pub padding: Literal,
        /// Jectivity to enforce structurally.
        pub jectivity: JectivityAttr,
        /// Number of defined entries a partial function may have; irrelevant for total functions.
        pub size: Range<i32>
    }
    impl State<DeclarationPtr> {
        /// The matrices are indexed by the function's own domain (e.g. `bool`, not necessarily
        /// `int(1..n)`), so a one-based position must be translated to the domain value at that
        /// position, not passed through as a raw integer.
        fn domain_index_expr(&self, index: i32) -> Expression {
            self.domain_values[(index - 1) as usize].clone().into()
        }

        /// Return the codomain value at a one-based position (domain-enumeration order).
        pub fn value_expr(&self, index: i32) -> Expression {
            Expression::SafeIndex(
                Metadata::new(),
                Moo::new(Reference::new(self.values_matrix.clone()).into()),
                vec![self.domain_index_expr(index)],
            )
        }

        /// Return whether the position at a one-based index is defined.
        pub fn defined_expr(&self, index: i32) -> Expression {
            match &self.flags_matrix {
                Some(flags) => Expression::SafeIndex(
                    Metadata::new(),
                    Moo::new(Reference::new(flags.clone()).into()),
                    vec![self.domain_index_expr(index)],
                ),
                None => true.into(),
            }
        }

        /// The number of defined entries, as `sum(toInt(flags_matrix[i]))` unrolled over every
        /// position (avoiding a comprehension, since `essence_expr!` cannot build one and this
        /// representation otherwise unrolls structural constraints by static position anyway).
        pub fn defined_count_expr(&self, n: i32) -> Option<Expression> {
            self.flags_matrix.as_ref()?;
            let counts: Vec<Expression> = (1..=n)
                .map(|i| {
                    let flag = self.defined_expr(i);
                    essence_expr!(toInt(&flag))
                })
                .collect();
            Some(Expression::Sum(
                Metadata::new(),
                Moo::new(into_matrix_expr!(counts)),
            ))
        }
    }
    fn init(dom: DomainPtr) -> Result<State<DomainPtr>, ReprInitError> {
        let domain_err = |msg: &str| ReprInitError::UnsupportedDomain(
            dom.clone(),
            FunctionExplicit::NAME,
            String::from(msg),
        );
        let Some(GroundDomain::Function(attr, domain, codomain)) = dom.as_ground() else {
            return Err(domain_err("expected a ground function domain"));
        };

        let domain_values: Vec<Literal> = domain.values()
            .map_err(|e| domain_err(&format!("could not enumerate function domain: {e}")))?
            .collect();
        let codomain_values: Vec<Literal> = codomain.values()
            .map_err(|e| domain_err(&format!("could not enumerate function codomain: {e}")))?
            .collect();
        let padding = codomain_values
            .first()
            .cloned()
            .ok_or_else(|| domain_err("function codomain is empty"))?;

        let index_dom = domain.clone().into();
        let values_matrix = Domain::matrix(codomain.clone().into(), vec![index_dom]);
        let flags_matrix = match attr.partiality {
            PartialityAttr::Total => None,
            PartialityAttr::Partial => {
                let index_dom = domain.clone().into();
                Some(Domain::matrix(Domain::bool(), vec![index_dom]))
            }
        };

        Ok(State {
            values_matrix,
            flags_matrix,
            domain_values: Moo::new(domain_values),
            codomain_values: Moo::new(codomain_values),
            padding,
            jectivity: attr.jectivity.clone(),
            size: attr.size.clone(),
        })
    }
    fn structural(state: &State<DeclarationPtr>) -> Vec<Expression> {
        let n = state.domain_values.len() as i32;
        let mut constraints = Vec::new();

        if let Some(count) = state.defined_count_expr(n) {
            match state.size {
                Range::Unbounded => {}
                Range::Single(size) => constraints.push(essence_expr!(&count = &size)),
                Range::UnboundedR(min) => constraints.push(essence_expr!(&count >= &min)),
                Range::UnboundedL(max) => constraints.push(essence_expr!(&count <= &max)),
                Range::Bounded(min, max) => {
                    constraints.push(essence_expr!(r"(&count >= &min) /\ (&count <= &max)"));
                }
            }
        }

        let injective = matches!(
            state.jectivity,
            JectivityAttr::Injective | JectivityAttr::Bijective
        );
        let surjective = matches!(
            state.jectivity,
            JectivityAttr::Surjective | JectivityAttr::Bijective
        );

        if injective {
            if state.flags_matrix.is_none() {
                // Total: every position is always defined, so a plain allDiff is exact.
                let matrix_ref = Reference::new(state.values_matrix.clone()).into();
                constraints.push(Expression::AllDiff(Metadata::new(), Moo::new(matrix_ref)));
            } else {
                for i in 1..=n {
                    for j in (i + 1)..=n {
                        let i_undefined = Expression::Not(
                            Metadata::new(),
                            Moo::new(state.defined_expr(i)),
                        );
                        let j_undefined = Expression::Not(
                            Metadata::new(),
                            Moo::new(state.defined_expr(j)),
                        );
                        let neq = Expression::Neq(
                            Metadata::new(),
                            Moo::new(state.value_expr(i)),
                            Moo::new(state.value_expr(j)),
                        );
                        constraints.push(Expression::Or(
                            Metadata::new(),
                            Moo::new(matrix_expr![i_undefined, j_undefined, neq]),
                        ));
                    }
                }
            }
        }

        if surjective {
            for value in state.codomain_values.iter() {
                let value_expr: Expression = value.clone().into();
                let hits: Vec<Expression> = (1..=n)
                    .map(|i| {
                        let matches_value = Expression::Eq(
                            Metadata::new(),
                            Moo::new(state.value_expr(i)),
                            Moo::new(value_expr.clone()),
                        );
                        Expression::And(
                            Metadata::new(),
                            Moo::new(matrix_expr![state.defined_expr(i), matches_value]),
                        )
                    })
                    .collect();
                constraints.push(Expression::Or(Metadata::new(), Moo::new(into_matrix_expr![hits])));
            }
        }

        constraints
    }
    fn down(state: &State<DomainPtr>, value: Literal) -> Result<State<Literal>, ReprDownError> {
        let Literal::AbstractLiteral(AbstractLiteral::Function(pairs)) = value else {
            return Err(ReprDownError::BadValue(value, String::from("expected a function literal")));
        };

        let mut values = Vec::with_capacity(state.domain_values.len());
        let mut flags = Vec::with_capacity(state.domain_values.len());
        let mut matched = 0usize;
        for domain_value in state.domain_values.iter() {
            if let Some((_, v)) = pairs.iter().find(|(k, _)| k == domain_value) {
                values.push(v.clone());
                flags.push(Literal::Bool(true));
                matched += 1;
            } else {
                values.push(state.padding.clone());
                flags.push(Literal::Bool(false));
            }
        }
        if matched != pairs.len() {
            return Err(ReprDownError::BadValue(
                AbstractLiteral::Function(pairs).into(),
                String::from("function literal has a key outside its domain"),
            ));
        }
        if state.flags_matrix.is_none() && matched != state.domain_values.len() {
            return Err(ReprDownError::BadValue(
                AbstractLiteral::Function(pairs).into(),
                String::from("total function literal is missing an entry for some domain value"),
            ));
        }

        let index_dom = match state.values_matrix.as_ref() {
            conjure_cp::ast::Domain::Ground(gd) => match gd.as_ref() {
                GroundDomain::Matrix(_, idx) => idx[0].clone(),
                _ => bug!("expected the values matrix to be ground matrix domain"),
            },
            _ => bug!("expected the values matrix domain to be ground"),
        };

        Ok(State {
            values_matrix: Literal::from(into_matrix![values; index_dom.clone()]),
            flags_matrix: state.flags_matrix.as_ref().map(|_| Literal::from(into_matrix![flags; index_dom])),
            domain_values: state.domain_values.clone(),
            codomain_values: state.codomain_values.clone(),
            padding: state.padding.clone(),
            jectivity: state.jectivity.clone(),
            size: state.size.clone(),
        })
    }
    fn up(state: State<Literal>) -> Literal {
        let Literal::AbstractLiteral(AbstractLiteral::Matrix(values, _)) = state.values_matrix else {
            bug!("expected function values to be a matrix, got {}", state.values_matrix)
        };
        let flags: Option<Vec<Literal>> = match state.flags_matrix {
            Some(Literal::AbstractLiteral(AbstractLiteral::Matrix(flags, _))) => Some(flags),
            Some(other) => bug!("expected function flags to be a matrix, got {other}"),
            None => None,
        };

        let pairs = state.domain_values.iter().cloned().zip(values).enumerate()
            .filter_map(|(i, (key, value))| {
                let defined = flags.as_ref().is_none_or(|flags| flags[i] == Literal::Bool(true));
                defined.then_some((key, value))
            })
            .collect();
        Literal::AbstractLiteral(AbstractLiteral::Function(pairs))
    }
);
