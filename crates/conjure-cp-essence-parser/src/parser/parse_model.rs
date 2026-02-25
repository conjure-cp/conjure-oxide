use std::sync::{Arc, RwLock};
use std::{fs, vec};

use conjure_cp_core::Model;
use conjure_cp_core::ast::{DeclarationPtr, Expression, Metadata, Moo};
use conjure_cp_core::context::Context;
#[allow(unused)]
use uniplate::Uniplate;

use super::find::parse_find_statement;
use super::letting::parse_letting_statement;
use super::util::{get_tree, named_children};
use crate::errors::{FatalParseError, ParseErrorCollection, RecoverableParseError};
use crate::expression::parse_expression;
use crate::field;

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
        Ok(m) => {
            // Check if there were any recoverable errors
            if !errors.is_empty() {
                return Err(Box::new(ParseErrorCollection::multiple(
                    errors,
                    Some(source_code),
                    Some(path.to_string()),
                )));
            }
            // Return model if no errors
            Ok(m)
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
) -> Result<Model, FatalParseError> {
    let (tree, source_code) = match get_tree(src) {
        Some(tree) => tree,
        None => {
            return Err(FatalParseError::TreeSitterError(
                "Failed to parse source code".to_string(),
            ));
        }
    };

    if tree.root_node().has_error() {
        // For now, return 'not implemented' for syntactic errors
        // TODO: connect to syntactic error parsing here for recoverable errors
        return Err(FatalParseError::NotImplemented(
            "Erroneous tree-sitter CST: Something in this input is not yet supported or there is a syntactic error. Syntactic error detection and reporting".to_string(),
        ));
    }

    let mut model = Model::new(context);
    let root_node = tree.root_node();
    let symbols_ptr = model.as_submodel().symbols_ptr_unchecked().clone();
    for statement in named_children(&root_node) {
        match statement.kind() {
            "single_line_comment" => {}
            "language_declaration" => {}
            "find_statement" => {
                let var_hashmap = parse_find_statement(
                    statement,
                    &source_code,
                    Some(symbols_ptr.clone()),
                    errors,
                )?;
                for (name, domain) in var_hashmap {
                    model
                        .as_submodel_mut()
                        .symbols_mut()
                        .insert(DeclarationPtr::new_find(name, domain));
                }
            }
            "bool_expr" | "atom" | "comparison_expr" => {
                let Some(expr) = parse_expression(
                    statement,
                    &source_code,
                    &statement,
                    Some(symbols_ptr.clone()),
                    errors,
                )?
                else {
                    continue;
                };
                model.as_submodel_mut().add_constraint(expr);
            }
            "language_label" => {}
            "letting_statement" => {
                let Some(letting_vars) = parse_letting_statement(
                    statement,
                    &source_code,
                    Some(symbols_ptr.clone()),
                    errors,
                )?
                else {
                    continue;
                };
                model.as_submodel_mut().symbols_mut().extend(letting_vars);
            }
            "dominance_relation" => {
                let inner = field!(statement, "expression");
                let Some(expr) = parse_expression(
                    inner,
                    &source_code,
                    &statement,
                    Some(symbols_ptr.clone()),
                    errors,
                )?
                else {
                    continue;
                };
                let dominance = Expression::DominanceRelation(Metadata::new(), Moo::new(expr));
                if model.dominance.is_some() {
                    errors.push(RecoverableParseError::new(
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
    
    // check for errors (keyword as identifier)
    keyword_as_identifier(root_node, &source_code, errors);
    
    Ok(model)
}

const KEYWORDS: [&str; 21] = [
    "forall", "exists", "such", "that", "letting", "find", "minimise", "maximise", "subject", "to",
    "where", "and", "or", "not", "if", "then", "else", "in", "sum", "product", "bool",
];

fn keyword_as_identifier(
    root: tree_sitter::Node,
    src: &str,
    errors: &mut Vec<RecoverableParseError>,
) {
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if (node.kind() == "variable" || node.kind() == "identifier" || node.kind() == "parameter")
            && let Ok(text) = node.utf8_text(src.as_bytes())
        {
            let ident = text.trim();
            if KEYWORDS.contains(&ident) {
                let start_point = node.start_position();
                let end_point = node.end_position();
                errors.push(RecoverableParseError::new(
                    format!("Keyword '{ident}' used as identifier"),
                    Some(tree_sitter::Range {
                        start_byte: node.start_byte(),
                        end_byte: node.end_byte(),
                        start_point,
                        end_point,
                    }),
                ));
            }
        }

        // push children onto stack
        for i in 0..node.child_count() {
            if let Some(child) = u32::try_from(i).ok().and_then(|i| node.child(i)) {
                stack.push(child);
            }
        }
    }
}

pub fn parse_essence(src: &str) -> Result<Model, Box<ParseErrorCollection>> {
    let context = Arc::new(RwLock::new(Context::default()));
    let mut errors = vec![];
    match parse_essence_with_context(src, context, &mut errors) {
        Ok(model) => {
            if !errors.is_empty() {
                Err(Box::new(ParseErrorCollection::multiple(
                    errors,
                    Some(src.to_string()),
                    None,
                )))
            } else {
                Ok(model)
            }
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

        let model = parse_essence(src).unwrap();

        let st = model.as_submodel().symbols();
        let x = st.lookup(&Name::user("x")).unwrap();
        let y = st.lookup(&Name::user("y")).unwrap();
        let z = st.lookup(&Name::user("z")).unwrap();
        assert_eq!(x.domain(), Some(domain_int!(1..4)));
        assert_eq!(y.domain(), Some(domain_int!(1..4)));
        assert_eq!(z.domain(), Some(domain_int!(1..4)));

        let constraints = model.as_submodel().constraints();
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

        let model = parse_essence(src).unwrap();
        let st = model.as_submodel().symbols();
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
