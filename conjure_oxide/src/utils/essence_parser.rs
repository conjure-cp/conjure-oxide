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
use conjure_core::{parse, Model};
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
            "find_statement" => {
                let var_hashmap = parse_find_statement(statement, &source_code);
                for (name, decision_variable) in var_hashmap {
                    model.add_variable(name, decision_variable);
                }
            }
            "constraint" => {
                let constraint = statement.child(1).unwrap();
                let expression = parse_constraint(constraint, &source_code);
                model.add_constraint(expression);
            }
            _ => {
                return Err(EssenceParseError::ParseError(Error::Parse(
                    "Error".to_owned(),
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
    let mut temp_symbols = BTreeSet::new();
    let mut domain: Option<Domain> = None;

    let mut cursor = root_node.walk();
    for node in root_node.named_children(&mut cursor) {
        match node.kind() {
            "variable_list" => {
                let mut cursor = node.walk();
                for variable in node.named_children(&mut cursor) {
                    let variable_name = &source_code[variable.start_byte()..variable.end_byte()];
                    temp_symbols.insert(variable_name);
                }
            }
            "domain" => {
                domain = Some(parse_domain(node, source_code));
            }
            _ => panic!("issue"),
        }
    }
    let domain = domain.expect("No domain found");

    for name in temp_symbols {
        let decision_variable = DecisionVariable::new(domain.clone());
        symbol_table.insert(Name::UserName(String::from(name)), decision_variable);
    }
    return symbol_table;
}

fn parse_domain(root_node: Node, source_code: &str) -> Domain {
    let mut cursor = root_node.walk();
    cursor.goto_first_child();
    match cursor.node().kind() {
        "bool_domain" => return Domain::BoolDomain,
        "int_domain" => return parse_int_domain(cursor.node(), source_code),
        _ => {
            panic!("No domain type found");
        }
    }
}

fn parse_int_domain(root_node: Node, source_code: &str) -> Domain {
    if root_node.child_count() == 1 {
        return Domain::IntDomain(vec![Range::Bounded(std::i32::MIN, std::i32::MAX)]);
    } else {
        let range_or_expr = root_node.child(2).expect("No range or expression found");
        match range_or_expr.kind() {
            "range_list" => {
                let mut cursor = range_or_expr.walk();
                let mut ranges: Vec<Range<i32>> = Vec::new();
                for range in range_or_expr.named_children(&mut cursor) {
                    match range.kind() {
                        "lower_bound_range" => {
                            let lower_bound_node = range.child_by_field_id(0).unwrap();
                            let lower_bound = &source_code
                                [lower_bound_node.start_byte()..lower_bound_node.end_byte()]
                                .parse::<i32>()
                                .unwrap();
                            ranges.push(Range::Bounded(*lower_bound, std::i32::MAX))
                        }
                        "upper_bound_range" => {
                            let upper_bound_node = range.child_by_field_id(1).unwrap();
                            let upper_bound = &source_code
                                [upper_bound_node.start_byte()..upper_bound_node.end_byte()]
                                .parse::<i32>()
                                .unwrap();
                            ranges.push(Range::Bounded(std::i32::MIN, *upper_bound))
                        }
                        "closed_range" => {
                            let mut cursor = range.walk();

                            cursor.goto_first_child();
                            let lower_bound_node = cursor.node();

                            cursor.goto_next_sibling();
                            cursor.goto_next_sibling();
                            let upper_bound_node = cursor.node();

                            let lower_bound = &source_code
                                [lower_bound_node.start_byte()..lower_bound_node.end_byte()]
                                .parse::<i32>()
                                .unwrap();
                            let upper_bound = &source_code
                                [upper_bound_node.start_byte()..upper_bound_node.end_byte()]
                                .parse::<i32>()
                                .unwrap();
                            ranges.push(Range::Bounded(*lower_bound, *upper_bound));
                        }
                        "integer" => {
                            let integer_value = &source_code[range.start_byte()..range.end_byte()]
                                .parse::<i32>()
                                .unwrap();
                            ranges.push(Range::Single(*integer_value));
                        }
                        _ => {}
                    }
                }
                return Domain::IntDomain(ranges);
            }
            "expression" => {
                //todo: add this code, right now returns infinite integer domain
                return Domain::IntDomain(vec![Range::Bounded(std::i32::MIN, std::i32::MAX)]);
            }
            _ => {
                panic!("No range or expression found");
            }
        }
    }
}

fn parse_constraint(root_node: Node, source_code: &str) -> Expression {
    match root_node.kind() {
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
            println!("addition");
            //TODO: right now assuming its a "+", add for if its a "-"
            //TODO: right now assuming its only two terms, really could be any number
            //(pos use goto_last_child because its vec and then Box)
            if root_node.child_count() > 1 {
                let term1 = parse_constraint(root_node.child_by_field_id(0).unwrap(), source_code);
                let term2 = parse_constraint(root_node.child_by_field_id(2).unwrap(), source_code);
                return Expression::SumEq(Metadata::new(), vec![term1], Box::new(term2));
            }
            let mut cursor = root_node.walk();
            cursor.goto_first_child();
            return parse_constraint(cursor.node(), source_code);
        }
        "term" => {
            println!("term");
            //TODO: right now assuming its a "/", add for if its a "*"
            //TODO: right now assuming its unsafe, could be safe
            //TODO: right now assuming its only two terms, really could be any number
            if root_node.child_count() > 1 {
                let mut cursor = root_node.walk();
                cursor.goto_first_child();
                let factor1 =
                    parse_constraint(cursor.node(), source_code);
                cursor.goto_next_sibling();
                cursor.goto_next_sibling();
                let factor2 =
                    parse_constraint(cursor.node(), source_code);
                return Expression::UnsafeDiv(
                    Metadata::new(), 
                    Box::new(factor1),
                    Box::new(factor2));
            }
            let mut cursor = root_node.walk();
            cursor.goto_first_child();
            return parse_constraint(cursor.node(), source_code);
        }
        "factor" => {
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
        "constant" => {
            //once the grammar is changed, this will be more complicated
            let mut cursor = root_node.walk();
            cursor.goto_first_child();
            let constant_value = &source_code[cursor.node().start_byte()..cursor.node().end_byte()]
                .parse::<i32>()
                .unwrap();
            //TODO: right now its only Int but could be bool too
            return Expression::Atomic(
                Metadata::new(),
                Atom::Literal(Literal::Int(*constant_value)),
            );
        }
        "variable" => {
            println!("variable");
            let mut cursor = root_node.walk();
            cursor.goto_first_child();
            let variable_name =
                String::from(&source_code[cursor.node().start_byte()..cursor.node().end_byte()]);
            println!("variable name: {variable_name}");
            return Expression::Atomic(
                Metadata::new(),
                Atom::Reference(Name::UserName(variable_name)),
            );
        }
        _ => panic!("Error"),
    }
}
