#![allow(clippy::legacy_numeric_constants)]
use conjure_core::ast::Declaration;
use conjure_core::error::Error;
use std::fs;
use std::rc::Rc;
use std::sync::{Arc, RwLock};
use tree_sitter::{Node, Parser, Tree};
use tree_sitter_essence::LANGUAGE;

use conjure_core::ast::{Atom, Domain, Expression, Literal, Name, Range, SymbolTable};

use crate::utils::conjure::EssenceParseError;
use conjure_core::context::Context;
use conjure_core::metadata::Metadata;
use conjure_core::{into_matrix_expr, matrix_expr, Model};
use std::collections::{BTreeMap, BTreeSet};

pub fn parse_essence_file_native(
    filepath: &str,
    context: Arc<RwLock<Context<'static>>>,
) -> Result<Model, EssenceParseError> {
    let (tree, source_code) = get_tree(filepath);
    let root_node = tree.root_node();

    if root_node.has_error() {
        let mut messages: Vec<String> = Vec::new();
        parse_error(root_node, &source_code, &mut messages);
        let messages_joined = messages.join(&format!("\n{}:", filepath));
        return Err(EssenceParseError::ParseError(Error::Parse(format!(
            "\n{}:{}",
            filepath,
            messages_joined)
        )));
    }

    let mut model = Model::new(context);
    for statement in named_children(&root_node) {
        match statement.kind() {
            "single_line_comment" => {}
            "find_statement_list" => {
                let var_hashmap = parse_find_statement(statement, &source_code);
                for (name, decision_variable) in var_hashmap {
                    model
                        .as_submodel_mut()
                        .symbols_mut()
                        .insert(Rc::new(Declaration::new_var(name, decision_variable)));
                }
            }
            "constraint_list" => {
                let mut constraint_vec: Vec<Expression> = Vec::new();
                for constraint in named_children(&statement) {
                    if constraint.kind() != "single_line_comment" {
                        constraint_vec.push(parse_constraint(constraint, &source_code, &statement));
                    }
                }
                model.as_submodel_mut().add_constraints(constraint_vec);
            }
            "language_label" => {}
            "letting_statement_list" => {
                let letting_vars = parse_letting_statement(statement, &source_code);
                model.as_submodel_mut().symbols_mut().extend(letting_vars);
            }
            "dominance_relation" => {
                let inner = statement
                    .child(1)
                    .expect("Expected a sub-expression inside `dominanceRelation`");
                let expr = parse_constraint(inner, &source_code, &statement);
                let dominance = Expression::DominanceRelation(Metadata::new(), Box::new(expr));
                if model.dominance.is_some() {
                    return Err(EssenceParseError::ParseError(Error::Parse(
                        "Duplicate dominance relation".to_owned(),
                    )));
                }
                model.dominance = Some(dominance);
            }
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

fn get_tree(filepath: &str) -> (Tree, String) {
    let source_code = fs::read_to_string(filepath)
        .unwrap_or_else(|_| panic!("Failed to read the source code file {}", filepath));
    let mut parser = Parser::new();
    parser.set_language(&LANGUAGE.into()).unwrap();
    (
        parser
            .parse(source_code.clone(), None)
            .expect("Failed to parse"),
        source_code,
    )
}

fn parse_find_statement(find_statement_list: Node, source_code: &str) -> BTreeMap<Name, Domain> {
    let mut vars = BTreeMap::new();

    for find_statement in named_children(&find_statement_list) {
        let mut temp_symbols = BTreeSet::new();

        let variable_list = find_statement
            .named_child(0)
            .expect("No variable list found");
        for variable in named_children(&variable_list) {
            let variable_name = &source_code[variable.start_byte()..variable.end_byte()];
            temp_symbols.insert(variable_name);
        }

        let domain = find_statement.named_child(1).expect("No domain found");
        let domain = parse_domain(domain, source_code);

        for name in temp_symbols {
            vars.insert(Name::UserName(String::from(name)), domain.clone());
        }
    }
    vars
}

fn parse_domain(domain: Node, source_code: &str) -> Domain {
    let domain = domain.child(0).expect("No domain");
    match domain.kind() {
        "bool_domain" => Domain::BoolDomain,
        "int_domain" => parse_int_domain(domain, source_code),
        "variable" => {
            let variable_name = &source_code[domain.start_byte()..domain.end_byte()];
            Domain::DomainReference(Name::UserName(String::from(variable_name)))
        }
        _ => panic!("Not bool or int domain"),
    }
}

fn parse_int_domain(int_domain: Node, source_code: &str) -> Domain {
    if int_domain.child_count() == 1 {
        Domain::IntDomain(vec![Range::Bounded(std::i32::MIN, std::i32::MAX)])
    } else {
        let mut ranges: Vec<Range<i32>> = Vec::new();
        let range_list = int_domain
            .named_child(0)
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

fn parse_letting_statement(letting_statement_list: Node, source_code: &str) -> SymbolTable {
    let mut symbol_table = SymbolTable::new();

    for letting_statement in named_children(&letting_statement_list) {
        let mut temp_symbols = BTreeSet::new();

        let variable_list = letting_statement.child(0).expect("No variable list found");
        for variable in named_children(&variable_list) {
            let variable_name = &source_code[variable.start_byte()..variable.end_byte()];
            temp_symbols.insert(variable_name);
        }

        let expr_or_domain = letting_statement
            .named_child(1)
            .expect("No domain or expression found for letting statement");
        match expr_or_domain.kind() {
            "expression" => {
                for name in temp_symbols {
                    symbol_table.insert(Rc::new(Declaration::new_value_letting(
                        Name::UserName(String::from(name)),
                        parse_constraint(expr_or_domain, source_code, &letting_statement_list),
                    )));
                }
            }
            "domain" => {
                for name in temp_symbols {
                    symbol_table.insert(Rc::new(Declaration::new_domain_letting(
                        Name::UserName(String::from(name)),
                        parse_domain(expr_or_domain, source_code),
                    )));
                }
            }
            _ => panic!("Unrecognized node in letting statement"),
        }
    }
    symbol_table
}

fn parse_constraint(constraint: Node, source_code: &str, root: &Node) -> Expression {
    match constraint.kind() {
        "constraint" | "expression" | "boolean_expr" | "comparison_expr" | "arithmetic_expr"
        | "primary_expr" | "sub_expr" => child_expr(constraint, source_code, root),
        "not_expr" => Expression::Not(
            Metadata::new(),
            Box::new(child_expr(constraint, source_code, root)),
        ),
        "abs_value" => Expression::Abs(
            Metadata::new(),
            Box::new(child_expr(constraint, source_code, root)),
        ),
        "negative_expr" => Expression::Neg(
            Metadata::new(),
            Box::new(child_expr(constraint, source_code, root)),
        ),
        "exponent" | "product_expr" | "sum_expr" | "comparison" | "and_expr" | "or_expr"
        | "implication" => {
            let expr1 = child_expr(constraint, source_code, root);
            let op = constraint.child(1).unwrap_or_else(|| {
                panic!(
                    "Error: missing node in expression of kind {}",
                    constraint.kind()
                )
            });
            let op_type = &source_code[op.start_byte()..op.end_byte()];
            let expr2_node = constraint.child(2).unwrap_or_else(|| {
                panic!(
                    "Error: missing node in expression of kind {}",
                    constraint.kind()
                )
            });
            let expr2 = parse_constraint(expr2_node, source_code, root);

            match op_type {
                "**" => Expression::UnsafePow(Metadata::new(), Box::new(expr1), Box::new(expr2)),
                "+" => Expression::Sum(Metadata::new(), vec![expr1, expr2]),
                "-" => Expression::Minus(Metadata::new(), Box::new(expr1), Box::new(expr2)),
                "*" => Expression::Product(Metadata::new(), vec![expr1, expr2]),
                "/" => {
                    //TODO: add checks for if division is safe or not
                    Expression::UnsafeDiv(Metadata::new(), Box::new(expr1), Box::new(expr2))
                }
                "%" => {
                    //TODO: add checks for if mod is safe or not
                    Expression::UnsafeMod(Metadata::new(), Box::new(expr1), Box::new(expr2))
                }
                "=" => Expression::Eq(Metadata::new(), Box::new(expr1), Box::new(expr2)),
                "!=" => Expression::Neq(Metadata::new(), Box::new(expr1), Box::new(expr2)),
                "<=" => Expression::Leq(Metadata::new(), Box::new(expr1), Box::new(expr2)),
                ">=" => Expression::Geq(Metadata::new(), Box::new(expr1), Box::new(expr2)),
                "<" => Expression::Lt(Metadata::new(), Box::new(expr1), Box::new(expr2)),
                ">" => Expression::Gt(Metadata::new(), Box::new(expr1), Box::new(expr2)),
                "/\\" => Expression::And(Metadata::new(), Box::new(matrix_expr![expr1, expr2])),
                "\\/" => Expression::Or(Metadata::new(), Box::new(matrix_expr![expr1, expr2])),
                "->" => Expression::Imply(Metadata::new(), Box::new(expr1), Box::new(expr2)),
                _ => panic!("Error: unsupported operator"),
            }
        }
        "quantifier_expr" => {
            let mut expr_list = Vec::new();
            for expr in named_children(&constraint) {
                expr_list.push(parse_constraint(expr, source_code, root));
            }

            let quantifier = constraint.child(0).unwrap_or_else(|| {
                panic!(
                    "Error: missing node in expression of kind {}",
                    constraint.kind()
                )
            });
            let quantifier_type = &source_code[quantifier.start_byte()..quantifier.end_byte()];

            match quantifier_type {
                "and" => Expression::And(Metadata::new(), Box::new(into_matrix_expr![expr_list])),
                "or" => Expression::Or(Metadata::new(), Box::new(into_matrix_expr![expr_list])),
                "min" => Expression::Min(Metadata::new(), Box::new(into_matrix_expr![expr_list])),
                "max" => Expression::Max(Metadata::new(), Box::new(into_matrix_expr![expr_list])),
                "sum" => Expression::Sum(Metadata::new(), expr_list),
                "allDiff" => {
                    Expression::AllDiff(Metadata::new(), Box::new(into_matrix_expr![expr_list]))
                }
                _ => panic!("Error: unsupported quantifier"),
            }
        }
        "constant" => {
            let child = constraint.child(0).unwrap_or_else(|| {
                panic!(
                    "Error: missing node in expression of kind {}",
                    constraint.kind()
                )
            });
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
            let variable_name =
                String::from(&source_code[constraint.start_byte()..constraint.end_byte()]);
            Expression::Atomic(
                Metadata::new(),
                Atom::Reference(Name::UserName(variable_name)),
            )
        }
        "from_solution" => match root.kind() {
            "dominance_relation" => {
                let inner = child_expr(constraint, source_code, root);
                match inner {
                    Expression::Atomic(_, _) => {
                        Expression::FromSolution(Metadata::new(), Box::new(inner))
                    }
                    _ => panic!("Expression inside a `fromSolution()` must be a variable name"),
                }
            }
            _ => panic!("`fromSolution()` is only allowed inside dominance relation definitions"),
        },
        _ => panic!("{} is not a recognized node kind", constraint.kind()),
    }
}

fn named_children<'a>(node: &'a Node<'a>) -> impl Iterator<Item = Node<'a>> + 'a {
    (0..node.named_child_count()).filter_map(|i| node.named_child(i))
}

fn child_expr(node: Node, source_code: &str, root: &Node) -> Expression {
    let child = node
        .named_child(0)
        .unwrap_or_else(|| panic!("Error: missing node in expression of kind {}", node.kind()));
    parse_constraint(child, source_code, root)
}

fn parse_error(node: Node, source_code: &str, messages: &mut Vec<String>) {
    let mut i = 0;
    for child_node in named_children(&node) {
        // if child_node.is_extra() {continue;}
        let x = node.field_name_for_named_child(i);
        i += 1;
        if x.is_some() {
            parse_error(child_node, source_code, messages);
            continue;
        }

        messages.push(get_line(child_node, source_code));
    }
}

fn get_line(node: Node, source_code: &str) -> String {
    let line = node.start_position().row + 1;
    let character = node.start_position().column + 1;
    let line_text = source_code
        .lines()
        .nth(line - 1)
        .unwrap_or("Line not found");
    let pointer_line = format!(" |{}^", " ".repeat(character));
    let message;
    if node.parent().unwrap().kind() == "program" {
        message = format!("Invalid {}", pretty(node.prev_sibling().unwrap().kind()));
    } else if node.kind() == "ERROR" {
        message = format!("Invalid {}", pretty(node.parent().unwrap().kind()));
    } else {
        message = format!("Invalid {}", pretty(node.kind()));
    }

    return format!(
        "{}:{}:\n |\n{}| {}\n{}\n{}\n",
        line,
        character,
        line,
        line_text,
        pointer_line,
        message
    );
}


fn pretty(kind: &str) -> String {
    let pretty_str = kind
        .replace("_", " ")
        .replace(" expr ", " expression ")
        .replace(" op ", " operator")
        .replace(" int ", " integer ");

    pretty_str
}
