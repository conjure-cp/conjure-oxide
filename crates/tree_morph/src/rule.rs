use crate::Commands;
use uniplate::Uniplate;

pub trait Rule<T, M>
where
    T: Uniplate,
{
    fn apply(&self, commands: &mut Commands<T, M>, subtree: &T, meta: &M) -> Option<T>;
}
