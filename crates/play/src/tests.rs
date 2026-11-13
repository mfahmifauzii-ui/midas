//! Pure tests: the compiler and the patch planner over documents built in memory. No network, no
//! window, no GPU.

use figcraft_color::{Color, Paint};
use figcraft_doc::proto::{Action, Interaction, Trigger};
use figcraft_doc::{Document, NodeId, NodeKind, TextData};

use crate::compile::{CONTROL_KEY, DISABLED_KEY, compile_frame, escape_text, is_web_url};
use crate::patch::{Plan, plan};
use crate::protocol::Command;

/// A page with one frame (400×300 at the origin) holding a rectangle, returns (doc, frame, rect).
fn fixture() -> (Document, NodeId, NodeId) {
    let mut d = Document::new("t");
    let page = d.first_page().unwrap();
    let frame = d.create(page, NodeKind::Frame, "Screen", 0.0, 0.0, 400.0, 300.0).unwrap();
    let rect = d.create(frame, NodeKind::Rectangle, "Box", 10.0, 20.0, 100.0, 50.0).unwrap();
    d.get_mut(rect).unwrap().fills = vec![Paint::solid(Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 })];
    (d, frame, rect)
}

fn patches(p: Plan) -> Vec<Command> {
    match p {
        Plan::Patch(cmds) => cmds,
        Plan::Reload(_) => panic!("expected patches, got a reload"),
    }
}

#[test]
fn compile_tags_every_element_with_its_node_id() {
    let (d, frame, rect) = fixture();
    let c = compile_frame(&d, frame).unwrap();
    assert!(c.html.starts_with(&format!("<div data-n=\"{}\"", frame.0)), "{}", c.html);
    assert!(c.html.contains(&format!("data-n=\"{}\"", rect.0)));
    assert_eq!(c.title, "Screen");
    assert_eq!((c.width, c.height), (400.0, 300.0));
}

#[test]
fn compile_places_children_absolutely_at_the_canvas_position() {
    let (d, frame, rect) = fixture();
    let c = compile_frame(&d, frame).unwrap();
    let rect_open = c.html.split(&format!("data-n=\"{}\"", rect.0)).nth(1).unwrap();
    assert!(rect_open.contains("left: 10px;"), "{rect_open}");
    assert!(rect_open.contains("top: 20px;"), "{rect_open}");
    assert!(rect_open.contains("width: 100px;") && rect_open.contains("height: 50px;"), "{rect_open}");
    assert!(rect_open.contains("background: linear-gradient(#FF0000, #FF0000)"), "{rect_open}");
}

#[test]
fn auto_layout_frame_becomes_a_flex_container_and_children_flex() {
    let (mut d, frame, rect) = fixture();
    d.get_mut(frame).unwrap().layout = Some(figcraft_doc::AutoLayout { gap: 8.0, padding: [4.0; 4], ..Default::default() });
    let c = compile_frame(&d, frame).unwrap();
    assert!(c.html.contains("display: flex;"), "{}", c.html);
    assert!(c.html.contains("gap: 8px;"));
    let rect_open = c.html.split(&format!("data-n=\"{}\"", rect.0)).nth(1).unwrap();
    assert!(rect_open.contains("position: relative;") && rect_open.contains("flex: none;"), "{rect_open}");
    assert!(!rect_open.contains("left: 10px"), "flex items take no absolute offset");
}

#[test]
fn hidden_nodes_stay_in_the_page_with_display_none() {
    let (mut d, _frame, rect) = fixture();
    d.get_mut(rect).unwrap().visible = false;
    let c = compile_frame(&d, _frame).unwrap();
    let rect_open = c.html.split(&format!("data-n=\"{}\"", rect.0)).nth(1).unwrap();
    assert!(rect_open.contains("display: none;"));
}

