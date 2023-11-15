use crate::ast::Model as ConjureModel;
use minion_rs::ast::Model as MinionModel;

impl TryFrom<ConjureModel> for MinionModel {
    // TODO: set this to equal ConjureError once it is merged.
    type Error = String;

    fn try_from(value: ConjureModel) -> Result<Self, Self::Error> {
        todo!()
    }
}
