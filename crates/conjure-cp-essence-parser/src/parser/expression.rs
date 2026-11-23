use std::cell::LazyCell;
use std::collections::HashMap;

use crate::errors::{FatalParseError, RecoverableParseError};
use crate::parser::atom::parse_constant;
use crate::parser::domain::parse_domain;
use crate::parser::{ParseContext, parse_ident, parse_name};

use crate::field;
use conjure_cp_core::ast::ac_operators::ACOperatorKind;
use conjure_cp_core::ast::comprehension::Comprehension;
use conjure_cp_core::ast::{
    AbstractLiteral, Atom, Domain, Expression, Field, IntVal, Metadata, Moo, Name, Range,
};
use conjure_cp_core::{domain_int, matrix_expr, range};
use tree_sitter::Node;

fn register(
    builtin: &mut HashMap<&'static str, (u8, Builtin)>,
    name: &'static str,
    n_args: u8,
    func: impl Into<Builtin>,
) {
    builtin.insert(name, (n_args, func.into()));
}

enum Builtin {
    Unary(fn(Expression) -> Expression),
    Binary(fn(Expression, Expression) -> Expression),
    Ternary(fn(Expression, Expression, Expression) -> Expression),
    Quaternary(fn(Expression, Expression, Expression, Expression) -> Expression),
    OptBinary(fn(Option<Expression>, Expression) -> Expression),
}

#[rustfmt::skip]
const BUILTIN_TABLE: LazyCell<HashMap<&'static str, Builtin>> = LazyCell::new(|| {
    fn moo(a: Expression) -> Moo<Expression> { Moo::new(a) }

    use Builtin as B;
    use Expression as E;
    use Metadata as M;

    HashMap::from([
        ("sum",                B::Unary(|a| E::Sum          (M::new(), moo(a)))),
        ("product",            B::Unary(|a| E::Product      (M::new(), moo(a)))),
        ("min",                B::Unary(|a| E::Min          (M::new(), moo(a)))),
        ("max",                B::Unary(|a| E::Max          (M::new(), moo(a)))),
        ("and",                B::Unary(|a| E::And          (M::new(), moo(a)))),
        ("or",                 B::Unary(|a| E::Or           (M::new(), moo(a)))),
        ("allDifferent",       B::Unary(|a| E::AllDiff      (M::new(), moo(a)))),
        ("factorial",          B::Unary(|a| E::Factorial    (M::new(), moo(a)))),
        ("parts",              B::Unary(|a| E::Parts        (M::new(), moo(a)))),
        ("participants",       B::Unary(|a| E::Participants (M::new(), moo(a)))),
        ("defined",            B::Unary(|a| E::Defined      (M::new(), moo(a)))),
        ("range",              B::Unary(|a| E::Range        (M::new(), moo(a)))),
        ("permInverse",        B::Unary(|a| E::PermInverse  (M::new(), moo(a)))),
        ("toInt",              B::Unary(|a| E::ToInt        (M::new(), moo(a)))),
        ("toSet",              B::Unary(|a| E::ToSet        (M::new(), moo(a)))),
        ("toMSet",             B::Unary(|a| E::ToMSet       (M::new(), moo(a)))),
        ("toRelation",         B::Unary(|a| E::ToRelation   (M::new(), moo(a)))),

        ("allDifferentExcept", B::Binary(|a, b| E::AllDifferentExcept (M::new(), moo(a), moo(b)))),
        ("catchUndef",         B::Binary(|a, b| E::CatchUndef         (M::new(), moo(a), moo(b)))),
        ("together",           B::Binary(|a, b| E::Together           (M::new(), moo(a), moo(b)))),
        ("apart",              B::Binary(|a, b| E::Apart              (M::new(), moo(a), moo(b)))),
        ("party",              B::Binary(|a, b| E::Party              (M::new(), moo(a), moo(b)))),
        ("image",              B::Binary(|a, b| E::Image              (M::new(), moo(a), moo(b)))),
        ("imageSet",           B::Binary(|a, b| E::ImageSet           (M::new(), moo(a), moo(b)))),
        ("preImage",           B::Binary(|a, b| E::PreImage           (M::new(), moo(a), moo(b)))),
        ("inverse",            B::Binary(|a, b| E::Inverse            (M::new(), moo(a), moo(b)))),
        ("restrict",           B::Binary(|a, b| E::Restrict           (M::new(), moo(a), moo(b)))),
        ("compose",            B::Binary(|a, b| E::Compose            (M::new(), moo(a), moo(b)))),
        ("table",              B::Binary(|a, b| E::Table              (M::new(), moo(a), moo(b)))),
        ("negativeTable",      B::Binary(|a, b| E::NegativeTable      (M::new(), moo(a), moo(b)))),

        ("flatten",            B::OptBinary(|a, b| E::Flatten (M::new(), a.map(moo), moo(b)))),

        ("gcc",                B::Ternary(|a, b, c| E::Gcc     (M::new(), moo(a), moo(b), moo(c)))),
        ("atLeast",            B::Ternary(|a, b, c| E::AtLeast (M::new(), moo(a), moo(b), moo(c)))),
        ("atMost",             B::Ternary(|a, b, c| E::AtMost  (M::new(), moo(a), moo(b), moo(c)))),

        // ("pred",       B::Unary(|a|     E::Pred(      m(), Moo::new(a)))),
        // ("succ",       B::Unary(|a|     E::Succ(      m(), Moo::new(a)))),
    ])

});

