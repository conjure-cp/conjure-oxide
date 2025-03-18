#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, RwLock};
use log::kv::value;
use serde_json::Value;
use serde_json::Value as JsonValue;

use crate::ast::Declaration;
use crate::ast::{
    AbstractLiteral, Atom, Domain, Expression, Literal, Name, Range, SetAttr, SymbolTable,
};
use crate::context::Context;
use crate::error::{Error, Result};
use crate::metadata::Metadata;
use crate::{bug, error, into_matrix_expr, throw_error, Model};
macro_rules! parser_trace {
    ($($arg:tt)+) => {
        log::trace!(target:"jsonparser",$($arg)+)
    };
}

macro_rules! parser_debug {
    ($($arg:tt)+) => {
        log::debug!(target:"jsonparser",$($arg)+)
    };
}

pub fn model_from_json(str: &str, context: Arc<RwLock<Context<'static>>>) -> Result<Model> {
    let mut m = Model::new(context);
    let v: JsonValue = serde_json::from_str(str)?;
    let statements = v["mStatements"]
        .as_array()
        .ok_or(error!("mStatements is not an array"))?;

    for statement in statements {
        let entry = statement
            .as_object()
            .ok_or(error!("mStatements contains a non-object"))?
            .iter()
            .next()
            .ok_or(error!("mStatements contains an empty object"))?;

        match entry.0.as_str() {
            "Declaration" => {
                let decl = entry
                    .1
                    .as_object()
                    .ok_or(error!("Declaration is not an object".to_owned()))?;

                // One field in the declaration should tell us what kind it is.
                //
                // Find it, ignoring the other fields.
                //
                // e.g. FindOrGiven,

                let mut valid_decl: bool = false;
                let submodel = m.as_submodel_mut();
                for (kind, value) in decl {
                    match kind.as_str() {
                        "FindOrGiven" => {
                            parse_variable(value, &mut submodel.symbols_mut())?;
                            valid_decl = true;
                            break;
                        }
                        "Letting" => {
                            parse_letting(value, &mut submodel.symbols_mut())?;
                            valid_decl = true;
                            break;
                        }
                        _ => continue,
                    }
                }

                if !valid_decl {
                    throw_error!("Declaration is not a valid kind")?;
                }
            }
            "SuchThat" => {
                let constraints_arr = match entry.1.as_array() {
                    Some(x) => x,
                    None => bug!("SuchThat is not a vector"),
                };

                let constraints: Vec<Expression> = constraints_arr
                    .iter()
                    .map(|x| parse_expression(x).unwrap())
                    .collect();
                m.as_submodel_mut().add_constraints(constraints);
                // println!("Nb constraints {}", m.constraints.len());
            }
            otherwise => bug!("Unhandled Statement {:#?}", otherwise),
        }
    }

    Ok(m)
}

fn parse_variable(v: &JsonValue, symtab: &mut SymbolTable) -> Result<()> {
    let arr = v.as_array().ok_or(error!("FindOrGiven is not an array"))?;
    let name = arr[1]
        .as_object()
        .ok_or(error!("FindOrGiven[1] is not an object"))?["Name"]
        .as_str()
        .ok_or(error!("FindOrGiven[1].Name is not a string"))?;

    let name = Name::UserName(name.to_owned());

    let domain = arr[2]
        .as_object()
        .ok_or(error!("FindOrGiven[2] is not an object"))?
        .iter()
        .next()
        .ok_or(error!("FindOrGiven[2] is an empty object"))?;

    let domain = parse_domain(domain.0, domain.1)?;

    symtab
        .insert(Rc::new(Declaration::new_var(name.clone(), domain)))
        .ok_or(Error::Parse(format!(
            "Could not add {name} to symbol table as it already exists"
        )))
}

