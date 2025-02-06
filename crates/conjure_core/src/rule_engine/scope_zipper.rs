use std::sync::{Arc, RwLock};

use uniplate::zipper::ZipperBi;

use crate::{
    ast::{Expression, SymbolTable},
    Model,
};

// TODO: persuade myself that the symbol tables update correctly for exprs in the symbol-table.

/// A zipper over expressions in the tree and their scopes.
pub struct ScopeZipper {
    /// Zipper over the expressions in the constraints tree and the symbol-table.
    zipper: ZipperBi<Expression, Model>,

    /// The symbol table of our scope.
    focus_sym_tab: Arc<RwLock<SymbolTable>>,

    /// Extra structural constraints added by rules.
    focus_structural_constraints: Vec<Expression>,

    path: Vec<PathSegment>,
}

struct PathSegment {
    sym_tab: Arc<RwLock<SymbolTable>>,
    structural_constraints: Vec<Expression>,
}

impl ScopeZipper {
    #[allow(clippy::unwrap_used)]
    pub fn new(model: Model) -> Self {
        ScopeZipper {
            focus_sym_tab: model.symbols_ptr().clone(),
            focus_structural_constraints: vec![],
            zipper: ZipperBi::new(model).unwrap(),
            path: vec![],
        }
    }
    pub fn go_up(&mut self) -> Option<()> {
        self.zipper.go_up()?;

        let focus = self.zipper.focus_mut();

        //  If we just exited a scope, reconstruct it.
        if let Expression::Scope(_, model) = focus {
            // Add the new top level expressions to the sub-model.
            model.add_constraints(self.focus_structural_constraints.clone());

            // The symbol table is mutable so we don't need to do anything to it here.

            // We are now in the parent scope.
            let seg = self.path.pop()?;
            self.focus_sym_tab = seg.sym_tab;
            self.focus_structural_constraints = seg.structural_constraints;
        }

        Some(())
    }

    pub fn go_left(&mut self) -> Option<()> {
        self.zipper.go_left()
    }

    pub fn go_right(&mut self) -> Option<()> {
        self.zipper.go_right()
    }

    pub fn go_down(&mut self) -> Option<()> {
        let old_focus = self.zipper.focus().clone();

        self.zipper.go_down()?;

        // If we walked into a Scope, update the symbol table and structural constraints foci.
        if let Expression::Scope(_, model) = old_focus {
            self.path.push(PathSegment {
                sym_tab: self.focus_sym_tab.clone(),
                structural_constraints: self.focus_structural_constraints.clone(),
            });

            self.focus_structural_constraints = vec![];
            self.focus_sym_tab = model.symbols_ptr();
        }

        Some(())
    }

    /// Moves the cursor to the top-most, left-most expression in the model.
    pub fn go_to_top(&mut self) {
        while let Some(()) = self.go_up() {}
        while let Some(()) = self.go_left() {}
    }

    /// Rebuilds the (global) model.
    pub fn rebuild_model(mut self) -> Model {
        while let Some(()) = self.go_up() {}
        let mut model = self.zipper.rebuild_root();
        model.add_constraints(self.focus_structural_constraints);
        model
    }

    /// Moves the focus to the next node in a pre-order traversal of this zipper, and returns it.
    ///  
    /// Returns `None` if the current focus is the root node.
    pub fn next_preorder(&mut self) -> Option<(&mut Expression, Arc<RwLock<SymbolTable>>)> {
        self.go_down()
            .or_else(|| self.go_right())
            .or_else(|| self.go_up())?;

        // TODO: return &mut of symbol table instead?
        Some((self.zipper.focus_mut(), self.focus_sym_tab.clone()))
    }
}
