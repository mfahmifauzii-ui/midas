# FigCraft fork: provenance and decisions

Status: permanent fork. Written 2026-11-13 with the Play spike (Phase 0 and Phase 1 of
[`midas-on-figcraft-analysis.md`](midas-on-figcraft-analysis.md)).

## Provenance

- Base: FigCraft workspace version 0.18.1 (`Cargo.toml`), copied as a working tree. This copy is not
  a git checkout, so there is no upstream commit hash in it. The upstream is `storytold/figcraft`
  (MIT OR Apache-2.0, `LICENSE-MIT`, `LICENSE-APACHE`, `NOTICE`); keep those files and the
  copyright notice in every copy.
- Midas (the ArtCraft-team web prototype studio, branch `feat/fahmi`) is the source of the features
  this fork adds. Its analysis is [`midas-on-figcraft-analysis.md`](midas-on-figcraft-analysis.md).

## What this fork is

- A permanent fork. The egui editor, the design features and the FigCraft behaviour are kept.
  FigCraft behaviour wins every overlap, as the analysis's section 3.4 lists.
- Midas features are added on top: a behaviour model, managed layers, a Play window and the rest of
  the plan in the analysis. Each Midas feature lands as its own phase and has its own tests.
- The house rules in `AGENTS.md` still apply. Where this fork deviates, the deviation is recorded
  below and nowhere else.

## Recorded deviation from Rust Craft rule 4

Rule 4 (`AGENTS.md`, "Rust Craft repositories") says: no handwritten JavaScript or TypeScript, and
no Tauri, Electron or webview shell, in native Craft apps.

This fork deviates for the **Play window only**:

- `crates/play-host` (`figcraft-play-host`) opens a webview (wry) in a separate process, started as
  `figcraft --play`.
- The Play runtime, a small script that applies patches and reports events, is handwritten
  JavaScript. It is a string constant in `crates/play-host`, not a `.js` file in the tree.
- Nothing else in the editor may use a webview or handwritten JavaScript. The editor (`crates/ui-egui`,
  `apps/figcraft`) must not depend on wry. The check is that `cargo xtask layers` keeps wry out of
  every crate below the app layer, and the Play runtime lives only in `figcraft-play-host`.

The deviation is ours. It is not an exception in the shared rules and does not change
`storytold/craft-contrib`. Propose any change to those rules there.

## Midas code: licence before copy

- Midas has no `LICENSE` file at its root. Midas code may be copied into this repository only after
  its owner has licensed it under MIT OR Apache-2.0 in writing, and that licence is recorded here.
- No Midas code was copied in Phase 0 or Phase 1. The Play spike is written from FigCraft's own code
  (`crates/doc`, `crates/layout`, `crates/text`, `crates/svg`) and from the CSS declarations already
  in this repository (`crates/engine/src/dev/codegen`).
- Status of the owner's licence grant: **not received** (as of this note).

## ATTRIBUTION.md rows by hand

`cargo xtask assets` checks files under `assets/`, `docs/brand/` and `docs/images/` only. It does
not see data embedded in Rust source. So any embedded third-party data gets its `ATTRIBUTION.md`
row added by hand, in the same change that adds the data. Known cases:

- Solar icon glyph paths (CC BY 4.0), if they are ever embedded. Not present yet: no row until the
  data lands.
- Antd-derived token values (MIT), if they are ever embedded. Not present yet.

Fonts stay in `storytold/craft-fonts`, as `AGENTS.md` requires.
