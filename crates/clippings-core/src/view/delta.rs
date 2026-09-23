//! Deltas (spec section 5.12): which displayed parents the client must
//! refresh after a rebuild.

use super::render::View;
use std::collections::BTreeSet;

/// Parent IDs to refresh, `None` meaning the top level. A view mode or
/// grouping change always refreshes the whole tree.
pub fn delta(old: &View, new: &View, whole_tree: bool) -> Vec<Option<String>> {
    if whole_tree {
        return vec![None];
    }
    let mut refresh: BTreeSet<Option<String>> = BTreeSet::new();
    for (id, node) in &new.nodes {
        if old.nodes.get(id) != Some(node) {
            refresh.insert(new.parents.get(id).cloned().flatten());
        }
    }
    let parents: BTreeSet<&Option<String>> =
        old.children.keys().chain(new.children.keys()).collect();
    for p in parents {
        if old.children.get(p) != new.children.get(p) {
            match p {
                None => {
                    refresh.insert(None);
                }
                Some(id) if new.nodes.contains_key(id) => {
                    refresh.insert(Some(id.clone()));
                }
                // A removed parent disappears through its own parent's list.
                Some(_) => {}
            }
        }
    }
    if refresh.contains(&None) {
        return vec![None];
    }
    refresh.into_iter().collect()
}