pub fn parse_expression(ctx: &mut ParseContext, node: Node) -> Result<Expression, FatalParseError> {
    let m = Metadata::new();
    let expr = match node.kind() {
        // basics
        "identifier" => {
            todo!()
            // let Some(atom) = parse_variable(ctx, &node)? else {
            //     todo!();
            // };
            // Expression::Atomic(Metadata::new(), atom)
        }
        "sub_expr" => parse_expression(ctx, field!(node, "expression"))?,

        // literals
        "constant" => {
            let lit = parse_constant(ctx, &node)?;
            Expression::Atomic(m, Atom::Literal(lit))
        }
        "tuple_literal" => {
            let elems: Vec<_> = node
                .named_children(&mut node.walk())
                .map(|n| parse_expression(ctx, n))
                .collect::<Result<_, _>>()?;

            Expression::AbstractLiteral(m, AbstractLiteral::Tuple(elems))
        }
        "matrix_literal" => {
            let elems = node
                .children_by_field_name("elements", &mut node.walk())
                .filter(Node::is_named)
                .map(|n| parse_expression(ctx, n))
                .collect::<Result<_, _>>()?;

            let index_domain = node
                .child_by_field_name("domain")
                .map(|n| parse_domain(ctx, n))
                .transpose()?
                .unwrap_or_else(|| Domain::int(Vec::<Range<IntVal>>::new()));

            Expression::AbstractLiteral(m, AbstractLiteral::Matrix(elems, index_domain))
        }
        "set_literal" => {
            let elems = node
                .named_children(&mut node.walk())
                .map(|n| parse_expression(ctx, n))
                .collect::<Result<_, _>>()?;
            Expression::AbstractLiteral(m, AbstractLiteral::Set(elems))
        }
        "mset_literal" => {
            let elems = node
                .named_children(&mut node.walk())
                .map(|n| parse_expression(ctx, n))
                .collect::<Result<_, _>>()?;
            Expression::AbstractLiteral(m, AbstractLiteral::MSet(elems))
        }
        "sequence_literal" => {
            let elems = node
                .named_children(&mut node.walk())
                .map(|n| dbg!(parse_expression(ctx, dbg!(n))))
                .collect::<Result<_, _>>()?;
            Expression::AbstractLiteral(m, AbstractLiteral::Sequence(elems))
        }
        "function_literal" => {
            let pairs = node
                .named_children(&mut node.walk())
                .map(|n| {
                    Ok::<(_, _), FatalParseError>((
                        parse_expression(ctx, field!(n, "key"))?,
                        parse_expression(ctx, field!(n, "value"))?,
                    ))
                })
                .collect::<Result<_, _>>()?;
            Expression::AbstractLiteral(m, AbstractLiteral::Function(pairs))
        }
        "record_literal" => {
            let pairs = node
                .children_by_field_name("name_value_pair", &mut node.walk())
                .map(|n| {
                    Ok::<_, FatalParseError>(Field {
                        name: parse_name(ctx, &field!(n, "name"))?,
                        value: parse_expression(ctx, field!(n, "value"))?,
                    })
                })
                .collect::<Result<_, _>>()?;
            Expression::AbstractLiteral(m, AbstractLiteral::Record(pairs))
        }
        "variant_literal" => {
            let nvp = field!(node, "name_value_pair");
            let pair = Field {
                name: parse_name(ctx, &field!(nvp, "name"))?,
                value: parse_expression(ctx, field!(nvp, "value"))?,
            };
            Expression::AbstractLiteral(m, AbstractLiteral::Variant(Moo::new(pair)))
        }
        "relation_literal" => {
            let relations = node
                .children_by_field_name("tuple", &mut node.walk())
                .map(|n| {
                    n.children_by_field_name("element", &mut n.walk())
                        .map(|c| parse_expression(ctx, c))
                        .collect::<Result<_, _>>()
                })
                .collect::<Result<_, _>>()?;
            Expression::AbstractLiteral(m, AbstractLiteral::Relation(relations))
        }
        "partition_literal" => {
            let parts = node
                .children_by_field_name("part", &mut node.walk())
                .map(|n| {
                    n.children_by_field_name("element", &mut n.walk())
                        .map(|c| parse_expression(ctx, c))
                        .collect::<Result<_, _>>()
                })
                .collect::<Result<_, _>>()?;
            Expression::AbstractLiteral(m, AbstractLiteral::Partition(parts))
        }
        "permutation_literal" => {
            let cycles = node
                .children_by_field_name("cycle", &mut node.walk())
                .map(|n| {
                    n.children_by_field_name("element", &mut n.walk())
                        .map(|c| parse_expression(ctx, c))
                        .collect::<Result<_, _>>()
                })
                .collect::<Result<_, _>>()?;
            Expression::AbstractLiteral(m, AbstractLiteral::Permutation(cycles))
        }
        // binary operations
        "binary_expr" => parse_binary_expression(ctx, &node)?,
        // unary operations
        "not_expr" => {
            let inner = parse_expression(ctx, field!(node, "expression"))?;
            Expression::Not(m, Moo::new(inner))
        }
        "negative_expr" => {
            let inner = parse_expression(ctx, field!(node, "expression"))?;
            Expression::Neg(m, Moo::new(inner))
        }
        "abs_value_expr" => {
            let inner = parse_expression(ctx, field!(node, "expression"))?;
            Expression::Abs(m, Moo::new(inner))
        }
        "factorial_expr" => {
            let inner = parse_expression(ctx, field!(node, "expression"))?;
            Expression::Factorial(m, Moo::new(inner))
        }
        // other expressions
        "apply_expr" => parse_function_application(ctx, node)?,
        "index_or_slice" => {
            let collection = parse_expression(ctx, field!(node, "collection"))?;
            let indices = node
                .children_by_field_name("indices", &mut node.walk())
                .map(|n| {
                    // TODO: null index
                    parse_expression(ctx, field!(n, "index"))
                })
                .collect::<Result<_, _>>()?;

            Expression::UnsafeIndex(m, Moo::new(collection), indices)
        }
        "from_solution" => {
            let _variable = parse_name(ctx, &field!(node, "variable"))?;
            // Some(Expression::FromSolution(m, Atom::Reference(variable))
            todo!("unresolved refs")
        }
        "quantifier_expr" => {
            let (_operator, _skip_operator): (fn(_, _) -> Expression, ACOperatorKind) =
                match parse_ident(ctx, &field!(node, "operator")).as_ref() {
                    "forAll" => (Expression::And, ACOperatorKind::And),
                    "exists" => (Expression::Or, ACOperatorKind::Or),
                    "sum" => (Expression::Sum, ACOperatorKind::Sum),
                    "min" => (Expression::Min, ACOperatorKind::Min),
                    "max" => (Expression::Max, ACOperatorKind::Max),
                    _ => unreachable!(),
                };

            let _domain_node = node.child_by_field_name("domain");
            let _collection_node = node.child_by_field_name("collection");
            let _return_expression = parse_expression(ctx, field!(node, "expression"))?;

            // let mut builder = ComprehensionBuilder::new(ctx.symbols.unwrap()); // TODO: symbol table

            let _bindings = node
                .children_by_field_name("variables", &mut node.walk())
                .map(|n| parse_pattern_bindings(ctx, &n))
                .collect::<Result<Vec<_>, _>>()?;

            // builder.expression_generator()
            // for guard_node in node.children_by_field_name("guard", &mut node.walk()) {}
            // let comprehension = builder.with_return_value(return_expression);

            // let wrapped_comprehension =
            //     Expression::Comprehension(Metadata::new(), comprehension.into());

            // operator(m, Moo::new(wrapped_comprehension))
            todo!()
        }
        "comprehension_expr" => {
            // let return_expression = parse_expression(ctx, field!(node, "expression"))?;
            // let cursor = &mut node.walk();
            // let qualifiers = node.children_by_field_name("generator", cursor).map(|n| {
            //     let variables = parse_pattern_bindings(ctx, &field!(n, "variable"))?;
            //     if let Some(domain_node) = node.child_by_field_name("domain") {
            //         let domain = parse_domain(ctx, domain_node)?;
            //         // let decl = DeclarationPtr::new_domain_letting(domain)
            //         // ComprehensionQualifier::Generator { ptr: domain }
            //         todo!()
            //     } else if let Some(collection_node) = node.child_by_field_name("collection") {
            //         todo!()
            //     } else {
            //         unreachable!();
            //     }
            //     Ok::<_, FatalParseError>(())
            // });
            //
            // Expression::Comprehension(
            //     m,
            //     Moo::new(Comprehension {
            //         return_expression,
            //         qualifiers: todo!(),
            //         skip_operator: todo!(),
            //         symbols: todo!(),
            //     }),
            // )
            todo!()
        }
        _ => {
            dbg!(node.kind());
            ctx.record_error(RecoverableParseError::new(
                format!("Unexpected expression type: '{}'", node.kind()),
                Some(node.range()),
            ));
            todo!()
        }
    };
    Ok(expr)
}

