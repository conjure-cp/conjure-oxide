use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};
use std::{fs, vec};

use conjure_cp_core::Model;
use conjure_cp_core::ast::DeclarationPtr;
use conjure_cp_core::ast::assertions::debug_assert_model_well_formed;
use conjure_cp_core::context::Context;
#[allow(unused)]
use uniplate::Uniplate;

use super::ParseContext;
use super::dominance::parse_dominance_relation;
use super::find::{parse_find_statement, parse_given_statement};
use super::letting::parse_letting_statement;
use super::objective::parse_objective_statement;
use super::util::{TypecheckingContext, get_tree};
use crate::diagnostics::source_map::SourceMap;
use crate::errors::{FatalParseError, ParseErrorCollection, RecoverableParseError};
use crate::expression::parse_expression;
use crate::syntax_errors::detect_syntactic_errors;
use tree_sitter::Tree;

/// Parse an Essence file into a Model using the tree-sitter parser.
pub fn parse_essence_file_native(
    path: &str,
    context: Arc<RwLock<Context<'static>>>,
) -> Result<Model, Box<ParseErrorCollection>> {
    let source_code = fs::read_to_string(path)
        .unwrap_or_else(|_| panic!("Failed to read the source code file {path}"));

    let mut errors = vec![];
    let model = parse_essence_with_context(&source_code, context, &mut errors);

    match model {
        Ok(Some(m)) => {
            debug_assert_model_well_formed(&m, "tree-sitter");
            Ok(m)
        }
        Ok(None) => {
            // Recoverable errors were found, return them as a ParseErrorCollection
            Err(Box::new(ParseErrorCollection::multiple(
                errors,
                Some(source_code),
                Some(path.to_string()),
            )))
        }
        Err(fatal) => {
            // Fatal error - wrap in ParseErrorCollection::Fatal
            Err(Box::new(ParseErrorCollection::fatal(fatal)))
        }
    }
}

pub fn parse_essence_with_context(
    src: &str,
    context: Arc<RwLock<Context<'static>>>,
    errors: &mut Vec<RecoverableParseError>,
) -> Result<Option<Model>, FatalParseError> {
    match parse_essence_with_context_and_map(src, context, errors, None)? {
        (Some(model), _source_map) => Ok(Some(model)),
        (None, _source_map) => Ok(None),
    }
}

