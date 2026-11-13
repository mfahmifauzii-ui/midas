//! The CSS of one node. Where the rules below are exact copies of the Inspect code's declarations
//! (`engine/src/dev/codegen/css.rs`, `codegen.rs`: solid and gradient fills, shadows, blurs, flex
//! alignment, borders) they are kept verbatim; Play owns the rest, because it must match the canvas
//! rather than read well.

use std::collections::BTreeMap;
use std::sync::Arc;

use figcraft_color::{BlendMode, Gradient, GradientKind, ImageScaleMode, PaintKind};
use figcraft_doc::{
    AutoLayout, ContentAlign, CrossAlign, Document, EffectKind, LayoutMode, MainAlign, Node, NodeId, NodeKind, Shadow, TextAlignH, TextCase,
    TextDecoration, TextData,
};
use figcraft_text::{Face, TextLayout};

/// Where a node sits: the root frame at the window's corner, a flex item of an auto layout frame,
/// or absolutely placed in its parent's box (the canvas's own placement).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    Root,
    Flex,
    Absolute,
}

/// The state table of the spike: fixed rules for controls and links (`.fc-ctl`), the only nodes
/// whose pointer and focus states the page styles. Inline styles win over these rules, so a node
/// with its own `filter` (a layer blur) keeps its filter in every state; that is a known limit.
pub const STATE_TABLE: &str = "\
.fc-ctl { cursor: pointer; }
.fc-ctl:hover { filter: brightness(0.94); }
.fc-ctl:active { filter: brightness(0.86); }
.fc-ctl:focus-visible { outline: 2px solid #0D99FF; outline-offset: 2px; }
.fc-ctl.fc-disabled { opacity: 0.4; pointer-events: none; }
";

/// The base rules every element gets. Inline styles override them.
pub const BASE_CSS: &str = "\
html, body { margin: 0; padding: 0; overflow: hidden; }
.fc { box-sizing: border-box; margin: 0; padding: 0; border: 0 solid transparent; font: inherit; color: inherit; }
input.fc, textarea.fc { background: none; outline: none; }
";

/// A number as CSS writes it: no trailing zeros, no NaN.
pub fn num(v: f64) -> String {
    if v.is_finite() { format!("{v}") } else { "0".into() }
}

fn px(v: f64) -> String {
    format!("{}px", num(v))
}

/// The declarations of `n` placed as `place` in a parent (`parent_flex`: the parent is an auto
/// layout frame). Returns `None` for ids the document lacks.
pub fn node_style(doc: &Document, id: NodeId, place: Place, fonts: &mut Fonts) -> Option<String> {
    let n = doc.node(id)?;
    let mut out: Vec<(String, String)> = Vec::new();
    let mut put = |k: &str, v: String| out.push((k.to_string(), v));
    match place {
        Place::Root => {
            put("position", "absolute".into());
            put("left", "0px".into());
            put("top", "0px".into());
        }
        Place::Flex => {
            put("position", "relative".into());
            put("flex", "none".into());
        }
        Place::Absolute => {
            put("position", "absolute".into());
            let c = n.transform.as_coeffs();
            put("left", px(c[4]));
            put("top", px(c[5]));
            if !(is_close(c[0], 1.0) && is_close(c[3], 1.0) && c[1].abs() < 1e-9 && c[2].abs() < 1e-9) {
                put("transform", format!("matrix({}, {}, {}, {}, 0, 0)", num(c[0]), num(c[1]), num(c[2]), num(c[3])));
                put("transform-origin", "0 0".into());
            }
        }
    }
    put("width", px(n.width));
    put("height", px(n.height));
    if !n.visible {
        put("display", "none".into());
    }
    if let Some(l) = container_layout(n) {
        layout_decls(l, &mut put);
    }
    if n.clip && is_container(n) {
        put("overflow", "hidden".into());
    }
    if is_plain_ellipse(n) {
        put("border-radius", "50%".into());
    } else if let Some(r) = n.corners.uniform().filter(|r| *r > 0.0) {
        put("border-radius", px(r));
    } else if n.corners.radii.iter().any(|r| *r > 0.0) {
        let corners: Vec<String> = n.corners.radii.iter().map(|r| px(*r)).collect();
        put("border-radius", corners.join(" "));
    }
    if n.opacity < 1.0 {
        put("opacity", num(f64::from(n.opacity)));
    }
    if !matches!(n.blend, BlendMode::Normal | BlendMode::PassThrough) {
        put("mix-blend-mode", n.blend.css().into());
    }
    fills_decls(doc, n, &mut put);
    stroke_decls(n, &mut put);
    effect_decls(n, &mut put);
    if let NodeKind::Text(tx) = &n.kind {
        let layout = figcraft_text::layout_node(n);
        text_decls(tx, layout.as_ref().map(|(l, dy)| (l.as_ref(), *dy)), fonts, &mut put);
    }
    Some(out.iter().map(|(k, v)| format!("{k}: {v};")).collect::<Vec<_>>().join(" "))
}

fn is_close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

/// A full ellipse without a hole: CSS draws it as a rounded box. Arcs and rings are paths.
pub fn is_plain_ellipse(n: &Node) -> bool {
    match n.kind {
        NodeKind::Ellipse { sweep, inner, .. } => figcraft_geom::full_sweep(sweep) && inner == 0.0,
        _ => false,
    }
}

/// Containers take children: frames, groups, sections, components and instances.
pub fn is_container(n: &Node) -> bool {
    matches!(
        n.kind,
        NodeKind::Frame | NodeKind::Group | NodeKind::Section { .. } | NodeKind::Component { .. } | NodeKind::ComponentSet { .. } | NodeKind::Instance { .. } | NodeKind::BooleanOp { .. }
    )
}

/// The auto layout a container lays its children out with, when the flex model can (grid frames
/// keep the canvas placement: their children carry their laid-out transforms).
pub fn container_layout(n: &Node) -> Option<&AutoLayout> {
    n.layout.as_ref().filter(|l| l.mode != LayoutMode::Grid && is_container(n))
}

/// The flex declarations of an auto layout frame (`engine/src/dev/codegen/css.rs` `layout_decls`,
/// the same alignment tables as the canvas).
fn layout_decls(l: &AutoLayout, put: &mut impl FnMut(&str, String)) {
    put("display", "flex".into());
    put("flex-direction", if l.mode == LayoutMode::Vertical { "column" } else { "row" }.into());
    if l.wrap {
        put("flex-wrap", "wrap".into());
    }
    put("gap", px(l.gap));
    let [pt, pr, pb, pl] = l.padding;
    put("padding", [pt, pr, pb, pl].map(px).join(" "));
    let justify = match l.main_align {
        MainAlign::Min => "flex-start",
        MainAlign::Center => "center",
        MainAlign::Max => "flex-end",
        MainAlign::SpaceBetween => "space-between",
        MainAlign::SpaceAround => "space-around",
        MainAlign::SpaceEvenly => "space-evenly",
    };
    put("justify-content", justify.into());
    let align = match l.cross_align {
        CrossAlign::Min => "flex-start",
        CrossAlign::Center => "center",
        CrossAlign::Max => "flex-end",
        CrossAlign::Baseline => "baseline",
    };
    put("align-items", align.into());
    if l.wrap {
        let spread = l.content_align == ContentAlign::SpaceBetween;
        put(
            "align-content",
            if spread {
                "space-between"
            } else if align == "baseline" {
                "flex-start"
            } else {
                align
            }
            .into(),
        );
    }
}

/// Fills: solid colours, gradients and images as CSS backgrounds. The canvas paints fills in
/// order, so the last fill is on top; a CSS background list puts its first layer on top.
fn fills_decls(doc: &Document, n: &Node, put: &mut impl FnMut(&str, String)) {
    if matches!(n.kind, NodeKind::Text(_)) {
        // Text takes its colour from its first solid fill (glyph gradients are not drawn).
        if let Some(c) = n.fills.iter().find(|p| p.visible).and_then(|p| p.solid_color()) {
            put("color", c.to_css());
        }
        return;
    }
    let layers: Vec<String> = n
        .fills
        .iter()
        .rev()
        .filter(|p| p.visible)
        .filter_map(|p| match &p.kind {
            PaintKind::Solid { color } => Some(format!("linear-gradient({0}, {0})", color.with_alpha(color.a * p.opacity).to_css())),
            PaintKind::Gradient(g) => Some(css_gradient(g, n.width, n.height, p.opacity)),
            PaintKind::Image(img) => image_layer(doc, &img.image, img.scale_mode),
            PaintKind::Pattern(_) => None,
        })
        .collect();
    if !layers.is_empty() {
        put("background", layers.join(", "));
    }
}

/// Copied from `engine/src/dev/codegen/css.rs` (`css_gradient`).
fn css_gradient(g: &Gradient, w: f64, h: f64, opacity: f32) -> String {
    let stops = g
        .sorted_stops()
        .iter()
        .map(|s| format!("{} {}%", s.color.with_alpha(s.color.a * opacity).to_css(), (s.position * 100.0).round()))
        .collect::<Vec<_>>()
        .join(", ");
    let (dx, dy) = ((g.to.0 - g.from.0) * w, (g.to.1 - g.from.1) * h);
    let angle = if dx == 0.0 && dy == 0.0 { 180.0 } else { dx.atan2(-dy).to_degrees().rem_euclid(360.0) };
    let pct = |v: f64| format!("{}%", num(v * 100.0));
    let at = format!("at {} {}", pct(g.from.0), pct(g.from.1));
    match g.kind {
        GradientKind::Linear => format!("linear-gradient({}deg, {stops})", num(angle)),
        GradientKind::Radial | GradientKind::Diamond => {
            let (rx, ry) = (((g.width.0 - g.from.0) * w).hypot((g.width.1 - g.from.1) * h), dx.hypot(dy));
            let rel = |r: f64, side: f64| if side > 0.0 { pct(r / side) } else { "50%".into() };
            format!("radial-gradient({} {} {at}, {stops})", rel(rx, w), rel(ry, h))
        }
        GradientKind::Angular => format!("conic-gradient(from {}deg {at}, {stops})", num(angle)),
    }
}

/// An image fill as a CSS background layer, embedded as a data URL. `None` for an image the
/// document doesn't hold.
fn image_layer(doc: &Document, hash: &str, mode: ImageScaleMode) -> Option<String> {
    use base64::Engine as _;
    let img = doc.images.get(hash)?;
    let bytes = img.bytes.as_slice();
    let data = base64::engine::general_purpose::STANDARD.encode(bytes);
    let (size, repeat) = match mode {
        ImageScaleMode::Fill => ("cover", "no-repeat"),
        ImageScaleMode::Fit => ("contain", "no-repeat"),
        ImageScaleMode::Crop => ("100% 100%", "no-repeat"),
        ImageScaleMode::Tile => ("auto", "repeat"),
    };
    Some(format!("url(\"data:{};base64,{data}\") center / {size} {repeat}", sniff_mime(bytes)))
}

/// The media type of image bytes, by their magic numbers.
fn sniff_mime(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        "image/png"
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "image/jpeg"
    } else if bytes.starts_with(b"GIF8") {
        "image/gif"
    } else if bytes.len() > 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        "image/webp"
    } else {
        "application/octet-stream"
    }
}

