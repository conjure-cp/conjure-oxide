use anyhow::{anyhow, bail, Result};
use versions::Versioning;

const CONJURE_MIN_VERSION: &str = "2.5.1";

pub fn conjure_executable() -> Result<String> {
    let path = std::env::var("PATH")?;
    let mut paths = std::env::split_paths(&path);
    let conjure_dir = paths
        .find(|path| path.join("conjure").exists())
        .ok_or(anyhow!("Could not find conjure in PATH"))?;
    let conjure_exec = conjure_dir
        .join("conjure")
        .to_str()
        .ok_or(anyhow!("Could not unwrap conjure executable path"))?
        .to_string();

    let mut cmd = std::process::Command::new(&conjure_exec);
    let output = cmd.arg("--version").output()?;
    let stdout = String::from_utf8(output.stdout)?;
    let stderr = String::from_utf8(output.stderr)?;

    if !stderr.is_empty() {
        bail!("Stderr is not empty: ".to_string() + &stderr);
    }
    let first = stdout
        .lines()
        .next()
        .ok_or(anyhow!("Could not read conjure's stdout"))?;
    if first != "Conjure: The Automated Constraint Modelling Tool" {
        bail!("'conjure' points to an incorrect executable");
    }
    let second = stdout
        .lines()
        .nth(1)
        .ok_or(anyhow!("Could not read conjure's stdout"))?;
    let version = second
        .strip_prefix("Release version ")
        .ok_or(anyhow!("Could not read conjure's stdout"))?;
    if Versioning::new(version) < Versioning::new(CONJURE_MIN_VERSION) {
        bail!(
            "Conjure version is too old (<{}): {}",
            CONJURE_MIN_VERSION,
            version
        );
    }

    Ok(conjure_exec)
}
