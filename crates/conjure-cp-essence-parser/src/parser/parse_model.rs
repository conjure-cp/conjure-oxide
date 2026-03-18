use std::sync::{Arc, RwLock};
use std::{fs, vec};

use conjure_cp_core::Model;
use conjure_cp_core::ast::assertions::debug_assert_model_well_formed;
use conjure_cp_core::ast::{DeclarationPtr, Expression, Metadata, Moo};
use conjure_cp_core::context::Context;
#[allow(unused)]
use uniplate::Uniplate;

use super::ParseContext;
use super::find::parse_find_statement;
use super::keyword_checks::keyword_as_identifier;
use super::letting::parse_letting_statement;
use super::util::{TypecheckingContext, get_tree};
use crate::diagnostics::diagnostics_api::SymbolKind;
use crate::diagnostics::source_map::{HoverInfo, SourceMap, span_with_hover};
use crate::errors::{FatalParseError, ParseErrorCollection, RecoverableParseError};
use crate::expression::parse_expression;
use crate::field;
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
        Some((model, _source_map)) => Ok(Some(model)),
        None => Ok(None),
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
) -> Result<Option<(Model, SourceMap)>, FatalParseError> {
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

    if tree.root_node().has_error() {
        detect_syntactic_errors(src, &tree, errors);
        return Ok(None);
    }

    let mut model = Model::new(context);
    let mut source_map = SourceMap::default();
    let root_node = tree.root_node();

    // Create a ParseContext
    let mut ctx = ParseContext::new(
        &source_code,
        &root_node,
        Some(model.symbols_ptr_unchecked().clone()),
        errors,
        &mut source_map,
    );

    let mut cursor = root_node.walk();
    for statement in root_node.children(&mut cursor) {
        /*
           since find and letting are unnamed children
           hover info is added here.
           other unnamed children will be skipped.
        */
        if statement.kind() == "find" {
            span_with_hover(
                &statement,
                ctx.source_code,
                ctx.source_map,
                HoverInfo {
                    description: "Find keyword".to_string(),
                    kind: Some(SymbolKind::Find),
                    ty: None,
                    decl_span: None,
                },
            );
        } else if statement.kind() == "letting" {
            span_with_hover(
                &statement,
                ctx.source_code,
                ctx.source_map,
                HoverInfo {
                    description: "Letting keyword".to_string(),
                    kind: Some(SymbolKind::Letting),
                    ty: None,
                    decl_span: None,
                },
            );
        }

        if !statement.is_named() {
            continue;
        }

        match statement.kind() {
            "single_line_comment" => {}
            "language_declaration" => {}
            "find_statement" => {
                let var_hashmap = parse_find_statement(&mut ctx, statement)?;
                for (name, domain) in var_hashmap {
                    model
                        .symbols_mut()
                        .insert(DeclarationPtr::new_find(name, domain));
                }
            }
            "bool_expr" | "atom" | "comparison_expr" => {
                ctx.typechecking_context = TypecheckingContext::Boolean;
                let Some(expr) = parse_expression(&mut ctx, statement)? else {
                    continue;
                };
                model.add_constraint(expr);
            }
            "language_label" => {}
            "letting_statement" => {
                let Some(letting_vars) = parse_letting_statement(&mut ctx, statement)? else {
                    continue;
                };
                model.symbols_mut().extend(letting_vars);
            }
            "dominance_relation" => {
                let inner = field!(statement, "expression");
                let Some(expr) = parse_expression(&mut ctx, inner)? else {
                    continue;
                };
                let dominance = Expression::DominanceRelation(Metadata::new(), Moo::new(expr));
                if model.dominance.is_some() {
                    ctx.record_error(RecoverableParseError::new(
                        "Duplicate dominance relation".to_string(),
                        None,
                    ));
                    continue;
                }
                model.dominance = Some(dominance);
            }
            _ => {
                return Err(FatalParseError::internal_error(
                    format!("Unexpected top-level statement: {}", statement.kind()),
                    Some(statement.range()),
                ));
            }
        }
    }

    // Check if there were any recoverable errors
    if !errors.is_empty() {
        return Ok(None);
    }
    // otherwise return the model
    Ok(Some((model, source_map)))
}

pub fn parse_essence(src: &str) -> Result<(Model, SourceMap), Box<ParseErrorCollection>> {
    let context = Arc::new(RwLock::new(Context::default()));
    let mut errors = vec![];
    match parse_essence_with_context_and_map(src, context, &mut errors, None) {
        Ok(Some((model, source_map))) => {
            debug_assert_model_well_formed(&model, "tree-sitter");
            Ok((model, source_map))
        }
        Ok(None) => {
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
    use conjure_cp_core::ast::{Atom, Expression, Metadata, Moo, Name};
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
}
