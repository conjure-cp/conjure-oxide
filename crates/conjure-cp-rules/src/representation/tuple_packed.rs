use super::prelude::*;
use conjure_cp::ast::{GroundDomain, Range, Reference};
use conjure_cp::domain_int;
use conjure_cp::representation::ReprInitError;

register_representation!(
    TuplePacked
    struct State<T> {
        /// The single packed integer variable / domain / literal
        pub packed: T,
        /// Domain sizes for each element (number of values in each element domain)
        pub sizes: Vec<i32>,
        /// Strides for each element; stride[i] = product of sizes[i+1..n].
        pub strides: Vec<i32>,
        /// Minimum values for each element domain (offset for encoding)
        pub mins: Vec<i32>,
        /// The total number of packed values (product of sizes)
        pub total_size: i32,
    }
    impl State<DeclarationPtr> {
        pub fn packed_ref(&self) -> Reference {
            Reference::new(self.packed.clone())
        }
        pub fn packed_expr(&self) -> Expression {
            Expression::from(self.packed_ref())
        }
    }
    impl<T> State<T> {
        /// Encode tuple element values into a single packed integer.
        /// Each value is offset by `mins[i]` and multiplied by `strides[i]`.
        pub fn encode(&self, vals: &[i32]) -> i32 {
            vals.iter()
                .enumerate()
                .map(|(i, v)| (v - self.mins[i]) * self.strides[i])
                .sum()
        }

        /// Encode tuple literal expressions into a packed integer literal expression.
        /// Returns `Err(RuleNotApplicable)` if any entry is not an integer literal.
        pub fn encode_lit_entries(&self, entries: &[Expression]) -> Result<Expression, ApplicationError> {
            let mut packed_val: i32 = 0;
            for (i, entry) in entries.iter().enumerate() {
                match entry {
                    Expression::Atomic(_, Atom::Literal(Literal::Int(v))) => {
                        packed_val += (*v - self.mins[i]) * self.strides[i];
                    }
                    _ => return Err(RuleNotApplicable),
                }
            }
            Ok(Expression::from(Literal::Int(packed_val)))
        }
    }
    fn init(dom: DomainPtr) -> Result<State<DomainPtr>, ReprInitError> {
        let domain_err = |msg: &str| {
            ReprInitError::UnsupportedDomain(dom.clone(), TuplePacked::NAME, String::from(msg))
        };

        let Some(gd_tuple) = dom.as_tuple_ground() else {
            return Err(domain_err("expected a ground tuple domain"));
        };

        // Collect element domain sizes and minima
        let mut sizes = Vec::with_capacity(gd_tuple.len());
        let mut mins = Vec::with_capacity(gd_tuple.len());

        for (i, elem_dom) in gd_tuple.iter().enumerate() {
            let GroundDomain::Int(ranges) = elem_dom.as_ref() else {
                return Err(domain_err(&format!("element {i} is not an integer domain")));
            };

            if !Range::is_contiguous(ranges) {
                return Err(domain_err(&format!(
                    "element {i} has non-contiguous ranges; packed repr requires contiguous int domains"
                )));
            }

            let lo = Range::low_of(ranges)
                .ok_or_else(|| domain_err(&format!("element {i} has an unbounded or empty domain")))?;

            let span = Range::total_length(ranges)
                .ok_or_else(|| domain_err(&format!("element {i} has an unbounded range")))?;

            sizes.push(span);
            mins.push(*lo);
        }

        // Compute strides
        let n = sizes.len();
        let mut strides = vec![1i32; n];
        for i in (0..n.saturating_sub(1)).rev() {
            strides[i] = strides[i + 1].checked_mul(sizes[i + 1])
                .ok_or_else(|| domain_err("packed representation would overflow i32"))?;
        }

        let total_size = strides[0].checked_mul(sizes[0])
            .ok_or_else(|| domain_err("packed representation would overflow i32"))?;

        let packed = domain_int!(0..(total_size - 1));
        Ok(State { packed, sizes, strides, mins, total_size })
    }
    fn structural(_state: &State<DeclarationPtr>) -> Vec<Expression> {
        vec![]
    }
    fn down(state: &State<DomainPtr>, value: Literal) -> Result<State<Literal>, ReprDownError> {
        let Literal::AbstractLiteral(AbstractLiteral::Tuple(vals)) = value else {
            return Err(ReprDownError::BadValue(value, String::from("expected a tuple literal")));
        };
        if vals.len() != state.sizes.len() {
            let msg = format!("expected {} elements, got {}", state.sizes.len(), vals.len());
            return Err(ReprDownError::BadValue(Literal::AbstractLiteral(AbstractLiteral::Tuple(vals)), msg));
        }

        // Extract integer values
        let int_vals: Vec<i32> = vals.iter().enumerate().map(|(i, v)| {
            if let Literal::Int(n) = v { Ok(*n) }
            else { Err(ReprDownError::BadValue(v.clone(), format!("element {i} is not an integer literal"))) }
        }).collect::<Result<_, _>>()?;

        Ok(State {
            packed: Literal::Int(state.encode(&int_vals)),
            sizes: state.sizes.clone(),
            strides: state.strides.clone(),
            mins: state.mins.clone(),
            total_size: state.total_size,
        })
    }
    fn up(state: State<Literal>) -> Literal {
        let Literal::Int(packed_val) = state.packed else {
            bug!("expected an integer literal for packed value, got {}", state.packed);
        };
        let mut remaining = packed_val;
        let vals = state.strides.iter().zip(&state.mins).map(|(&stride, &min)| {
            let idx = remaining / stride;
            remaining %= stride;
            Literal::Int(idx + min)
        }).collect();
        Literal::AbstractLiteral(AbstractLiteral::Tuple(vals))
    }
);