fn parse_letting(v: &JsonValue, symtab: &mut SymbolTable) -> Result<()> {
    let arr = v.as_array().ok_or(error!("Letting is not an array"))?;
    let name = arr[0]
        .as_object()
        .ok_or(error!("Letting[0] is not an object"))?["Name"]
        .as_str()
        .ok_or(error!("Letting[0].Name is not a string"))?;
    let name = Name::UserName(name.to_owned());
    // value letting
    if let Some(value) = parse_expression(&arr[1]) {
        symtab
            .insert(Rc::new(Declaration::new_value_letting(name.clone(), value)))
            .ok_or(Error::Parse(format!(
                "Could not add {name} to symbol table as it already exists"
            )))
    } else {
        // domain letting
        let domain = &arr[1]
            .as_object()
            .ok_or(error!("Letting[1] is not an object".to_owned()))?["Domain"]
            .as_object()
            .ok_or(error!("Letting[1].Domain is not an object"))?
            .iter()
            .next()
            .ok_or(error!("Letting[1].Domain is an empty object"))?;

        let domain = parse_domain(domain.0, domain.1)?;

        symtab
            .insert(Rc::new(Declaration::new_domain_letting(
                name.clone(),
                domain,
            )))
            .ok_or(Error::Parse(format!(
                "Could not add {name} to symbol table as it already exists"
            )))
    }
}

fn parse_domain(domain_name: &str, domain_value: &JsonValue) -> Result<Domain> {
    match domain_name {
        "DomainInt" => Ok(parse_int_domain(domain_value)?),
        "DomainBool" => Ok(Domain::BoolDomain),
        "DomainReference" => Ok(Domain::DomainReference(Name::UserName(
            domain_value
                .as_array()
                .ok_or(error!("DomainReference is not an array"))?[0]
                .as_object()
                .ok_or(error!("DomainReference[0] is not an object"))?["Name"]
                .as_str()
                .ok_or(error!("DomainReference[0].Name is not a string"))?
                .into(),
        ))),
        "DomainSet" => {
            let dom = domain_value.get(2).and_then(|v| v.as_object());
            let domain_obj = dom.expect("domain object exists");
            let domain = domain_obj
                .iter()
                .next()
                .ok_or(Error::Parse("DomainSet is an empty object".to_owned()))?;
            let domain = match domain_name {
                "DomainInt" => {
                    println!("DomainInt: {:#?}", domain.1);
                    Ok(parse_int_domain(domain.1)?)
                }
                "DomainBool" => Ok(Domain::BoolDomain),
                _ => Err(Error::Parse(
                    "FindOrGiven[2] is an unknown object".to_owned(),
                )),
            }?;
            print!("{:?}", domain);
            Ok(Domain::DomainSet(SetAttr::None, Box::new(domain)))
        }

        "DomainMatrix" => {
            let domain_value = domain_value
                .as_array()
                .ok_or(error!("Domain matrix is not an array"))?;

            let indexed_by_domain = domain_value[0].clone();
            let (index_domain_name, index_domain_value) = indexed_by_domain
                .as_object()
                .ok_or(error!("DomainMatrix[0] is not an object"))?
                .iter()
                .next()
                .ok_or(error!(""))?;

            let (value_domain_name, value_domain_value) = domain_value[1]
                .as_object()
                .ok_or(error!(""))?
                .iter()
                .next()
                .ok_or(error!(""))?;

            // Conjure stores a 2-d matrix as a matrix of a matrix.
            //
            // Therefore, the index is always a Domain.

            let mut index_domains: Vec<Domain> = vec![];

            index_domains.push(parse_domain(index_domain_name, index_domain_value)?);

            // We want to store 2-d matrices as a matrix with two index domains, not a matrix in a
            // matrix.
            //
            // Walk through the value domain until it is not a DomainMatrix, adding the index to
            // our list of indices.
            let mut value_domain = parse_domain(value_domain_name, value_domain_value)?;
            while let Domain::DomainMatrix(new_value_domain, mut indices) = value_domain {
                index_domains.append(&mut indices);
                value_domain = *new_value_domain.clone()
            }

            Ok(Domain::DomainMatrix(Box::new(value_domain), index_domains))
        }

        _ => Err(Error::Parse(
            "FindOrGiven[2] is an unknown object".to_owned(), // consider covered
        )),
    }
}

