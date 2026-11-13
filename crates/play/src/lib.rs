//! FigCraft Play: compiles one frame to HTML and CSS for the Play window, and plans the patches
//! that keep the window in step with later edits. Pure: no webview, no GUI, no network. The
//! window itself lives in `figcraft-play-host`, a separate process (see `docs/FORK.md`).
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]
//!
//! Every element carries `data-n="<NodeId>"`. Positions and sizes come from the canvas's own
//! layout (the document after `normalize`), so the page places boxes where the canvas drew them.
//! Auto layout frames become flex containers with the canvas's alignment tables; text is shaped by
//! the same font bytes and broken only where the canvas wrapped it.

pub mod compile;
pub mod patch;
pub mod protocol;
pub mod style;

pub use compile::{CONTROL_KEY, Compiled, Control, DISABLED_KEY, PlayError, compile_frame, control, is_web_url, link};
pub use patch::{PATCH_LIMIT, Plan, plan};
pub use protocol::{Command, Event, Inbound};

#[cfg(test)]
mod tests;
