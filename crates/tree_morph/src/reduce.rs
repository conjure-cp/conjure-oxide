use crate::{helpers::one_or_select, prelude::*};
use uniplate::Uniplate;

// TODO: (Felix) dirty/clean optimisation: replace tree with a custom tree structure,
//               which contains the original tree and adds metadata fields?

// TODO: (Felix) add logging via `log` crate; possibly need tree type to be Debug?
//               could be a crate feature?

// TODO: (Felix) add "control" rules; e.g. ignore a subtree to a certain depth?
//               test by ignoring everything once a metadata field is set? e.g. "reduce until contains X"

/// TODO: rewrite for new interface
///
/// Exhaustively reduce a tree using a given transformation function.
///
/// The transformation function is called on each node in the tree (in left-most, outer-most order) along with
/// the metadata and a `Commands` object for side-effects.
///
/// - When the transformation function returns `Some(new_node)` for some node, that node is replaced with `new_node`.
///     Any side-effects are then applied at the root of the tree and the traversal begins again.
/// - Once the transformation function returns `None` for all nodes, the reduction is complete.
///
/// The `Commands` object is used to apply side-effects after a transformation is made.
/// This can be used to update metadata or perform arbitrary transformations on the entire tree,
/// which are reflected in the next traversal iteration.
///
/// # Parameters
/// - `transform`: A function which takes a mutable reference to a `Commands` object, a reference to the current node, and a reference to the metadata.
///               The function should return `Some(new_node)` if the node was transformed, or `None` otherwise.
/// - `tree`: The tree to reduce.
/// - `meta`: Metadata to be passed to the transformation function. This persists across all transformations.
///
/// # Returns
/// A tuple containing the reduced tree and the final metadata.
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

/// Exhaustively reduce a tree by applying the given rules at each node.
///
/// Rules are applied in the order they are given. If multiple rules can be applied to a node,
/// the `select` function is used to choose which rule to apply.
///
/// `Reduction`s encapsulate the result of applying a rule at a given node, holding the resulting node
/// and any side-effects. An iterator of these objects (along with the rule they result from)
/// is given to the `select` function, and the one returned is applied to the tree as in the `reduce` function.
///
/// # Parameters
/// - `rules`: A slice of rules to apply to the tree.
/// - `select`: A function which takes the current node and an iterator of rule-`Reduction` pairs and returns the selected `Reduction`.
/// - `tree`: The tree to reduce.
/// - `meta`: Metadata to be passed to the transformation function. This persists across all transformations.
///
/// # Returns
/// A tuple containing the reduced tree and the final metadata.
pub fn reduce_with_rules<T, M, R, S>(rules: &[R], select: S, tree: T, meta: M) -> (T, M)
where
    T: Uniplate,
    R: Rule<T, M>,
    S: Fn(&T, &mut dyn Iterator<Item = (&R, Reduction<T, M>)>) -> Option<Reduction<T, M>>,
{
    reduce_with_rule_groups(&[rules], select, tree, meta)
}

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
