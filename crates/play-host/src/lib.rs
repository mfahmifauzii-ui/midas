//! The Play window's host: a webview (wry) window in its own process. `figcraft --play` runs
//! [`run`]; the editor writes [`figcraft_play::Command`]s as JSON lines to its stdin, and the host
//! logs each user interaction as one JSON line `{node, event, value?}` on stderr.
//!
//! This is the one place in the fork with a webview and handwritten JavaScript (the recorded
//! deviation in `docs/FORK.md`). The editor never links it: it starts the program as a child, so a
//! crash here ends the child and leaves the editor running.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

use std::io::BufRead;

use figcraft_play::{Command, Inbound};
use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy, EventLoopWindowTarget};
use tao::window::{Window, WindowBuilder};
use wry::{NewWindowResponse, WebView, WebViewBuilder};

/// What the event loop receives from the other threads.
#[derive(Debug)]
enum UserEvent {
    /// A command from the editor (stdin).
    Command(Box<Command>),
    /// A message from the page's runtime (IPC).
    Inbound(Inbound),
}

/// The page the window opens with: the runtime, and an empty stage that the first `load` fills.
const SHELL: &str = concat!(
    "<!doctype html><html><head><meta charset=\"utf-8\"><style id=\"fc-style\"></style></head>",
    "<body><div id=\"fc-stage\"></div><script>",
    include_str!("runtime.js"),
    "</script></body></html>"
);

/// Run the Play window until it closes or the editor closes stdin. Returns the process exit code.
pub fn run() -> i32 {
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    spawn_stdin_reader(event_loop.create_proxy());
    let proxy = event_loop.create_proxy();
    let mut host = Host::default();
    event_loop.run(move |event, target, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::UserEvent(UserEvent::Command(cmd)) => {
                if !host.command(*cmd, target, &proxy) {
                    *control_flow = ControlFlow::Exit;
                }
            }
            Event::UserEvent(UserEvent::Inbound(msg)) => host.inbound(msg),
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => *control_flow = ControlFlow::Exit,
            _ => {}
        }
    })
}

/// Reads commands from stdin, one JSON object per line, until stdin closes (then closes the window).
fn spawn_stdin_reader(proxy: EventLoopProxy<UserEvent>) {
    std::thread::spawn(move || {
        let stdin = std::io::stdin();
        for line in stdin.lock().lines() {
            let Ok(line) = line else { break };
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<Command>(&line) {
                Ok(cmd) => {
                    if proxy.send_event(UserEvent::Command(Box::new(cmd))).is_err() {
                        return;
                    }
                }
                Err(e) => eprintln!("figcraft play: bad command: {e}"),
            }
        }
        let _ = proxy.send_event(UserEvent::Command(Box::new(Command::Close)));
    });
}

/// The window and its webview, made on the first `load`. Commands wait until the page is ready.
#[derive(Default)]
struct Host {
    window: Option<Window>,
    webview: Option<WebView>,
    ready: bool,
    pending: Vec<Command>,
}

impl Host {
    /// Handle one command. `false` when the window should close.
    fn command(&mut self, cmd: Command, target: &EventLoopWindowTarget<UserEvent>, proxy: &EventLoopProxy<UserEvent>) -> bool {
        match cmd {
            Command::Close => return false,
            Command::Load { ref title, width, height, .. } if self.webview.is_none() => {
                if let Err(e) = self.open(title, width, height, target, proxy) {
                    eprintln!("figcraft play: cannot open the window: {e}");
                    return false;
                }
            }
            _ => {}
        }
        if self.ready {
            self.apply(&cmd);
        } else {
            self.pending.push(cmd);
        }
        true
    }

    fn open(&mut self, title: &str, width: f64, height: f64, target: &EventLoopWindowTarget<UserEvent>, proxy: &EventLoopProxy<UserEvent>) -> Result<(), String> {
        let window = WindowBuilder::new()
            .with_title(title)
            .with_inner_size(LogicalSize::new(width.clamp(16.0, 10_000.0), height.clamp(16.0, 10_000.0)))
            .build(target)
            .map_err(|e| e.to_string())?;
        let ipc = proxy.clone();
        let webview = WebViewBuilder::new()
            .with_html(SHELL)
            .with_ipc_handler(move |req| match serde_json::from_str::<Inbound>(req.body()) {
                Ok(msg) => {
                    let _ = ipc.send_event(UserEvent::Inbound(msg));
                }
                Err(e) => eprintln!("figcraft play: bad page message: {e}"),
            })
            .with_navigation_handler(|url| allow_navigation(&url))
            .with_new_window_req_handler(|url, _features| {
                open_link(&url);
                NewWindowResponse::Deny
            })
            .build(&window)
            .map_err(|e| e.to_string())?;
        self.window = Some(window);
        self.webview = Some(webview);
        Ok(())
    }

    /// A message from the page: the runtime is ready, or the user did something to log.
    fn inbound(&mut self, msg: Inbound) {
        match msg {
            Inbound::Ready => {
                self.ready = true;
                for cmd in std::mem::take(&mut self.pending) {
                    self.apply(&cmd);
                }
            }
            Inbound::Event { .. } => {
                if let Some(ev) = msg.as_event() {
                    match serde_json::to_string(&ev) {
                        Ok(line) => eprintln!("{line}"),
                        Err(e) => eprintln!("figcraft play: cannot log an event: {e}"),
                    }
                }
            }
        }
    }

    /// Run one command in the page.
    fn apply(&self, cmd: &Command) {
        let Some(webview) = self.webview.as_ref() else { return };
        let Ok(json) = serde_json::to_string(cmd) else { return };
        let script = match cmd {
            Command::Load { html, css, .. } => {
                let (Ok(h), Ok(c)) = (serde_json::to_string(html), serde_json::to_string(css)) else { return };
                format!("fcLoad({h}, {c});")
            }
            _ => format!("fcApply([{json}]);"),
        };
        if let Err(e) = webview.evaluate_script(&script) {
            eprintln!("figcraft play: cannot update the page: {e}");
        }
    }
}

/// The page itself may navigate (its own loads); any other address opens outside the window.
fn allow_navigation(url: &str) -> bool {
    let internal = url.starts_with("about:") || url.starts_with("data:") || url.starts_with("wry:") || url.contains("wry.localhost");
    if internal {
        return true;
    }
    open_link(url);
    false
}

/// Opens an http or https link in the user's browser; other schemes are refused.
fn open_link(url: &str) {
    let lower = url.trim_start().to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        if let Err(e) = webbrowser::open(url) {
            eprintln!("figcraft play: cannot open the link: {e}");
        }
    }
}
