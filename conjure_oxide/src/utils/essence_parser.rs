#![allow(clippy::legacy_numeric_constants)]
use conjure_core::error::Error;
use std::fs;
use std::sync::{Arc, RwLock};
use tree_sitter::{Node, Parser, Tree};
use tree_sitter_essence::LANGUAGE;

use conjure_core::ast::{Atom, DecisionVariable, Domain, Expression, Literal, Name, Range};

use crate::utils::conjure::EssenceParseError;
use conjure_core::context::Context;
use conjure_core::metadata::Metadata;
use conjure_core::Model;
use std::collections::{BTreeMap, BTreeSet};

pub fn parse_essence_file_native(
    path: &str,
    filename: &str,
    extension: &str,
    context: Arc<RwLock<Context<'static>>>,
) -> Result<Model, EssenceParseError> {
    let (tree, source_code) = get_tree(path, filename, extension);

    let mut model = Model::new_empty(context);
    let root_node = tree.root_node();
    for statement in named_children(&root_node) {
        match statement.kind() {
            "single_line_comment" => {}
            "find_statement_list" => {
                let var_hashmap = parse_find_statement(statement, &source_code);
                for (name, decision_variable) in var_hashmap {
                    model.add_variable(name, decision_variable);
                }
            }
            "constraint_list" => {
                let mut constraint_vec: Vec<Expression> = Vec::new();
                for constraint in named_children(&statement) {
                    if constraint.kind() != "single_line_comment" {
                        constraint_vec.push(parse_constraint(constraint, &source_code));
                    }
                }
                model.constraints.extend(constraint_vec);
            }
            "e_prime_label" => {}
            _ => {
                let kind = statement.kind();
                return Err(EssenceParseError::ParseError(Error::Parse(
                    format!("Unrecognized top level statement kind: {kind}").to_owned(),
                )));
            }
        }
    }
    Ok(model)
}

fn get_tree(path: &str, filename: &str, extension: &str) -> (Tree, String) {
    let source_code = fs::read_to_string(format!("{path}/{filename}.{extension}"))
        .expect("Failed to read the source code file");
    let mut parser = Parser::new();
    parser.set_language(&LANGUAGE.into()).unwrap();
    (
        parser
            .parse(source_code.clone(), None)
            .expect("Failed to parse"),
        source_code,
    )
}

fn parse_find_statement(
    find_statement_list: Node,
    source_code: &str,
) -> BTreeMap<Name, DecisionVariable> {
    let mut vars = BTreeMap::new();

    for find_statement in named_children(&find_statement_list) {
        let mut temp_symbols = BTreeSet::new();

        let variable_list = find_statement
            .child_by_field_name("variable_list")
            .expect("No variable list found");
        for variable in named_children(&variable_list) {
            let variable_name = &source_code[variable.start_byte()..variable.end_byte()];
            temp_symbols.insert(variable_name);
        }

        let domain = find_statement
            .child_by_field_name("domain")
            .expect("No domain found");
        let domain = parse_domain(domain, source_code);

        for name in temp_symbols {
            let decision_variable = DecisionVariable::new(domain.clone());
            vars.insert(Name::UserName(String::from(name)), decision_variable);
        }
    }
    vars
}

fn parse_domain(domain: Node, source_code: &str) -> Domain {
    let domain = domain.child(0).expect("No domain");
    match domain.kind() {
        "bool_domain" => Domain::BoolDomain,
        "int_domain" => parse_int_domain(domain, source_code),
        _ => panic!("Not bool or int domain"),
    }
}

fn parse_int_domain(int_domain: Node, source_code: &str) -> Domain {
    //TODO implement functionality for non-simple ints
    if int_domain.child_count() == 1 {
        Domain::IntDomain(vec![Range::Bounded(std::i32::MIN, std::i32::MAX)])
    } else {
        let mut ranges: Vec<Range<i32>> = Vec::new();
        let range_list = int_domain
            .child_by_field_name("range_list")
            .expect("No range list found (expression ranges not supported yet");
        for int_range in named_children(&range_list) {
            match int_range.kind() {
                "integer" => {
                    let integer_value = &source_code[int_range.start_byte()..int_range.end_byte()]
                        .parse::<i32>()
                        .unwrap();
                    ranges.push(Range::Single(*integer_value));
                }
                "int_range" => {
                    let lower_bound: Option<i32>;
                    let upper_bound: Option<i32>;
                    let range_component = int_range.child(0).expect("Error with integer range");
                    match range_component.kind() {
                        "expression" => {
                            lower_bound = Some(
                                source_code
                                    [range_component.start_byte()..range_component.end_byte()]
                                    .parse::<i32>()
                                    .unwrap(),
                            );

                            if let Some(range_component) = range_component.next_named_sibling() {
                                upper_bound = Some(
                                    source_code
                                        [range_component.start_byte()..range_component.end_byte()]
                                        .parse::<i32>()
                                        .unwrap(),
                                );
                            } else {
                                upper_bound = None;
                            }
                        }
                        ".." => {
                            lower_bound = None;
                            let range_component = range_component
                                .next_sibling()
                                .expect("Error with integer range");
                            upper_bound = Some(
                                source_code
                                    [range_component.start_byte()..range_component.end_byte()]
                                    .parse::<i32>()
                                    .unwrap(),
                            );
                        }
                        _ => panic!("unsupported int range type"),
                    }

                    match (lower_bound, upper_bound) {
                        (Some(lb), Some(ub)) => ranges.push(Range::Bounded(lb, ub)),
                        (Some(lb), None) => ranges.push(Range::Bounded(lb, std::i32::MAX)),
                        (None, Some(ub)) => ranges.push(Range::Bounded(std::i32::MIN, ub)),
                        _ => panic!("Unsupported int range type"),
                    }
                }
                _ => panic!("unsupported int range type"),
            }
        }
        Domain::IntDomain(ranges)
    }
}