/// Strokes: the first solid stroke as CSS borders (inside the box, which `border-box` sizing gives).
/// Centre and outside alignment and dashes are not drawn (see `docs/play-spike-results.md`).
fn stroke_decls(n: &Node, put: &mut impl FnMut(&str, String)) {
    let Some(c) = n.strokes.iter().find(|p| p.visible).and_then(|p| p.solid_color()) else { return };
    if matches!(n.kind, NodeKind::Text(_)) || n.stroke.weight <= 0.0 {
        return;
    }
    let color = c.to_css();
    match n.stroke.sides {
        None => put("border", format!("{} solid {color}", px(n.stroke.weight))),
        Some(sides) => {
            let named = ["top", "right", "bottom", "left"];
            for (side, w) in named.into_iter().zip(sides) {
                if w > 0.0 {
                    put(&format!("border-{side}"), format!("{} solid {color}", px(w)));
                }
            }
        }
    }
}

/// Shadows and blurs (copied from `engine/src/dev/codegen/css.rs` `effect_decls`).
fn effect_decls(n: &Node, put: &mut impl FnMut(&str, String)) {
    let shadows: Vec<String> = n
        .effects
        .iter()
        .filter(|e| e.visible)
        .filter_map(|e| match &e.kind {
            EffectKind::DropShadow(s) => Some(shadow_css(s, "")),
            EffectKind::InnerShadow(s) => Some(shadow_css(s, "inset ")),
            _ => None,
        })
        .collect();
    if !shadows.is_empty() {
        put("box-shadow", shadows.join(", "));
    }
    for e in n.effects.iter().filter(|e| e.visible) {
        match &e.kind {
            EffectKind::LayerBlur(b) => put("filter", format!("blur({}px)", num(b.radius / 2.0))),
            EffectKind::BackgroundBlur(b) => put("backdrop-filter", format!("blur({}px)", num(b.radius / 2.0))),
            EffectKind::Glass(g) => put("backdrop-filter", format!("blur({}px)", num(g.sanitized().frost / 2.0))),
            _ => {}
        }
    }
}

