#![allow(clippy::legacy_numeric_constants)]
use tree_sitter::Node;

use super::util::named_children;
use conjure_core::ast::{Domain, Name, Range};

/// Parse an Essence variable domain into its Conjure AST representation.
pub fn parse_domain(domain: Node, source_code: &str) -> Domain {
    match domain.kind() {
        "domain" => parse_domain(domain.child(0).expect("No domain found"), source_code),
        "bool_domain" => Domain::BoolDomain,
        "int_domain" => parse_int_domain(domain, source_code),
        "identifier" => {
            let variable_name = &source_code[domain.start_byte()..domain.end_byte()];
            Domain::DomainReference(Name::UserName(String::from(variable_name)))
        }
        "tuple_domain" => parse_tuple_domain(domain, source_code),
        "matrix_domain" => parse_matrix_domain(domain, source_code),
        _ => panic!("{} is not a supported domain type", domain.kind()),
    }
}

/// Parse an integer domain. Can be a single integer or a range.
fn parse_int_domain(int_domain: Node, source_code: &str) -> Domain {
    if int_domain.child_count() == 1 {
        Domain::IntDomain(vec![Range::Bounded(std::i32::MIN, std::i32::MAX)])
    } else {
        let mut ranges: Vec<Range<i32>> = Vec::new();
        let range_list = int_domain
            .child_by_field_name("ranges")
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
                    if let Some(lower_bound_node) = int_range.child_by_field_name("lower") {
                        lower_bound = Some(
                            source_code[lower_bound_node.start_byte()..lower_bound_node.end_byte()]
                                .parse::<i32>()
                                .unwrap(),
                        );
                    } else {
                        lower_bound = None;
                    }
                    if let Some(upper_bound_node) = int_range.child_by_field_name("upper") {
                        upper_bound = Some(
                            source_code[upper_bound_node.start_byte()..upper_bound_node.end_byte()]
                                .parse::<i32>()
                                .unwrap(),
                        );
                    } else {
                        upper_bound = None;
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

fn parse_tuple_domain(tuple_domain: Node, source_code: &str) -> Domain {
    let mut domains: Vec<Domain> = Vec::new();
    for domain in named_children(&tuple_domain) {
        domains.push(parse_domain(domain, source_code));
    }
    Domain::DomainTuple(domains)
}

fn parse_matrix_domain(matrix_domain: Node, source_code: &str) -> Domain {
    let mut domains: Vec<Domain> = Vec::new();
    let index_domain_list = matrix_domain
        .child_by_field_name("index_domain_list")
        .expect("No index domains found for matrix domain");
    for domain in named_children(&index_domain_list) {
        domains.push(parse_domain(domain, source_code));
    }
    let value_domain = parse_domain(
        matrix_domain
            .child_by_field_name("value_domain")
            .expect("No value domain found for matrix domain"),
        source_code,
    );
    Domain::DomainMatrix(Box::new(value_domain), domains)
}
