//! The wire types between the editor and the Play host process. One JSON object per line.
//!
//! Editor to host (stdin): [`Command`]. Host to editor: the host logs each [`Event`] as one JSON
//! line on its stderr; the runtime in the webview sends [`Inbound`] to the host.

use std::collections::BTreeMap;

use figcraft_doc::NodeId;
use serde::{Deserialize, Serialize};

/// A command for the Play window. `Load` replaces the whole frame; the others patch the DOM.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "lowercase")]
pub enum Command {
    /// Open (first time) or replace the frame. `html` is the frame's element tree, `css` the
    /// stylesheet (fonts, state table and every node's rules are inline `style` attributes).
    Load { title: String, width: f64, height: f64, html: String, css: String },
    /// Replace a node's attributes (`attrs`, except `style`) and inline `style`.
    Set { n: NodeId, attrs: BTreeMap<String, String>, style: String },
    /// Replace a node's element and its subtree (structure or text changed).
    Replace { n: NodeId, html: String },
    /// Remove a node's element.
    Remove { n: NodeId },
    /// Close the window and exit.
    Close,
}

/// A user interaction the Play page reports. Logged by the host as `{node, event, value?}`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub node: NodeId,
    /// `click`, `enter`, `leave`, `down`, `up`, `focus`, `blur`, `input` or `submit`.
    pub event: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

/// What the webview runtime sends the host.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Inbound {
    /// The page's runtime is ready to take commands.
    Ready,
    /// A user interaction, flattened as [`Event`].
    Event {
        node: NodeId,
        event: String,
        #[serde(default)]
        value: Option<String>,
    },
}

impl Inbound {
    /// The event as the log line the host writes, if this is an event.
    pub fn as_event(&self) -> Option<Event> {
        match self {
            Inbound::Ready => None,
            Inbound::Event { node, event, value } => Some(Event { node: *node, event: event.clone(), value: value.clone() }),
        }
    }
}
