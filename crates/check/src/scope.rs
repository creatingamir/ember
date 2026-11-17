//! Scopes: declaring and finding locals, and recording what the checker learns about a node.

use std::collections::HashMap;

use ast::{Ident, NodeId};

use super::{Binding, Checker, LocalInfo, PRINT, Ty};

impl Checker {
    /// Opens a new scope.
    pub(super) fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    /// Closes the innermost scope.
    pub(super) fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    /// Finds a visible parameter or local by name.
    pub(super) fn lookup_local(&self, name: &str) -> Option<LocalInfo> {
        for scope in self.scopes.iter().rev() {
            if let Some(local) = scope.get(name) {
                return Some(*local);
            }
        }
        None
    }

    /// Tells whether a name is a user function or the built-in `print`.
    pub(super) fn is_function(&self, name: &str) -> bool {
        self.functions.contains_key(name) || name == PRINT
    }

    /// Declares a parameter, `let` or loop variable; true means valid (and so checked for unused).
    pub(super) fn declare(&mut self, name: &Ident, id: NodeId, ty: Ty) -> bool {
        // `_` is never added to a scope, so it can be declared any number of times.
        if name.name == "_" {
            self.record_local_type(id, ty);
            return true;
        }

        let local_taken = self.lookup_local(&name.name).is_some();
        let function_taken = self.functions.contains_key(&name.name);
        if local_taken || function_taken {
            let message = format!("`{}` is already declared", name.name);
            self.error(message, name.span);
        }
        if local_taken {
            return false;
        }

        self.record_local_type(id, ty);
        // A local that clashes with a function is still added, so reading it is not a second error.
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.name.clone(), LocalInfo { id, ty });
        }
        !function_taken
    }

    /// Stores the type of a declared local, unless it is the error marker.
    fn record_local_type(&mut self, id: NodeId, ty: Ty) {
        if let Ty::Known(known) = ty {
            self.checked.local_types.insert(id, known);
        }
    }

    /// Stores the type of an expression, unless it is the error marker.
    pub(super) fn record_expr_type(&mut self, id: NodeId, ty: Ty) {
        if let Ty::Known(known) = ty {
            self.checked.expr_types.insert(id, known);
        }
    }

    /// Stores what a name, assignment or call refers to.
    pub(super) fn record_binding(&mut self, id: NodeId, binding: Binding) {
        self.checked.bindings.insert(id, binding);
    }
}