/*
    this function is used by both the file-based parser and the LSP parser (which needs the source map)
    the LSP parser can also optionally pass in a pre-parsed tree to avoid parsing twice (which is how caching is implemented)
    if the tree is not passed in, we will parse it from scratch (this is what the file-based parser does)
    when cache is dirty, LSP has to call parse_essence_with_context_and_map with None for the tree,
    which will cause it to re-parse the source code and update the cache (Model = ast, SorceMap = map)
*/
pub fn parse_essence_with_context_and_map(
    src: &str,
    context: Arc<RwLock<Context<'static>>>,
    errors: &mut Vec<RecoverableParseError>,
    tree: Option<&Tree>,
) -> Result<(Option<Model>, SourceMap), FatalParseError> {
    let (tree, source_code) = if let Some(tree) = tree {
        (tree.clone(), src.to_string())
    } else {
        match get_tree(src) {
            Some(tree) => tree,
            None => {
                return Err(FatalParseError::TreeSitterError(
                    "Failed to parse source code".to_string(),
                ));
            }
        }
    };

    let has_syntax_errors = tree.root_node().has_error();
    if has_syntax_errors {
        detect_syntactic_errors(src, &tree, errors);
    }

    // don't detect semantic errors if there are syntactic errors, but still parse for source map.
    let mut suppressed_semantic_errors = Vec::new();
    let semantic_errors: &mut Vec<RecoverableParseError> = if has_syntax_errors {
        &mut suppressed_semantic_errors
    } else {
        errors
    };

    let mut model = Model::new(context);
    let mut source_map = SourceMap::default();
    let mut declaration_spans = BTreeMap::new();
    let root_node = tree.root_node();

    // Create a ParseContext
    let mut ctx = ParseContext::new(
        &source_code,
        &root_node,
        Some(model.symbols_ptr_unchecked().clone()),
        semantic_errors,
        &mut source_map,
        &mut declaration_spans,
    );

    let mut cursor = root_node.walk();
    for statement in root_node.children(&mut cursor) {
        if !statement.is_named() || statement.is_error() || statement.kind() == "ERROR" {
            continue;
        }

        ctx.typechecking_context = TypecheckingContext::Unknown;
        ctx.inner_typechecking_context = TypecheckingContext::Unknown;

        match statement.kind() {
            "single_line_comment" => {}
            "language_declaration" => {}
            "find_statement" => {
                let parsed = parse_find_statement(&mut ctx, statement)?;
                for (name, domain) in parsed.declarations {
                    let decl = if parsed.auxiliary {
                        DeclarationPtr::new_find_auxiliary(name, domain)
                    } else {
                        DeclarationPtr::new_find(name, domain)
                    };
                    model.symbols_mut().insert(decl);
                }
            }
            "given_statement" => {
                let var_hashmap = parse_given_statement(&mut ctx, statement)?;
                for (name, domain) in var_hashmap {
                    model
                        .symbols_mut()
                        .insert(DeclarationPtr::new_given(name, domain));
                }
            }
            "bool_expr" | "atom" | "comparison_expr" => {
                ctx.typechecking_context = TypecheckingContext::Boolean;
                let Some(expr) = parse_expression(&mut ctx, statement)? else {
                    continue;
                };
                model.add_constraint(expr);
            }
            "where_statement" => {
                ctx.typechecking_context = TypecheckingContext::Boolean;
                let mut cursor = statement.walk();
                for condition in statement.named_children(&mut cursor) {
                    let Some(expr) = parse_expression(&mut ctx, condition)? else {
                        continue;
                    };
                    model.add_instantiation_condition(expr);
                }
            }
            "language_label" => {}
            "letting_statement" => {
                let Some(letting_vars) = parse_letting_statement(&mut ctx, statement)? else {
                    continue;
                };
                model.symbols_mut().extend(letting_vars);
            }
            "dominance_relation" => {
                let Some(dominance) = parse_dominance_relation(&mut ctx, &statement)? else {
                    continue;
                };
                if model.dominance.is_some() {
                    ctx.record_error(RecoverableParseError::new(
                        "Duplicate dominance relation".to_string(),
                        None,
                    ));
                    continue;
                }
                model.dominance = Some(dominance);
            }
            "objective_statement" => {
                let Some(objective) = parse_objective_statement(&mut ctx, &statement)? else {
                    continue;
                };
                if model.objective.is_some() {
                    ctx.record_error(RecoverableParseError::new(
                        "Duplicate objective statement".to_string(),
                        None,
                    ));
                    continue;
                }
                model.objective = Some(objective);
            }
            _ => {
                ctx.record_error(RecoverableParseError::new(
                    format!("Unexpected top-level statement: {}", statement.kind()),
                    Some(statement.range()),
                ));
                continue;
            }
        }
    }

    // Check if there were any recoverable errors
    if !errors.is_empty() {
        return Ok((None, source_map));
    }
    // otherwise return the model
    Ok((Some(model), source_map))
}

pub fn parse_essence(src: &str) -> Result<(Model, SourceMap), Box<ParseErrorCollection>> {
    let context = Arc::new(RwLock::new(Context::default()));
    let mut errors = vec![];
    match parse_essence_with_context_and_map(src, context, &mut errors, None) {
        Ok((Some(model), source_map)) => {
            debug_assert_model_well_formed(&model, "tree-sitter");
            Ok((model, source_map))
        }
        Ok((None, _source_map)) => {
            // Recoverable errors were found, return them as a ParseErrorCollection
            Err(Box::new(ParseErrorCollection::multiple(
                errors,
                Some(src.to_string()),
                None,
            )))
        }
        Err(fatal) => Err(Box::new(ParseErrorCollection::fatal(fatal))),
    }
}

