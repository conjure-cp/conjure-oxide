use crate::shared::representation_prelude::*;
use crate::types::relation::binary_attrs::{binary_relation_constraints, quantify};
use conjure_cp::ast::ac_operators::ACOperatorKind;
use conjure_cp::ast::matrix::{enumerate_indices, flatten_enumerate, unflatten_matrix};
use conjure_cp::ast::{BinaryAttr, Domain, DomainOpError, GroundDomain, Moo, Range, Reference};
use conjure_cp::essence_expr;

/// A relation is too large to represent densely once its columns' cartesian product exceeds this
/// many cells (one Boolean decision variable per cell).
const MAX_CELLS: u64 = 10_000;

register_representation!(
    RelationOccurrence("occurrence")
    struct State<T> {
        /// The relation, channelled to one dense Boolean matrix indexed by every column domain:
        /// cell `[v1,...,vN]` holds whether `(v1,...,vN)` is a member. Only applicable when every
        /// column can index a matrix (finite discrete: bool/int) -- a set-typed column, for
        /// instance, only `RelationAsSet` supports.
        pub matrix_decl: T,
        /// Binary-relation attributes to enforce structurally, if any. Only ever non-empty for a
        /// binary relation whose two columns have the same domain (`init` rejects any other
        /// combination).
        pub binary_attrs: Vec<BinaryAttr>,
        /// The shared column domain, present only when `binary_attrs` is non-empty.
        pub binary_domain: Option<DomainPtr>,
        /// The relation's column (index) domains, needed to decode/encode literals and to build
        /// the cardinality sum.
        pub inner_domains: Moo<Vec<Moo<GroundDomain>>>,
        /// Concrete cardinality bounds derived from the domain's size attribute.
        pub cardinality: (i32, i32)
    }
    impl State<DeclarationPtr> {
        /// `sum([toInt(matrix[i0,...,iN]) | i0 : dom0, ..., iN : domN])`, i.e. the relation's
        /// cardinality: the number of `true` cells in the matrix.
        pub fn cardinality_expr(&self) -> Expression {
            let matrix_ref: Expression = Reference::new(self.matrix_decl.clone()).into();
            let inner_domains: Vec<DomainPtr> =
                self.inner_domains.iter().map(|d| d.clone().into()).collect();
            let names: Vec<String> = (0..inner_domains.len()).map(|i| format!("i{i}")).collect();
            let name_refs: Vec<&str> = names.iter().map(String::as_str).collect();
            quantify(&inner_domains, &name_refs, ACOperatorKind::Sum, |vars| {
                let cell = Expression::SafeIndex(
                    Metadata::new(),
                    Moo::new(matrix_ref.clone()),
                    vars.to_vec(),
                );
                essence_expr!(toInt(&cell))
            })
        }

        /// Lower equality between a dense relation and a relation literal to matrix equality,
        /// leaving `MatrixComponents`' own literal-equality machinery to finish the job.
        pub fn equality_to_literal_expr(
            &self,
            tuples: &[Vec<Literal>],
        ) -> Result<Expression, DomainOpError> {
            let matrix_ref: Expression = Reference::new(self.matrix_decl.clone()).into();
            let matrix_lit = tuples_to_matrix_literal(&self.inner_domains, tuples)?;
            Ok(Expression::Eq(
                Metadata::new(),
                Moo::new(matrix_ref),
                Moo::new(matrix_lit.into()),
            ))
        }
    }
    fn init(dom: DomainPtr) -> Result<State<DomainPtr>, ReprInitError> {
        let domain_err = |msg: &str| ReprInitError::UnsupportedDomain(
            dom.clone(),
            RelationOccurrence::NAME,
            String::from(msg),
        );
        let Some(GroundDomain::Relation(attr, inner_doms)) = dom.as_ground() else {
            return Err(domain_err("expected a ground relation domain"));
        };
        for d in inner_doms.iter() {
            if !can_index_matrix(d) {
                return Err(domain_err(
                    "occurrence representation requires every column to be a matrix-indexable (bool/int) domain",
                ));
            }
        }

        let dims = inner_doms
            .iter()
            .map(|d| d.length())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| domain_err(&format!("could not enumerate a relation column domain: {e}")))?;
        let total_cells: u64 = dims.iter().product();
        if total_cells > MAX_CELLS {
            return Err(domain_err("relation is too large for a dense occurrence matrix"));
        }
        let cardinality = cardinality_bounds(&attr.size, total_cells)
            .ok_or_else(|| domain_err("invalid or unsupported relation cardinality"))?;

        let binary_domain = if attr.binary.is_empty() {
            None
        } else {
            let [domain, codomain] = inner_doms.as_slice() else {
                return Err(domain_err(
                    "binary relation attributes (reflexive/symmetric/etc) only apply to binary relations",
                ));
            };
            if domain != codomain {
                return Err(domain_err(
                    "binary relation attributes (reflexive/symmetric/etc) require both columns to share the same domain",
                ));
            }
            Some(domain.clone().into())
        };

        let matrix_decl = Domain::matrix(
            Domain::bool(),
            inner_doms.iter().map(|d| d.clone().into()).collect(),
        );
        Ok(State {
            matrix_decl,
            binary_attrs: attr.binary.clone(),
            binary_domain,
            inner_domains: Moo::new(inner_doms.clone()),
            cardinality,
        })
    }
    fn structural(state: &State<DeclarationPtr>) -> Vec<Expression> {
        let matrix_ref: Expression = Reference::new(state.matrix_decl.clone()).into();
        let count = state.cardinality_expr();
        let (min, max) = state.cardinality;
        let mut constraints = if min == max {
            vec![essence_expr!(&count = &min)]
        } else {
            vec![essence_expr!(r"(&count >= &min) /\ (&count <= &max)")]
        };

        if let Some(binary_domain) = &state.binary_domain {
            let member = |x: &Expression, y: &Expression| {
                Expression::SafeIndex(
                    Metadata::new(),
                    Moo::new(matrix_ref.clone()),
                    vec![x.clone(), y.clone()],
                )
            };
            constraints.extend(binary_relation_constraints(
                binary_domain,
                &state.binary_attrs,
                &member,
            ));
        }
        constraints
    }
    fn down(state: &State<DomainPtr>, value: Literal) -> Result<State<Literal>, ReprDownError> {
        let Literal::AbstractLiteral(AbstractLiteral::Relation(tuples)) = value else {
            return Err(ReprDownError::BadValue(value, String::from("expected a relation literal")));
        };

        let (min, max) = state.cardinality;
        let tuples_sz = tuples.len() as i32;
        if tuples_sz < min || tuples_sz > max {
            return Err(ReprDownError::BadValue(
                AbstractLiteral::Relation(tuples).into(),
                format!("expected between {min} and {max} tuples, got {tuples_sz}"),
            ));
        }

        let matrix_decl = tuples_to_matrix_literal(&state.inner_domains, &tuples).map_err(|e| {
            ReprDownError::BadValue(
                AbstractLiteral::Relation(tuples.clone()).into(),
                format!("could not enumerate a relation column domain: {e}"),
            )
        })?;

        Ok(State {
            matrix_decl,
            binary_attrs: state.binary_attrs.clone(),
            binary_domain: state.binary_domain.clone(),
            inner_domains: state.inner_domains.clone(),
            cardinality: state.cardinality,
        })
    }
    fn up(state: State<Literal>) -> Literal {
        let Literal::AbstractLiteral(matrix_lit @ AbstractLiteral::Matrix(..)) = state.matrix_decl else {
            bug!("expected a relation-occurrence value to be a matrix, got {}", state.matrix_decl)
        };
        let tuples = flatten_enumerate(matrix_lit)
            .filter_map(|(index, cell)| match cell {
                Literal::Bool(true) | Literal::Int(1) => Some(index),
                Literal::Bool(false) | Literal::Int(0) => None,
                other => bug!("expected a Boolean relation-occurrence cell, got {other}"),
            })
            .collect();
        Literal::AbstractLiteral(AbstractLiteral::Relation(tuples))
    }
);

