use crate::errors::EssenceParseError;
use conjure_cp_core::parse::model_from_json;
use conjure_cp_core::{Model, context::Context};
use std::sync::{Arc, RwLock};

pub fn parse_essence_file(
    path: &str,
    context: Arc<RwLock<Context<'static>>>,
) -> Result<Model, EssenceParseError> {
    let mut cmd = std::process::Command::new("conjure");
    let output = match cmd
        .arg("pretty")
        .arg("--output-format=astjson")
        .arg(path)
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
                "Error parsing output from conjure: {e:#?}"
            )));
        }
    };

    let parsed_model = model_from_json(&astjson, context)?;
    Ok(parsed_model)
}
