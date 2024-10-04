use std::collections::VecDeque;
use uniplate::Uniplate;

pub enum Command<T, M>
where
    T: Uniplate,
{
    Transform(fn(&T) -> T),
    MutMeta(fn(&mut M)),
}

/// A queue of commands to be applied after every successful rule application.
pub struct Commands<T, M>
where
    T: Uniplate,
{
    commands: VecDeque<Command<T, M>>,
}

impl<T, M> Commands<T, M>
where
    T: Uniplate,
{
    pub fn new() -> Self {
        Self {
            commands: VecDeque::new(),
        }
    }

    /// Apply the given transformation to the root node.
    /// Commands are applied in order after the rule is applied.
    pub fn transform(&mut self, f: fn(&T) -> T) {
        self.commands.push_back(Command::Transform(f));
    }

    /// Update the associated metadata.
    /// Commands are applied in order after the rule is applied.
    pub fn mut_meta(&mut self, f: fn(&mut M)) {
        self.commands.push_back(Command::MutMeta(f));
    }

    // Consumes and applies the commands currently in the queue.
    pub(crate) fn apply(&mut self, mut tree: T, mut meta: M) -> (T, M) {
        while let Some(cmd) = self.commands.pop_front() {
            match cmd {
                Command::Transform(f) => tree = f(&tree),
                Command::MutMeta(f) => f(&mut meta),
            }
        }
        (tree, meta)
    }

    pub(crate) fn clear(&mut self) {
        self.commands.clear();
    }
}