fn can_index_matrix(dom: &GroundDomain) -> bool {
    matches!(dom, GroundDomain::Bool | GroundDomain::Int(_))
}

/// Builds the dense (nested) matrix literal for a relation's tuple list: cell `[v1,...,vN]` is
/// `true` iff `(v1,...,vN)` appears in `tuples`. Shared by `down` and constant-equality lowering.
fn tuples_to_matrix_literal(
    inner_domains: &[Moo<GroundDomain>],
    tuples: &[Vec<Literal>],
) -> Result<Literal, DomainOpError> {
    let dims: Vec<usize> = inner_domains
        .iter()
        .map(|d| d.length().map(|n| n as usize))
        .collect::<Result<_, _>>()?;
    let strides = strides_for(&dims);

    let flat: Vec<Literal> = enumerate_indices(inner_domains.to_vec())
        .map(|index| {
            let present = tuples.iter().any(|tuple| {
                tuple.len() == index.len()
                    && tuple
                        .iter()
                        .zip(&index)
                        .all(|(a, b)| a.essence_cmp(b).is_eq())
            });
            Literal::Bool(present)
        })
        .collect();
    Ok(unflatten_matrix(&flat, inner_domains, &strides))
}

/// Standard row-major strides: `strides[i]` is the number of cells spanned by one step along
/// dimension `i`, i.e. the product of every later dimension's length. The final entry is unused
/// by [`unflatten_matrix`]'s base case, but is filled in for a well-formed slice.
fn strides_for(dims: &[usize]) -> Vec<usize> {
    let mut strides = vec![1usize; dims.len()];
    for i in (0..dims.len().saturating_sub(1)).rev() {
        strides[i] = strides[i + 1] * dims[i + 1];
    }
    strides
}

fn cardinality_bounds(size: &Range<i32>, inner_len: u64) -> Option<(i32, i32)> {
    let inner_len = i32::try_from(inner_len).ok()?;
    let (min, max) = match size {
        Range::Unbounded => (0, inner_len),
        Range::Single(n) => (*n, *n),
        Range::UnboundedR(min) => (*min, inner_len),
        Range::UnboundedL(max) => (0, *max),
        Range::Bounded(min, max) => (*min, *max),
    };
    let max = max.min(inner_len);
    (0 <= min && min <= max).then_some((min, max))
}
