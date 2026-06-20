//! Remote coding host: run a headless `lingcode serve` HTTP+SSE server from the
//! IDE so the LingCode web remote-control (or any client) can drive this
//! machine's agent.
//!
//! The macOS app implements this with an in-process Swift `LingCodeServer`
//! (Darwin `NWListener`) — Apple-only, so it can't be reused here. Instead, the
//! cross-platform `lingcode` CLI already ships a complete headless server
//! (`lingcode serve`: sessions, SSE event streams, permissions, PTY, files), so
//! this crate just *manages its lifecycle* — spawn it, surface its address, and
//! stop it — the same "delegate to the CLI" approach as `lingcode_cloud`.
//!
//! The running process is held in a global so it survives the status modal being
//! closed (a server you have to keep a window open for is not a server). We spawn
//! `lingcode remote` (not bare `lingcode serve`): it registers this machine as a
//! relay host, starts a private loopback server, and tunnels the two — giving the
//! web remote-control **zero-setup** reach (no SSH/port config). Closing the modal
//! leaves it serving; Stop kills it.

use std::{path::PathBuf, process::Stdio, sync::Arc};

use futures::{AsyncBufReadExt as _, StreamExt as _};
use gpui::{
    App, AppContext as _, Context, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable,
    Global, ReadGlobal as _, Render, SharedString, Subscription, Task, Window, actions,
};
use ui::prelude::*;
use util::ResultExt as _;
use util::process::Child;
use workspace::{AppState, DismissDecision, ModalView, Workspace};

/// LingCode web remote-control client (drives a remote LingCode host's agent).
const REMOTE_CONTROL_URL: &str = "https://lingcode.dev/remote-control.html";

actions!(
    lingcode_remote,
    [
        /// Start the headless `lingcode serve` remote-coding server.
        StartRemoteServer,
        /// Stop the running remote-coding server.
        StopRemoteServer,
    ]
);

pub fn init(_: Arc<AppState>, cx: &mut App) {
    let server = cx.new(|_| RemoteServer::default());
    cx.set_global(GlobalRemoteServer(server));

    cx.observe_new(
        |workspace: &mut Workspace, _window, _cx: &mut Context<Workspace>| {
            workspace.register_action(|workspace, _: &StartRemoteServer, window, cx| {
                let server = GlobalRemoteServer::global(cx).0.clone();
                server.update(cx, |server, cx| server.start(cx));
                workspace.toggle_modal(window, cx, move |_window, cx| {
                    RemoteServerModal::new(server.clone(), cx)
                });
            });
            workspace.register_action(|_workspace, _: &StopRemoteServer, _window, cx| {
                let server = GlobalRemoteServer::global(cx).0.clone();
                server.update(cx, |server, cx| server.stop(cx));
            });
        },
    )
    .detach();
}

struct GlobalRemoteServer(Entity<RemoteServer>);

impl Global for GlobalRemoteServer {}

#[derive(Default)]
pub struct RemoteServer {
    running: Option<Running>,
    /// Surfaced on spawn failure or after the server exits.
    last_error: Option<SharedString>,
}

struct Running {
    child: Child,
    /// Parsed from the server's `listening on http://host:port` line.
    url: Option<SharedString>,
    lines: Vec<SharedString>,
    _reader: Task<()>,
}

impl RemoteServer {
    fn is_running(&self) -> bool {
        self.running.is_some()
    }

    fn start(&mut self, cx: &mut Context<Self>) {
        if self.running.is_some() {
            cx.notify();
            return;
        }
        self.last_error = None;

        let program = which::which("lingcode").unwrap_or_else(|_| PathBuf::from("lingcode"));
        let mut command = util::command::new_std_command(&program);
        // `lingcode remote` = zero-setup: it registers this machine as a relay
        // host, starts a private loopback server, and tunnels the two — so the
        // web remote-control reaches it with no SSH/port config.
        command.args(["remote"]);

        let mut child = match Child::spawn(command, Stdio::null(), Stdio::piped(), Stdio::null()) {
            Ok(child) => child,
            Err(err) => {
                self.last_error = Some(format!("Failed to launch `lingcode remote`: {err}").into());
                cx.notify();
                return;
            }
        };
        let Some(stdout) = child.stdout.take() else {
            self.last_error = Some("Failed to capture server output".into());
            cx.notify();
            return;
        };

        let reader = cx.spawn(async move |this, cx| {
            let mut lines = futures::io::BufReader::new(stdout).lines();
            while let Some(line) = lines.next().await {
                let Ok(line) = line else { break };
                if line.trim().is_empty() {
                    continue;
                }
                this.update(cx, |server, cx| {
                    if let Some(running) = server.running.as_mut() {
                        if running.url.is_none() {
                            if let Some(url) = extract_url(&line) {
                                running.url = Some(url.into());
                            }
                        }
                        running.lines.push(line.into());
                        if running.lines.len() > 100 {
                            running.lines.remove(0);
                        }
                        cx.notify();
                    }
                })
                .ok();
            }
            // stdout closed → the server process exited.
            this.update(cx, |server, cx| {
                if server.running.take().is_some() {
                    server.last_error = Some(
                        "Remote server stopped (is `lingcode` signed in to LingCode Cloud? run \
                         `lingcode remote` in a terminal to see why)."
                            .into(),
                    );
                    cx.notify();
                }
            })
            .ok();
        });

        self.running = Some(Running {
            child,
            url: None,
            lines: Vec::new(),
            _reader: reader,
        });
        cx.notify();
    }

