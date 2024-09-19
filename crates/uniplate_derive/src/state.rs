//! Global(ish) Parser State variables

use crate::prelude::*;
use std::collections::VecDeque;

use self::ast::HasBaseType;

/// State variables for biplate derive.
pub struct ParserState {
    /// The type we are deriving Biplate on.
    pub from: ast::PlateableType,

    /// The current target type.
    pub to: Option<ast::PlateableType>,

    /// The data structure itself.
    pub data: ast::Data,

    /// Information about the current instance being generated.
    pub current_instance: Option<ast::InstanceMeta>,

    /// Instances left to generate.
    pub instances_to_generate: VecDeque<ast::InstanceMeta>,
}

impl ParserState {
    pub fn new(inp: ast::DeriveInput) -> Self {
        let data = inp.data;
        let from: ast::PlateableType = data.clone().into();

        let mut instances_to_generate: VecDeque<ast::InstanceMeta> = inp.instance_metadata.into();

        // always generate Biplate<From,From>
        instances_to_generate.push_front(ast::InstanceMeta::Biplate(ast::BiplateInstanceMeta {
            to: ast::Type::Plateable(from.clone()),
            walk_into: Vec::new(),
        }));

        Self {
            current_instance: None,
            to: None,
            instances_to_generate,
            from,
            data,
        }
    }

    pub fn next_instance(&mut self) -> Option<()> {
        let next_instance = self.instances_to_generate.pop_back();
        self.current_instance = next_instance;

        self.to = match &self.current_instance {
            Some(ast::InstanceMeta::Uniplate(_)) => Some(self.from.clone()),
            Some(ast::InstanceMeta::Biplate(b)) => {
                let ast::Type::Plateable(t) = b.clone().to else {
                    // TODO: better error for this?
                    // probably should be in the parser not here!
                    unreachable!();
                };
                Some(t)
            }
            None => None,
        };

        if self.current_instance.is_none() {
            return None;
        };

        Some(())
    }

    /// Checks if a type can be walked into or not

    /// This acts similarly to ast::InstanceMeta::walk_into_type but also considers the
    /// current to and from type as walkable.
    pub fn walk_into_type(&self, typ: &ast::Type) -> bool {
        let Some(base_typ) = typ.base_typ() else {
            return false;
        };

        if base_typ == self.to.clone().expect("").base_typ() {
            return true;
        };

        if base_typ == self.from.base_typ() {
            return true;
        };

        self.current_instance.clone().expect("").walk_into_type(typ)
    }
}
