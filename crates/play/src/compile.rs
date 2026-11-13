//! Compiles a frame to its HTML element tree. Every element carries `data-n="<NodeId>"`, so the
//! events and the patches (see `patch.rs`) address nodes by id.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use figcraft_color::{Color, PaintKind};
use figcraft_doc::{Document, Node, NodeId, NodeKind, TextData};
use figcraft_doc::proto::{Action, Trigger};
use figcraft_geom::kurbo::BezPath;

use crate::style::{BASE_CSS, Fonts, Place, STATE_TABLE, container_layout, is_container, is_plain_ellipse, node_style};

/// Why a frame can't be compiled.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum PlayError {
    #[error("no node {0:?} in the document")]
    NoNode(NodeId),
    #[error("node {0:?} is not a frame or another container")]
    NotAFrame(NodeId),
}

/// A compiled frame: what the Play window loads.
#[derive(Clone, Debug, PartialEq)]
pub struct Compiled {
    pub title: String,
    pub width: f64,
    pub height: f64,
    /// The root element's HTML (the frame and its subtree).
    pub html: String,
    /// The page stylesheet: base rules, the state table, the fonts.
    pub css: String,
}

/// The `plugin_data` key that marks a text layer as a control, or a layer as disabled.
pub const CONTROL_KEY: &str = "play.control";
pub const DISABLED_KEY: &str = "play.disabled";

/// A control a text layer or a layer becomes in the page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    /// A single-line `<input>` (text layers only).
    Input,
    /// A multi-line `<textarea>` (text layers only).
    Textarea,
    /// A focusable box that shows the button states (any layer).
    Button,
}

/// The control a node is marked with (`plugin_data["play.control"]`), if any.
pub fn control(n: &Node) -> Option<Control> {
    match n.plugin_data.get(CONTROL_KEY).map(String::as_str)? {
        "input" if matches!(n.kind, NodeKind::Text(_)) => Some(Control::Input),
        "textarea" if matches!(n.kind, NodeKind::Text(_)) => Some(Control::Textarea),
        "button" => Some(Control::Button),
        _ => None,
    }
}

/// The URL an `OnClick` interaction opens, when it is an http or https link (`Action::OpenUrl`).
pub fn link(n: &Node) -> Option<&str> {
    n.interactions
        .iter()
        .filter(|i| matches!(i.trigger, Trigger::OnClick))
        .flat_map(|i| i.actions.iter())
        .find_map(|a| match a {
            Action::OpenUrl { url, .. } if is_web_url(url) => Some(url.as_str()),
            _ => None,
        })
}

/// Only http and https links are followed (the prototype spec's rule).
pub fn is_web_url(url: &str) -> bool {
    let lower = url.trim_start().to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

/// Compile `frame` (a frame or another container) to a [`Compiled`] page.
pub fn compile_frame(doc: &Document, frame: NodeId) -> Result<Compiled, PlayError> {
    let n = doc.node(frame).ok_or(PlayError::NoNode(frame))?;
    if !is_container(n) {
        return Err(PlayError::NotAFrame(frame));
    }
    let mut fonts = Fonts::default();
    let html = element_html(doc, frame, Place::Root, &mut fonts).ok_or(PlayError::NoNode(frame))?;
    let body = page_background(doc, frame).map(|c| format!("body {{ background: {}; }}\n", c.to_css())).unwrap_or_default();
    let css = format!("{BASE_CSS}{STATE_TABLE}{body}{}", fonts.css());
    Ok(Compiled { title: n.name.clone(), width: n.width, height: n.height, html, css })
}

/// The background of the page `id` sits on.
pub fn page_background(doc: &Document, id: NodeId) -> Option<Color> {
    let mut cur = Some(id);
    let mut guard = 0;
    while let Some(c) = cur {
        let n = doc.node(c)?;
        if let NodeKind::Page { background, .. } = n.kind {
            return Some(background);
        }
        cur = n.parent;
        guard += 1;
        if guard > 10_000 {
            return None;
        }
    }
    None
}

/// Where `id` is placed: the root for the frame being compiled, a flex item when its parent is an
/// auto layout frame, else absolute.
pub fn place_of(doc: &Document, id: NodeId, root: NodeId) -> Place {
    if id == root {
        return Place::Root;
    }
    match doc.node(id).and_then(|n| n.parent).and_then(|p| doc.node(p)) {
        Some(p) if container_layout(p).is_some() => Place::Flex,
        _ => Place::Absolute,
    }
}

/// The element of `id` and its subtree. `None` when the node is missing.
pub fn element_html(doc: &Document, id: NodeId, place: Place, fonts: &mut Fonts) -> Option<String> {
    let mut out = String::new();
    write_element(doc, id, place, fonts, &mut out)?;
    Some(out)
}

/// The HTML tag of `n` (the element the node compiles to).
pub fn tag_of(n: &Node) -> &'static str {
    if let NodeKind::Text(_) = n.kind {
        return match control(n) {
            Some(Control::Input) => "input",
            Some(Control::Textarea) => "textarea",
            _ if link(n).is_some() => "a",
            _ => "div",
        };
    }
    if is_path_shape(n) {
        return "svg";
    }
    if link(n).is_some() { "a" } else { "div" }
}

