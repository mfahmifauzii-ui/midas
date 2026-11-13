//! The `figcraft-play-host` program: `figcraft-play-host --play` opens the Play window and speaks
//! JSON lines on stdin and stderr with the editor. The editor starts it as a child process; if it
//! crashes or the editor exits, only this process ends (docs/FORK.md).

#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

fn main() {
    // `--play` is how the editor names the mode it starts; the host has no other mode. A different
    // argument is refused rather than opening a window the editor would not talk to.
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("--play") | None => {}
        Some(other) => {
            eprintln!("figcraft-play-host: unknown argument `{other}` (expected --play)");
            std::process::exit(2);
        }
    }
    std::process::exit(figcraft_play_host::run());
}
