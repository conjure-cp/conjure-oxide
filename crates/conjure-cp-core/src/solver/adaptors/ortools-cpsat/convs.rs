use crate::solver::{SolverError, SolverResult};
use super::proto::CpModelProto;
use std::collections::HashMap;
use crate::Model;
use crate::ast::Name;

struct TranslationContext {
    var_mapping: HashMap<Name, i32>,
}

pub(super) fn model_to_cp_sat(model: Model) -> SolverResult<CpModelProto> {
    let mut cp_model = CpModelProto::default();
    // variables translation cycle
    // constraint translation cycle
    
    Ok(cp_model)
}