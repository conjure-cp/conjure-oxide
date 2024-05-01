//! Uniplate and Traversal utilities for the AST.

use super::Expression;
use im::vector;
use uniplate::biplate::Uniplate;

use crate::ast::constants::Constant;

//NOTE (niklasdewally): Temporary manual implementation until the macro is sorted out
impl Uniplate for Expression {
    fn uniplate(
        &self,
    ) -> (
        uniplate::Tree<Self>,
        Box<dyn Fn(uniplate::Tree<Self>) -> Self>,
    ) {
        use uniplate::Tree::*;
        use Expression::*;
        match self {
            Nothing => (Zero, Box::new(|_| Nothing)),
            Constant(m, c) => {
                let m = m.clone(); // allows us to move m into the closure.
                let c = c.clone();
                (Zero, Box::new(move |_| (Constant(m.clone(), c.clone()))))
            }
            Reference(m, n) => {
                let m = m.clone();
                let n = n.clone();
                (Zero, Box::new(move |_| (Reference(m.clone(), n.clone()))))
            }
            Sum(m, es) => {
                let m = m.clone();
                (
                    Many(es.iter().map(|e| One(e.clone())).collect()),
                    Box::new(move |x| {
                        let Many(es) = x else { panic!() };
                        let es: Vec<Expression> = es
                            .into_iter()
                            .map(|e| {
                                let One(e) = e else { panic!() };
                                e
                            })
                            .collect();
                        Sum(m.clone(), es)
                    }),
                )
            }
            Min(m, es) => {
                let m = m.clone();
                (
                    Many(es.iter().map(|e| One(e.clone())).collect()),
                    Box::new(move |x| {
                        let Many(es) = x else { panic!() };
                        let es: Vec<Expression> = es
                            .into_iter()
                            .map(|e| {
                                let One(e) = e else { panic!() };
                                e
                            })
                            .collect();
                        Min(m.clone(), es)
                    }),
                )
            }
            And(m, es) => {
                let m = m.clone();
                (
                    Many(es.iter().map(|e| One(e.clone())).collect()),
                    Box::new(move |x| {
                        let Many(es) = x else { panic!() };
                        let es: Vec<Expression> = es
                            .into_iter()
                            .map(|e| {
                                let One(e) = e else { panic!() };
                                e
                            })
                            .collect();
                        And(m.clone(), es)
                    }),
                )
            }
            Or(m, es) => {
                let m = m.clone();
                (
                    Many(es.iter().map(|e| One(e.clone())).collect()),
                    Box::new(move |x| {
                        let Many(es) = x else { panic!() };
                        let es: Vec<Expression> = es
                            .into_iter()
                            .map(|e| {
                                let One(e) = e else { panic!() };
                                e
                            })
                            .collect();
                        Or(m.clone(), es)
                    }),
                )
            }
            AllDiff(m, es) => {
                let m = m.clone();
                (
                    Many(es.iter().map(|e| One(e.clone())).collect()),
                    Box::new(move |x| {
                        let Many(es) = x else { panic!() };
                        let es: Vec<Expression> = es
                            .into_iter()
                            .map(|e| {
                                let One(e) = e else { panic!() };
                                e
                            })
                            .collect();
                        AllDiff(m.clone(), es)
                    }),
                )
            }

            Not(m, e) => {
                let m = m.clone();
                (
                    One(*e.clone()),
                    Box::new(move |x| {
                        let One(e) = x else { panic!() };
                        Not(m.clone(), Box::new(e))
                    }),
                )
            }
            Bubble(m, e1, e2) => {
                let m = m.clone();
                (
                    Many(vector![One(*e1.clone()), One(*e2.clone())]),
                    Box::new(move |x| {
                        let Many(es) = x else { panic!() };
                        let One(e1) = &es[0] else { panic!() };
                        let One(e2) = &es[1] else { panic!() };
                        Bubble(m.clone(), Box::new(e1.clone()), Box::new(e2.clone()))
                    }),
                )
            }
            Eq(m, e1, e2) => {
                let m = m.clone();
                (
                    Many(vector![One(*e1.clone()), One(*e2.clone())]),
                    Box::new(move |x| {
                        let Many(es) = x else { panic!() };
                        let One(e1) = &es[0] else { panic!() };
                        let One(e2) = &es[1] else { panic!() };
                        Eq(m.clone(), Box::new(e1.clone()), Box::new(e2.clone()))
                    }),
                )
            }
            Neq(m, e1, e2) => {
                let m = m.clone();
                (
                    Many(vector![One(*e1.clone()), One(*e2.clone())]),
                    Box::new(move |x| {
                        let Many(es) = x else { panic!() };
                        let One(e1) = &es[0] else { panic!() };
                        let One(e2) = &es[1] else { panic!() };
                        Neq(m.clone(), Box::new(e1.clone()), Box::new(e2.clone()))
                    }),
                )
            }
            Geq(m, e1, e2) => {
                let m = m.clone();
                (
                    Many(vector![One(*e1.clone()), One(*e2.clone())]),
                    Box::new(move |x| {
                        let Many(es) = x else { panic!() };
                        let One(e1) = &es[0] else { panic!() };
                        let One(e2) = &es[1] else { panic!() };
                        Geq(m.clone(), Box::new(e1.clone()), Box::new(e2.clone()))
                    }),
                )
            }
            Leq(m, e1, e2) => {
                let m = m.clone();
                (
                    Many(vector![One(*e1.clone()), One(*e2.clone())]),
                    Box::new(move |x| {
                        let Many(es) = x else { panic!() };
                        let One(e1) = &es[0] else { panic!() };
                        let One(e2) = &es[1] else { panic!() };
                        Leq(m.clone(), Box::new(e1.clone()), Box::new(e2.clone()))
                    }),
                )
            }
            Gt(m, e1, e2) => {
                let m = m.clone();
                (
                    Many(vector![One(*e1.clone()), One(*e2.clone())]),
                    Box::new(move |x| {
                        let Many(es) = x else { panic!() };
                        let One(e1) = &es[0] else { panic!() };
                        let One(e2) = &es[1] else { panic!() };
                        Gt(m.clone(), Box::new(e1.clone()), Box::new(e2.clone()))
                    }),
                )
            }
            Lt(m, e1, e2) => {
                let m = m.clone();
                (
                    Many(vector![One(*e1.clone()), One(*e2.clone())]),
                    Box::new(move |x| {
                        let Many(es) = x else { panic!() };
                        let One(e1) = &es[0] else { panic!() };
                        let One(e2) = &es[1] else { panic!() };
                        Lt(m.clone(), Box::new(e1.clone()), Box::new(e2.clone()))
                    }),
                )
            }
            SafeDiv(m, e1, e2) => {
                let m = m.clone();
                (
                    Many(vector![One(*e1.clone()), One(*e2.clone())]),
                    Box::new(move |x| {
                        let Many(es) = x else { panic!() };
                        let One(e1) = &es[0] else { panic!() };
                        let One(e2) = &es[1] else { panic!() };
                        SafeDiv(m.clone(), Box::new(e1.clone()), Box::new(e2.clone()))
                    }),
                )
            }
            UnsafeDiv(m, e1, e2) => {
                let m = m.clone();
                (
                    Many(vector![One(*e1.clone()), One(*e2.clone())]),
                    Box::new(move |x| {
                        let Many(es) = x else { panic!() };
                        let One(e1) = &es[0] else { panic!() };
                        let One(e2) = &es[1] else { panic!() };
                        UnsafeDiv(m.clone(), Box::new(e1.clone()), Box::new(e2.clone()))
                    }),
                )
            }
            DivEq(m, e1, e2, e3) => {
                let m = m.clone();
                (
                    Many(vector![
                        One(*e1.clone()),
                        One(*e2.clone()),
                        One(*e3.clone())
                    ]),
                    Box::new(move |x| {
                        let Many(es) = x else { panic!() };
                        let One(e1) = &es[0] else { panic!() };
                        let One(e2) = &es[1] else { panic!() };
                        let One(e3) = &es[2] else { panic!() };
                        DivEq(
                            m.clone(),
                            Box::new(e1.clone()),
                            Box::new(e2.clone()),
                            Box::new(e3.clone()),
                        )
                    }),
                )
            }
            Ineq(m, e1, e2, e3) => {
                let m = m.clone();
                (
                    Many(vector![
                        One(*e1.clone()),
                        One(*e2.clone()),
                        One(*e3.clone())
                    ]),
                    Box::new(move |x| {
                        let Many(es) = x else { panic!() };
                        let One(e1) = &es[0] else { panic!() };
                        let One(e2) = &es[1] else { panic!() };
                        let One(e3) = &es[2] else { panic!() };
                        Ineq(
                            m.clone(),
                            Box::new(e1.clone()),
                            Box::new(e2.clone()),
                            Box::new(e3.clone()),
                        )
                    }),
                )
            }
            SumEq(m, es, e) => {
                let m = m.clone();
                let field_1 = Many(es.iter().map(|e| One(e.clone())).collect());
                let field_2 = One(*e.clone());
                let children = Many(vector![field_1, field_2]);
                (
                    children,
                    Box::new(move |x| {
                        let Many(x) = x else { panic!() };
                        let Many(es) = &x[0] else { panic!() };
                        let One(e) = &x[1] else { panic!() };
                        let es: Vec<Expression> = es
                            .iter()
                            .map(|e| {
                                let One(e) = e else { panic!() };
                                e.clone()
                            })
                            .collect();
                        SumEq(m.clone(), es, Box::new(e.clone()))
                    }),
                )
            }
            SumGeq(m, es, e) => {
                let m = m.clone();
                let field_1 = Many(es.iter().map(|e| One(e.clone())).collect());
                let field_2 = One(*e.clone());
                let children = Many(vector![field_1, field_2]);
                (
                    children,
                    Box::new(move |x| {
                        let Many(x) = x else { panic!() };
                        let Many(es) = &x[0] else { panic!() };
                        let One(e) = &x[1] else { panic!() };
                        let es: Vec<Expression> = es
                            .iter()
                            .map(|e| {
                                let One(e) = e else { panic!() };
                                e.clone()
                            })
                            .collect();
                        SumGeq(m.clone(), es, Box::new(e.clone()))
                    }),
                )
            }
            SumLeq(m, es, e) => {
                let m = m.clone();
                let field_1 = Many(es.iter().map(|e| One(e.clone())).collect());
                let field_2 = One(*e.clone());
                let children = Many(vector![field_1, field_2]);
                (
                    children,
                    Box::new(move |x| {
                        let Many(x) = x else { panic!() };
                        let Many(es) = &x[0] else { panic!() };
                        let One(e) = &x[1] else { panic!() };
                        let es: Vec<Expression> = es
                            .iter()
                            .map(|e| {
                                let One(e) = e else { panic!() };
                                e.clone()
                            })
                            .collect();
                        SumLeq(m.clone(), es, Box::new(e.clone()))
                    }),
                )
            }
        }
    }
}