fn parse_int_domain(v: &JsonValue) -> Result<Domain> {
    let mut ranges = Vec::new();
    let arr = v
        .as_array()
        .ok_or(error!("DomainInt is not an array".to_owned()))?[1]
        .as_array()
        .ok_or(error!("DomainInt[1] is not an array".to_owned()))?;
    for range in arr {
        let range = range
            .as_object()
            .ok_or(error!("DomainInt[1] contains a non-object"))?
            .iter()
            .next()
            .ok_or(error!("DomainInt[1] contains an empty object"))?;
        match range.0.as_str() {
            "RangeBounded" => {
                let arr = range
                    .1
                    .as_array()
                    .ok_or(error!("RangeBounded is not an array".to_owned()))?;
                let mut nums = Vec::new();
                for item in arr.iter() {
                    let num = parse_domain_value_int(item)
                        .ok_or(error!("Could not parse int domain constant"))?;
                    nums.push(num);
                }
                ranges.push(Range::Bounded(nums[0], nums[1]));
            }
            "RangeSingle" => {
                let num = parse_domain_value_int(range.1)
                    .ok_or(error!("Could not parse int domain constant"))?;
                ranges.push(Range::Single(num));
            }
            _ => return throw_error!("DomainInt[1] contains an unknown object"),
        }
    }
    Ok(Domain::IntDomain(ranges))
}

/// Parses a (possibly) integer value inside the range of a domain
///
/// 1. (positive number) Constant/ConstantInt/1
///
/// 2. (negative number) Op/MkOpNegate/Constant/ConstantInt/1
///
/// Unlike `parse_constant` this handles the negation operator. `parse_constant` expects the
/// negation to already have been handled as an expression; however, here we do not expect domain
/// values to be part of larger expressions, only negated.
///
fn parse_domain_value_int(obj: &JsonValue) -> Option<i32> {
    parser_trace!("trying to parse domain value: {}", obj);

    fn try_parse_positive_int(obj: &JsonValue) -> Option<i32> {
        parser_trace!(".. trying as a positive domain value: {}", obj);
        // Positive number: Constant/ConstantInt/1

        let leaf_node = obj
            .pointer("/Constant/ConstantInt/1")
            .or_else(|| obj.pointer("/ConstantInt/1"))?;

        match leaf_node.as_i64()?.try_into() {
            Ok(x) => {
                parser_trace!(".. success!");
                Some(x)
            }
            Err(_) => {
                println!(
                    "Could not convert integer constant to i32: {:#?}",
                    leaf_node
                );
                None
            }
        }
    }

    fn try_parse_negative_int(obj: &JsonValue) -> Option<i32> {
        // Negative number: Op/MkOpNegate/Constant/ConstantInt/1

        // Unwrap negation operator, giving us a Constant/ConstantInt/1
        //
        // This is just a positive constant, so try to parse it as such

        parser_trace!(".. trying as a negative domain value: {}", obj);
        let inner_constant_node = obj.pointer("/Op/MkOpNegate")?;
        let inner_num = try_parse_positive_int(inner_constant_node)?;

        parser_trace!(".. success!");
        Some(-inner_num)
    }

    try_parse_positive_int(obj).or_else(|| try_parse_negative_int(obj))
}

// this needs an explicit type signature to force the closures to have the same type
type BinOp = Box<dyn Fn(Metadata, Box<Expression>, Box<Expression>) -> Expression>;
type UnaryOp = Box<dyn Fn(Metadata, Box<Expression>) -> Expression>;
type VecOp = Box<dyn Fn(Metadata, Vec<Expression>) -> Expression>;