    fn stop(&mut self, cx: &mut Context<Self>) {
        if let Some(mut running) = self.running.take() {
            running.child.kill().log_err();
            self.last_error = None;
        }
        cx.notify();
    }
}

/// Pull the first `http(s)://…` token out of a server log line.
fn extract_url(line: &str) -> Option<String> {
    let start = line.find("http://").or_else(|| line.find("https://"))?;
    let rest = &line[start..];
    let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
    Some(rest[..end].to_string())
}

struct RemoteServerModal {
    server: Entity<RemoteServer>,
    focus_handle: FocusHandle,
    _subscription: Subscription,
}

impl RemoteServerModal {
    fn new(server: Entity<RemoteServer>, cx: &mut Context<Self>) -> Self {
        let _subscription = cx.observe(&server, |_, _, cx| cx.notify());
        Self {
            server,
            focus_handle: cx.focus_handle(),
            _subscription,
        }
    }
}

impl ModalView for RemoteServerModal {
    fn on_before_dismiss(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> DismissDecision {
        // Dismissing the panel does NOT stop the server (it keeps serving in the
        // background); reopen via Start Remote Server to manage it.
        DismissDecision::Dismiss(true)
    }
}

impl Focusable for RemoteServerModal {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EventEmitter<DismissEvent> for RemoteServerModal {}

impl Render for RemoteServerModal {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let server = self.server.read(cx);
        let running = server.is_running();
        let url = server.running.as_ref().and_then(|r| r.url.clone());
        let lines = server
            .running
            .as_ref()
            .map(|r| r.lines.clone())
            .unwrap_or_default();
        let error = server.last_error.clone();

        let log = v_flex().gap_0p5().children(lines.iter().map(|line| {
            Label::new(line.clone())
                .size(LabelSize::Small)
                .color(Color::Muted)
        }));

        let status = if running {
            match &url {
                Some(url) => h_flex()
                    .gap_2()
                    .child(Label::new("Serving at").color(Color::Muted))
                    .child(Label::new(url.clone()).color(Color::Accent))
                    .into_any_element(),
                None => Label::new("Starting server…")
                    .color(Color::Muted)
                    .into_any_element(),
            }
        } else if let Some(error) = &error {
            Label::new(error.clone())
                .color(Color::Error)
                .into_any_element()
        } else {
            Label::new("The remote server is not running.")
                .color(Color::Muted)
                .into_any_element()
        };

        let mut controls = h_flex().gap_2();
        if running {
            controls = controls
                .child(Button::new("stop", "Stop Server").on_click(cx.listener(
                    |this, _, _, cx| {
                        this.server.update(cx, |server, cx| server.stop(cx));
                    },
                )))
                .child(
                    Button::new("open", "Open Web Client")
                        .on_click(cx.listener(|_, _, _, cx| cx.open_url(REMOTE_CONTROL_URL))),
                );
        } else {
            controls = controls.child(Button::new("start", "Start Server").on_click(cx.listener(
                |this, _, _, cx| {
                    this.server.update(cx, |server, cx| server.start(cx));
                },
            )));
        }
        controls = controls.child(
            Button::new("close", "Close")
                .on_click(cx.listener(|_, _, _, cx| cx.emit(DismissEvent))),
        );

        v_flex()
            .key_context("LingCodeRemote")
            .track_focus(&self.focus_handle)
            .elevation_3(cx)
            .w(px(540.))
            .p_4()
            .gap_3()
            .child(Label::new("Remote Coding Server").size(LabelSize::Large))
            .child(status)
            .child(log)
            .child(controls)
    }
}
