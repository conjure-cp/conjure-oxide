use std::collections::HashMap;

use serde_json::Value;

use crate::ast::{DecisionVariable, Domain, Expression, Model, Name, Range};
use crate::error::{Error, Result};
use serde_json::Value as JsonValue;

pub fn parse_json(str: &str) -> Result<Model> {
    let mut m = Model::new();
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
                let constraints: Vec<Expression> = entry
                    .1
                    .as_array()
                    .unwrap()
                    .iter()
                    .flat_map(parse_expression)
                    .collect();
                m.constraints.extend(constraints);
                // println!("Nb constraints {}", m.constraints.len());
            }
            otherwise => panic!("Unhandled 0 {:#?}", otherwise),
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

fn parse_expression(obj: &JsonValue) -> Option<Expression> {
    // this needs an explicit type signature to force the closures to have the same type
    type BinOp = Box<dyn Fn(Box<Expression>, Box<Expression>) -> Expression>;
    let binary_operators: HashMap<&str, BinOp> = [
        ("MkOpEq", Box::new(Expression::Eq) as Box<dyn Fn(_, _) -> _>),
        (
            "MkOpNeq",
            Box::new(Expression::Neq) as Box<dyn Fn(_, _) -> _>,
        ),
        (
            "MkOpGeq",
            Box::new(Expression::Geq) as Box<dyn Fn(_, _) -> _>,
        ),
        (
            "MkOpLeq",
            Box::new(Expression::Leq) as Box<dyn Fn(_, _) -> _>,
        ),
        ("MkOpGt", Box::new(Expression::Gt) as Box<dyn Fn(_, _) -> _>),
        ("MkOpLt", Box::new(Expression::Lt) as Box<dyn Fn(_, _) -> _>),
    ]
    .into_iter()
    .collect();

    let mut binary_operator_names = binary_operators.iter().map(|x| x.0);

    match obj {
        Value::Object(op) if op.contains_key("Op") => {
            match &op["Op"] {
                Value::Object(bin_op)
                    if binary_operator_names.any(|key| bin_op.contains_key(*key)) =>
                {
                    // we know there is a single key value pair in this object
                    // extract the value, ignore the key
                    let (key, value) = bin_op.into_iter().next()?;

                    let constructor = binary_operators.get(key.as_str())?;

                    match &value {
                        Value::Array(bin_op_args) if bin_op_args.len() == 2 => {
                            let arg1 = parse_expression(&bin_op_args[0])?;
                            let arg2 = parse_expression(&bin_op_args[1])?;
                            Some(constructor(Box::new(arg1), Box::new(arg2)))
                        }
                        otherwise => panic!("Unhandled 3 {:#?}", otherwise),
                    }
                }
                Value::Object(op_sum) if op_sum.contains_key("MkOpSum") => {
                    let args = &op_sum["MkOpSum"]["AbstractLiteral"]["AbsLitMatrix"][1];
                    let args_parsed: Vec<Expression> = args
                        .as_array()?
                        .iter()
                        .map(|x| parse_expression(x).unwrap())
                        .collect();
                    Some(Expression::Sum(args_parsed))
                }
                otherwise => panic!("Unhandled 2 {:#?}", otherwise),
            }
        }
        Value::Object(refe) if refe.contains_key("Reference") => {
            let name = refe["Reference"].as_array()?[0].as_object()?["Name"].as_str()?;
            Some(Expression::Reference(Name::UserName(name.to_string())))
        }
        Value::Object(constant) if constant.contains_key("Constant") => {
            match &constant["Constant"] {
                Value::Object(int) if int.contains_key("ConstantInt") => {
                    Some(Expression::ConstantInt(
                        int["ConstantInt"].as_array()?[1]
                            .as_i64()?
                            .try_into()
                            .unwrap(),
                    ))
                }
                otherwise => panic!("Unhandled 4 {:#?}", otherwise),
            }
        }
        otherwise => panic!("Unhandled 1 {:#?}", otherwise),
    }
}

impl Model {
    pub fn from_json(str: &str) -> Result<Model> {
        parse_json(str)
    }
}