fn parse_expression(obj: &JsonValue) -> Option<Expression> {
    let binary_operators: HashMap<&str, BinOp> = [
        (
            "MkOpEq",
            Box::new(Expression::Eq) as Box<dyn Fn(_, _, _) -> _>,
        ),
        (
            "MkOpNeq",
            Box::new(Expression::Neq) as Box<dyn Fn(_, _, _) -> _>,
        ),
        (
            "MkOpGeq",
            Box::new(Expression::Geq) as Box<dyn Fn(_, _, _) -> _>,
        ),
        (
            "MkOpLeq",
            Box::new(Expression::Leq) as Box<dyn Fn(_, _, _) -> _>,
        ),
        (
            "MkOpGt",
            Box::new(Expression::Gt) as Box<dyn Fn(_, _, _) -> _>,
        ),
        (
            "MkOpLt",
            Box::new(Expression::Lt) as Box<dyn Fn(_, _, _) -> _>,
        ),
        (
            "MkOpGt",
            Box::new(Expression::Gt) as Box<dyn Fn(_, _, _) -> _>,
        ),
        (
            "MkOpLt",
            Box::new(Expression::Lt) as Box<dyn Fn(_, _, _) -> _>,
        ),
        (
            "MkOpDiv",
            Box::new(Expression::UnsafeDiv) as Box<dyn Fn(_, _, _) -> _>,
        ),
        (
            "MkOpMod",
            Box::new(Expression::UnsafeMod) as Box<dyn Fn(_, _, _) -> _>,
        ),
        (
            "MkOpMinus",
            Box::new(Expression::Minus) as Box<dyn Fn(_, _, _) -> _>,
        ),
        (
            "MkOpImply",
            Box::new(Expression::Imply) as Box<dyn Fn(_, _, _) -> _>,
        ),
        (
            "MkOpPow",
            Box::new(Expression::UnsafePow) as Box<dyn Fn(_, _, _) -> _>,
        ),
    ]
    .into_iter()
    .collect();

    let unary_operators: HashMap<&str, UnaryOp> = [
        (
            "MkOpNot",
            Box::new(Expression::Not) as Box<dyn Fn(_, _) -> _>,
        ),
        (
            "MkOpNegate",
            Box::new(Expression::Neg) as Box<dyn Fn(_, _) -> _>,
        ),
        (
            "MkOpTwoBars",
            Box::new(Expression::Abs) as Box<dyn Fn(_, _) -> _>,
        ),
        (
            "MkOpAnd",
            Box::new(Expression::And) as Box<dyn Fn(_, _) -> _>,
        ),
        ("MkOpOr", Box::new(Expression::Or) as Box<dyn Fn(_, _) -> _>),
        (
            "MkOpMin",
            Box::new(Expression::Min) as Box<dyn Fn(_, _) -> _>,
        ),
        (
            "MkOpMax",
            Box::new(Expression::Max) as Box<dyn Fn(_, _) -> _>,
        ),
        (
            "MkOpAllDiff",
            Box::new(Expression::AllDiff) as Box<dyn Fn(_, _) -> _>,
        ),
    ]
    .into_iter()
    .collect();

    let vec_operators: HashMap<&str, VecOp> = [
        (
            "MkOpSum",
            Box::new(Expression::Sum) as Box<dyn Fn(_, _) -> _>,
        ),
        (
            "MkOpProduct",
            Box::new(Expression::Product) as Box<dyn Fn(_, _) -> _>,
        ),
    ]
    .into_iter()
    .collect();

    let mut binary_operator_names = binary_operators.iter().map(|x| x.0);
    let mut unary_operator_names = unary_operators.iter().map(|x| x.0);
    let mut vec_operator_names = vec_operators.iter().map(|x| x.0);
    #[allow(clippy::unwrap_used)]
    match obj {
        Value::Object(op) if op.contains_key("Op") => match &op["Op"] {
            Value::Object(bin_op) if binary_operator_names.any(|key| bin_op.contains_key(*key)) => {
                Some(parse_bin_op(bin_op, binary_operators).unwrap())
            }
            Value::Object(un_op) if unary_operator_names.any(|key| un_op.contains_key(*key)) => {
                Some(parse_unary_op(un_op, unary_operators).unwrap())
            }
            Value::Object(vec_op) if vec_operator_names.any(|key| vec_op.contains_key(*key)) => {
                Some(parse_vec_op(vec_op, vec_operators).unwrap())
            }

            Value::Object(op)
                if op.contains_key("MkOpIndexing") || op.contains_key("MkOpSlicing") =>
            {
                parse_indexing_slicing_op(op)
            }
            otherwise => bug!("Unhandled Op {:#?}", otherwise),
        },
        Value::Object(refe) if refe.contains_key("Reference") => {
            let name = refe["Reference"].as_array()?[0].as_object()?["Name"].as_str()?;
            Some(Expression::Atomic(
                Metadata::new(),
                Atom::Reference(Name::UserName(name.to_string())),
            ))
        }
        Value::Object(abslit) if abslit.contains_key("AbstractLiteral") => {
            if abslit["AbstractLiteral"].as_object()?.contains_key("AbsLitSet") {
                Some(parse_abs_lit(&abslit["AbstractLiteral"]["AbsLitSet"]).unwrap())
            } else {
                Some(parse_abstract_matrix_as_expr(obj).unwrap())
            }
        }

        Value::Object(constant) if constant.contains_key("Constant") => Some(
            parse_constant(constant)
                .or_else(|| parse_abstract_matrix_as_expr(obj))
                .unwrap(),
        ),

        Value::Object(constant) if constant.contains_key("ConstantAbstract") => {
            Some(parse_abstract_matrix_as_expr(obj).unwrap())
        }

        Value::Object(constant) if constant.contains_key("ConstantInt") => {
            Some(parse_constant(constant).unwrap())
        }
        Value::Object(constant) if constant.contains_key("ConstantBool") => {
            Some(parse_constant(constant).unwrap())
        }
        

        _ => None,
    }
}