fn shadow_css(s: &Shadow, prefix: &str) -> String {
    format!("{prefix}{}px {}px {}px {}px {}", num(s.offset.0), num(s.offset.1), num(s.radius), num(s.spread), s.color.to_css())
}

/// The text declarations: the face the layout shaped with, the canvas's size, line height and
/// tracking, and the alignment and decoration of the style.
fn text_decls(
    tx: &TextData,
    layout: Option<(&TextLayout, f64)>,
    fonts: &mut Fonts,
    put: &mut impl FnMut(&str, String),
) {
    let st = &tx.style;
    let size = st.font_size;
    let face = figcraft_text::find_face(&st.font_family, &st.font_style);
    if let Some(f) = &face {
        put("font-family", format!("{}, sans-serif", fonts.register(f)));
        put("font-weight", f.weight.to_string());
        if f.italic {
            put("font-style", "italic".into());
        }
    } else {
        put("font-family", format!("\"{}\", sans-serif", st.font_family));
    }
    put("font-size", px(size));
    let line = layout.and_then(|(l, _)| l.lines.first().map(|line| line.height)).unwrap_or(size * 1.2);
    put("line-height", px(line));
    put("letter-spacing", px(st.letter_spacing.resolve(size)));
    put("white-space", "pre".into());
    put("text-align", match tx.align_h {
        TextAlignH::Left => "left",
        TextAlignH::Center => "center",
        TextAlignH::Right => "right",
        TextAlignH::Justified => "justify",
    }
    .into());
    match st.decoration {
        TextDecoration::None => {}
        TextDecoration::Underline => put("text-decoration", "underline".into()),
        TextDecoration::Strikethrough => put("text-decoration", "line-through".into()),
    }
    match st.case {
        TextCase::Upper => put("text-transform", "uppercase".into()),
        TextCase::Lower => put("text-transform", "lowercase".into()),
        TextCase::Title => put("text-transform", "capitalize".into()),
        TextCase::Original | TextCase::SmallCaps => {}
    }
}