mod test {
    #[allow(unused_imports)]
    use crate::parse_essence;
    #[allow(unused_imports)]
    use conjure_cp_core::ast::{
        Atom, DeclarationKind, Expression, Metadata, Moo, Name, OXIDE_INT_MAX, OXIDE_INT_MIN,
        ReturnType, Typeable,
    };
    #[allow(unused_imports)]
    use conjure_cp_core::{domain_int, matrix_expr, range};
    #[allow(unused_imports)]
    use std::ops::Deref;

    #[test]
    pub fn test_parse_xyz() {
        let src = "
        find x, y, z : int(1..4)
        such that x + y + z = 4
        such that x >= y
        ";

        let (model, _source_map) = parse_essence(src).unwrap();

        let st = model.symbols();
        let x = st.lookup(&Name::user("x")).unwrap();
        let y = st.lookup(&Name::user("y")).unwrap();
        let z = st.lookup(&Name::user("z")).unwrap();
        assert_eq!(x.domain(), Some(domain_int!(1..4)));
        assert_eq!(y.domain(), Some(domain_int!(1..4)));
        assert_eq!(z.domain(), Some(domain_int!(1..4)));

        let constraints = model.constraints();
        assert_eq!(constraints.len(), 2);

        let c1 = constraints[0].clone();
        let x_e = Expression::Atomic(Metadata::new(), Atom::new_ref(x));
        let y_e = Expression::Atomic(Metadata::new(), Atom::new_ref(y));
        let z_e = Expression::Atomic(Metadata::new(), Atom::new_ref(z));
        assert_eq!(
            c1,
            Expression::Eq(
                Metadata::new(),
                Moo::new(Expression::Sum(
                    Metadata::new(),
                    Moo::new(matrix_expr!(
                        Expression::Sum(
                            Metadata::new(),
                            Moo::new(matrix_expr!(x_e.clone(), y_e.clone()))
                        ),
                        z_e
                    ))
                )),
                Moo::new(Expression::Atomic(Metadata::new(), 4.into()))
            )
        );

        let c2 = constraints[1].clone();
        assert_eq!(
            c2,
            Expression::Geq(Metadata::new(), Moo::new(x_e), Moo::new(y_e))
        );
    }

    #[test]
    pub fn test_parse_bare_int_domain_is_full() {
        let src = "given a : int";
        let (model, _source_map) = parse_essence(src).unwrap();

        let st = model.symbols();
        let a = st.lookup(&Name::user("a")).unwrap();
        assert_eq!(a.domain(), Some(domain_int!(OXIDE_INT_MIN..OXIDE_INT_MAX)));
    }

    #[test]
    pub fn test_parse_empty_int_domain() {
        let src = "find x : int()";
        let (model, _source_map) = parse_essence(src).unwrap();

        let st = model.symbols();
        let x = st.lookup(&Name::user("x")).unwrap();
        assert_eq!(x.domain(), Some(domain_int!()));
    }

    #[test]
    pub fn test_pretty_int_domain_reference_bound_without_extra_parentheses() {
        let src = "
        given n : int
        find x : int(1..n)
        ";

        let (model, _source_map) = parse_essence(src).unwrap();

        assert!(model.to_string().contains("find x: int(1..n)\n"));
    }

