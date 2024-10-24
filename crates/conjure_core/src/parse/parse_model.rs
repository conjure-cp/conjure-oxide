use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use serde_json::Value;
use serde_json::Value as JsonValue;

use crate::ast::{Constant, DecisionVariable, Domain, Expression, Name, Range};
use crate::context::Context;
use crate::error::{Error, Result};
use crate::metadata::Metadata;
use crate::Model;

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

/// Triggers a panic with a detailed bug report message, while ensuring the panic is ignored in coverage reports.
///
/// This macro is useful in situations where an unreachable code path is hit or when a bug occurs.
///
/// # Parameters
///
/// - `msg`: A string expression describing the cause of the panic or bug.
///
/// ```
macro_rules! bug {
    ($msg:expr) => {{
        let full_message = format!(
            r#"
This should never happen, sorry!

However, it did happen, so it must be a bug. Please report it to us!

Conjure Oxide is actively developed and maintained. We will get back to you as soon as possible.

You can help us by providing a minimal failing example.

Issue tracker: http://github.com/conjure-cp/conjure-oxide/issues

{}
"#,
            $msg
        );

        let f = #[coverage(off)]
        || {
            panic!("{}", full_message);
        };
        f()
    }};
}

pub fn model_from_json(str: &str, context: Arc<RwLock<Context<'static>>>) -> Result<Model> {
    let mut m = Model::new_empty(context);
    let v: JsonValue = serde_json::from_str(str)?;
    let statements = v["mStatements"]
        .as_array()
        .ok_or(Error::Parse("mStatements is not an array".to_owned()))?;

    for statement in statements {
        let entry = statement
            .as_object()
            .ok_or(Error::Parse("mStatements contains a non-object".to_owned()))?
            .iter()
            .next()
            .ok_or(Error::Parse(
                "mStatements contains an empty object".to_owned(),
            ))?;
        match entry.0.as_str() {
            "Declaration" => {
                let (name, var) = parse_variable(entry.1)?;
                m.add_variable(name, var);
            }
            "SuchThat" => {
                let constraints_arr = match entry.1.as_array() {
                    Some(x) => x,
                    None => bug!("SuchThat is not a vector"),
                };

                let constraints: Vec<Expression> =
                    constraints_arr.iter().flat_map(parse_expression).collect();
                m.add_constraints(constraints);
                // println!("Nb constraints {}", m.constraints.len());
            }
            otherwise => panic!("Unhandled Statement {:#?}", otherwise),
        }
    }

    Ok(m)
}

fn parse_variable(v: &JsonValue) -> Result<(Name, DecisionVariable)> {
    let arr = v
        .as_object()
        .ok_or(Error::Parse("Declaration is not an object".to_owned()))?["FindOrGiven"]
        .as_array()
        .ok_or(Error::Parse("FindOrGiven is not an array".to_owned()))?;
    let name = arr[1]
        .as_object()
        .ok_or(Error::Parse("FindOrGiven[1] is not an object".to_owned()))?["Name"]
        .as_str()
        .ok_or(Error::Parse(
            "FindOrGiven[1].Name is not a string".to_owned(),
        ))?;
    let name = Name::UserName(name.to_owned());
    let domain = arr[2]
        .as_object()
        .ok_or(Error::Parse("FindOrGiven[2] is not an object".to_owned()))?
        .iter()
        .next()
        .ok_or(Error::Parse("FindOrGiven[2] is an empty object".to_owned()))?;
    let domain = match domain.0.as_str() {
        "DomainInt" => Ok(parse_int_domain(domain.1)?),
        "DomainBool" => Ok(Domain::BoolDomain),
        _ => Err(Error::Parse(
            "FindOrGiven[2] is an unknown object".to_owned(),
        )),
    }?;
    Ok((name, DecisionVariable { domain }))
}

