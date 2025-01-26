use crate::commands::Commands;
use uniplate::Uniplate;

pub struct Reduction<T, M>
where
    T: Uniplate,
{
    pub new_tree: T,
    pub(crate) commands: Commands<T, M>,
}

impl<T, M> Reduction<T, M>
where
    T: Uniplate,
{
    pub(crate) fn apply_transform<F>(transform: F, tree: &T, meta: &M) -> Option<Self>
    where
        F: Fn(&mut Commands<T, M>, &T, &M) -> Option<T>,
    {
        let mut commands = Commands::new();
        let new_tree = transform(&mut commands, &tree, &meta)?;
        Some(Self { new_tree, commands })
    }
}