#[test]
fn a_text_layer_marked_as_input_becomes_an_input_with_its_value() {
    let (mut d, frame, _) = fixture();
    let page = d.first_page().unwrap();
    let _ = page;
    let t = d.create(frame, NodeKind::Text(TextData::new("Hello & <you>")), "Name", 10.0, 100.0, 200.0, 30.0).unwrap();
    d.get_mut(t).unwrap().plugin_data.insert(CONTROL_KEY.into(), "input".into());
    let c = compile_frame(&d, frame).unwrap();
    assert!(c.html.contains(&format!("<input data-n=\"{}\"", t.0)), "{}", c.html);
    assert!(c.html.contains("value=\"Hello &amp; &lt;you&gt;\""), "{}", c.html);
    assert!(c.html.contains("type=\"text\""));
}

#[test]
fn an_open_url_action_becomes_a_link_only_for_http_and_https() {
    let (mut d, frame, rect) = fixture();
    let link = |url: &str| Interaction {
        trigger: Trigger::OnClick,
        actions: vec![Action::OpenUrl { url: url.into(), new_tab: true }],
        transition: Default::default(),
        duration_ms: 0,
        easing: Default::default(),
        delay_ms: 0,
    };
    d.get_mut(rect).unwrap().interactions = vec![link("https://example.com/a?b=1")];
    let c = compile_frame(&d, frame).unwrap();
    assert!(c.html.contains("<a data-n=") && c.html.contains("href=\"https://example.com/a?b=1\""), "{}", c.html);
    assert!(c.html.contains("target=\"_blank\""));

    d.get_mut(rect).unwrap().interactions = vec![link("javascript:alert(1)")];
    let c = compile_frame(&d, frame).unwrap();
    assert!(!c.html.contains("href="), "only http(s) links are followed");
}

#[test]
fn controls_get_the_state_table_and_disabled_marker() {
    let (mut d, frame, rect) = fixture();
    {
        let n = d.get_mut(rect).unwrap();
        n.plugin_data.insert(CONTROL_KEY.into(), "button".into());
        n.plugin_data.insert(DISABLED_KEY.into(), "true".into());
    }
    let c = compile_frame(&d, frame).unwrap();
    assert!(c.css.contains(".fc-ctl:hover") && c.css.contains(".fc-ctl:active"));
    assert!(c.css.contains(".fc-ctl:focus-visible") && c.css.contains(".fc-ctl.fc-disabled"));
    assert!(c.html.contains("fc-ctl fc-disabled"), "{}", c.html);
    assert!(c.html.contains("tabindex=\"0\""));
}

#[test]
fn escaping_and_link_rules() {
    assert_eq!(escape_text("a<b>&c"), "a&lt;b&gt;&amp;c");
    assert!(is_web_url("https://x.test"));
    assert!(is_web_url("  HTTP://x.test"));
    assert!(!is_web_url("file:///etc/passwd"));
    assert!(!is_web_url("javascript:void(0)"));
}

#[test]
fn a_style_change_is_a_single_set_patch() {
    let (before, frame, rect) = fixture();
    let mut after = before.clone();
    after.get_mut(rect).unwrap().fills = vec![Paint::solid(Color { r: 0.0, g: 0.0, b: 1.0, a: 1.0 })];
    let cmds = patches(plan(&before, &after, frame).unwrap());
    assert_eq!(cmds.len(), 1, "{cmds:?}");
    match &cmds[0] {
        Command::Set { n, style, .. } => {
            assert_eq!(*n, rect);
            assert!(style.contains("#0000FF"), "{style}");
        }
        other => panic!("expected a set, got {other:?}"),
    }
}

#[test]
fn moving_a_node_is_a_set_with_its_new_position() {
    let (before, frame, rect) = fixture();
    let mut after = before.clone();
    after.get_mut(rect).unwrap().transform = figcraft_geom::kurbo::Affine::translate((33.0, 44.0));
    let cmds = patches(plan(&before, &after, frame).unwrap());
    assert!(matches!(&cmds[..], [Command::Set { n, style, .. }] if *n == rect && style.contains("left: 33px;") && style.contains("top: 44px;")), "{cmds:?}");
}

#[test]
fn an_added_child_rebuilds_its_parent_and_nothing_else() {
    let (before, frame, _) = fixture();
    let mut after = before.clone();
    let added = after.create(frame, NodeKind::Rectangle, "New", 1.0, 2.0, 3.0, 4.0).unwrap();
    let cmds = patches(plan(&before, &after, frame).unwrap());
    assert_eq!(cmds.len(), 1, "{cmds:?}");
    match &cmds[0] {
        Command::Replace { n, html } => {
            assert_eq!(*n, frame);
            assert!(html.contains(&format!("data-n=\"{}\"", added.0)));
        }
        other => panic!("expected a replace, got {other:?}"),
    }
}

