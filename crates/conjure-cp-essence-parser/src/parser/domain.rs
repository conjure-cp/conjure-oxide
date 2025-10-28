use super::util::named_children;
use crate::EssenceParseError;
use conjure_cp_core::ast::{Domain, Name, Range, RecordEntry};
use tree_sitter::Node;

/// Parse an Essence variable domain into its Conjure AST representation.
pub fn parse_domain(domain: Node, source_code: &str) -> Result<Domain, EssenceParseError> {
    match domain.kind() {
        "domain" => parse_domain(domain.child(0).expect("No domain found"), source_code),
        "bool_domain" => Ok(Domain::Bool),
        "int_domain" => Ok(parse_int_domain(domain, source_code)),
        "identifier" => {
            let variable_name = &source_code[domain.start_byte()..domain.end_byte()];
            Ok(Domain::Reference(Name::user(variable_name)))
        }
        "tuple_domain" => parse_tuple_domain(domain, source_code),
        "matrix_domain" => parse_matrix_domain(domain, source_code),
        "record_domain" => parse_record_domain(domain, source_code),
        "set_domain" => parse_set_domain(domain, source_code),
        _ => panic!("{} is not a supported domain type", domain.kind()),
    }
}

/// Parse an integer domain. Can be a single integer or a range.
fn parse_int_domain(int_domain: Node, source_code: &str) -> Domain {
    if int_domain.child_count() == 1 {
        Domain::Int(vec![Range::Bounded(i32::MIN, i32::MAX)])
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
                        (Some(lb), None) => ranges.push(Range::Bounded(lb, i32::MAX)),
                        (None, Some(ub)) => ranges.push(Range::Bounded(i32::MIN, ub)),
                        _ => panic!("Unsupported int range type"),
                    }
                }
                _ => panic!("unsupported int range type"),
            }
        }
        Domain::Int(ranges)
    }
}

fn parse_tuple_domain(tuple_domain: Node, source_code: &str) -> Result<Domain, EssenceParseError> {
    let mut domains: Vec<Domain> = Vec::new();
    for domain in named_children(&tuple_domain) {
        domains.push(parse_domain(domain, source_code)?);
    }
    Ok(Domain::Tuple(domains))
}

fn parse_matrix_domain(
    matrix_domain: Node,
    source_code: &str,
) -> Result<Domain, EssenceParseError> {
    let mut domains: Vec<Domain> = Vec::new();
    let index_domain_list = matrix_domain
        .child_by_field_name("index_domain_list")
        .expect("No index domains found for matrix domain");
    for domain in named_children(&index_domain_list) {
        domains.push(parse_domain(domain, source_code)?);
    }
    let value_domain = parse_domain(
        matrix_domain.child_by_field_name("value_domain").ok_or(
            EssenceParseError::syntax_error(
                "Expected a value domain".to_string(),
                Some(matrix_domain.range()),
            ),
        )?,
        source_code,
    )?;
    Ok(Domain::Matrix(Box::new(value_domain), domains))
}

fn parse_record_domain(
    record_domain: Node,
    source_code: &str,
) -> Result<Domain, EssenceParseError> {
    let mut record_entries: Vec<RecordEntry> = Vec::new();
    for record_entry in named_children(&record_domain) {
        let name_node = record_entry
            .child_by_field_name("name")
            .expect("No name found for record entry");
        let name = Name::user(&source_code[name_node.start_byte()..name_node.end_byte()]);
        let domain_node = record_entry
            .child_by_field_name("domain")
            .expect("No domain found for record entry");
        let domain = parse_domain(domain_node, source_code)?;
        record_entries.push(RecordEntry { name, domain });
    }
    Ok(Domain::Record(record_entries))
}

fn parse_set_domain(
    set_domain: Node,
    source_code: &str,
) -> Result<Domain, EssenceParseError> {
    let set_attribute = None;
    
    for attribute_or_domain in named_children(&set_domain) {
        if attribute_or_domain.kind() == "set_attribute" {
            let attribute = attribute_or_domain.child_by_field_name("attribute").ok_or(
                EssenceParseError::syntax_error(
                    "Expected attribute for set attribute".to_string(),
                    Some(attribute_or_domain.range()),
                ),
            )?;
            let attribute_str = &source_code[attribute.start_byte()..attribute.end_byte()];
            let attribute_value_node = attribute_or_domain.child_by_field_name("attribute_value").ok_or(
                EssenceParseError::syntax_error(
                    "Expected attribute_value for set attribute".to_string(),
                    Some(attribute_or_domain.range()),
                ),
            )?;
            let attribute_value = &source_code[attribute_value_node.start_byte()..attribute_value_node.end_byte()];
            switch attribute_str {
                "size" => set_attribute = SetAttr::Size(i32::from_str(attribute_value))?,
                "minSize" => set_attribute = SetAttr::MinSize(i32::from_str(attribute_value))?,
                "maxSize" => set_attribute = SetAttr::MaxSize(i32::from_str(attribute_value))?,
            }
        } else {
            let domain = parse_domain(attribute_or_domain, source_code)?;
            return Ok(Domain::Set(set_attribute, Box::new(domain)));
        }
    }
}
