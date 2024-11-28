use conjure_core::error::Error;
use std::fs;
use std::sync::{Arc, RwLock};
use tree_sitter::{Node, Parser, Tree};
use tree_sitter_essence::LANGUAGE;

use conjure_core::ast::{
    Atom, DecisionVariable, Domain, Expression, Literal, Name, Range, SymbolTable,
};

use crate::utils::conjure::EssenceParseError;
use conjure_core::context::Context;
use conjure_core::metadata::Metadata;
use conjure_core::Model;
use std::collections::BTreeSet;
use std::collections::HashMap;

pub fn parse_essence_file_native(
    path: &str,
    filename: &str,
    extension: &str,
    context: Arc<RwLock<Context<'static>>>,
) -> Result<Model, EssenceParseError> {
    let (tree, source_code) = get_tree(path, filename, extension);

    let mut model = Model::new_empty(context);
    let root_node = tree.root_node();
    let mut cursor = root_node.walk();
    for statement in root_node.named_children(&mut cursor) {
        match statement.kind() {
            "single_line_comment" => {}
            //"e_prime_label" => {}
            "find_statement_list" => {
                let var_hashmap = parse_find_statement(statement, &source_code);
                for (name, decision_variable) in var_hashmap {
                    model.add_variable(name, decision_variable);
                }
            }
            "constraint_list" => {
                let expression: Expression;
                if statement.child_count() > 2 {
                    let mut constraint_vec: Vec<Expression> = Vec::new();
                    let mut cursor = statement.walk();
                    for constraint in statement.named_children(&mut cursor) {
                        constraint_vec.push(parse_constraint(constraint, &source_code));
                    }
                    expression = Expression::And(Metadata::new(), constraint_vec);
                } else {
                    expression = parse_constraint(statement.child(1).unwrap(), &source_code);
                }

                model.add_constraint(expression);
            }
            _ => {
                return Err(EssenceParseError::ParseError(Error::Parse(
                    "Unrecognized top level statement".to_owned(),
                )))
            }
        }
    }
    return Ok(model);
}

fn get_tree(path: &str, filename: &str, extension: &str) -> (Tree, String) {
    let source_code = fs::read_to_string(format!("{path}/{filename}.{extension}"))
        .expect("Failed to read the source code file");
    let mut parser = Parser::new();
    parser.set_language(&LANGUAGE.into()).unwrap();
    return (
        parser
            .parse(source_code.clone(), None)
            .expect("Failed to parse"),
        source_code,
    );
}

fn parse_find_statement(root_node: Node, source_code: &str) -> HashMap<Name, DecisionVariable> {
    let mut symbol_table = SymbolTable::new();

    let mut cursor = root_node.walk();
    for find_statement in root_node.named_children(&mut cursor) {
        let mut temp_symbols = BTreeSet::new();

        let variable_list = find_statement
            .child_by_field_name("variable_list")
            .expect("No variable list found");
        let mut cursor = variable_list.walk();
        for variable in variable_list.named_children(&mut cursor) {
            let variable_name = &source_code[variable.start_byte()..variable.end_byte()];
            temp_symbols.insert(variable_name);
        }

        let domain = find_statement
            .child_by_field_name("domain")
            .expect("No domain found");
        let domain = Some(parse_domain(domain, source_code));
        let domain = domain.expect("No domain found");

        for name in temp_symbols {
            let decision_variable = DecisionVariable::new(domain.clone());
            symbol_table.insert(Name::UserName(String::from(name)), decision_variable);
        }
    }
    return symbol_table;
}

fn parse_domain(root_node: Node, source_code: &str) -> Domain {
    let mut cursor = root_node.walk();
    cursor.goto_first_child();
    match cursor.node().kind() {
        "bool_domain" => return Domain::BoolDomain,
        "int_domain" => return parse_int_domain(cursor.node(), source_code),
        _ => panic!("Not bool or int domain")
    }
}