/// Shapes drawn as SVG paths: vectors, polygons, stars, lines, and arcs or rings of ellipses.
pub fn is_path_shape(n: &Node) -> bool {
    match n.kind {
        NodeKind::Vector { .. } | NodeKind::Polygon { .. } | NodeKind::Star { .. } | NodeKind::Line => true,
        NodeKind::Ellipse { .. } => !is_plain_ellipse(n),
        _ => false,
    }
}

/// The attributes of the element (everything but `style`), as the patch protocol carries them.
pub fn attrs_of(doc: &Document, id: NodeId) -> BTreeMap<String, String> {
    let mut a = BTreeMap::new();
    let Some(n) = doc.node(id) else { return a };
    let ctl = control(n);
    let linked = link(n);
    let mut class = String::from("fc");
    if ctl.is_some() || linked.is_some() {
        class.push_str(" fc-ctl");
    }
    if n.plugin_data.get(DISABLED_KEY).is_some_and(|v| v == "true") {
        class.push_str(" fc-disabled");
    }
    a.insert("class".into(), class);
    match ctl {
        Some(Control::Input) => {
            a.insert("type".into(), "text".into());
            a.insert("value".into(), text_of(n).unwrap_or_default());
            a.insert("tabindex".into(), "0".into());
            a.insert("spellcheck".into(), "false".into());
        }
        Some(Control::Textarea) => {
            a.insert("tabindex".into(), "0".into());
            a.insert("spellcheck".into(), "false".into());
        }
        Some(Control::Button) => {
            a.insert("tabindex".into(), "0".into());
        }
        None => {}
    }
    if let Some(url) = linked {
        a.insert("href".into(), url.to_string());
        a.insert("target".into(), "_blank".into());
        a.insert("rel".into(), "noopener noreferrer".into());
    }
    a
}

/// The characters of a text layer.
fn text_of(n: &Node) -> Option<String> {
    match &n.kind {
        NodeKind::Text(t) => Some(t.characters.clone()),
        _ => None,
    }
}

/// The element's own content apart from its children and attributes: a shape key that changes
/// exactly when the element's HTML must be rebuilt (tag, text lines, path data).
pub fn shape_key(doc: &Document, id: NodeId) -> String {
    let Some(n) = doc.node(id) else { return String::new() };
    let mut key = tag_of(n).to_string();
    match &n.kind {
        NodeKind::Text(t) => {
            // An input's characters travel as its `value` attribute, so they are not structural.
            if control(n) != Some(Control::Input) {
                key.push('|');
                // The raw characters too: laid-out lines alone can match for different text.
                key.push_str(&t.characters);
                key.push('\u{1}');
                key.push_str(&lines_html(n, t).join("\u{1}"));
            }
        }
        _ if is_path_shape(n) => {
            if let Some(p) = figcraft_doc::geometry::shape_path(n) {
                key.push('|');
                key.push_str(&path_d(&p));
            }
        }
        _ => {}
    }
    key
}

fn path_d(p: &BezPath) -> String {
    figcraft_svg::path_d(p)
}

