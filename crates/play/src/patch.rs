//! Turns the difference between the last document the window shows and the current one into
//! patches. Only the nodes [`Document::changed_ids`] reports are looked at, so an edit costs about
//! its own size, not the document's.

use std::collections::BTreeSet;

use figcraft_doc::{Document, NodeId};

use crate::compile::{Compiled, PlayError, attrs_of, compile_frame, element_html, place_of, shape_key};
use crate::protocol::Command;
use crate::style::{Fonts, node_style};

/// Above this many changed nodes the window is reloaded rather than patched.
pub const PATCH_LIMIT: usize = 4000;

/// What to do to bring the window from `before` to `after`.
#[derive(Clone, Debug, PartialEq)]
pub enum Plan {
    /// Apply these commands, in order.
    Patch(Vec<Command>),
    /// Load this compiled frame again (too many changes, a new font, or a resized frame).
    Reload(Compiled),
}

/// Plan the commands from `before` (the last document sent) to `after` for `frame`.
pub fn plan(before: &Document, after: &Document, frame: NodeId) -> Result<Plan, PlayError> {
    let root_after = after.node(frame).ok_or(PlayError::NoNode(frame))?;
    let root_before = before.node(frame);
    let reload = |doc: &Document| compile_frame(doc, frame).map(Plan::Reload);
    let Some(changed) = after.changed_ids(before, PATCH_LIMIT) else { return reload(after) };
    // A resized or renamed frame changes the window itself; a new font needs its @font-face.
    if root_before.is_none_or(|b| b.width != root_after.width || b.height != root_after.height || b.name != root_after.name) {
        return reload(after);
    }
    // Nodes to look at: the changed ones, plus the children of any auto layout frame that changed,
    // since their flex placement follows the frame (before and after: a layout may have come or gone).
    let mut work: BTreeSet<NodeId> = BTreeSet::new();
    for id in changed {
        work.insert(id);
        for doc in [before, after] {
            if let Some(n) = doc.node(id) {
                if n.layout.is_some() {
                    work.extend(n.children.iter().copied());
                }
            }
        }
    }
    // A changed text layer may use a face the page hasn't loaded (its bytes travel with the page).
    let new_faces = text_faces(after, work.iter().copied());
    if !new_faces.is_empty() && !new_faces.is_subset(&text_faces(before, frame_nodes(before, frame))) {
        return reload(after);
    }

    let mut removes: Vec<NodeId> = Vec::new();
    let mut replaces: BTreeSet<NodeId> = BTreeSet::new();
    let mut sets: Vec<NodeId> = Vec::new();
    for &id in &work {
        let was = in_frame(before, id, frame);
        let now = in_frame(after, id, frame);
        match (was, now) {
            (true, false) => removes.push(id),
            (false, false) => {}
            (false, true) => {
                // A new node: its parent's children changed, so the parent is rebuilt.
                if let Some(p) = after.node(id).and_then(|n| n.parent) {
                    replaces.insert(p);
                }
            }
            (true, true) => {
                let (Some(b), Some(a)) = (before.node(id), after.node(id)) else { continue };
                if b.children != a.children || shape_key(before, id) != shape_key(after, id) {
                    replaces.insert(id);
                } else {
                    sets.push(id);
                }
            }
        }
    }

    // A rebuilt subtree already carries its descendants: drop the commands below it.
    let rebuilt: BTreeSet<NodeId> = replaces.clone();
    let replaces: Vec<NodeId> = rebuilt.iter().copied().filter(|id| !has_ancestor_in(after, *id, &rebuilt)).collect();
    let sets: Vec<NodeId> = sets.into_iter().filter(|id| !rebuilt.contains(id) && !has_ancestor_in(after, *id, &rebuilt)).collect();

    let mut out = Vec::new();
    for id in removes {
        out.push(Command::Remove { n: id });
    }
    for id in replaces {
        let place = place_of(after, id, frame);
        let mut fonts = Fonts::default();
        if let Some(html) = element_html(after, id, place, &mut fonts) {
            out.push(Command::Replace { n: id, html });
        }
    }
    for id in sets {
        let place = place_of(after, id, frame);
        let mut fonts = Fonts::default();
        let (Some(style), before_style) = (node_style(after, id, place, &mut fonts), node_style(before, id, place_of(before, id, frame), &mut Fonts::default())) else {
            continue;
        };
        let attrs = attrs_of(after, id);
        let attrs_before = attrs_of(before, id);
        if Some(&style) != before_style.as_ref() || attrs != attrs_before {
            out.push(Command::Set { n: id, attrs, style });
        }
    }
    Ok(Plan::Patch(out))
}

/// The ids of the font faces the text layers among `ids` use (their @font-face must be loaded).
fn text_faces(doc: &Document, ids: impl Iterator<Item = NodeId>) -> BTreeSet<u32> {
    let mut faces = BTreeSet::new();
    for id in ids {
        let Some(n) = doc.node(id) else { continue };
        if let figcraft_doc::NodeKind::Text(t) = &n.kind {
            if let Some(face) = figcraft_text::find_face(&t.style.font_family, &t.style.font_style) {
                faces.insert(face.id);
            }
        }
    }
    faces
}

/// Every node of `frame`'s subtree.
fn frame_nodes(doc: &Document, frame: NodeId) -> impl Iterator<Item = NodeId> + '_ {
    let mut stack = vec![frame];
    std::iter::from_fn(move || {
        let id = stack.pop()?;
        if let Some(n) = doc.node(id) {
            stack.extend(n.children.iter().copied());
        }
        Some(id)
    })
}

/// Whether `id` is the frame or inside it.
fn in_frame(doc: &Document, id: NodeId, frame: NodeId) -> bool {
    let mut cur = Some(id);
    let mut guard = 0;
    while let Some(c) = cur {
        if c == frame {
            return true;
        }
        let Some(n) = doc.node(c) else { return false };
        cur = n.parent;
        guard += 1;
        if guard > 10_000 {
            return false;
        }
    }
    false
}

/// Whether an ancestor of `id` (not `id` itself) is in `set`.
fn has_ancestor_in(doc: &Document, id: NodeId, set: &BTreeSet<NodeId>) -> bool {
    let mut cur = doc.node(id).and_then(|n| n.parent);
    let mut guard = 0;
    while let Some(c) = cur {
        if set.contains(&c) {
            return true;
        }
        cur = doc.node(c).and_then(|n| n.parent);
        guard += 1;
        if guard > 10_000 {
            break;
        }
    }
    false
}

