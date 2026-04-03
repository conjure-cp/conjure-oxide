// crates/conjure-cp-rules/src/representation/sat_direct_int.rs
use super::prelude::*;
use crate::utils::lit_to_bool;
use conjure_cp::ast::{Domain, Range, domains::Int};
use std::collections::HashMap;
use std::collections::VecDeque;
use std::hash::Hash;

register_representation!(
    IntToBoolDirect
    struct State<T: Eq + Hash> {
        // Mapping of each possible value i of the original integer x to a boolean b_i <-> (x = i)
        pub vals: HashMap<Int, T>
    }
    pub fn init(dom: DomainPtr) -> Result<State<DomainPtr>, ReprInitError> {
        let Some(rngs) = dom.as_int_ground() else {
            return Err(ReprInitError::UnsupportedDomain(dom, IntToBoolDirect::NAME, String::from("expected a ground int domain")));
        };
        let Some(itr) = Range::values(rngs) else {
            return Err(ReprInitError::UnsupportedDomain(dom, IntToBoolDirect::NAME, String::from("domain is not enumerable")));
        };
        let vals: HashMap<Int, DomainPtr> = itr.map(|v| (v, Domain::bool())).collect();
        Ok(State { vals })
    }
    fn structural(_: &State<DeclarationPtr>) -> Vec<Expression> {
        // let elems: Vec<&DeclarationPtr> = state.vals.right_values().collect();
        // let n = elems.len();
        // let mut res = Vec::<Expression>::with_capacity(n);
        // for i in 0..n {
        //     // the i-th bool variable
        //     let this = Reference::from(elems[i].clone());

        //     // all other bool variables from this representation
        //     let others: Vec<Expression> = chain!(&elems[0..i], &elems[i + 1..n])
        //         .map(|d| Reference::from((*d).clone()).into()).collect();
        //     let others_mat = into_matrix_expr!(others);

        //     // if b_i is true, all others must be false
        //     res.push(essence_expr!(&this <-> !or(&others_mat)));
        // }
        // res
        vec![]
    }
    fn down(state: &State<DomainPtr>, value: Literal) -> Result<State<Literal>, ReprDownError> {
        let Literal::Int(x) = value else {
            return Err(ReprDownError::BadValue(value, String::from("expected an int literal")))
        };
        let mut vals: HashMap<Int, Literal> = state.vals.keys().map(|k| (*k, false.into())).collect();
        vals.insert(x, true.into());
        println!("{:#?}", vals);
        Ok(State { vals })
    }
    fn up(state: State<Literal>) -> Literal {
        let mut ans = None;
        for (k, v) in state.vals.into_iter() {
            if lit_to_bool(&v) {
                if ans.is_some() {
                    bug!("more than one value was true");
                }
                ans = Some(Literal::from(k));
            }
        }
        ans.unwrap_or_else(|| bug!("none of the given values were true"))
    }
    fn repr_vars(state: &State<DeclarationPtr>) -> VecDeque<DeclarationPtr> {
        state.vals.values().cloned().collect()
    }
);
