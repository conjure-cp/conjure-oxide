use std::collections::HashMap;
use std::path::{Path, PathBuf};
use crate::solver::adaptors::savilerow::SavileRow;
use rand::Rng;

pub struct SavileRowAsSolver {
    solver_adaptor: SavileRow,//use custom solveradaptor
    tmp_dir: PathBuf,
}

impl SavileRowAsSolver {

    //constructor to initialise solver_adaptor with custom temporary directory
    pub fn new() -> Self {
        let solver_adaptor = SavileRow::new();
        let tmp_dir = std::env::temp_dir();
        SavileRowAsSolver { solver_adaptor, tmp_dir }
    }

    //main method to solve essence file
    pub fn solve_essence_file(&self, essence_file: &Path) ->  Result<Vec<HashMap<String, String>, EssenceParseError>> {
        // TODO
        Ok(vec![])
    }
}