fn parse_abs_lit(abs_set: &Value) -> Option<Expression> {
    let values = abs_set.as_array()?; // Ensure it's an array
    let expressions = values
        .iter()
        .map(parse_expression)
        .map(|x| x.expect("invalid subexpression")) // Ensure valid expressions
        .collect::<Vec<Expression>>(); // Collect all expressions

    Some(Expression::AbstractLiteral(
        Metadata::new(),
        AbstractLiteral::Set(expressions),
    ))
}


fn parse_bin_op(
    bin_op: &serde_json::Map<String, Value>,
    binary_operators: HashMap<&str, BinOp>,
) -> Option<Expression> {
    // we know there is a single key value pair in this object
    // extract the value, ignore the key
    let (key, value) = bin_op.into_iter().next()?;

    let constructor = binary_operators.get(key.as_str())?;

    match &value {
        Value::Array(bin_op_args) if bin_op_args.len() == 2 => {
            let arg1 = parse_expression(&bin_op_args[0])?;
            let arg2 = parse_expression(&bin_op_args[1])?;
            Some(constructor(Metadata::new(), Box::new(arg1), Box::new(arg2)))
        }
        otherwise => bug!("Unhandled parse_bin_op {:#?}", otherwise),
    }
}

fn parse_indexing_slicing_op(op: &serde_json::Map<String, Value>) -> Option<Expression> {
    // we know there is a single key value pair in this object
    // extract the value, ignore the key
    let (key, value) = op.into_iter().next()?;

    // we know that this is meant to be a mkopindexing, so anything that goes wrong from here is a
    // bug!

    // Conjure does a[1,2,3] as MkOpIndexing(MkOpIndexing(MkOpIndexing(a,3),2),1).
    //
    // And  a[1,..,3] as MkOpIndexing(MkOpSlicing(MkOpIndexing(a,3)),1).
    //
    // However, we want this in a flattened form: Index(a, [1,2,3])
    let mut target: Expression;
    let mut indices: Vec<Option<Expression>> = vec![];

    // true if this has no slicing, false otherwise.
    let mut all_known = true;

    match key.as_str() {
        "MkOpIndexing" => {
            match &value {
                Value::Array(op_args) if op_args.len() == 2 => {
                    target = parse_expression(&op_args[0]).expect("expected an expression");
                    indices.push(Some(
                        parse_expression(&op_args[1]).expect("expected an expression"),
                    ));
                }
                otherwise => bug!("Unknown object inside MkOpIndexing: {:#?}", otherwise),
            };
        }

        "MkOpSlicing" => {
            all_known = false;
            match &value {
                Value::Array(op_args) if op_args.len() == 3 => {
                    target = parse_expression(&op_args[0]).expect("expected an expression");
                    indices.push(None);
                }
                otherwise => bug!("Unknown object inside MkOpSlicing: {:#?}", otherwise),
            };
        }

        _ => {
            return None;
        }
    }

    loop {
        match &mut target {
            Expression::UnsafeIndex(_, new_target, new_indices) => {
                indices.extend(new_indices.iter().cloned().map(Some));
                target = *new_target.clone();
            }

            Expression::UnsafeSlice(_, new_target, new_indices) => {
                all_known = false;
                indices.append(new_indices);
                target = *new_target.clone();
            }

            _ => {
                // not a slice or an index, we have reached the target.
                break;
            }
        }
    }

    indices.reverse();

    if all_known {
        Some(Expression::UnsafeIndex(
            Metadata::new(),
            Box::new(target),
            indices.into_iter().map(|x| x.unwrap()).collect(),
        ))
    } else {
        Some(Expression::UnsafeSlice(
            Metadata::new(),
            Box::new(target),
            indices,
        ))
    }
}

