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
    pub to: Option<ast::PlateableType>,

    /// Instances of Biplate<To> left to generate.
    pub tos_left: VecDeque<ast::PlateableType>,

    /// All valid biplatable types inside this one.
    pub tos: Vec<ast::PlateableType>,
}

impl ParserState {
    pub fn new(data: ast::Data) -> Self {
        Self {
            from: todo!(),
            to: todo!(),
            tos_left: todo!(),
            tos: todo!(),
            data,
        }
    }
}