    #[test]
    pub fn test_parse_letting_index() {
        let src = "
        letting a be [ [ 1,2,3 ; int(1,2,4) ], [ 1,3,2 ; int(1,2,4) ], [ 3,2,1 ; int(1,2,4) ] ; int(-2..0) ]
        find b: int(1..5)
        such that
        b < a[-2,2],
        allDiff(a[-2,..])
        ";

        let (model, _source_map) = parse_essence(src).unwrap();
        let st = model.symbols();
        let a_decl = st.lookup(&Name::user("a")).unwrap();
        let a = a_decl.as_value_letting().unwrap().deref().clone();
        assert_eq!(
            a,
            matrix_expr!(
                matrix_expr!(1.into(), 2.into(), 3.into() ; domain_int!(1, 2, 4)),
                matrix_expr!(1.into(), 3.into(), 2.into() ; domain_int!(1, 2, 4)),
                matrix_expr!(3.into(), 2.into(), 1.into() ; domain_int!(1, 2, 4));
                domain_int!(-2..0)
            )
        )
    }

    #[test]
    pub fn test_parse_chained_and_multi_index() {
        let src = "
        find x : (bool, (bool, int(1..4)))
        such that
            x[2][1] = true,
            x[2,1] = true
        ";

        let (model, _source_map) = parse_essence(src).unwrap();
        let constraints = model.constraints();
        assert_eq!(constraints.len(), 2);
        for constraint in constraints {
            let Expression::Eq(_, lhs, _) = constraint else {
                panic!("expected an equality constraint");
            };
            assert_eq!(lhs.return_type(), ReturnType::Bool);
        }
    }

    #[test]
    pub fn test_multi_dimensional_matrix_index_return_type() {
        let src = "
        find a : matrix indexed by [int(1..2), int(1..2)] of int(1..4)
        such that a[1,1] = 1
        ";

        let (model, _source_map) = parse_essence(src).unwrap();
        let constraints = model.constraints();
        let Expression::Eq(_, lhs, _) = &constraints[0] else {
            panic!("expected an equality constraint");
        };
        assert_eq!(lhs.return_type(), ReturnType::Int);
    }

    #[test]
    pub fn value_letting_retains_symbolic_integer_domain() {
        let src = "
        given v: int(1..)
        given b: int(1..)
        given r: int(1..)
        letting rv be r * v
        letting ceilrv be rv / b + toInt(rv % b != 0)
        ";

        let (model, _source_map) = parse_essence(src).unwrap();
        let symbols = model.symbols();

        for name in ["rv", "ceilrv"] {
            let declaration = symbols.lookup(&Name::user(name)).unwrap();
            assert!(
                declaration.domain().is_some(),
                "{name} should have a domain"
            );
            assert!(matches!(
                declaration.kind().deref(),
                DeclarationKind::ValueLetting(_, Some(_))
            ));
        }
        drop(symbols);

        let (params, _source_map) = parse_essence(
            "
            letting v be 8
            letting b be 28
            letting r be 14
            ",
        )
        .unwrap();
        let model = conjure_cp_core::instantiate::instantiate_model(model, params).unwrap();
        let symbols = model.symbols();
        let rv = symbols.lookup(&Name::user("rv")).unwrap();
        assert_eq!(
            rv.domain().unwrap().resolve().unwrap().as_ref(),
            domain_int!(112).resolve().unwrap().as_ref()
        );
    }

    #[test]
    pub fn test_parse_table_in_quantifier() {
        let src = "
        find x, y, z : int(1..3)
        such that forAll i : int(1..1) . table([x,y,z], [[1,2,3]])
        ";

        let (model, _source_map) = parse_essence(src).unwrap();
        let constraints = model.constraints();
        assert_eq!(constraints.len(), 1);

        let Expression::And(_, comprehension_expr) = &constraints[0] else {
            panic!("expected forAll to parse as an And over a comprehension");
        };
        let Expression::Comprehension(_, comprehension) = comprehension_expr.as_ref() else {
            panic!("expected forAll body to be a comprehension");
        };

        assert!(matches!(
            comprehension.return_expression,
            Expression::Table(_, _, _)
        ));
    }