#[test]
fn a_removed_node_is_a_remove_patch() {
    let (before, frame, rect) = fixture();
    let mut after = before.clone();
    after.remove(rect).unwrap();
    let cmds = patches(plan(&before, &after, frame).unwrap());
    assert!(cmds.iter().any(|c| *c == Command::Remove { n: rect }), "{cmds:?}");
    // The parent's children changed, so it is rebuilt too (the removal is then a no-op in the page).
    assert!(cmds.iter().any(|c| matches!(c, Command::Replace { n, .. } if *n == frame)), "{cmds:?}");
}

#[test]
fn text_content_is_a_replace_but_an_input_value_is_a_set() {
    let (mut before, frame, _) = fixture();
    let text = before.create(frame, NodeKind::Text(TextData::new("one")), "T", 0.0, 0.0, 100.0, 20.0).unwrap();
    before.get_mut(text).unwrap().plugin_data.insert(CONTROL_KEY.into(), "input".into());
    let mut after = before.clone();
    if let NodeKind::Text(t) = &mut after.get_mut(text).unwrap().kind {
        t.characters = "two".into();
    }
    let cmds = patches(plan(&before, &after, frame).unwrap());
    assert!(cmds.iter().any(|c| matches!(c, Command::Set { n, attrs, .. } if *n == text && attrs.get("value").map(String::as_str) == Some("two"))), "{cmds:?}");

    // A plain text layer's characters are its content: rebuilt, not set.
    let mut plain_before = before.clone();
    plain_before.get_mut(text).unwrap().plugin_data.remove(CONTROL_KEY);
    let mut plain_after = plain_before.clone();
    if let NodeKind::Text(t) = &mut plain_after.get_mut(text).unwrap().kind {
        t.characters = "three".into();
    }
    let cmds = patches(plan(&plain_before, &plain_after, frame).unwrap());
    assert!(cmds.iter().any(|c| matches!(c, Command::Replace { n, .. } if *n == text)), "{cmds:?}");
}

#[test]
fn a_resized_frame_reloads_the_window() {
    let (before, frame, _) = fixture();
    let mut after = before.clone();
    after.get_mut(frame).unwrap().width = 500.0;
    assert!(matches!(plan(&before, &after, frame).unwrap(), Plan::Reload(_)));
}

#[test]
fn an_auto_layout_change_restyles_the_children_as_flex_items() {
    let (before, frame, rect) = fixture();
    let mut after = before.clone();
    after.get_mut(frame).unwrap().layout = Some(figcraft_doc::AutoLayout::default());
    let cmds = patches(plan(&before, &after, frame).unwrap());
    assert!(cmds.iter().any(|c| matches!(c, Command::Set { n, style, .. } if *n == frame && style.contains("display: flex;"))), "{cmds:?}");
    assert!(cmds.iter().any(|c| matches!(c, Command::Set { n, style, .. } if *n == rect && style.contains("flex: none;"))), "{cmds:?}");
}

#[test]
fn an_unchanged_document_plans_nothing() {
    let (before, frame, _) = fixture();
    let after = before.clone();
    assert_eq!(patches(plan(&before, &after, frame).unwrap()), Vec::<Command>::new());
}

#[test]
fn a_non_container_is_refused() {
    let (d, _frame, rect) = fixture();
    assert!(compile_frame(&d, rect).is_err());
}

#[test]
fn protocol_round_trips_as_json_lines() {
    let cmd = Command::Set { n: NodeId(7), attrs: [("class".to_string(), "fc".to_string())].into(), style: "left: 1px;".into() };
    let line = serde_json::to_string(&cmd).unwrap();
    assert!(line.starts_with("{\"op\":\"set\",\"n\":7"), "{line}");
    assert_eq!(serde_json::from_str::<Command>(&line).unwrap(), cmd);
}