fn parse_int_domain(root_node: Node, source_code: &str) -> Domain {
    //TODO implement functionality for non-simple ints
    if root_node.child_count() == 1{
        return Domain::IntDomain(vec![Range::Bounded(std::i32::MIN, std::i32::MAX)]);
    } else {
        let mut ranges: Vec<Range<i32>> = Vec::new();
        let range_list = root_node
            .child_by_field_name("range_list")
            .expect("No range list found (expression ranges not supported yet");
        let mut cursor = range_list.walk();
        for int_range in range_list.named_children(&mut cursor) {
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
                    let mut cursor = int_range.walk();
                    
                    cursor.goto_first_child();
                    match cursor.node().kind() {
                        "expression" => {
                            let mut cursor2 = cursor.node().walk();
                            cursor2.goto_descendant(7);
                            
                            lower_bound = Some(*&source_code
                                [cursor.node().start_byte()..cursor.node().end_byte()]
                                .parse::<i32>()
                                .unwrap());
                    
                            cursor.goto_next_sibling();
                            if !cursor.goto_next_sibling() {
                                upper_bound = None;
                            } else {
                                upper_bound = Some(*&source_code
                                    [cursor.node().start_byte()..cursor.node().end_byte()]
                                    .parse::<i32>()
                                    .unwrap());
                            }
                        }
                        ".." => {
                            lower_bound = None;
                            cursor.goto_next_sibling();
                            upper_bound = Some(*&source_code
                                [cursor.node().start_byte()..cursor.node().end_byte()]
                                .parse::<i32>()
                                .unwrap());
                        }
                        _ => panic!("unsupported int range type")
                    }
                    
                    match (lower_bound, upper_bound) {
                        (Some(lb), Some(ub)) => {
                            ranges.push(Range::Bounded(lb, ub))
                        }
                        (Some(lb), None) => {
                            ranges.push(Range::Bounded(lb, std::i32::MAX))
                        }
                        (None, Some(ub)) => {
                            ranges.push(Range::Bounded(std::i32::MIN, ub))
                        }
                        _ => panic!("Unsupported int range type")
                    }
                }
                _ => panic!("unsupported int range type")
            }
        }
        return Domain::IntDomain(ranges);
    }
}