fn parse_unary_op(
    un_op: &serde_json::Map<String, Value>,
    unary_operators: HashMap<&str, UnaryOp>,
) -> Option<Expression> {
    let (key, value) = un_op.into_iter().next()?;
    let constructor = unary_operators.get(key.as_str())?;

    let arg = parse_expression(value)?;
    Some(constructor(Metadata::new(), Box::new(arg)))
}

fn parse_vec_op(
    vec_op: &serde_json::Map<String, Value>,
    vec_operators: HashMap<&str, VecOp>,
) -> Option<Expression> {
    let (key, value) = vec_op.into_iter().next()?;
    let constructor = vec_operators.get(key.as_str())?;

    parser_debug!("Trying to parse vec_op: {key} ...");

    let mut args_parsed: Option<Vec<Option<Expression>>> = None;
    if let Some(abs_lit_matrix) = value.pointer("/AbstractLiteral/AbsLitMatrix/1") {
        parser_trace!("... containing a matrix of literals");
        args_parsed = abs_lit_matrix.as_array().map(|x| {
            x.iter()
                .map(parse_expression)
                .collect::<Vec<Option<Expression>>>()
        });
    }
    // the input of this expression is constant - e.g. or([]), or([false]), min([2]), etc.
    else if let Some(const_abs_lit_matrix) =
        value.pointer("/Constant/ConstantAbstract/AbsLitMatrix/1")
    {
        parser_trace!("... containing a matrix of constants");
        args_parsed = const_abs_lit_matrix.as_array().map(|x| {
            x.iter()
                .map(parse_expression)
                .collect::<Vec<Option<Expression>>>()
        });
    }

    let args_parsed = args_parsed?;

    let number_of_args = args_parsed.len();
    parser_debug!("... with {number_of_args} args {args_parsed:#?}");

    let valid_args: Vec<Expression> = args_parsed.into_iter().flatten().collect();
    if number_of_args != valid_args.len() {
        None
    } else {
        parser_debug!("... success!");
        Some(constructor(Metadata::new(), valid_args))
    }
}

