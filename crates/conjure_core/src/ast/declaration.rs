use std::sync::atomic::{AtomicU32, Ordering};

use derivative::Derivative;
use serde::{Deserialize, Serialize};
use uniplate::derive::Uniplate;
use uniplate::{Biplate, Tree};

use super::name::Name;
use super::serde::{DefaultWithId, HasId, ObjId};
use super::types::Typeable;
use super::{DecisionVariable, Domain, Expression, ReturnType};

static ID_COUNTER: AtomicU32 = AtomicU32::new(0);

#[derive(Derivative)]
#[derivative(PartialEq)]
#[derive(Debug, Serialize, Deserialize, Eq, Uniplate)]
#[biplate(to=Expression)]
#[uniplate(walk_into=[DeclarationKind])]
pub struct Declaration {
    /// The name of the declared symbol.
    name: Name,

    /// The kind of the declaration.
    kind: DeclarationKind,

    /// A unique id for this declaration.
    ///
    /// This is mainly used for serialisation and debugging.
    #[derivative(PartialEq = "ignore")] // eq by value not id.
    id: ObjId,
}

// I don't know why I need this one -- nd
//
// Without it, the derive macro for Declaration errors...
impl Biplate<Declaration> for DeclarationKind {
    fn biplate(&self) -> (Tree<Declaration>, Box<dyn Fn(Tree<Declaration>) -> Self>) {
        let self2 = self.clone();
        (Tree::Zero, Box::new(move |_| self2.clone()))
    }
}

/// A specific kind of declaration.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Uniplate)]
#[biplate(to=Expression)]
pub enum DeclarationKind {
    DecisionVariable(DecisionVariable),
    ValueLetting(Expression),
    DomainLetting(Domain),
    Given(Domain),
}

impl Declaration {
    /// Creates a new declaration.
    pub fn new(name: Name, kind: DeclarationKind) -> Declaration {
        let id = ID_COUNTER.fetch_add(1, Ordering::Relaxed);
        Declaration { name, kind, id }
    }

    /// Creates a new decision variable declaration.
    pub fn new_var(name: Name, domain: Domain) -> Declaration {
        let id = ID_COUNTER.fetch_add(1, Ordering::Relaxed);
        Declaration {
            name,
            kind: DeclarationKind::DecisionVariable(DecisionVariable::new(domain)),
            id,
        }
    }

    /// Creates a new domain letting declaration.
    pub fn new_domain_letting(name: Name, domain: Domain) -> Declaration {
        let id = ID_COUNTER.fetch_add(1, Ordering::Relaxed);
        Declaration {
            name,
            kind: DeclarationKind::DomainLetting(domain),
            id,
        }
    }

    /// Creates a new value letting declaration.
    pub fn new_value_letting(name: Name, value: Expression) -> Declaration {
        let id = ID_COUNTER.fetch_add(1, Ordering::Relaxed);
        Declaration {
            name,
            kind: DeclarationKind::ValueLetting(value),
            id,
        }
    }

    /// Creates a new given declaration.
    pub fn new_given(name: Name, domain: Domain) -> Declaration {
        let id = ID_COUNTER.fetch_add(1, Ordering::Relaxed);
        Declaration {
            name,
            kind: DeclarationKind::Given(domain),
            id,
        }
    }

    /// The name of this declaration.
    pub fn name(&self) -> &Name {
        &self.name
    }

    /// The kind of this declaration.
    pub fn kind(&self) -> &DeclarationKind {
        &self.kind
    }

    /// The domain of this declaration, if it is known.
    pub fn domain(&self) -> Option<&Domain> {
        match self.kind() {
            DeclarationKind::DecisionVariable(var) => Some(&var.domain),
            DeclarationKind::ValueLetting(_) => None,
            DeclarationKind::DomainLetting(domain) => Some(domain),
            DeclarationKind::Given(domain) => Some(domain),
        }
    }

    /// This declaration as a decision variable, if it is one.
    pub fn as_var(&self) -> Option<&DecisionVariable> {
        if let DeclarationKind::DecisionVariable(var) = self.kind() {
            Some(var)
        } else {
            None
        }
    }

    /// This declaration as a mutable decision variable, if it is one.
    pub fn as_var_mut(&mut self) -> Option<&mut DecisionVariable> {
        if let DeclarationKind::DecisionVariable(var) = &mut self.kind {
            Some(var)
        } else {
            None
        }
    }

    /// This declaration as a domain letting, if it is one.
    pub fn as_domain_letting(&self) -> Option<&Domain> {
        if let DeclarationKind::DomainLetting(domain) = self.kind() {
            Some(domain)
        } else {
            None
        }
    }

    /// This declaration as a mutable domain letting, if it is one.
    pub fn as_domain_letting_mut(&mut self) -> Option<&mut Domain> {
        if let DeclarationKind::DomainLetting(domain) = &mut self.kind {
            Some(domain)
        } else {
            None
        }
    }

    /// This declaration as a value letting, if it is one.
    pub fn as_value_letting(&self) -> Option<&Expression> {
        if let DeclarationKind::ValueLetting(expr) = &self.kind {
            Some(expr)
        } else {
            None
        }
    }

    /// This declaration as a mutable value letting, if it is one.
    pub fn as_value_letting_mut(&mut self) -> Option<&mut Expression> {
        if let DeclarationKind::ValueLetting(expr) = &mut self.kind {
            Some(expr)
        } else {
            None
        }
    }

    /// Returns a clone of this declaration with a new name.
    pub fn with_new_name(mut self, name: Name) -> Declaration {
        self.name = name;
        self
    }
}

impl HasId for Declaration {
    fn id(&self) -> ObjId {
        self.id
    }
}

impl DefaultWithId for Declaration {
    fn default_with_id(id: ObjId) -> Self {
        Self {
            name: Name::User("_UNKNOWN".into()),
            kind: DeclarationKind::ValueLetting(false.into()),
            id,
        }
    }
}

impl Clone for Declaration {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            kind: self.kind.clone(),
            id: ID_COUNTER.fetch_add(1, Ordering::Relaxed),
        }
    }
}

impl Typeable for Declaration {
    fn return_type(&self) -> Option<ReturnType> {
        match self.kind() {
            DeclarationKind::DecisionVariable(var) => var.return_type(),
            DeclarationKind::ValueLetting(expression) => expression.return_type(),
            DeclarationKind::DomainLetting(domain) => domain.return_type(),
            DeclarationKind::Given(domain) => domain.return_type(),
        }
    }
}