    #[test]
    pub fn test_parse_objective_statement() {
        let src = "
        find cost : int(0..10)
        minimising cost
        such that cost = 5
        ";

        let (model, _source_map) = parse_essence(src).unwrap();
        assert!(matches!(
            model.objective.as_ref().unwrap().direction,
            conjure_cp_core::ast::OptimiseDirection::Minimising
        ));

        let st = model.symbols();
        let objective = model.objective.as_ref().unwrap();
        let cost = st.lookup(&Name::user("cost")).unwrap();
        assert_eq!(
            objective.expression,
            Expression::Atomic(Metadata::new(), Atom::new_ref(cost))
        );
    }

    #[test]
    pub fn test_parse_pareto_in_dominance_relation() {
        let src = "
        find x : int(0..3)

        dominance relation
            pareto(minimising x)
        ";

        let (model, _source_map) = parse_essence(src).unwrap();
        let st = model.symbols();
        let x = st.lookup(&Name::user("x")).unwrap();
        let x_e = Expression::Atomic(Metadata::new(), Atom::new_ref(x.clone()));
        let x_prev = Expression::FromSolution(Metadata::new(), Moo::new(Atom::new_ref(x)));

        assert_eq!(
            model.dominance,
            Some(Expression::DominanceRelation(
                Metadata::new(),
                Moo::new(Expression::And(
                    Metadata::new(),
                    Moo::new(matrix_expr!(
                        Expression::Leq(
                            Metadata::new(),
                            Moo::new(x_e.clone()),
                            Moo::new(x_prev.clone())
                        ),
                        Expression::Lt(Metadata::new(), Moo::new(x_e), Moo::new(x_prev))
                    ))
                ))
            ))
        );
    }

    #[test]
    pub fn test_parse_pareto_with_mixed_directions() {
        let src = "
        find x : int(0..3)
        find y : int(0..3)

        dominance relation
            pareto(minimising x, maximising y)
        ";

        let (model, _source_map) = parse_essence(src).unwrap();
        let st = model.symbols();
        let x = st.lookup(&Name::user("x")).unwrap();
        let y = st.lookup(&Name::user("y")).unwrap();
        let x_e = Expression::Atomic(Metadata::new(), Atom::new_ref(x.clone()));
        let y_e = Expression::Atomic(Metadata::new(), Atom::new_ref(y.clone()));
        let x_prev = Expression::FromSolution(Metadata::new(), Moo::new(Atom::new_ref(x)));
        let y_prev = Expression::FromSolution(Metadata::new(), Moo::new(Atom::new_ref(y)));

        assert_eq!(
            model.dominance,
            Some(Expression::DominanceRelation(
                Metadata::new(),
                Moo::new(Expression::And(
                    Metadata::new(),
                    Moo::new(matrix_expr!(
                        Expression::Leq(
                            Metadata::new(),
                            Moo::new(x_e.clone()),
                            Moo::new(x_prev.clone())
                        ),
                        Expression::Geq(
                            Metadata::new(),
                            Moo::new(y_e.clone()),
                            Moo::new(y_prev.clone())
                        ),
                        Expression::Or(
                            Metadata::new(),
                            Moo::new(matrix_expr!(
                                Expression::Lt(Metadata::new(), Moo::new(x_e), Moo::new(x_prev)),
                                Expression::Gt(Metadata::new(), Moo::new(y_e), Moo::new(y_prev))
                            ))
                        )
                    ))
                ))
            ))
        );
    }

    #[test]
    pub fn test_parse_pareto_over_expression_component() {
        let src = "
        find x : int(0..3)

        dominance relation
            pareto(minimising x + 1)
        ";

        let (model, _source_map) = parse_essence(src).unwrap();
        let st = model.symbols();
        let x = st.lookup(&Name::user("x")).unwrap();
        let x_e = Expression::Atomic(Metadata::new(), Atom::new_ref(x.clone()));
        let x_prev = Expression::FromSolution(Metadata::new(), Moo::new(Atom::new_ref(x)));
        let one = Expression::Atomic(Metadata::new(), 1.into());
        let current = Expression::Sum(
            Metadata::new(),
            Moo::new(matrix_expr!(x_e.clone(), one.clone())),
        );
        let previous = Expression::Sum(Metadata::new(), Moo::new(matrix_expr!(x_prev, one)));

        assert_eq!(
            model.dominance,
            Some(Expression::DominanceRelation(
                Metadata::new(),
                Moo::new(Expression::And(
                    Metadata::new(),
                    Moo::new(matrix_expr!(
                        Expression::Leq(
                            Metadata::new(),
                            Moo::new(current.clone()),
                            Moo::new(previous.clone())
                        ),
                        Expression::Lt(Metadata::new(), Moo::new(current), Moo::new(previous))
                    ))
                ))
            ))
        );
    }

