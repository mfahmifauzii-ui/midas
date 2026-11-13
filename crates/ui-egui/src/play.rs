//! The Play window from the editor's side. It starts `figcraft --play` as a child process, sends
//! it the compiled frame as JSON lines on stdin, and keeps it in step with later edits through
//! `figcraft_play::plan`. The webview lives in that child, so a crash there never reaches the editor
//! (docs/FORK.md). Nothing here depends on wry.

use std::io::Write;
use std::process::{Child, ChildStdin, Command as Process, Stdio};

use figcraft_doc::{Document, NodeId};
use figcraft_play::{Command, Plan};

/// A running Play window and what it last showed.
pub struct PlayLink {
    child: Child,
    stdin: ChildStdin,
    frame: NodeId,
    /// The document the window shows (the baseline for the next plan).
    sent: Document,
}

impl PlayLink {
    /// Start the Play window for `frame` of `doc`.
    pub fn open(doc: &Document, frame: NodeId) -> Result<Self, String> {
        let compiled = figcraft_play::compile_frame(doc, frame).map_err(|e| e.to_string())?;
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let mut child = Process::new(exe)
            .arg("--play")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .spawn()
            .map_err(|e| format!("cannot start the Play window: {e}"))?;
        let Some(stdin) = child.stdin.take() else {
            let _ = child.kill();
            return Err("the Play window has no input".into());
        };
        let mut link = Self { child, stdin, frame, sent: doc.clone() };
        link.send(&Command::Load {
            title: compiled.title,
            width: compiled.width,
            height: compiled.height,
            html: compiled.html,
            css: compiled.css,
        })?;
        Ok(link)
    }

    /// Bring the window up to date with `doc`. `false` when the window has closed or the frame is gone.
    pub fn sync(&mut self, doc: &Document) -> bool {
        if matches!(self.child.try_wait(), Ok(Some(_)) | Err(_)) {
            return false;
        }
        let commands = match figcraft_play::plan(&self.sent, doc, self.frame) {
            Ok(Plan::Patch(cmds)) => cmds,
            Ok(Plan::Reload(c)) => vec![Command::Load { title: c.title, width: c.width, height: c.height, html: c.html, css: c.css }],
            Err(_) => return false,
        };
        for cmd in &commands {
            if self.send(cmd).is_err() {
                return false;
            }
        }
        if !commands.is_empty() {
            self.sent = doc.clone();
        }
        true
    }

    fn send(&mut self, cmd: &Command) -> Result<(), String> {
        let line = serde_json::to_string(cmd).map_err(|e| e.to_string())?;
        writeln!(self.stdin, "{line}").and_then(|()| self.stdin.flush()).map_err(|e| e.to_string())
    }
}

impl Drop for PlayLink {
    fn drop(&mut self) {
        let _ = self.send(&Command::Close);
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
