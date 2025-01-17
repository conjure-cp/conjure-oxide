use crate::{helpers::one_or_select, Commands, Reduction, Rule};
use uniplate::Uniplate;

// TODO: (Felix) dirty/clean optimisation: replace tree with a custom tree structure,
//               which contains the original tree and adds metadata fields?

// TODO: (Felix) add logging via `log` crate; possibly need tree type to be Debug?
//               could be a crate feature?

// TODO: (Felix) add "control" rules; e.g. ignore a subtree to a certain depth?
//               test by ignoring everything once a metadata field is set? e.g. "reduce until contains X"

/// Exhaustively transform a tree with the given list of functions.
///
/// Each transform function is applied to every node before the next function is tried.
/// When any change is made, the tree is updated and side-effects are applied before the process
/// restarts with the first transform function.
///
/// Once the last transform function makes no changes, this function returns the updated tree and metadata.
pub fn reduce<T, M, F>(transforms: &[F], mut tree: T, mut meta: M) -> (T, M)
where
    T: Uniplate,
    F: Fn(&mut Commands<T, M>, &T, &M) -> Option<T>,
{
    let mut new_tree = tree;
    'main: loop {
        tree = new_tree;
        for transform in transforms.iter() {
            // Try each transform on the entire tree before moving to the next
            for (node, ctx) in tree.contexts() {
                let red_opt = Reduction::apply_transform(transform, &node, &meta);

                if let Some(mut red) = red_opt {
                    (new_tree, meta) = red.commands.apply(ctx(red.new_tree), meta);

                    // Restart with the first transform every time a change is made
                    continue 'main;
                }
            }
        }
        // All transforms were attempted without change
        break;
    }
    (tree, meta)
}

/// Exhaustively transform a tree with the given list of rules.
///
/// If multiple rules apply to a node, the `select` function is used to choose which one to apply.
///
/// This is a special case of [`reduce_with_rule_groups`] with a single rule group.
pub fn reduce_with_rules<T, M, R, S>(rules: &[R], select: S, tree: T, meta: M) -> (T, M)
where
    T: Uniplate,
    R: Rule<T, M>,
    S: Fn(&T, &mut dyn Iterator<Item = (&R, Reduction<T, M>)>) -> Option<Reduction<T, M>>,
{
    reduce_with_rule_groups(&[rules], select, tree, meta)
}

/// Exhaustively transform a tree with the given list of rule groups.
/// A 'rule group' represents a higher-priority set of rules which are applied to the entire tree before subsequent groups.
///
/// If multiple rules apply to a node, the `select` function is used to choose which one to apply.
///
/// This is an abstraction over [`reduce`], where each transform function attempts a rule group on each node.
pub fn reduce_with_rule_groups<T, M, R, S>(groups: &[&[R]], select: S, tree: T, meta: M) -> (T, M)
where
    T: Uniplate,
    R: Rule<T, M>,
    S: Fn(&T, &mut dyn Iterator<Item = (&R, Reduction<T, M>)>) -> Option<Reduction<T, M>>,
{
    let transforms: Vec<_> = groups
        .iter()
        .map(|group| {
            |commands: &mut Commands<T, M>, subtree: &T, meta: &M| {
                let applicable = &mut group.iter().filter_map(|rule| {
                    Reduction::apply_transform(|c, t, m| rule.apply(c, t, m), subtree, meta)
                        .map(|r| (rule, r))
                });
                let selection = one_or_select(&select, subtree, applicable);
                selection.map(|r| {
                    // Ensure commands used by the engine are the ones resulting from this reduction
                    *commands = r.commands;
                    r.new_tree
                })
            }
        })
        .collect();
    reduce(&transforms, tree, meta)
}