    #[test]
    pub fn test_parse_permutation_domain_literal_and_operators() {
        let src = "
        find p : permutation (numMoved 3) of int(1..5)
        letting q be permutation((1,2,3),(4,5))
        find x, y : int(1..5)
        such that y = image(p, x)
        such that inverse(p, q)
        such that q = permInverse(p)
        such that y = image(compose(p, q), x)
        ";

        let (model, _source_map) = parse_essence(src).unwrap();

        let st = model.symbols();
        let p = st.lookup(&Name::user("p")).unwrap();
        let ground = p.domain().unwrap().resolve().unwrap();
        let conjure_cp_core::ast::GroundDomain::Permutation(attrs, inner) = ground.as_ref() else {
            panic!("expected a permutation domain, got {ground}");
        };
        assert_eq!(attrs.num_moved, range!(3));
        assert_eq!(**inner, *domain_int!(1..5).resolve().unwrap());

        let constraints = model.constraints();
        assert_eq!(constraints.len(), 4);
        assert!(matches!(constraints[0], Expression::Eq(_, _, _)));
        assert!(matches!(constraints[1], Expression::Inverse(_, _, _)));
        assert!(matches!(constraints[2], Expression::Eq(_, _, _)));
        let Expression::Eq(_, _, rhs) = &constraints[2] else {
            unreachable!()
        };
        assert!(matches!(rhs.as_ref(), Expression::PermInverse(_, _)));
        assert!(matches!(constraints[3], Expression::Eq(_, _, _)));
        let Expression::Eq(_, _, rhs) = &constraints[3] else {
            unreachable!()
        };
        let Expression::Image(_, compose_expr, _) = rhs.as_ref() else {
            unreachable!()
        };
        assert!(matches!(compose_expr.as_ref(), Expression::Compose(_, _, _)));
    }

    #[test]
    pub fn test_parse_permutation_unattributed_domain_and_empty_literal() {
        // `letting q be permutation()`'s own domain would need type inference from its (empty)
        // literal, which -- like the equivalent empty-partition-literal case -- is not yet
        // implemented (`GroundDomain::from_literal_vec`'s `AbstractLiteral::Permutation` arm is a
        // deliberate `todo!()`, mirroring Partition's own pre-existing gap); so this test only
        // inspects the parsed literal's AST shape directly, without triggering that inference.
        let src = "
        find p : permutation of int(1..3)
        letting q be permutation()
        find x : int(1..3)
        such that x = image(p, 1)
        ";

        let (model, _source_map) = parse_essence(src).unwrap();

        let st = model.symbols();
        let p = st.lookup(&Name::user("p")).unwrap();
        let ground = p.domain().unwrap().resolve().unwrap();
        let conjure_cp_core::ast::GroundDomain::Permutation(attrs, _) = ground.as_ref() else {
            panic!("expected a permutation domain, got {ground}");
        };
        assert_eq!(attrs.num_moved, conjure_cp_core::ast::Range::Unbounded);

        let q_decl = st.lookup(&Name::user("q")).unwrap();
        let q = q_decl.as_value_letting().unwrap().deref().clone();
        assert_eq!(
            q,
            Expression::AbstractLiteral(
                Metadata::new(),
                conjure_cp_core::ast::AbstractLiteral::Permutation(vec![])
            )
        );
    }
}
