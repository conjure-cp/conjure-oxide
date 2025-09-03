use uniplate::Uniplate;

pub struct EventHandlers<T: Uniplate, M> {
    pub on_enter: Vec<fn(&T, &mut M)>,
    pub on_exit: Vec<fn(&T, &mut M)>,
}

impl<T: Uniplate, M> EventHandlers<T, M> {
    pub fn new() -> Self {
        EventHandlers {
            on_enter: vec![],
            on_exit: vec![],
        }
    }

    pub fn trigger_on_enter(&self, node: &T, meta: &mut M) {
        for f in self.on_enter.iter() {
            f(node, meta)
        }
    }

    pub fn trigger_on_exit(&self, node: &T, meta: &mut M) {
        for f in self.on_exit.iter() {
            f(node, meta)
        }
    }
}
