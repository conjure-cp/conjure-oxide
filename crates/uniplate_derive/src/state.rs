//! Global(ish) Parser State variables

use crate::prelude::*;
use std::collections::VecDeque;

/// State variables for biplate derive.
pub struct ParserState {
    /// The type we are deriving Biplate on.
    pub from: ast::PlateableType,

    /// The data structure itself.
    pub data: ast::Data,

    /// The current To type.
    pub to: Option<syn::Path>,

    /// Instances of Biplate<To> left to generate.
    pub tos_left: VecDeque<syn::Path>,

    /// All valid biplatable types inside this one.
    pub tos: Vec<syn::Path>,
}

impl ParserState {
    pub fn new(data: ast::Data) -> Self {
        let mut tos = data.get_platable_types();
        let from: ast::PlateableType = data.clone().into();
        tos.push(from.base_typ.clone());
        Self {
            to: None,
            tos_left: tos.clone().into(),
            tos,
            from,
            data,
        }
    }
}
