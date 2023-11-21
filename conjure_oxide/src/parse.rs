use crate::ast::{DecisionVariable, Domain, Model, Name, Range};
use crate::error::{Error, Result};
use serde_json::Value as JsonValue;

pub fn parse_json(str: &str) -> Result<Model> {
    let mut m = Model::new();
    let v: JsonValue = serde_json::from_str(str)?;
    let constraints = v["mStatements"]
        .as_array()
        .ok_or(Error::Parse("mStatements is not an array".to_owned()))?;

    for con in constraints {
        let entry = con
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
            "SuchThat" => parse_constraint(entry.1)?,
            _ => {
                return Err(Error::Parse(
                    "mStatements contains an unknown object".to_owned(),
                ))
            }
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
                for i in 0..2 {
                    let num =
                        &arr[i]["Constant"]["ConstantInt"][1]
                            .as_i64()
                            .ok_or(Error::Parse(
                                "Could not parse int domain constant".to_owned(),
                            ))?;
                    let num32 = i32::try_from(*num).map_err(|_| {
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

fn parse_constraint(obj: &JsonValue) -> Result<()> {
    Ok(())
}

impl Model {
    pub fn from_json(str: &String) -> Result<Model> {
        parse_json(str)
    }
}
