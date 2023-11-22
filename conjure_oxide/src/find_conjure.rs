use anyhow::{anyhow, bail, Result};
use versions::Versioning;

const CONJURE_MIN_VERSION: &str = "2.5.1";
const CORRECT_FIRST_LINE: &str = "Conjure: The Automated Constraint Modelling Tool";

/// Checks if the conjure executable is present in PATH and if it is the correct version.
/// Returns () on success and an error on failure.
pub fn conjure_executable() -> Result<()> {
    let mut cmd = std::process::Command::new("conjure");
    let output = cmd.arg("--version").output()?;
    let stdout = String::from_utf8(output.stdout)?;
    let stderr = String::from_utf8(output.stderr)?;

    if !stderr.is_empty() {
        bail!("'conjure' results in error: ".to_string() + &stderr);
    }
    let first = stdout
        .lines()
        .next()
        .ok_or(anyhow!("Could not read stdout"))?;
    if first != CORRECT_FIRST_LINE {
        let path = std::env::var("PATH")?;
        let paths = std::env::split_paths(&path);
        let num_conjures = paths.filter(|path| path.join("conjure").exists()).count();
        if num_conjures > 1 {
            bail!(
                "Conjure may be present in PATH after a conflicting name. \
            Make sure to prepend the correct path to Conjure to PATH."
            )
        } else {
            bail!("The correct Conjure executable is not present in PATH.")
        }
    }
    let version = stdout
        .lines()
        .nth(1)
        .ok_or(anyhow!("Could not read Conjure's stdout"))?
        .strip_prefix("Release version ")
        .ok_or(anyhow!("Could not read Conjure version"))?;
    if Versioning::new(version) < Versioning::new(CONJURE_MIN_VERSION) {
        bail!(
            "Conjure version is too old (< {}): {}",
            CONJURE_MIN_VERSION,
            version
        );
    }
    Ok(())
}
