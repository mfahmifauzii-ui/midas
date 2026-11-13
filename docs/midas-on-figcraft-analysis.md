# Midas on FigCraft: analysis

Date: 2026-11-13. Read-only assessment. No code, tests or packages were changed or added, and nothing was built or run.

How to read this: every claim cites a file I opened. Midas paths are written `MIDAS: <repo-relative path>`; FigCraft paths are repo-relative. Numbers are marked **measured** (counted from files) or **estimated**. What I could not check is in [Not verified](#not-verified).

What the inventory reflects: Midas at branch `feat/fahmi`, last commit `1266957 Fill picker rework to Figma layout, stroke paints, fill parity`, with **61 modified files in the working tree** (`git status --short | wc -l`, measured). I read the working-tree files, so the inventory includes uncommitted work. FigCraft is workspace version 0.18.1, stage alpha (`ROADMAP.md`).

## 0. Premises and constraints that frame everything

1. **Precedence rule.** FigCraft behaviour is kept for every FigCraft feature. `MIDAS: docs/behavior-spec.md` governs Midas-only features. Overlaps are listed in [section 3.4](#34-overlaps-and-who-wins) with the winner named.
2. **The house rules forbid the Play design asked for.** `AGENTS.md` (Rust Craft repositories, rule 4): "No handwritten JavaScript or TypeScript, and no Tauri, Electron or webview shell, in those apps." A wry Play window is a webview shell; the Play runtime in Midas (toggle, press, FLIP, wave: `MIDAS: packages/builder-core/src/prototype/`) is handwritten JS. The fork has to either (a) drop that rule for the fork, or (b) generate every line of the Play runtime from Rust and ship no `.js` source. This is a decision for the owner; the plan below works under either, but (b) costs more. The rules say they apply "in every Craft repository" and are changed in `storytold/craft-contrib`, so a fork outside that organisation has to record its own deviation.
3. **Midas's own plan was a Tauri shell with web panels; this fork replaces that.** `MIDAS: docs/rust-plan.md` phases 2 to 7 (Tauri, Rust document model, Rust canvas, panel UI decision) are superseded: the egui editor is the base. Phase 7 there ("keep web panels or move to egui") is decided: egui. Play stays DOM, as that plan says.
4. **Text parity is the largest technical risk.** FigCraft shapes canvas text in Rust (`crates/text`, harfrust). Midas's Play lays text out in the browser, and its parity gate compares canvas and Play within 0.5 px (`MIDAS: AGENTS.md`, "Parity gate"). `MIDAS: docs/rust-migration-analysis.md` already warned that shaping text in Rust makes canvas and Play drift. Strategy in [section 4.5](#45-text-parity).
5. **Load rules conflict.** FigCraft repairs damaged files on open and reports issues (`crates/format/src/lib.rs`: `doc.validate()` then `doc.repair()`). Midas requires behavior data with a wrong version or shape to **fail to open** with an explicit error, never repaired (`MIDAS: AGENTS.md`, Permanent rules). Midas's check must be its own validation step after parse, whose failure is an error, not an issue.
6. **Collaboration cost of new fields.** `crates/collab/src/change.rs` `node_props!` makes the build fail if `Node` has a field not listed, and document-level data needs a new `DocProp` or `Entry` variant. Every new node or document field therefore touches the collab crate.
7. **Assets and licences.** `MIDAS: packages/builder-core/src/behavior-engine/solar-glyphs.js` embeds Solar icon paths, CC BY 4.0. `CC-BY-4.0` is on the licence allowlist, but `cargo xtask assets` only checks files under `assets/`, `docs/brand/`, `docs/images/` (`AGENTS.md`), so glyph path data embedded in code will not be caught: add an `ATTRIBUTION.md` row by hand. Antd-derived token data is MIT. Midas has **no LICENSE file** at its root (measured: `ls | grep -i licen` found none), so its code must be licensed MIT OR Apache-2.0 by its owner before it is copied into this repository. Nothing from `docs/brand/`.
8. **Command catalog.** "Everything is a command" and ids follow the incumbent's catalog (`AGENTS.md`). `xtask/src/main.rs` `parity()` passes an `extra` list to `catalog::parity`; I did not read how it treats ids outside the catalog, so confirm before adding `behavior.*` ids.

## 1. FigCraft document model

### 1.1 Structures (all `crates/doc/src` unless noted)

| Concept | Type and place |
|---|---|
| Document | `Document` in `lib.rs:194`: name, `pages: Vec<NodeId>`, `nodes: imbl::OrdMap<NodeId, Arc<Node>>` (persistent map, cheap snapshots), `styles`, `variables`, `images`, `libraries`, comments, board and Dev Mode data, `next_id` allocator (per-client id space for live sessions). |
| Node | `Node` in `node.rs:449`: id, name, `kind: NodeKind`, parent, children, visible, locked, opacity, blend, `transform: Affine`, width, height, `fills`/`strokes: Vec<Paint>`, `stroke: StrokeProps`, `effects`, `corners`, `constraints`, `layout: Option<AutoLayout>`, `child_layout`, `clip`, `is_mask`, `grids`, `interactions`, `overflow`, `styles`, `bound`, `var_modes`, `source`, `exposed`, `overrides`, `prop_refs`, `plugin_data`, annotations, code links, dev resources, `board`. |
| Node kinds | `NodeKind` (`node.rs:86`): Page (with `flows`, `prototype` settings), Frame, Group, Section, Rectangle, Ellipse, Polygon, Star, Line, Vector, Text, BooleanOp, Component (with `props: BTreeMap<String, PropDef>`), ComponentSet, Instance (with `component` and `props`), Slice. |
| Paint | `Paint` in `color/src/lib.rs:678`: `PaintKind` is `Solid`, `Gradient`, `Image`, `Pattern`; plus opacity, visible, blend, colour alpha. **No Video paint, no Noise paint** (gap 12 in `docs/gaps.md`). |
| Stroke | `Node.strokes: Vec<Paint>` (several stroke paints) and `StrokeProps` (`props.rs:84`: align, join, cap, dash, per-side widths, width profile, brush, arrowheads per the Midas study). |
| Variables | `Variables { collections, variables }` in `styles.rs:370`. `Variable` has `resolvedType` Color, Number, String or Boolean; values per mode id; `scopes`; `extends`/`overrides` (extended collections); `codeSyntax`. Aliases, colour alpha and colour mix are `VariableValue` cases. **No typography or effect variable type** (Midas has both). Collections have modes; a node chooses modes in `var_modes`; field bindings are `Node.bound: field name -> VariableId` (`fills/0/color`, `width`, ...). |
| Components | Main `Component` nodes inside `ComponentSet` (variants named `Property=Value, ...`), `PropDef` (`node.rs:42`): Boolean, Text, InstanceSwap (with `preferred`), Variant, Slot (with `preferred`). Layers reference a property through `Node.prop_refs`. An instance is `Instance { component, props }` whose descendants are real nodes mirroring the main through `Node.source`, with `overrides` naming locally changed property groups (`assets/components.rs` header). |
| Prototype | `Node.interactions: Vec<Interaction>` (`proto.rs:777`): `Trigger` (click, hover, press, drag, mouse enter/leave/down/up, after delay, key press), `Vec<Action>`, `Transition`, duration, easing. `Action` includes Navigate, Back, ScrollTo, OpenOverlay, SwapOverlay, CloseOverlay, OpenUrl, ChangeTo, SetVariable, SetVariableMode, Conditional. Page-level `Flow` and `PrototypeSettings` (device, responsive, background). |

### 1.2 Serialization and how it evolves

- `.figcraft` is a zip: `document.json` (header `format: "figcraft"`, `version: 1`, image index, then the `Document` as JSON), `images/<sha256>`, optional `thumbnail.png`, `versions/`, `branches/`, and a published library (`crates/format/src/lib.rs` header, `FORMAT`, `VERSION = 1`).
- Evolution is additive: new fields use `#[serde(default, skip_serializing_if = ...)]`, and `VERSION` has stayed at 1. A file with `version` greater than the reader's is refused (`FormatError::Newer`). Unknown fields are **ignored on read** (I found no `deny_unknown_fields` in `crates/doc`, `crates/color` or `crates/format/src/lib.rs`), so an older reader silently drops Midas data and rewrites the file without it. Reading is lenient and repairs structure (`Document::repair`).
- Other readers in `crates/format`: `rest` (REST JSON), `fig` (`.fig` import and experimental export), `sketch`, `penpot`. A Midas file importer would follow this pattern, but Midas's strict "no migration, regenerate old files" rule means it is not needed ([section 7](#7-plan)).

### 1.3 Command API and undo/redo

- `spec!(id, label, params_doc, history, run)` in `crates/engine/src/cmd.rs`; every area module exposes `specs()` and a single `registry()` collects them. About 500 `spec!` sites in `crates/engine/src/cmd/` (measured with grep, excluding tool code in `canvas/`).
- `Session::execute` (`crates/engine/src/session/exec.rs`): look the command up, refuse unknown params, snapshot `(doc, selection, page, uid)`, run under `guard::run` (a panic becomes `EngineError::Internal`), then the check chain (`check_instance_structure`, `check_preview`, `check_live_access`, `check_finite_geometry`), then `normalize_edit`. Any `Err` or panic restores the snapshot. This is the equivalent of Midas's "Not applied" rollback.
- `derived::normalize(doc, before)` (`crates/engine/src/derived.rs`) runs after every edit on the touched nodes: variable bindings and style links, then text boxes, groups, instances following mains, auto layout re-flow. It consumes `doc.take_dirty()`.
- Undo is whole-document snapshots: `History { undo, redo }` of `HistoryEntry { label, doc: Arc<Document>, selection, page }` (`doc_state.rs`), cheap because of `imbl`. Gestures use `Interaction` (the document before, previews re-applied from it). A batch is one undo step (`command.batch`).
- Programmatic calls never open dialogs; dialogs are requested through `Session::request_ui` (`AGENTS.md`).

### 1.4 Where Midas data could live: recommendation

Constraints that decide it (all checked in code):

- `Node.plugin_data: BTreeMap<String, String>` already saves (`document.json`), syncs (`PluginData` in `node_props!`), copies, and is writable by plugins and agents through `node.set {props: {pluginData}}` (`crates/engine/src/cmd/props.rs:1107`, merging arbitrary keys). It has no schema and no validation.
- Instance sync (`crates/engine/src/assets/components/sync.rs`, `fresh_root`): the rebuilt instance root starts as `comp.clone()` and then takes explicit fields back from the old root, including `plugin_data`. Any **new typed field** is copied from the main to the instance unless `fresh_root` resets it. For Midas that is a bug to prevent: "a copy gets the master's props store, not the binding" (`MIDAS: docs/behavior-spec.md`, Instancing). With `plugin_data` the instance keeps its own value, which happens to match.
- Midas requires one `behavior` key with a `version`, strict shape checks, and a hard failure on mismatch (`MIDAS: packages/builder-core/src/behavior-engine/data.js`: shapes are master `{templateId, variables}`, instance `{props}`, managed layer `{slot}`, canvas part `{part}`, container `{group}`, affix text `{text}`).

| | A. `plugin_data["midas.behavior"]` (JSON string) | B. Typed `Node.behavior` and `Document.behavior` | C. Typed schema over plugin_data storage |
|---|---|---|---|
| Collab | free (already a property) | one `node_props!` line plus new `DocProp` | free |
| Format | free | additive field, reader validates | free |
| Instance sync | free (instance keeps its own) | must edit `fresh_root`, `copy_in_file`, library copy | free |
| Plugin/agent can corrupt it | yes (`node.set pluginData`) | no, only behavior commands write it | yes unless reserved-key refusal is added |
| Strict load rule | possible, parse on open | natural: serde enum, error on mismatch | possible |
| Cost of parsing | JSON parse per read | none | JSON parse per read |
| Inspect/MCP visibility | already listed as `pluginData` | add to `automation/inspect.rs` | already listed |

**Recommendation: B, a typed field**, with `plugin_data` used only for the first spike. Reasons: the strict-load rule and "only behavior commands write it" are Midas invariants, and string-typed storage that a plugin can overwrite breaks both. Shape it as the `data.js` union: `enum Behavior { Binding{template_id, variables}, Props(map), Managed{slot}, Part{part}, Group{group}, Text{text} }` on `Node`, plus `Document.behavior: Option<BehaviorDoc{version, templates}>` for Custom templates (`MIDAS: docs/play-from-tsx.md` section 5). Costs to budget: collab entries, `fresh_root` reset, `inspect` output, `.fig`/Penpot export warnings, tests that sweep every node field. Decision-gate for the owner: if collab with Midas data is not needed in the first phases, A is cheaper and B can follow.

## 2. Midas feature inventory

Source of truth: `MIDAS: AGENTS.md`, `docs/behavior-spec.md`, `docs/roadmap.md`, `docs/prototype-spec.md`, `docs/slots.md`, `docs/play-from-tsx.md`, `docs/rust-plan.md`, `docs/specs/*`, plus code. "Built" below means the docs and code say so; none of it was run by me.

### 2.1 Midas features FigCraft lacks

| # | Feature | Built or planned | Evidence |
|---|---|---|---|
| 1 | **Behavior engine**: schema-driven templates (anatomy, props, tokens, states, events), pure data, no per-component engine code | Built | `packages/builder-core/src/behavior-engine/` (about 17k lines in `src/*/*.js`, measured with `wc -l`), `templates/` |
| 2 | **Built-in templates**: button, input (text, password, textarea kinds), input-number, form-item, checkbox, radio, switch, icon-button, alert, tag, badge, avatar, divider, text, title, paragraph, link, card, list | Built (list parked per `docs/slots.md`; Select does not exist) | `packages/builder-core/src/templates/*.js` |
| 3 | **Custom templates** stored in the document as `behavior: {version, templates}`, validated like built-ins | Built | `behavior-engine/custom-templates.js`, `AGENTS.md` |
| 4 | **Binding model**: component to template, variable-prop to one collection, auto-match by name, dimensions locked, modes are the prop values | Built | `behavior-spec.md` Decisions, `binding.js`, `modes.js` |
| 5 | **Behavior panel** (master: template picker and collection per dimension; instance: props, states, slots block, diagnostics, preview state) | Built | `apps/studio/src/engine/behaviorPanel.js`, `behavior-engine/panel.js` |
| 6 | **Managed slot/part layers**: icon slots, managed text, containers, glyphs; layers exist on instances only and come and go with props; anatomy order helpers | Built | `anatomy.js`, `managed-layers.js`, `template-master.js`, `AGENTS.md` decisions on `indexAmongLaterSlots`, `managedPartLayer` |
| 7 | **No-hidden-layers rule**: a master or instance never holds a hidden, opacity-0 or zero-size layer; conditional parts are made while the condition holds and removed when it fails | Built | `packages/builder-core/test/no-hidden-layers.test.js`, `AGENTS.md` decision "Diagnostics part-missing" |
| 8 | **Content slots** with `SlotDef`: min, max, `accepts`/`onlyAccepts` template ids, `preferred`, `showEmpty`, `repeat`, `seed`, `holds` (instance, content, image); diagnostics; picker restrictions | Built; defaults and repeated children wait for the screen emitter | `content-slots.js`, `slot-rules.js`, `slot-property.js`, `slot-diagnostics.js`, `docs/slots.md` |
| 9 | **Auto variable**: Generate base (Primitives, Semantic, Typography from a bundled snapshot), `roles.json`, `antd-sizes.json`, Auto Fix aliasing component variables by schema role, `bind-tokens` binding node properties, eight colour presets | Built; token JSON source is not in the repo, snapshot is hand-edited | `base-tokens/`, `auto-fix.js` (530 lines), `bind-tokens.js`, `templates/color-presets.js` |
| 10 | **Auto design system**: one undo step builds base, icons, then a master and demo instance of every registered template on a "Design System" page in dependency order | Built | `behavior-engine/design-system.js`, `icon-set.js`, `AGENTS.md` decision "Generate Design System" |
| 11 | **Composite variable types**: typography (whole-style alias per mode, editor window) and effect (shadows) variables; variables bound to paint parameters (pattern source, tile type, video source) and to stroke paints | Built | `typography.js`, `typography-edit.js`, `AGENTS.md` (Typography, Effect tokens, fill params) |
| 12 | **Diagnostics and load rules**: `dimension-unbound`, `variable-missing`, `part-missing`, `slot-*`, token-unresolved reasons; phase 1 per node and phase 2 whole-document checks; fail-to-open on mismatch; lazy page loading | Built | `diagnostics.js`, `document-check.js`, `data.js`, `behavior-spec.md` |
| 13 | **State model**: persistent states (disabled, checked, indeterminate, readOnly) derive bindings; preview states (hover, active, focus) recolour only, canvas lookup never stored | Built | `state-bindings.js`, `preview.js`, `behavior-spec.md` rework section |
| 14 | **Play as live DOM**: real `<input>`, `<a>`, `<button>`, CSS `:hover/:active/:focus/:disabled`, motion (FLIP, wave, reveal), toggle/step/copy/dismiss press actions, viewer chrome, scaling modes; canvas-vs-Play 0.5 px parity gate | Built (browser parity suite has known failures, `AGENTS.md`) | `packages/builder-core/src/prototype/dom-tree.js` (995 lines), `apps/studio/src/engine/play.js`, `press.js`, `toggle.js` |
| 15 | **Prototype extras**: conditional with a strict validator and its own error codes, `navigate-to`, `back`, `open-link`, after-delay, per-frame scroll behaviour, copy/paste link remapping, rollback with "Not applied" | Built; set variable, overlays, dropdown/popover, form submit are still roadmap | `docs/prototype-spec.md`, `docs/roadmap.md` Fase 2 |
| 16 | **Typed React export**: component emitter (`.jsx`/`.tsx` plus CSS) from the schema, typed only for templates with slots; screen emitter and Play-from-TSX are designs, not built | Partly built | `behavior-engine/codegen.js` (1346 lines), `docs/play-from-tsx.md` ("design, not implemented") |
| 17 | **Library publish and React package**: publish a versioned library, server builds an npm package with Tailwind CSS, private registry, per-component documentation site | Built on the server and in `library/`; the docs-site "showcase" and several behaviours are still plan | `apps/server/src/routes/{libraries,registry,docs}.ts`, `packages/builder-core/src/library/`, `docs/react-package.md`, `plan.md` |
| 18 | **Server stack**: Postgres, S3, auth, roles, API tokens, files, a read-only MCP server (8 tools) | Built | `apps/server/src/routes/`, `docs/mcp.md` |
| 19 | **Video paint** (fill and its parameters, autoplay/loop/sound) | Built | `geometry/fills.js`, `AGENTS.md` "Fill parity" |
| 20 | **FormItem validation, Code Builder, base code, icon server** | Design/spec only; not built | `docs/form-validation.md`, `docs/specs/development-spec.md`, `docs/roadmap.md` Fase 4 |

`plan.md` and the roadmap disagree on library publish: `plan.md` (dated 2026-10-29) says it exists, and the roadmap lists it as Fase 3 unchecked. The code (`apps/server/src/routes/libraries.ts`, `package.ts`, `docs-site.ts`) supports `plan.md`; `docs/roadmap.md` is stale on this point.

### 2.2 Midas features FigCraft already has in some form

| Midas | FigCraft | How close |
|---|---|---|
| Variables, collections, modes, aliases, node `varModes` | `Variables`, `Collection`, `Mode`, `Node.var_modes`, `Node.bound` | Same model for colour, number, string, boolean. Differs for typography and effect. |
| Instance modes survive sync | `fresh_root` keeps `var_modes` | Same rule. |
| Variant and Size as collection modes | Same mechanism exists; FigCraft also has component-set variants | Compatible; Midas picks modes, not variants. |
| Component props, instance swap, content slot with `preferred` | `PropDef` Boolean, Text, InstanceSwap, Variant, Slot; `override` group `slot` | FigCraft slots lack min, max, `accepts`, `repeat`, `seed`. |
| Conditional, set variable, navigate, back, open link, after delay, overlays, scroll | `Action`, `Trigger`, `proto_run.rs`, `ui-egui/src/present/` | FigCraft is ahead (below). |
| Flows and a start frame | `Page.flows`, `Flow { name, start }`, several flows per page | FigCraft is ahead (Midas keeps one start per page). |
| Fill picker: Solid, gradients, Pattern, Image, blend | `ui-egui/src/widgets/colorpicker.rs` | Missing Video; Midas disables Noise too. |
| Stroke paint of any fill type | `Node.strokes: Vec<Paint>` | FigCraft is ahead (several stroke paints). |
| Stroke geometry | `pathops` (`stroke_outline_of`, dashes, caps, per-side), already adapted into `MIDAS: crates/midas-core` (`THIRD_PARTY_NOTICES`) | Same origin. |
| Component-to-code | `engine/src/dev/component_codegen.rs` (React, Vue, SwiftUI, Compose, Flutter), `dev/codegen/html.rs` | Different philosophy: from the design tree, not from a schema. |
| MCP | `crates/mcp` (4.7k lines, read and write tools) | FigCraft is ahead (Midas MCP is read-only). |
| Libraries and publish | `format/src/library.rs`, `assets/library*`, `apps/figcraft-server` | Partly: FigCraft has team libraries without accounts (gap 10). |

### 2.3 Where FigCraft is ahead

Auto layout, grids and constraints; vector networks, booleans on curves, masks; text engine with bidi and shaping; effects (blur, glass, noise, texture); `.fig`, Sketch, Penpot, REST import; PDF, SVG, Android Vector Drawable export; live collaboration, branching, version history; plugins (WASM sandbox, widgets); Dev Mode; boards; 24 locales; accessibility work; prototype actions including Smart Animate, overlays, variables and expressions, devices. All of it is kept.

## 3. Mapping: Midas-only features onto FigCraft

### 3.1 Proposed crates and layers

Layering is enforced by `xtask/src/layers.rs` (`TABLE` of crate to layer). A crate not in `TABLE` is silently skipped (`let Some(&l) = layer.get(&name) else { continue }`), so every new crate must be added there.

| New crate | Layer | Holds |
|---|---|---|
| `figcraft-behavior` | L2 (depends on doc, color, layout) | Schema types and validator, registry, template data (JSON, embedded), binding, token resolver, anatomy planner, slot rules, Auto Fix plan, base-token importer, diagnostics, load-rule validation. Pure functions over `Document`, no egui. |
| `figcraft-play` | L4 (depends on doc, behavior, svg, text, color, geom) | Scene-to-HTML/CSS compiler, stylesheet emitter (ported `codegen.js` rules), incremental patch generator, wire protocol types. Pure, compiles for wasm. |
| `figcraft-play-host` (binary) | app | wry/tao host. Desktop only. |
| engine additions | L5 | `cmd/behavior.rs` commands, hooks in `exec.rs` and `derived.rs`. |
| ui-egui additions | L6 | Behavior tab, pickers, Play launcher. |

Also add `wry` and `tao` to the forbidden-below-L6 list in `layers.rs` next to `egui`, `eframe`, `rfd`, `winit`.

### 3.2 Hooks in the engine

- `Session::execute` check chain (`session/exec.rs`): add `check_behavior_edit(id, &before)` returning `EngineError::Disabled`, so a behavior-breaking edit is rolled back with the reason. This is Midas's `behaviorProblem` backstop and "Not applied" toast.
- `derived::normalize` (`derived.rs`): add a `behavior::sync` pass on touched nodes after instances follow mains and before auto layout re-flow. It should only visit the touched masters and instances, which is the equivalent of Midas's commit-scope (`MIDAS: packages/builder-core/src/commit-scope.js`) skipping work for plain layers.
- Open: `crates/format` load path runs Midas validation after parse and fails with an error (premise 5).

### 3.3 Mapping table

Sizes: S under about 1k lines, M about 1k to 5k, L above 5k (**estimated** Rust lines including tests; no FigCraft or Midas hours are claimed). Risk is Low, Medium or High.

| Midas feature | Plugs into | Data needed | Size | Risk | Depends on |
|---|---|---|---|---|---|
| Typed behavior data and load rules (1.4) | `doc/node.rs`, `lib.rs`, `collab/change.rs`, `format`, `assets/components/sync.rs` | `Behavior` enum, `BehaviorDoc`, `BEHAVIOR_VERSION` | M | Medium (collab, sync reset) | none |
| Template schema, registry, validator | `figcraft-behavior` | JSON templates, `validate-schema.js` rules (897 lines) | L | Medium | typed data |
| Binding, auto-match, props store, `visibleIf`, derived props | `figcraft-behavior`, `cmd/behavior.rs` | `Binding`, `Props` | M | Low | schema |
| Token resolver and state bindings (persistent states, preview lookup) | `figcraft-behavior`, `render` (preview lookup) | tokens from collections and modes | M | Medium (preview must not enter the document) | binding |
| Typography and effect variable types | `doc/styles.rs`, `Variables::resolve`, collab, `.fig` export, codegen, variable UI | two new `VariableType`s and values | L | High (touches every variable consumer; FigCraft text styles bind per field, Midas aliases whole styles) | none |
| Managed slot/part layers and anatomy sync | `figcraft-behavior`, `assets/components` guard and sync | `Managed`, `Part`, `Group` markers | L | **High** (instance structure lock, see below) | typed data, schema |
| No-hidden-layers rule | `check_behavior_edit`, diagnostics | rule over masters and instances | S | Low (but see overlaps) | anatomy |
| Content slot rules (`SlotDef`, min/max/accepts/repeat/seed) | `PropDef::Slot` extension, `figcraft-behavior` | extend `PropDef::Slot` with optional fields (serde defaults) | M | Medium | schema |
| Auto variable: Generate base, Auto Fix, bind-tokens, colour presets | `figcraft-behavior`, commands `behavior.generateBase`, `behavior.autoFix` | `base-snapshot.json`, `roles.json`, `antd-sizes.json`, `ant-map.json` | L | Medium (data files, provenance) | variable types, schema |
| Auto design system | commands plus a layout planner | `ICON_SET` glyph paths, template order | M | Medium (icons are code-embedded CC BY 4.0 data) | auto variable, anatomy |
| Diagnostics | `figcraft-behavior`, Behavior panel | codes from `diagnostics.js`, `document-check.js` | M | Low | schema |
| Custom templates | `Document.behavior.templates`, panel schema box | validated JSON in the document | S | Low | validator |
| Behavior panel (UI) | `ui-egui/src/panels/` plus `RightTab::Behavior` | none | L | Medium | schema, diagnostics |
| Video paint | `color::PaintKind`, `render/paint.rs`, `format`, colour picker | `Video` kind | M | Medium (render per frame, file embedding) | none |
| Variable binding of paint params | `bound` key scheme, `variable_ui.rs` | keys like `fills/N/...` | S | Low | none |
| Play (DOM) | `figcraft-play`, `figcraft-play-host`, UI launcher | scene exposure (4.3) | L | **High** (text parity, runtime JS rule) | behavior for templates; spike needs none |
| Prototype validator codes and conditional rules | `engine/src/cmd/proto*.rs` | Midas error codes | S | Low | none |
| Typed React/TSX export | extend `dev/component_codegen.rs`, or new emitter in `figcraft-behavior` | schema | L | High (two emitters; see 4.4) | behavior, play |
| Library publish, registry, docs site, server | keep Midas server, or port later | none now | L | n/a | decision, not in this plan |
| FormItem validation, Code Builder, icon server | not built in Midas | n/a | n/a | n/a | out of scope |

### 3.4 Overlaps and who wins

| Area | FigCraft | Midas | Winner and note |
|---|---|---|---|
| Layer visibility | Boolean component property can toggle `visible` (`prop_refs["visible"]`) | No hidden layers: parts are added and removed | Both. The rule is enforced only for nodes bound to a behavior template; FigCraft-style properties stay for everything else. |
| Instance structure | Instances never gain or lose layers except in slots (`assets/components/guard.rs` `check_structure_edit`) | Managed layers exist only on instances | Midas for template instances; FigCraft guard stays elsewhere. Needs a guard and sync exemption (below). |
| Slots | `PropDef::Slot { preferred }`, content owned by instance | `SlotDef` with min, max, accepts, repeat, seed | Extend FigCraft's slot; Midas rules apply when a template declares them. |
| Prototype actions | Set variable, overlays, expressions, Smart Animate | Conditional with a validator | FigCraft's action set wins; add Midas's validator codes for conditional. |
| Fill picker | Existing | Rework to Figma layout | FigCraft's; add Video. |
| Variable types | 4 types | plus typography and effect | Midas types are additive. |
| File load | repair and report | fail to open | Midas's for `behavior` data only. |
| Undo | document snapshots | transaction history | FigCraft's. |
| Clean room | Figma behaviour from public docs | antd structure reference, MIT | Both apply to their own domains. |

### 3.5 The structure conflict (highest-risk item)

`check_structure_edit` refuses any command that adds, removes or reorders layers inside an instance outside its slots (`assets/components/guard.rs`), and `fresh_root` and the sync rebuild an instance from its main, mapping only layers the main has (`sync.rs`: `order` is built from `comp.children`). Midas managed icon slots, managed text and conditional containers are instance-only layers.

Options:
- **(a) Exemption (recommended).** Mark managed layers with the typed `Managed` marker. `structure_locked` and `check_structure_edit` skip nodes carrying it, `sync_instance` preserves instance-only children that carry it (reinserting them at their stored index), and `behavior::sync` owns their lifecycle. Cost: a careful change to `sync.rs` and `patch.rs` (the incremental patcher, 548 lines).
- **(b) Model icon slots as FigCraft Slot properties.** Content then lives in the slot override. This fits "slot", but Midas says an icon slot is a managed layer with a nested icon instance bound by engine-owned bindings, and master holds no icon choice. It loses the one-owner rule.
- **(c) Put every part in the main and drive `visible`.** Violates no-hidden-layers.

I verified the guard and the root rebuild; I did not read `patch.rs` or `instances.rs` in full, so the effort for (a) is an estimate.

## 4. Play

### 4.1 How FigCraft preview works today

- Present mode is pixel-based. `ui-egui/src/present/` (about 4.4k lines, measured) holds the `Presenter` (`present.rs:263`): current frame, history, overlays, scroll offsets, a `design` document (variables and variants changed by the prototype) and a laid-out `doc`. `draw.rs` renders the current frame with `figcraft_render::render_node` into an egui texture; transitions and Smart Animate build intermediate documents and render them (`transition.rs`, `smart_interpolate`). Pointer input maps to hotspots (`layout.rs`, `input.rs`); actions run in the engine (`engine/src/proto_run.rs`) and in `present/actions.rs`.
- Consequences for Midas: no real text input, no real links, no CSS states. Hover, press and focus come only from interactions (`OnHover`, `MouseEnter`, `ChangeTo`) swapping variants. Prototype state is in `ui-egui` (L6), so the state machine is not reusable by a second front end without moving it to the engine. I did not measure how much of `present/` is pure state.

### 4.2 Proposal: Play as a separate webview window

Keep Present unchanged (it keeps Smart Animate, devices, controllers). Add **Play** as a second mode, as in `MIDAS: docs/prototype-spec.md` ("Play"):

```
Document (Session)  --changed_ids(before)-->  figcraft-play::compile / diff
        |                                              |  JSON patches (stdio or IPC)
   player state machine  <---- events {node, kind} ----+----  figcraft-play-host  (wry window: DOM)
```

- **Process.** Run the host as a separate process speaking JSON lines (a `--play` subprocess), not as a second native window inside eframe. Reasons: a webview crash cannot crash the editor (consistent with "never crash"); no entanglement with the forked winit and egui-wgpu (`Cargo.toml` pins storytold forks of egui and winit); wry's event-loop requirements (main thread on macOS) stay out of eframe's loop. Trade-offs: IPC plumbing, window focus and placement, one more binary to package. An in-process child viewport is possible (egui 0.37 has several windows per `docs/hardware-parity.md`) but unverified with wry.
- **Compiler** (`figcraft-play`, pure, wasm-clean): walk the page from the start frame and emit one element per node with `data-n="<NodeId>"`. Auto layout frames become flex containers using the same alignment tables as the canvas (`figcraft_layout`), other frames absolute positioning, clipping per `clip`. Fills, strokes, effects map to CSS or inline SVG; vectors to inline SVG built from the same path data as `figcraft_svg`. The existing HTML/CSS emitter `engine/src/dev/codegen/html.rs` and `css.rs` (432 and 398 lines) is a starting point, but it is an Inspect-panel code view ("a starting point, not a pixel-exact translation", `dev/codegen.rs` header). Reuse its CSS declarations only where they are exact; do not run Play and Inspect code on two diverging emitters long term.
- **Incremental patches.** `normalize` consumes `take_dirty`, so the player must not use it. Use `Document::changed_ids(&before, limit)` (`crates/doc`, used in `session/exec.rs` `finite_check_candidates`) between the last sent snapshot and the current one. Per changed node: recompile only that node's own style and attributes, hash it, and send `set(n, attrs, style)`; structural changes (children list differs) send `replace(n, html)`; removed ids send `remove(n)`. Layout changes ripple to descendants of an auto layout parent, so recompile children of any touched auto layout frame.
- **Events back.** The host posts `{node, event: click|enter|leave|down|up|focus|blur|input|submit, value?}`. The editor feeds clicks and hovers into the existing prototype action logic and answers with `show(frame, transition)`. The state machine in `present/` needs to move below L6 to be shared (estimated M).
- **Smart Animate** has no direct DOM equivalent; FLIP-like tweening can cover position and size (Midas does FLIP for toggles) but not fills, text or strokes per `docs/prototyping-parity.md`. Keep Smart Animate in Present; Play falls back to dissolve and says so.
- **wasm.** wry is desktop only (my understanding; not verified for FreeBSD or Wayland). The compiler crate has no wry dependency. The web build can host Play in an iframe through the existing `?host=parent` bridge (`apps/figcraft-web`), out of scope for the spike.

### 4.3 What the scene must expose

| Need | Source in FigCraft today | Gap |
|---|---|---|
| Stable node ids | `NodeId` | none |
| Post-layout box per node | `transform`, `width`, `height` after `normalize` | none for absolute; flex needs the auto layout fields (`layout`, `child_layout`) which exist |
| Element kind: button, input, textarea, link, label | none | typed `Behavior` marker (or `plugin_data` in the spike) |
| State styling: hover, active, focus, disabled | `Interaction` triggers only | per-state token overrides from the behavior stylesheet; disabled as a persistent prop |
| Typed input: value, placeholder, max length, type, masking | none | control props in `Behavior::Props` |
| Real links | `Action::OpenUrl { url }` exists | map it to `<a href>`; Midas allows http and https only (`prototype-spec.md`) |
| Click and event routing | hotspots in Present | emit `data-n` and a single delegated listener |
| Fonts | `crates/text` font database, `CRAFT_FONTS_DIR` embed | serve the same font bytes to the webview (see 4.5) |
| Images | `Document.images` by hash | serve through `data:` or a `play://` handler |
| Motion | transitions in `proto.rs` | CSS transition table plus Midas motion tokens |
| Reduced motion | none | `prefers-reduced-motion` |
| Disabled layers | none | no pointer events inside a disabled root |

### 4.4 The runtime-JS question

Midas's Play behaviours (toggle, press, FLIP, wave, reveal, copy, dismiss, step) are JS today. Under rule 4, either keep a small generated runtime emitted from Rust (a Rust `const` string built by the compiler, no `.js` source in the tree), or relax the rule for the fork. Prefer CSS for everything stateful that CSS can do (`:hover`, `:active`, `:focus-visible`, `:disabled`, `:has()`), and keep the runtime to event delegation and patch application.

### 4.5 Text parity

The 0.5 px gate (`MIDAS: AGENTS.md`) cannot hold unchanged. Proposed strategy: (1) load the same font bytes in the webview that `crates/text` shapes with; (2) emit every text box at the width and height the canvas computed, with explicit line breaks only where the canvas wrapped, so wrapping cannot differ; (3) let only `<input>` and `<textarea>` contents be browser-shaped; (4) relax the gate for text boxes to a metric tolerance (to be set from a first measurement) and keep 0.5 px for boxes, borders and non-text parts. Whether this reaches acceptable parity is not verified.

## 5. egui constraints

- **Structure.** `ui-egui` is a thin shell: panels are functions over `&mut FigApp` reading `app.session` and acting through `app.run(id, params)` or `set_props` (`lib.rs` header, `app/commands.rs`). Right-panel tabs are `RightTab { Design, Prototype, Inspect }` (`state.rs:2`); `panels/right.rs` (881 lines) plus `panels/right/{effects,inspect,menu,paints,stroke}.rs` build the sections with `inspect_section` and `prop_row`. A Behavior tab fits as a fourth `RightTab` and `panels/behavior/`.
- **Theming.** `theme::Tokens` (about 40 colour fields, UI3 light and dark, `theme.rs`) with WCAG contrast guarantees noted in the field docs. Midas has two kinds of tokens: the design's variables (data) and the editor's chrome. Only the chrome uses `Tokens`.
- **Widgets.** `widgets.rs` (1474 lines): `number_field` with scrubbing, `text_field`, `multiline_field`, `combo_field`, `dropdown`, `segmented`/`segmented_text`, `checkbox`, `radio`, `slider`, `swatch`, `disclosure`, `section_header`. These cover the controls a schema-driven panel needs (enum, boolean, string, number, segmented).
- **Text input and IME.** Fields use egui text edit through `widgets::text_input`. On-canvas text editing has its own IME tracker (`app/ime.rs`: one `observe` per pass, "fork contract"), tied to the pinned `storytold/egui` and `winit` forks. Panels are not where IME is hard; the canvas editor is, and it already exists. Do not add a second IME consumer without reading `app/ime.rs` and `a11y.rs`.
- **Accessibility.** `a11y.rs` exists, but `docs/hardware-parity.md` notes eframe 0.37 gives only the main window an AccessKit adapter, and gap 22 says screen readers never ran.
- **Localization.** Strings go through `i18n::tr`, with 24 locales and `cargo xtask i18n-report` (`docs/gaps.md` gap 5). New panels need keys.
- **Missing for the Midas panels**:
  - *Fill picker*: exists (`widgets/colorpicker.rs`: solid, gradient, image, pattern, blend, eyedropper). Missing Video, and parameter binding for pattern and video; I did not check for CSS/HSL text entry (the file has `model_fields`).
  - *Behavior panel*: new. Needs a generic schema row renderer (enum, boolean, string, number, image, icon pick, variable-prop dropdown), a state selector, a slots block, a diagnostics list with an Auto Fix button, and a picker for "component in file" (the assets swap search is the nearest existing code).
  - *Variable binding UI*: `panels/variable_ui.rs` (677 lines) binds `Color`, `Number`, `String`, `Boolean` variables to field keys. Missing: typography and effect pickers and editors (a floating window with per-member alias), and mode chips for bound parameters.
  - *Floating windows docked beside the right panel*: FigCraft opens pickers as egui windows (`colorpicker::window_id`, `popover.rs`, `windows.rs`). Docking is a placement rule to add.

## 6. Renderer

- **Boundary.** There is no intermediate scene or display list. `figcraft_render::render_view(doc, page, &View, &Options) -> Option<Pixmap>` draws straight from the `Document` onto tiny-skia (`render/src/lib.rs`), `render_node` rasterises for export, `Retained` re-renders only changed areas. The canvas draws on a worker thread and shows the result as an egui texture (`ui-egui/src/canvas/render_worker.rs`); overlays are drawn by egui. `docs/architecture.md` states the CPU renderer is the single source of pixels for canvas, exports and the browser.
- **GPU option.** FigCraft's own estimate for a GPU path is 30 to 60 h (gap 7, `docs/gaps.md`, **estimated**). Replacing only the interactive canvas with vello or wgpu is feasible in principle, keeping tiny-skia for export and pixel tests, but it needs a new display-list layer between `Document` and the two backends, because today `draw.rs` (1184 lines) walks the document directly. It also has to cover blur, glass, grain, backdrop effects, masks and blend modes, or accept a CPU fallback for those. Risks: canvas and export would no longer be pixel identical, so pixel tests need tolerances; vello's pinned wgpu version has to match `egui-wgpu` in the forked egui (`docs/hardware-parity.md` mentions wgpu 30; vello's requirement not verified); text glyph runs would come from `crates/text` outlines.
- **Midas's stance** is the same (`MIDAS: docs/rust-plan.md` Phase 5, gated). Recommendation: do not start it before Play and behavior are in; revisit when Midas's 5k and 20k node targets (`MIDAS: docs/perf-baseline.md`) are measured against FigCraft's retained CPU renderer. No benchmark was run for this document.

## 7. Plan

### 7.1 Phases, smallest vertical slice first

| Phase | Slice | Exit |
|---|---|---|
| 0. Fork hygiene | Record provenance and the rule deviations (premise 2); add `docs/midas-on-figcraft-analysis.md` as the baseline; decide licence of Midas code and the `ATTRIBUTION.md` rows. | Decisions written; no code. |
| 1. **Spike: Play window for one frame** | New `figcraft-play` (pure) and `figcraft-play-host` (wry, separate process). Compile one selected frame to static HTML/CSS; real `<input>` for a text layer flagged control, real `<a>` for `OpenUrl`, CSS hover/active/focus/disabled from a fixed table; push one edit as a patch via `changed_ids`; click events logged back. Marker data in `plugin_data` only, no model change. | Type in the input; hover and press visibly respond; an edit on the canvas patches the DOM without reload; boxes within a tolerance of the canvas render, measured once. |
| 2. Behavior data | Typed `Behavior` on `Node` and `Document`, collab entries, `fresh_root` reset, strict load validation, `inspect` output. | Round trip through save, load, collab, copy, library copy; mismatch fails to open. |
| 3. Behavior core | `figcraft-behavior`: schema, validator, registry, binding, auto-match, token resolver, state bindings, diagnostics; JSON templates for Button and Input. | Ported tests green (below). |
| 4. Managed layers | Anatomy planner, instance structure exemption, no-hidden-layers check, `behavior.*` commands, `check_behavior_edit`. | Toggling a prop on and off restores the default tree; edits that break structure roll back. |
| 5. Auto variable and design system | Base snapshot, Auto Fix, bind-tokens, Generate Design System, typography and effect variable types. | Idempotent runs; same outputs as Midas fixtures. |
| 6. UI | Behavior tab, typography and effect editors, Video fill, parameter binding. | Panels drive commands only; screenshots reviewed (`AGENTS.md`: look at the result). |
| 7. Play complete | Template stylesheet and motion, toggle and press actions, scaling modes, viewer chrome, parity gates with a webview test harness. | Gates (4.5) defined and green. |
| 8. Export and library | TSX emitter decision (extend `dev/component_codegen.rs` or the schema emitter), library publish decision (keep Midas server or port). | Owner decision first. |
| 9. GPU canvas (gated) | Display-list layer and vello or wgpu backend for interactive use. | Only if profiling shows the CPU canvas is the bottleneck. |

A `.midas` importer is not planned: Midas's rule is "old files are regenerated" (`AGENTS.md`). The equivalent is porting the fixture generators (`MIDAS: scripts/generated-part.mjs`, `make-testing-file.js`) to Rust test builders. `MIDAS: testing.midas` (200 KB, flat per-page node arrays with `parentId`, `x/y/w/h`, `vars`, `behavior`) is the reference for what a generated file contains.

### 7.2 Midas tests and spec items to port as Rust tests

JS test volume (**measured, approximate**): `packages/builder-core/test` 254 files, `apps/studio/test` 71 files plus 30 browser files, about 2,368 `it`/`test` matches across builder-core, studio and server tests. `MIDAS: AGENTS.md` quotes older baselines (842 and 53), which are stale.

Port (pure logic, no browser):
- `no-hidden-layers`, `always-visible-slot`, `anatomy-no-text-slot`, `icon-slot`, `icon-slot-order`, `parented-slot`, `primary-slot`: anatomy and managed layers.
- `auto-fix`, `auto-fix-collections`, `auto-match`, `design-system`, `design-system-run`, `baseVariables` (studio): auto variable and design system.
- `slot-rules`, `slot-list`, `slot-property`, `slot-diagnostics`, `slot-accepts-templates`, `content-slots`, `template-slots`: slots.
- `document-check`, lazy-open (`behavior-lazy-open.test.js`), `data.js` load rules: strict load.
- Prototype `validate`, `conditional`, `plan` (copy/paste/delete remap): validator codes (`conditional-empty`, `condition-*`, `trigger-action-mismatch`, `overflow-on-instance`).
- Codegen snapshots (`icon-button-wave` pins full-output hashes of Button and Input exports).
- Behavior-spec items: auto-match rules, token resolver reason codes (`dimension-unbound` ... `no-dimension`), persistent versus preview state rules, lineHeight in px, one `behavior` key with `version`, fail-to-open.

Cannot port without a webview harness: every `*.browser.test.mjs` (canvas vs Play 0.5 px, export vs Play, strict parity, divider, avatar, affix). These need Play host automation (for example driving wry and reading the DOM) and are phase 7 work.

### 7.3 Licence obligations

- Keep `LICENSE-MIT`, `LICENSE-APACHE` and `NOTICE` ("Copyright (c) 2026 ArtCraft Team and the FigCraft contributors"). Code adapted into the fork keeps its headers.
- Midas `THIRD_PARTY_NOTICES` already records code adapted from FigCraft; keep it for any copy that goes back.
- Midas code needs an explicit licence from its owner (premise 7) before it joins this tree; it has none today.
- `ATTRIBUTION.md` rows for: Solar glyphs (CC BY 4.0, embedded in code), antd-derived token values (MIT), any bundled placeholder image (`placeholder-image.js`). Fonts stay in `storytold/craft-fonts`.
- No ArtCraft logos, name or wordmark from `docs/brand/` in Midas branding; the fork needs its own app icon and name before release.
- New dependencies (wry, tao, any template or JSON tooling): check each crate's licence against the allowlist in `AGENTS.md`. I found no `deny.toml` or `about.toml`, so nothing enforces it automatically.

## Not verified

- Nothing was built, run or benchmarked, in either repo. All claims are from reading source and docs.
- Midas's inventory reflects `feat/fahmi` at `1266957` plus 61 uncommitted modified files; I did not diff them or check that every "built" item works. `AGENTS.md` itself lists known failing browser tests and unchecked browser behaviour.
- `docs/roadmap.md` (Midas) is stale on library publish; I resolved it from `apps/server` code existing, not from running it.
- wry on Linux (Wayland), FreeBSD, and alongside the forked winit and egui-wgpu; whether a webview child window can live in an eframe viewport; wry's licence and its transitive crates.
- Licences of any new dependency (no `deny.toml` found).
- Text parity feasibility (4.5): no measurement was taken.
- `patch.rs` and `instances.rs` of the components module, the full `present/` state machine, and `catalog::parity`'s treatment of extra command ids were not read in full, so the sizes for the structure exemption, the Present refactor and command-id handling are estimates.
- Whether `.fig` or Penpot export should carry Midas data, and how collab merges concurrent edits of a `Behavior` value (property-level last-writer-wins is my reading of `change.rs`).
- vello's wgpu version against the forked egui-wgpu.
- Whether FigCraft's colour picker offers CSS and HSL entry.
- JS test counts are grep matches, not a test run.
