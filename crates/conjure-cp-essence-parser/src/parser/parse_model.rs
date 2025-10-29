use std::fs;
use std::sync::{Arc, RwLock};

use conjure_cp_core::Model;
use conjure_cp_core::ast::{DeclarationPtr, Expression, Metadata, Moo};
use conjure_cp_core::context::Context;
#[allow(unused)]
use uniplate::Uniplate;

use super::find::parse_find_statement;
use super::letting::parse_letting_statement;
use super::util::{get_tree, named_children};
use crate::errors::EssenceParseError;
use crate::expression::parse_expression;

/// Parse an Essence file into a Model using the tree-sitter parser.
pub fn parse_essence_file_native(
    path: &str,
    context: Arc<RwLock<Context<'static>>>,
) -> Result<Model, EssenceParseError> {
    let source_code = fs::read_to_string(path)
        .unwrap_or_else(|_| panic!("Failed to read the source code file {path}"));
    parse_essence_with_context(&source_code, context)
}

pub fn parse_essence_with_context(
    src: &str,
    context: Arc<RwLock<Context<'static>>>,
) -> Result<Model, EssenceParseError> {
    let (tree, source_code) = match get_tree(src) {
        Some(tree) => tree,
        None => {
            return Err(EssenceParseError::TreeSitterError(
                "Failed to parse source code".to_string(),
            ));
        }
    };

    let mut model = Model::new(context);
    // let symbols = model.as_submodel().symbols().clone();
    let root_node = tree.root_node();
    let symbols_ptr = model.as_submodel().symbols_ptr_unchecked().clone();
    for statement in named_children(&root_node) {
        match statement.kind() {
            "single_line_comment" => {}
            "language_declaration" => {}
            "find_statement" => {
                let var_hashmap = parse_find_statement(statement, &source_code)?;
                for (name, domain) in var_hashmap {
                    model
                        .as_submodel_mut()
                        .symbols_mut()
                        .insert(DeclarationPtr::new_var(name, domain));
                }
            }
            "bool_expr" | "atom" | "comparison_expr" => {
                model.as_submodel_mut().add_constraint(parse_expression(
                    statement,
                    &source_code,
                    &statement,
                    Some(&symbols_ptr),
                )?);
            }
            "language_label" => {}
            "letting_statement" => {
                let letting_vars =
                    parse_letting_statement(statement, &source_code, Some(&symbols_ptr))?;
                model.as_submodel_mut().symbols_mut().extend(letting_vars);
            }
            "dominance_relation" => {
                let inner = statement
                    .child_by_field_name("expression")
                    .expect("Expected a sub-expression inside `dominanceRelation`");
                let expr = parse_expression(inner, &source_code, &statement, Some(&symbols_ptr))?;
                let dominance = Expression::DominanceRelation(Metadata::new(), Moo::new(expr));
                if model.dominance.is_some() {
                    return Err(EssenceParseError::syntax_error(
                        "Duplicate dominance relation".to_string(),
                        None,
                    ));
                }
                model.dominance = Some(dominance);
            }
            "ERROR" => {
                let raw_expr = &source_code[statement.start_byte()..statement.end_byte()];
                return Err(EssenceParseError::syntax_error(
                    format!("'{raw_expr}' is not a valid expression"),
                    Some(statement.range()),
                ));
            }
            _ => {
                let kind = statement.kind();
                return Err(EssenceParseError::syntax_error(
                    format!("Unrecognized top level statement kind: {kind}"),
                    Some(statement.range()),
                ));
            }
        }
    }
    Ok(model)
}

pub fn parse_essence(src: &str) -> Result<Model, EssenceParseError> {
    let context = Arc::new(RwLock::new(Context::default()));
    parse_essence_with_context(src, context)
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
