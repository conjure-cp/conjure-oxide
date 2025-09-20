use uniplate::Uniplate;

pub struct EventHandlers<T: Uniplate, M> {
    on_enter: Vec<fn(&T, &mut M)>,
    on_exit: Vec<fn(&T, &mut M)>,
}

impl<T: Uniplate, M> EventHandlers<T, M> {
    pub fn new() -> Self {
        EventHandlers {
            on_enter: vec![],
            on_exit: vec![],
        }
    }

    pub(crate) fn trigger_on_enter(&self, node: &T, meta: &mut M) {
        for f in self.on_enter.iter() {
            f(node, meta)
        }
    }

    pub(crate) fn trigger_on_exit(&self, node: &T, meta: &mut M) {
        for f in self.on_exit.iter() {
            f(node, meta)
        }
    }

    pub(crate) fn add_on_enter(&mut self, on_enter_fn: fn(&T, &mut M)) {
        self.on_enter.push(on_enter_fn);
    }

    pub(crate) fn add_on_exit(&mut self, on_exit_fn: fn(&T, &mut M)) {
        self.on_exit.push(on_exit_fn);
    }
}