// Takes in { AbstractLiteral: .... }
fn parse_abstract_matrix_as_expr(value: &serde_json::Value) -> Option<Expression> {
    parser_trace!("trying to parse an abstract literal matrix");
    let (values, domain_name, domain_value) =
        if let Some(abs_lit_matrix) = value.pointer("/AbstractLiteral/AbsLitMatrix") {
            parser_trace!(".. found JSON pointer /AbstractLiteral/AbstractLitMatrix");
            let (domain_name, domain_value) =
                abs_lit_matrix.pointer("/0")?.as_object()?.iter().next()?;
            let values = abs_lit_matrix.pointer("/1")?;

            Some((values, domain_name, domain_value))
        }
        // the input of this expression is constant - e.g. or([]), or([false]), min([2]), etc.
        else if let Some(const_abs_lit_matrix) =
            value.pointer("/Constant/ConstantAbstract/AbsLitMatrix")
        {
            parser_trace!(".. found JSON pointer /Constant/ConstantAbstract/AbsLitMatrix");
            let (domain_name, domain_value) = const_abs_lit_matrix
                .pointer("/0")?
                .as_object()?
                .iter()
                .next()?;
            let values = const_abs_lit_matrix.pointer("/1")?;

            Some((values, domain_name, domain_value))
        } else if let Some(const_abs_lit_matrix) = value.pointer("/ConstantAbstract/AbsLitMatrix") {
            parser_trace!(".. found JSON pointer /ConstantAbstract/AbsLitMatrix");
            let (domain_name, domain_value) = const_abs_lit_matrix
                .pointer("/0")?
                .as_object()?
                .iter()
                .next()?;
            let values = const_abs_lit_matrix.pointer("/1")?;
            Some((values, domain_name, domain_value))
        } else {
            None
        }?;

    parser_trace!(".. found in domain and values in JSON:");
    parser_trace!(".. .. index domain name {domain_name}");
    parser_trace!(".. .. values {value}");

    let args_parsed = values.as_array().map(|x| {
        x.iter()
            .map(parse_expression)
            .map(|x| x.expect("invalid subexpression"))
            .collect::<Vec<Expression>>()
    })?;

    if !args_parsed.is_empty() {
        parser_trace!(
            ".. successfully parsed values as expressions: {}, ... ",
            args_parsed[0]
        );
    } else {
        parser_trace!(".. successfully parsed empty values ",);
    }
    match parse_domain(domain_name, domain_value) {
        Ok(domain) => {
            parser_trace!("... sucessfully parsed domain as {domain}");
            Some(into_matrix_expr![args_parsed;domain])
        }
        Err(_) => {
            parser_trace!("... failed to parse domain, creating a matrix without one.");
            Some(into_matrix_expr![args_parsed])
        }
    }
}

fn parse_constant(constant: &serde_json::Map<String, Value>) -> Option<Expression> {
    match &constant.get("Constant") {
        Some(Value::Object(int)) if int.contains_key("ConstantInt") => {
            let int_32: i32 = match int["ConstantInt"].as_array()?[1].as_i64()?.try_into() {
                Ok(x) => x,
                Err(_) => {
                    println!(
                        "Could not convert integer constant to i32: {:#?}",
                        int["ConstantInt"]
                    );
                    return None;
                }
            };

            Some(Expression::Atomic(
                Metadata::new(),
                Atom::Literal(Literal::Int(int_32)),
            ))
        }

        Some(Value::Object(b)) if b.contains_key("ConstantBool") => {
            let b: bool = b["ConstantBool"].as_bool()?;
            Some(Expression::Atomic(
                Metadata::new(),
                Atom::Literal(Literal::Bool(b)),
            ))
        }

        Some(Value::Object(int)) if int.contains_key("ConstantAbstract") => {
            if let Some(Value::Object(obj)) = int.get("ConstantAbstract") {
                if let Some(arr) = obj.get("AbsLitSet") {
                    return parse_abs_lit(arr);
                }
            }
            None
        }

        // sometimes (e.g. constant matrices) we can have a ConstantInt / Constant bool that is
        // not wrapped in Constant
        None => {
            let int_expr = constant["ConstantInt"]
                .as_array()
                .and_then(|x| x[1].as_i64())
                .and_then(|x| x.try_into().ok())
                .map(|x| Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Int(x))));

            if let e @ Some(_) = int_expr {
                return e;
            }

            let bool_expr = constant["ConstantBool"]
                .as_bool()
                .map(|x| Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Bool(x))));

            if let e @ Some(_) = bool_expr {
                return e;
            }

            bug!("Unhandled parse_constant {:#?}", constant);
        }
        otherwise => bug!("Unhandled parse_constant {:#?}", otherwise),
    }
}