fn parse_constraint(constraint: Node, source_code: &str) -> Expression {
    match constraint.kind() {
        "constraint" | "expression" => {
            let child = constraint
                .child(0)
                .expect("Error: mission node in constraint statement");
            parse_constraint(child, source_code)
        }
        "or_expr" => {
            let mut expr_vec = Vec::new();
            for expr in named_children(&constraint) {
                expr_vec.push(parse_constraint(expr, source_code));
            }
            Expression::Or(Metadata::new(), expr_vec)
        }
        "and_expr" => {
            let mut vec_exprs = Vec::new();
            for expr in named_children(&constraint) {
                vec_exprs.push(parse_constraint(expr, source_code));
            }
            Expression::And(Metadata::new(), vec_exprs)
        }
        "comparison" => {
            let expr1_node = constraint
                .child(0)
                .expect("Error with comparison expression");
            let expr1 = parse_constraint(expr1_node, source_code);
            let comp_op = expr1_node
                .next_sibling()
                .expect("Error with comparison expression");
            let op_type = &source_code[comp_op.start_byte()..comp_op.end_byte()];
            let expr2_node = comp_op
                .next_sibling()
                .expect("Error with comparison expression");
            let expr2 = parse_constraint(expr2_node, source_code);

            match op_type {
                "=" => Expression::Eq(Metadata::new(), Box::new(expr1), Box::new(expr2)),
                "!=" => Expression::Neq(Metadata::new(), Box::new(expr1), Box::new(expr2)),
                "<=" => Expression::Leq(Metadata::new(), Box::new(expr1), Box::new(expr2)),
                ">=" => Expression::Geq(Metadata::new(), Box::new(expr1), Box::new(expr2)),
                "<" => Expression::Lt(Metadata::new(), Box::new(expr1), Box::new(expr2)),
                ">" => Expression::Gt(Metadata::new(), Box::new(expr1), Box::new(expr2)),
                _ => panic!("Not a supported comp_op"),
            }
        }
        "math_expr" => {
            let expr1_node = constraint.child(0).expect("Error with math expression");
            let expr1 = parse_constraint(expr1_node, source_code);
            let math_op = expr1_node
                .next_sibling()
                .expect("Error with math expression");
            let op_type = &source_code[math_op.start_byte()..math_op.end_byte()];
            let expr2_node = math_op.next_sibling().expect("Error with math expression");
            let expr2 = parse_constraint(expr2_node, source_code);

            match op_type {
                "+" => Expression::Sum(Metadata::new(), vec![expr1, expr2]),
                "-" => {
                    panic!("Subtraction expressions not supported yet")
                }
                "*" => Expression::Product(Metadata::new(), vec![expr1, expr2]),
                "/" => {
                    //TODO: add checks for if division is safe or not
                    Expression::UnsafeDiv(Metadata::new(), Box::new(expr1), Box::new(expr2))
                }
                "%" => {
                    //TODO: add checks for if mod is safe or not
                    Expression::UnsafeMod(Metadata::new(), Box::new(expr1), Box::new(expr2))
                }
                _ => panic!("Not a supported math_op"),
            }
        }
        "not_expr" => {
            let constraint = constraint
                .child(1)
                .expect("Error: no node after 'not' node");
            let expr = parse_constraint(constraint, source_code);
            Expression::Not(Metadata::new(), Box::new(expr))
        }
        "sub_expr" => {
            println!("sub");
            let expr = constraint
                .named_child(0)
                .expect("Error with sub expression");
            parse_constraint(expr, source_code)
        }
        "min" => {
            let mut term_list = Vec::new();
            for term in named_children(&constraint) {
                term_list.push(parse_constraint(term, source_code));
            }
            Expression::Min(Metadata::new(), term_list)
        }
        "max" => {
            let mut term_list = Vec::new();
            for term in named_children(&constraint) {
                term_list.push(parse_constraint(term, source_code));
            }
            Expression::Max(Metadata::new(), term_list)
        }
        "sum" => {
            let mut term_list = Vec::new();
            for term in named_children(&constraint) {
                term_list.push(parse_constraint(term, source_code));
            }
            Expression::Sum(Metadata::new(), term_list)
        }
        "all_diff" => {
            let mut term_list = Vec::new();
            for term in named_children(&constraint) {
                term_list.push(parse_constraint(term, source_code));
            }
            Expression::AllDiff(Metadata::new(), term_list)
        }
        "constant" => {
            let child = constraint.child(0).expect("Error with constant");
            match child.kind() {
                "integer" => {
                    let constant_value = &source_code[child.start_byte()..child.end_byte()]
                        .parse::<i32>()
                        .unwrap();
                    Expression::Atomic(
                        Metadata::new(),
                        Atom::Literal(Literal::Int(*constant_value)),
                    )
                }
                "TRUE" => Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Bool(true))),
                "FALSE" => Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Bool(false))),
                _ => panic!("Error"),
            }
        }
        "variable" => {
            let child = constraint.child(0).expect("Error with varaible");
            let variable_name = String::from(&source_code[child.start_byte()..child.end_byte()]);
            Expression::Atomic(
                Metadata::new(),
                Atom::Reference(Name::UserName(variable_name)),
            )
        }
        _ => {
            let node_kind = constraint.kind();
            panic!("{node_kind} is not a recognized node kind");
        }
    }
}

fn named_children<'a>(node: &'a Node<'a>) -> impl Iterator<Item = Node<'a>> + 'a {
    (0..node.named_child_count()).filter_map(|i| node.named_child(i))
}