// fn bind_generator_pattern(
//     ctx: &mut ParseContext,
//     // builder: &mut ComprehensionBuilder,
//     var_node: &Node,
//     var_name: &Name,
//     lettings: &mut Vec<(DeclarationPtr, Expression)>,
// ) -> bool {
//     let mut bindings = Vec::new();
//     if !collect_pattern_bindings(ctx, var_node, &mut Vec::new(), &mut bindings) {
//         return false;
//     }
//
//     let symbols = builder.generator_symboltable();
//     let Some(generator_decl) = symbols.read().lookup_local(var_name) else {
//         ctx.record_error(RecoverableParseError::new(
//             format!("Generator pattern variable {var_name} is not in scope"),
//             Some(var_node.range()),
//         ));
//         return false;
//     };
//
//     for (name, path) in bindings {
//         let mut value = Expression::from(Reference::new(generator_decl.clone()));
//         for index in path {
//             value = Expression::SafeIndex(
//                 Metadata::new(),
//                 Moo::new(value),
//                 vec![Expression::from(Literal::Int(index))],
//             );
//         }
//         let decl = DeclarationPtr::new_value_letting(name, value.clone());
//         symbols.write().insert(decl.clone());
//         lettings.push((decl, value));
//     }
//
//     true
// }

fn parse_pattern_bindings(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Vec<(Name, Vec<i32>)>, FatalParseError> {
    let mut bindings = vec![];
    collect_pattern_bindings(ctx, node, vec![], &mut bindings)?;
    Ok(bindings)
}
/// Collects the names a generator pattern binds, each with its index path into the generator
/// variable: `(i, _)` binds `i` at path `[1]`, and `(i, (j, k))` binds `k` at path `[2, 2]`.
///
/// Patterns are written as tuples, so an element arrives wrapped in whatever expression nodes a
/// tuple element goes through; only the identifiers at the bottom matter here.
///
/// Returns false for a pattern this cannot read, having recorded an error.
fn collect_pattern_bindings(
    ctx: &mut ParseContext,
    node: &Node,
    path: Vec<i32>,
    bindings: &mut Vec<(Name, Vec<i32>)>,
) -> Result<(), FatalParseError> {
    for (i, child) in node.named_children(&mut node.walk()).enumerate() {
        match child.kind() {
            "identifier" => bindings.push((parse_name(ctx, &child)?, path.clone())),
            "placeholder" => {}
            "tuple" => {
                let mut new_path = path.clone();
                new_path.push(i as i32);
                collect_pattern_bindings(ctx, &child, new_path, bindings)?;
            }
            _ => unreachable!(),
        }
    }

    Ok(())
}

fn parse_function_application(
    ctx: &mut ParseContext,
    node: Node,
) -> Result<Expression, FatalParseError> {
    let args: Vec<Expression> = node
        .children_by_field_name("argument", &mut node.walk())
        .map(|n| parse_expression(ctx, n))
        .collect::<Result<_, _>>()?;

    let fn_node = field!(node, "function");

    if fn_node.kind() == "builtin_function" {
        let name = parse_ident(ctx, &fn_node);
        dbg!(&name);
        if let Some(builtin) = BUILTIN_TABLE.get(name.as_str()) {
            return Ok(match builtin {
                Builtin::Unary(f) => match <[_; 1]>::try_from(args) {
                    Ok([a]) => f(a),
                    Err(v) => todo!(),
                },
                Builtin::Binary(f) => match <[_; 2]>::try_from(args) {
                    Ok([a, b]) => f(a, b),
                    Err(v) => todo!(),
                },
                Builtin::Ternary(f) => match <[_; 3]>::try_from(args) {
                    Ok([a, b, c]) => f(a, b, c),
                    Err(v) => todo!(),
                },
                Builtin::Quaternary(f) => todo!(),
                Builtin::OptBinary(f) => todo!(),
            });
        }
    }

    let function = parse_expression(ctx, fn_node)?;

    let Ok([arg]) = <[_; 1]>::try_from(args) else {
        todo!("error");
    };

    Ok(Expression::Image(
        Metadata::new(),
        Moo::new(function),
        Moo::new(arg),
    ))
}

pub fn parse_binary_expression(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Expression, FatalParseError> {
    let left_node = field!(node, "left");
    let left = parse_expression(ctx, left_node)?;

    let right_node = field!(node, "right");
    let right = parse_expression(ctx, right_node)?;

    let m = Metadata::new();
    let op_token_node = field!(node, "operator");
    let operator_token = &ctx.source_code[op_token_node.start_byte()..op_token_node.end_byte()];
    Ok(match operator_token {
        "+" => Expression::Sum(m, Moo::new(matrix_expr![left, right; domain_int!(1..)])),
        "-" => Expression::Difference(m, Moo::new(left), Moo::new(right)),
        "*" => Expression::Product(m, Moo::new(matrix_expr![left, right; domain_int!(1..)])),
        "/" => Expression::UnsafeDiv(m, Moo::new(left), Moo::new(right)),
        "%" => Expression::UnsafeMod(m, Moo::new(left), Moo::new(right)),
        "**" => Expression::UnsafePow(m, Moo::new(left), Moo::new(right)),
        "/\\" => Expression::And(m, Moo::new(matrix_expr![left, right; domain_int!(1..)])),
        "\\/" => Expression::Or(m, Moo::new(matrix_expr![left, right; domain_int!(1..)])),
        "->" => Expression::Imply(m, Moo::new(left), Moo::new(right)),
        "<->" => Expression::Iff(m, Moo::new(left), Moo::new(right)),
        "==" => Expression::Eq(m, Moo::new(left), Moo::new(right)),
        "!=" => Expression::Neq(m, Moo::new(left), Moo::new(right)),
        "<" => Expression::Lt(m, Moo::new(left), Moo::new(right)),
        "<=" => Expression::Leq(m, Moo::new(left), Moo::new(right)),
        ">" => Expression::Gt(m, Moo::new(left), Moo::new(right)),
        ">=" => Expression::Geq(m, Moo::new(left), Moo::new(right)),
        "<lex" => Expression::LexLt(m, Moo::new(left), Moo::new(right)),
        "<=lex" => Expression::LexLeq(m, Moo::new(left), Moo::new(right)),
        ">lex" => Expression::LexGt(m, Moo::new(left), Moo::new(right)),
        ">=lex" => Expression::LexGeq(m, Moo::new(left), Moo::new(right)),
        "in" => Expression::In(m, Moo::new(left), Moo::new(right)),
        "subset" => Expression::Subset(m, Moo::new(left), Moo::new(right)),
        "subsetEq" => Expression::SubsetEq(m, Moo::new(left), Moo::new(right)),
        "supset" => Expression::Supset(m, Moo::new(left), Moo::new(right)),
        "supsetEq" => Expression::SupsetEq(m, Moo::new(left), Moo::new(right)),
        "substring" => Expression::Substring(m, Moo::new(left), Moo::new(right)),
        "subsequence" => Expression::Subsequence(m, Moo::new(left), Moo::new(right)),
        "union" => Expression::Union(m, Moo::new(left), Moo::new(right)),
        "intersect" => Expression::Intersect(m, Moo::new(left), Moo::new(right)),
        _ => todo!(),
    })
}