/// The @font-face rules of the faces a frame uses, with their bytes embedded so the webview shapes
/// with the same font as the canvas.
#[derive(Default)]
pub struct Fonts {
    faces: BTreeMap<u32, Arc<Face>>,
}

impl Fonts {
    /// The CSS family name of `face` (registered for the page).
    pub fn register(&mut self, face: &Arc<Face>) -> String {
        self.faces.entry(face.id).or_insert_with(|| Arc::clone(face));
        alias(face.id)
    }

    /// The faces registered, by id.
    pub fn ids(&self) -> impl Iterator<Item = u32> + '_ {
        self.faces.keys().copied()
    }

    /// The @font-face CSS of every registered face.
    pub fn css(&self) -> String {
        use base64::Engine as _;
        let mut out = String::new();
        for face in self.faces.values() {
            let data = base64::engine::general_purpose::STANDARD.encode(face.bytes.data());
            let style = if face.italic { "italic" } else { "normal" };
            out.push_str(&format!(
                "@font-face {{ font-family: \"{}\"; src: url(data:font/otf;base64,{data}); font-weight: {}; font-style: {style}; }}\n",
                alias(face.id),
                face.weight
            ));
        }
        out
    }
}

/// The CSS family name of face `id`.
pub fn alias(id: u32) -> String {
    format!("\"fc-face-{id}\"")
}
