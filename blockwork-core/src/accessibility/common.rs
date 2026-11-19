//! Selector matching shared by every platform's element tree.
use super::{UiNode, UiScope, UiSelector};

pub fn contains(value: &str, filter: &str) -> bool {
    filter.is_empty() || value.to_lowercase().contains(&filter.to_lowercase())
}

/// One element a search walked past. `object` is the platform's handle.
pub struct Found<E> {
    pub object: E,
    pub node: UiNode,
    pub showing: bool,
}

pub fn matches(selector: &UiSelector, window_filter: &str, node: &UiNode) -> bool {
    contains(&node.app, &selector.app)
        && contains(&node.window, window_filter)
        && contains(&node.name, &selector.name)
        && (selector.role.is_empty() || node.role.eq_ignore_ascii_case(&selector.role))
        && (selector.id.is_empty() || node.id == selector.id)
}

/// Fills in the selector that names `node` by everything it has.
pub fn with_selector(mut node: UiNode) -> UiNode {
    node.selector = UiSelector {
        scope: UiScope::App,
        app: node.app.clone(),
        window: node.window.clone(),
        name: node.name.clone(),
        role: node.role.clone(),
        id: node.id.clone(),
    };
    node.text = node.selector.to_text();
    node
}

/// The one element a block means. Hidden copies (a closed menu's items, an
/// inactive tab) give way to the one on screen; otherwise more than one
/// match is an error, never a guess.
pub fn resolve<E>(mut found: Vec<Found<E>>) -> Result<Option<Found<E>>, String> {
    if found.len() <= 1 {
        return Ok(found.pop());
    }
    let total = found.len();
    let mut showing: Vec<Found<E>> = found.into_iter().filter(|f| f.showing).collect();
    if showing.len() == 1 {
        return Ok(showing.pop());
    }
    let count = if showing.is_empty() {
        total
    } else {
        showing.len()
    };
    Err(format!(
        "{count} UI elements match; narrow the selector's name, role, ID or window (Pick element writes one that matches only its element)"
    ))
}

/// The index `resolve` would pick among `all` for `selector`, and how many
/// elements the selector matches.
fn resolve_in<E>(all: &[Found<E>], selector: &UiSelector) -> (Option<usize>, usize) {
    let hits: Vec<usize> = (0..all.len())
        .filter(|i| matches(selector, &selector.window, &all[*i].node))
        .collect();
    if hits.len() == 1 {
        return (Some(hits[0]), 1);
    }
    let showing: Vec<usize> = hits.iter().copied().filter(|i| all[*i].showing).collect();
    if showing.len() == 1 {
        return (Some(showing[0]), 1);
    }
    let count = if showing.is_empty() {
        hits.len()
    } else {
        showing.len()
    };
    (None, count)
}

/// The shortest selector that finds `all[target]` alone: by ID, by role and
/// name, then with the window added. Window titles often change (an open
/// document's name), so they're only added when needed.
fn shortest<E>(all: &[Found<E>], target: usize) -> (UiSelector, usize) {
    let n = &all[target].node;
    let base = |window: bool, name: bool, role: bool, id: bool| UiSelector {
        scope: UiScope::App,
        app: n.app.clone(),
        window: if window {
            n.window.clone()
        } else {
            String::new()
        },
        name: if name { n.name.clone() } else { String::new() },
        role: if role { n.role.clone() } else { String::new() },
        id: if id { n.id.clone() } else { String::new() },
    };
    let mut candidates = vec![];
    if !n.id.is_empty() {
        candidates.push(base(false, false, false, true));
        candidates.push(base(false, false, true, true));
    }
    if !n.name.is_empty() {
        candidates.push(base(false, true, true, false));
        candidates.push(base(true, true, true, false));
    } else {
        candidates.push(base(false, false, true, false));
        candidates.push(base(true, false, true, false));
    }
    candidates.push(base(true, true, true, !n.id.is_empty()));
    let mut last = (candidates[candidates.len() - 1].clone(), 0);
    for candidate in candidates {
        if candidate.require_target().is_err() {
            continue;
        }
        let (picked, count) = resolve_in(all, &candidate);
        if picked == Some(target) {
            return (candidate, 1);
        }
        last = (candidate, count.max(1));
    }
    last
}

/// `all[target]`'s node with the shortest selector that finds it alone.
pub fn picked<E>(all: &[Found<E>], target: usize) -> UiNode {
    let (selector, count) = shortest(all, target);
    let mut node = all[target].node.clone();
    node.text = selector.to_text();
    node.selector = selector;
    node.matches = count;
    node
}

/// Up to 100 elements of `all` that `selector` matches, each with the
/// shortest selector that finds it alone.
pub fn inspected<E>(all: &[Found<E>], selector: &UiSelector, skip_role: &str) -> Vec<UiNode> {
    (0..all.len())
        .filter(|i| matches(selector, &selector.window, &all[*i].node))
        .filter(|i| !all[*i].node.role.is_empty() && all[*i].node.role != skip_role)
        .take(100)
        .map(|i| picked(all, i))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(name: &str, role: &str, id: &str, window: &str, showing: bool) -> Found<()> {
        Found {
            object: (),
            node: with_selector(UiNode {
                app: "Editor".into(),
                window: window.into(),
                name: name.into(),
                role: role.into(),
                id: id.into(),
                enabled: true,
                selector: UiSelector::default(),
                text: String::new(),
                matches: 0,
            }),
            showing,
        }
    }

    #[test]
    fn shortest_selector_finds_its_element_alone() {
        let all = vec![
            found("Save", "button", "", "Doc 1", true),
            found("Save", "button", "", "Doc 2", true),
            found("Open", "button", "open", "Doc 1", true),
            found("Close", "button", "", "Doc 1", false),
            found("Close", "button", "", "Doc 1", true),
        ];
        let open = picked(&all, 2);
        assert_eq!((open.selector.id.as_str(), open.matches), ("open", 1));
        let save = picked(&all, 1);
        assert_eq!((save.selector.window.as_str(), save.matches), ("Doc 2", 1));
        // The hidden copy gives way to the one on screen.
        let close = picked(&all, 4);
        assert_eq!((close.selector.window.as_str(), close.matches), ("", 1));
        assert!(
            resolve(vec![
                found("A", "b", "", "", true),
                found("A", "b", "", "", true)
            ])
            .is_err()
        );
        assert_eq!(
            inspected(
                &all,
                &UiSelector {
                    name: "save".into(),
                    ..Default::default()
                },
                ""
            )
            .len(),
            2
        );
    }
}
