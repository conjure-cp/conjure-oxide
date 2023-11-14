use crate::ast::{DecisionVariable, Domain, Model, Name, Range};
use crate::error::{Error, Result};
use serde_json::Value as JsonValue;

use Error::ModelConstructError as CError;

pub fn parse_json(str: &String) -> Result<Model> {
    let mut m = Model::new();
    let v: JsonValue = serde_json::from_str(str)?;
    let constraints = v["mStatements"]
        .as_array()
        .ok_or(CError("mStatements is not an array".to_owned()))?;

    for con in constraints {
        let entry = con
            .as_object()
            .ok_or(CError("mStatements contains a non-object".to_owned()))?
            .iter()
            .next()
            .ok_or(CError("mStatements contains an empty object".to_owned()))?;
        match entry.0.as_str() {
            "Declaration" => {
                let (name, var) = parse_variable(entry.1)?;
                m.add_variable(name, var);
            }
            "SuchThat" => parse_constraint(entry.1)?,
            _ => return Err(CError("mStatements contains an unknown object".to_owned())),
        }
    }

    Ok(m)
}

fn parse_variable(v: &JsonValue) -> Result<(Name, DecisionVariable)> {
    let arr = v
        .as_object()
        .ok_or(CError("Declaration is not an object".to_owned()))?["FindOrGiven"]
        .as_array()
        .ok_or(CError("FindOrGiven is not an array".to_owned()))?;
    let name = arr[1]
        .as_object()
        .ok_or(CError("FindOrGiven[1] is not an object".to_owned()))?["Name"]
        .as_str()
        .ok_or(CError("FindOrGiven[1].Name is not a string".to_owned()))?;
    let name = Name::UserName(name.to_owned());
    let domain = arr[2]
        .as_object()
        .ok_or(CError("FindOrGiven[2] is not an object".to_owned()))?
        .iter()
        .next()
        .ok_or(CError("FindOrGiven[2] is an empty object".to_owned()))?;
    let domain = match domain.0.as_str() {
        "DomainInt" => Ok(parse_int_domain(domain.1)?),
        "DomainBool" => Ok(Domain::BoolDomain),
        _ => Err(CError("FindOrGiven[2] is an unknown object".to_owned())),
    }?;
    Ok((name, DecisionVariable { domain }))
}

fn parse_int_domain(v: &JsonValue) -> Result<Domain> {
    let mut ranges = Vec::new();
    let arr = v
        .as_array()
        .ok_or(CError("DomainInt is not an array".to_owned()))?[1]
        .as_array()
        .ok_or(CError("DomainInt[1] is not an array".to_owned()))?;
    for range in arr {
        let range = range
            .as_object()
            .ok_or(CError("DomainInt[1] contains a non-object".to_owned()))?
            .iter()
            .next()
            .ok_or(CError("DomainInt[1] contains an empty object".to_owned()))?;
        match range.0.as_str() {
            "RangeBounded" => {
                let arr = range
                    .1
                    .as_array()
                    .ok_or(CError("RangeBounded is not an array".to_owned()))?;
                let mut nums = Vec::new();
                for i in 0..2 {
                    let num = &arr[i]["Constant"]["ConstantInt"][1];
                    let num = num
                        .as_i64()
                        .ok_or(CError("ConstantInt[1] is not a number".to_owned()))?;
                    nums.push(num);
                }
                ranges.push(Range::Bounded(nums[0], nums[1]));
            }
            "RangeSingle" => {
                let num = &range.1["Constant"]["ConstantInt"][1];
                let num = num
                    .as_i64()
                    .ok_or(CError("ConstantInt[1] is not a number".to_owned()))?;
                ranges.push(Range::Single(num));
            }
            _ => return Err(CError("DomainInt[1] contains an unknown object".to_owned())),
        }
    }
    println!("IntDomain: {:?}", ranges);
    Ok(Domain::IntDomain(ranges))
}

fn parse_constraint(obj: &JsonValue) -> Result<()> {
    println!("Constraint");
    Ok(())
}

impl Model {
    pub fn from_json(str: &String) -> Result<Model> {
        parse_json(str)
    }
}