fn parse_constraint(root_node: Node, source_code: &str) -> Expression {
    match root_node.kind() {
        "constraint" => {
            let mut cursor = root_node.walk();
            cursor.goto_first_child();
            if cursor.node().kind() == "not" {
                cursor.goto_next_sibling();
                let expr = parse_constraint(cursor.node(), source_code);
                return Expression::Not(Metadata::new(), Box::new(expr));
            }
            return parse_constraint(cursor.node(), source_code);
        }
        "expression" => {
            if root_node.child_count() > 1 {
                let mut cursor = root_node.walk();
                let mut vec_exprs = Vec::new();
                for conjunction in root_node.named_children(&mut cursor) {
                    vec_exprs.push(parse_constraint(conjunction, source_code));
                }
                return Expression::Or(Metadata::new(), vec_exprs);
            }
            return parse_constraint(root_node.child(0).unwrap(), source_code);
        }
        "conjunction" => {
            if root_node.child_count() > 1 {
                let mut cursor = root_node.walk();
                let mut vec_exprs = Vec::new();
                for comparison in root_node.named_children(&mut cursor) {
                    vec_exprs.push(parse_constraint(comparison, source_code));
                }
                return Expression::And(Metadata::new(), vec_exprs);
            }
            let mut cursor = root_node.walk();
            cursor.goto_first_child();
            return parse_constraint(cursor.node(), source_code);
        }
        "comparison" => {
            //TODO: right now assuming there's only two but really could be any number, change
            if root_node.child_count() > 1 {
                let mut cursor = root_node.walk();

                cursor.goto_first_child();
                let expr1 = parse_constraint(cursor.node(), source_code);

                cursor.goto_next_sibling();
                let comp_op = cursor.node();
                let op_type = &source_code[comp_op.start_byte()..comp_op.end_byte()];

                cursor.goto_next_sibling();
                let expr2 = parse_constraint(cursor.node(), source_code);

                match op_type {
                    "=" => {
                        return Expression::Eq(Metadata::new(), Box::new(expr1), Box::new(expr2));
                    }
                    "!=" => {
                        return Expression::Neq(Metadata::new(), Box::new(expr1), Box::new(expr2));
                    }
                    "<=" => {
                        return Expression::Leq(Metadata::new(), Box::new(expr1), Box::new(expr2));
                    }
                    ">=" => {
                        return Expression::Geq(Metadata::new(), Box::new(expr1), Box::new(expr2));
                    }
                    "<" => {
                        return Expression::Lt(Metadata::new(), Box::new(expr1), Box::new(expr2));
                    }
                    ">" => {
                        return Expression::Gt(Metadata::new(), Box::new(expr1), Box::new(expr2));
                    }
                    _ => panic!("error!"),
                }
            }
            let mut cursor = root_node.walk();
            cursor.goto_first_child();
            return parse_constraint(cursor.node(), source_code);
        }
        "addition" => {
            //TODO: right now assuming its a "+", add for if its a "-"
            //TODO: still some issues with multiple, xyz test
            if root_node.child_count() > 1 {
                let mut expr_vec: Vec<Expression> = Vec::new();
                let mut cursor = root_node.walk();
                for term in root_node.named_children(&mut cursor) {
                    expr_vec.push(parse_constraint(term, source_code));
                }
                return Expression::Sum(Metadata::new(), expr_vec);
            }
            let mut cursor = root_node.walk();
            cursor.goto_first_child();
            return parse_constraint(cursor.node(), source_code);
        }
        "term" => {
            //TODO: right now assuming its a "/" or "%", add for if its a "*"
            //TODO: right now assuming its unsafe, could be safe
            //TODO: right now assuming its only two terms, really could be any number
            if root_node.child_count() > 1 {
                let mut cursor = root_node.walk();
                cursor.goto_first_child();
                let factor1 = parse_constraint(cursor.node(), source_code);
                cursor.goto_next_sibling();

                match cursor.node().kind() {
                    "/" => {
                        cursor.goto_next_sibling();
                        let factor2 = parse_constraint(cursor.node(), source_code);
                        return Expression::UnsafeDiv(
                            Metadata::new(),
                            Box::new(factor1),
                            Box::new(factor2),
                        );
                    }
                    "%" => {
                        cursor.goto_next_sibling();
                        let factor2 = parse_constraint(cursor.node(), source_code);
                        return Expression::UnsafeMod(
                            Metadata::new(),
                            Box::new(factor1),
                            Box::new(factor2),
                        );
                    }
                    _ => panic!("No multiplication implemented yet or other error"),
                }
            }
            let mut cursor = root_node.walk();
            cursor.goto_first_child();
            return parse_constraint(cursor.node(), source_code);
        }
        "factor" => {
            // eventually use first_child and sibling methods here
            let mut cursor = root_node.walk();
            cursor.goto_first_child();
            if root_node.child_count() > 1 {
                cursor.goto_next_sibling();
            }
            return parse_constraint(cursor.node(), source_code);
        }
        "min" => {
            let mut cursor = root_node.walk();
            let mut variable_list: Vec<Expression> = Vec::new();
            for variable in root_node.named_children(&mut cursor) {
                variable_list.push(parse_constraint(variable, source_code));
            }
            return Expression::Min(Metadata::new(), variable_list);
        }
        "max" => {
            let mut cursor = root_node.walk();
            let mut variable_list: Vec<Expression> = Vec::new();
            for variable in root_node.named_children(&mut cursor) {
                variable_list.push(parse_constraint(variable, source_code));
            }
            return Expression::Max(Metadata::new(), variable_list);
        }
        "sum" => {
            let mut expr_vec: Vec<Expression> = Vec::new();
            let mut cursor = root_node.walk();
            for factor in root_node.named_children(&mut cursor) {
                expr_vec.push(parse_constraint(factor, source_code));
            }
            return Expression::Sum(Metadata::new(), expr_vec);
        }
        "constant" => {
            let child = first_child(root_node);
            match child.kind() {
                "integer" => {
                    let constant_value = &source_code[child.start_byte()..child.end_byte()]
                        .parse::<i32>()
                        .unwrap();
                    return Expression::Atomic(
                        Metadata::new(),
                        Atom::Literal(Literal::Int(*constant_value)),
                    );
                }
                "TRUE" => {
                    return Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Bool(true)));
                }
                "FALSE" => {
                    return Expression::Atomic(
                        Metadata::new(),
                        Atom::Literal(Literal::Bool(false)),
                    );
                }
                _ => panic!("Error"),
            }
        }
        "variable" => {
            let child = first_child(root_node);
            let variable_name = String::from(&source_code[child.start_byte()..child.end_byte()]);
            return Expression::Atomic(
                Metadata::new(),
                Atom::Reference(Name::UserName(variable_name)),
            );
        }
        _ => {
            let node_kind = root_node.kind();
            panic!("{node_kind} is not a recognized node kind");
        }
    }
}

fn first_child(node: Node) -> Node {
    let mut cursor = node.walk();
    cursor.goto_first_child();
    return cursor.node();
}