fn parse_int_domain(v: &JsonValue) -> Result<Domain> {
    let mut ranges = Vec::new();
    let arr = v
        .as_array()
        .ok_or(Error::Parse("DomainInt is not an array".to_owned()))?[1]
        .as_array()
        .ok_or(Error::Parse("DomainInt[1] is not an array".to_owned()))?;
    for range in arr {
        let range = range
            .as_object()
            .ok_or(Error::Parse(
                "DomainInt[1] contains a non-object".to_owned(),
            ))?
            .iter()
            .next()
            .ok_or(Error::Parse(
                "DomainInt[1] contains an empty object".to_owned(),
            ))?;
        match range.0.as_str() {
            "RangeBounded" => {
                let arr = range
                    .1
                    .as_array()
                    .ok_or(Error::Parse("RangeBounded is not an array".to_owned()))?;
                let mut nums = Vec::new();
                for item in arr.iter() {
                    let num = item["Constant"]["ConstantInt"][1]
                        .as_i64()
                        .ok_or(Error::Parse(
                            "Could not parse int domain constant".to_owned(),
                        ))?;
                    let num32 = i32::try_from(num).map_err(|_| {
                        Error::Parse("Could not parse int domain constant".to_owned())
                    })?;
                    nums.push(num32);
                }
                ranges.push(Range::Bounded(nums[0], nums[1]));
            }
            "RangeSingle" => {
                let num = &range.1["Constant"]["ConstantInt"][1]
                    .as_i64()
                    .ok_or(Error::Parse(
                        "Could not parse int domain constant".to_owned(),
                    ))?;
                let num32 = i32::try_from(*num)
                    .map_err(|_| Error::Parse("Could not parse int domain constant".to_owned()))?;
                ranges.push(Range::Single(num32));
            }
            _ => {
                return Err(Error::Parse(
                    "DomainInt[1] contains an unknown object".to_owned(),
                ))
            }
        }
    }
    Ok(Domain::IntDomain(ranges))
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
    ]
    .into_iter()
    .collect();

    let unary_operators: HashMap<&str, UnaryOp> = [(
        "MkOpNot",
        Box::new(Expression::Not) as Box<dyn Fn(_, _) -> _>,
    )]
    .into_iter()
    .collect();

    let vec_operators: HashMap<&str, VecOp> = [
        (
            "MkOpSum",
            Box::new(Expression::Sum) as Box<dyn Fn(_, _) -> _>,
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

    let mut binary_operator_names = binary_operators.iter().map(|x| x.0);
    let mut unary_operator_names = unary_operators.iter().map(|x| x.0);
    let mut vec_operator_names = vec_operators.iter().map(|x| x.0);

    match obj {
        Value::Object(op) if op.contains_key("Op") => match &op["Op"] {
            Value::Object(bin_op) if binary_operator_names.any(|key| bin_op.contains_key(*key)) => {
                parse_bin_op(bin_op, binary_operators)
            }
            Value::Object(un_op) if unary_operator_names.any(|key| un_op.contains_key(*key)) => {
                parse_unary_op(un_op, unary_operators)
            }
            Value::Object(vec_op) if vec_operator_names.any(|key| vec_op.contains_key(*key)) => {
                parse_vec_op(vec_op, vec_operators)
            }
            otherwise => panic!("Unhandled Op {:#?}", otherwise),
        },
        Value::Object(refe) if refe.contains_key("Reference") => {
            let name = refe["Reference"].as_array()?[0].as_object()?["Name"].as_str()?;
            Some(Expression::Reference(
                Metadata::new(),
                Name::UserName(name.to_string()),
            ))
        }
        Value::Object(constant) if constant.contains_key("Constant") => parse_constant(constant),
        Value::Object(constant) if constant.contains_key("ConstantInt") => parse_constant(constant),
        Value::Object(constant) if constant.contains_key("ConstantBool") => {
            parse_constant(constant)
        }
        otherwise => panic!("Unhandled Expression {:#?}", otherwise),
    }
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
        otherwise => panic!("Unhandled parse_bin_op {:#?}", otherwise),
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

            Some(Expression::Constant(Metadata::new(), Constant::Int(int_32)))
        }

        Some(Value::Object(b)) if b.contains_key("ConstantBool") => {
            let b: bool = b["ConstantBool"].as_bool().unwrap();
            Some(Expression::Constant(Metadata::new(), Constant::Bool(b)))
        }

        // sometimes (e.g. constant matrices) we can have a ConstantInt / Constant bool that is
        // not wrapped in Constant
        None => {
            let int_expr = constant["ConstantInt"]
                .as_array()
                .and_then(|x| x[1].as_i64())
                .and_then(|x| x.try_into().ok())
                .map(|x| Expression::Constant(Metadata::new(), Constant::Int(x)));

            if let e @ Some(_) = int_expr {
                return e;
            }

            let bool_expr = constant["ConstantBool"]
                .as_bool()
                .map(|x| Expression::Constant(Metadata::new(), Constant::Bool(x)));

            if let e @ Some(_) = bool_expr {
                return e;
            }

            panic!("Unhandled parse_constant {:#?}", constant);
        }
        otherwise => panic!("Unhandled parse_constant {:#?}", otherwise),
    }
}
