use crate::parse::model_from_json;
use crate::Error as ParseErr;
use conjure_core::ast::Model;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EssenceParseError {
    #[error("Error running conjure pretty: {0}")]
    ConjurePrettyError(String),
    #[error("Error parsing essence file: {0}")]
    ParseError(ParseErr),
}

impl From<ParseErr> for EssenceParseError {
    fn from(e: ParseErr) -> Self {
        EssenceParseError::ParseError(e)
    }
}

pub fn parse_essence_file(path: &str, filename: &str) -> Result<Model, EssenceParseError> {
    let mut cmd = std::process::Command::new("conjure");
    let output = match cmd
        .arg("pretty")
        .arg("--output-format=astjson")
        .arg(format!("{path}/{filename}.essence"))
        .output()
    {
        Ok(output) => output,
        Err(e) => return Err(EssenceParseError::ConjurePrettyError(e.to_string())),
    };

    if !output.status.success() {
        let stderr_string = String::from_utf8(output.stderr)
            .unwrap_or("stderr is not a valid UTF-8 string".to_string());
        return Err(EssenceParseError::ConjurePrettyError(stderr_string));
    }

    let astjson = match String::from_utf8(output.stdout) {
        Ok(astjson) => astjson,
        Err(e) => {
            return Err(EssenceParseError::ConjurePrettyError(format!(
                "Error parsing output from conjure: {:#?}",
                e
            )))
        }
    };

    let parsed_model = model_from_json(&astjson)?;
    Ok(parsed_model)
}