/// Writes the element of `id` and its subtree.
fn write_element(doc: &Document, id: NodeId, place: Place, fonts: &mut Fonts, out: &mut String) -> Option<()> {
    let n = doc.node(id)?;
    let tag = tag_of(n);
    let style = node_style(doc, id, place, fonts)?;
    let mut open = format!("<{tag} data-n=\"{}\"", id.0);
    for (k, v) in attrs_of(doc, id) {
        let _ = write!(open, " {k}=\"{}\"", escape_attr(&v));
    }
    let _ = write!(open, " style=\"{}\"", escape_attr(&style));
    if is_path_shape(n) {
        if let Some(p) = figcraft_doc::geometry::shape_path(n) {
            let _ = write!(open, " viewBox=\"0 0 {} {}\" preserveAspectRatio=\"none\" overflow=\"visible\">", num_attr(n.width), num_attr(n.height));
            out.push_str(&open);
            out.push_str(&svg_path(n, &p));
        } else {
            open.push('>');
            out.push_str(&open);
        }
        out.push_str("</svg>");
        return Some(());
    }
    open.push('>');
    out.push_str(&open);
    match &n.kind {
        // A textarea's text is its content; an input's text is its `value` attribute.
        NodeKind::Text(_) if tag == "input" => return Some(()),
        NodeKind::Text(_) if tag == "textarea" => out.push_str(&escape_text(&text_of(n).unwrap_or_default())),
        NodeKind::Text(t) => out.push_str(&lines_html(n, t).join("<br>")),
        _ => {}
    }
    if is_container(n) {
        let child_place_flex = container_layout(n).is_some();
        for c in &n.children {
            let cp = if child_place_flex { Place::Flex } else { Place::Absolute };
            write_element(doc, *c, cp, fonts, out)?;
        }
    }
    let _ = write!(out, "</{tag}>");
    Some(())
}

/// The SVG path of a path shape, filled and stroked with its first solid paints.
fn svg_path(n: &Node, p: &BezPath) -> String {
    let fill = n
        .fills
        .iter()
        .rev()
        .find(|f| f.visible)
        .and_then(|f| match &f.kind {
            PaintKind::Solid { .. } => f.solid_color().map(|c| c.to_css()),
            _ => None,
        })
        .unwrap_or_else(|| "none".into());
    let stroke = n.strokes.iter().find(|s| s.visible).and_then(|s| s.solid_color()).map(|c| c.to_css());
    let mut el = format!("<path d=\"{}\" fill=\"{}\"", path_d(p), escape_attr(&fill));
    if let (Some(c), true) = (stroke, n.stroke.weight > 0.0) {
        let _ = write!(el, " stroke=\"{}\" stroke-width=\"{}\"", escape_attr(&c), num_attr(n.stroke.weight));
    } else {
        el.push_str(" stroke=\"none\"");
    }
    el.push_str("/>");
    el
}

/// The text of a layer as the canvas wrapped it: one entry per line of the canvas layout, with the
/// line break characters left out (`<br>` between entries). Without a layout, the text in one line.
pub fn lines_html(n: &Node, t: &TextData) -> Vec<String> {
    let Some((layout, _)) = figcraft_text::layout_node(n).filter(|(l, _)| !l.lines.is_empty()) else {
        return vec![escape_text(&t.characters)];
    };
    let chars: Vec<char> = t.characters.chars().collect();
    layout
        .lines
        .iter()
        .map(|line| {
            let slice: String = chars.get(line.char_start..line.char_end.min(chars.len())).unwrap_or_default().iter().collect();
            escape_text(slice.trim_end_matches(['\n', '\r', '\u{2028}']))
        })
        .collect()
}

/// A number for an SVG attribute.
fn num_attr(v: f64) -> String {
    crate::style::num(v)
}

/// HTML text content: `&`, `<` and `>` escaped.
pub fn escape_text(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            _ => o.push(c),
        }
    }
    o
}

/// An attribute value in double quotes: text escaped, and quotes too.
pub fn escape_attr(s: &str) -> String {
    escape_text(s).replace('"', "&quot;")
}
