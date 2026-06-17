//! LingCode Cloud integration for the editor: deploy the current project to
//! LingCode Cloud hosting and connect/disconnect a managed backend.
//!
//! All of the heavy lifting (auth, project detection, build, upload, backend
//! provisioning, MCP wiring) lives in the cross-platform `lingcode` CLI. This
//! crate is just the native surface: it registers workspace actions, spawns the
//! CLI, and renders a modal that streams the CLI's progress. Deploy uses the
//! CLI's `--ndjson` mode so we can render structured status; connect/disconnect
//! stream their plain stdout.

use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
};

use anyhow::{Context as _, Result};
use futures::{AsyncBufReadExt as _, AsyncReadExt as _, StreamExt as _};
use gpui::{
    App, AppContext as _, Context, DismissEvent, EventEmitter, FocusHandle, Focusable, Render,
    SharedString, Task, Window, actions,
};
use serde::Deserialize;
use ui::prelude::*;
use util::process::Child;
use workspace::{AppState, DismissDecision, ModalView, Workspace};

actions!(
    lingcode_cloud,
    [
        /// Deploy the current project to LingCode Cloud.
        DeployToCloud,
        /// Connect a LingCode Cloud managed backend to the current project.
        ConnectBackend,
        /// Disconnect the LingCode Cloud managed backend from the current project.
        DisconnectBackend,
        /// Open the LingCode Cloud backend console for the current project in your browser.
        OpenBackendConsole,
        /// Manage collaborators (owner/editor/viewer) for the current project.
        ShareCloudProject,
    ]
);

pub fn init(_: Arc<AppState>, cx: &mut App) {
    cx.observe_new(
        |workspace: &mut Workspace, _window, _cx: &mut Context<Workspace>| {
            workspace.register_action(|workspace, _: &DeployToCloud, window, cx| {
                open_task(workspace, CloudTask::Deploy, window, cx);
            });
            workspace.register_action(|workspace, _: &ConnectBackend, window, cx| {
                open_task(workspace, CloudTask::Connect, window, cx);
            });
            workspace.register_action(|workspace, _: &DisconnectBackend, window, cx| {
                open_task(workspace, CloudTask::Disconnect, window, cx);
            });
            // The backend console and project-sharing UIs are web apps served from
            // LingCode Cloud; auth is handled by the user's browser session, so we
            // just open them (matches the macOS app, which opens the same pages).
            workspace.register_action(|_workspace, _: &OpenBackendConsole, _window, cx| {
                cx.open_url(BACKEND_CONSOLE_URL);
            });
            workspace.register_action(|_workspace, _: &ShareCloudProject, _window, cx| {
                cx.open_url(PROJECT_SHARE_URL);
            });
        },
    )
    .detach();
}

/// LingCode Cloud account/backend admin console (web app).
const BACKEND_CONSOLE_URL: &str = "https://lingcode.dev/backends.html";
/// LingCode Cloud project + collaborators panel (web app).
const PROJECT_SHARE_URL: &str = "https://lingcode.dev/project.html";

#[derive(Clone, Copy, PartialEq)]
enum CloudTask {
    Deploy,
    Connect,
    Disconnect,
}

impl CloudTask {
    fn title(self) -> &'static str {
        match self {
            CloudTask::Deploy => "Deploy to LingCode Cloud",
            CloudTask::Connect => "Connect LingCode Cloud Backend",
            CloudTask::Disconnect => "Disconnect LingCode Cloud Backend",
        }
    }

    /// Arguments passed to the `lingcode` CLI. Deploy uses `--ndjson` so we can
    /// render structured progress; connect/disconnect stream plain stdout.
    fn args(self) -> Vec<&'static str> {
        match self {
            CloudTask::Deploy => vec!["cloud", "deploy", ".", "--ndjson"],
            CloudTask::Connect => vec!["cloud", "connect", "."],
            CloudTask::Disconnect => vec!["cloud", "disconnect", "."],
        }
    }

    fn is_ndjson(self) -> bool {
        matches!(self, CloudTask::Deploy)
    }
}

fn open_task(workspace: &mut Workspace, task: CloudTask, window: &mut Window, cx: &mut Context<Workspace>) {
    let cwd = workspace
        .project()
        .read(cx)
        .visible_worktrees(cx)
        .next()
        .map(|wt| wt.read(cx).abs_path().to_path_buf());
    let Some(cwd) = cwd else {
        log::error!("LingCode Cloud: no open project to run `{}`", task.title());
        return;
    };
    workspace.toggle_modal(window, cx, move |_window, cx| CloudModal::new(task, cwd, cx));
}

/// NDJSON progress events emitted by `lingcode cloud deploy --ndjson`. Mirrors
/// the `DeployEvent` union in the CLI's `src/cloud/deploy.ts`.
#[derive(Deserialize)]
#[serde(tag = "phase", rename_all = "snake_case")]
enum DeployEvent {
    Detect {
        pm: String,
        #[serde(rename = "outDir")]
        out_dir: String,
    },
    Build {
        line: String,
    },
    Package {
        bytes: u64,
    },
    Upload {
        status: String,
        mode: String,
    },
    Poll {
        #[serde(rename = "jobId")]
        job_id: String,
    },
    Done {
        url: String,
    },
    Error {
        message: String,
    },
}

enum Status {
    Running,
    Done,
    Error(SharedString),
}

pub struct CloudModal {
    focus_handle: FocusHandle,
    title: SharedString,
    lines: Vec<SharedString>,
    status: Status,
    url: Option<SharedString>,
    _task: Task<()>,
}

impl CloudModal {
    fn new(task: CloudTask, cwd: PathBuf, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        let run = cx.spawn(async move |this, cx| {
            let result = run_cloud_task(task, cwd, this.clone(), cx).await;
            this.update(cx, |modal, cx| {
                match result {
                    Ok(()) => {
                        if !matches!(modal.status, Status::Error(_)) {
                            modal.status = Status::Done;
                        }
                    }
                    Err(err) => modal.status = Status::Error(err.to_string().into()),
                }
                cx.notify();
            })
            .ok();
        });

        Self {
            focus_handle,
            title: task.title().into(),
            lines: Vec::new(),
            status: Status::Running,
            url: None,
            _task: run,
        }
    }

    fn push_line(&mut self, line: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.lines.push(line.into());
        if self.lines.len() > 200 {
            self.lines.remove(0);
        }
        cx.notify();
    }
}

/// Spawn the CLI and stream its output into the modal. Returns Err only on
/// spawn/transport failure; logical failures (e.g. an NDJSON `error` event or a
/// non-zero exit) are recorded on the modal directly.
async fn run_cloud_task(
    task: CloudTask,
    cwd: PathBuf,
    this: gpui::WeakEntity<CloudModal>,
    cx: &mut gpui::AsyncApp,
) -> Result<()> {
    let program = which::which("lingcode").unwrap_or_else(|_| PathBuf::from("lingcode"));
    let mut command = util::command::new_std_command(&program);
    command.args(task.args());
    command.current_dir(&cwd);

    let mut child = Child::spawn(command, Stdio::null(), Stdio::piped(), Stdio::piped())
        .with_context(|| format!("failed to launch `{}`", program.display()))?;
    let stdout = child.stdout.take().context("failed to capture stdout")?;
    let stderr = child.stderr.take().context("failed to capture stderr")?;

    let mut lines = futures::io::BufReader::new(stdout).lines();
    while let Some(line) = lines.next().await {
        let line = match line {
            Ok(line) => line,
            Err(err) => {
                log::warn!("LingCode Cloud: stdout read error: {err}");
                break;
            }
        };
        if line.is_empty() {
            continue;
        }
        let ndjson = task.is_ndjson();
        this.update(cx, |modal, cx| {
            if ndjson {
                match serde_json::from_str::<DeployEvent>(&line) {
                    Ok(DeployEvent::Detect { pm, out_dir }) => {
                        modal.push_line(format!("Detected {pm} → {out_dir}"), cx)
                    }
                    Ok(DeployEvent::Build { line }) => modal.push_line(format!("  {line}"), cx),
                    Ok(DeployEvent::Package { bytes }) => {
                        modal.push_line(format!("Packaged {} KB", bytes / 1024), cx)
                    }
                    Ok(DeployEvent::Upload { status, mode }) => {
                        modal.push_line(format!("Upload {status} ({mode})"), cx)
                    }
                    Ok(DeployEvent::Poll { job_id }) => {
                        modal.push_line(format!("Building on cloud… ({job_id})"), cx)
                    }
                    Ok(DeployEvent::Done { url }) => {
                        modal.url = Some(url.clone().into());
                        modal.status = Status::Done;
                        modal.push_line(format!("Live at {url}"), cx);
                    }
                    Ok(DeployEvent::Error { message }) => {
                        modal.status = Status::Error(message.clone().into());
                        modal.push_line(format!("Error: {message}"), cx);
                    }
                    Err(_) => modal.push_line(line.clone(), cx),
                }
            } else {
                modal.push_line(line.clone(), cx);
            }
        })
        .ok();
    }

    let status = child.status().await.context("failed to await CLI exit")?;
    if !status.success() {
        let mut err = String::new();
        let _ = futures::io::BufReader::new(stderr).read_to_string(&mut err).await;
        let message = err.trim();
        let message = if message.is_empty() {
            format!("`lingcode` exited with status {status}")
        } else {
            message.to_string()
        };
        this.update(cx, |modal, cx| {
            modal.status = Status::Error(message.clone().into());
            cx.notify();
        })
        .ok();
    }
    Ok(())
}

impl ModalView for CloudModal {
    fn on_before_dismiss(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> DismissDecision {
        DismissDecision::Dismiss(true)
    }
}

impl Focusable for CloudModal {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EventEmitter<DismissEvent> for CloudModal {}

impl Render for CloudModal {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let log = v_flex().gap_0p5().children(
            self.lines
                .iter()
                .map(|line| Label::new(line.clone()).size(LabelSize::Small).color(Color::Muted)),
        );

        let footer = match &self.status {
            Status::Running => h_flex()
                .child(Label::new("Working…").color(Color::Muted))
                .into_any_element(),
            Status::Error(message) => h_flex()
                .gap_2()
                .child(Label::new(message.clone()).color(Color::Error))
                .child(
                    Button::new("close", "Close")
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(DismissEvent))),
                )
                .into_any_element(),
            Status::Done => {
                let mut row = h_flex().gap_2();
                if let Some(url) = self.url.clone() {
                    row = row.child(Label::new(url.clone()).color(Color::Accent)).child(
                        Button::new("open", "Open").on_click(cx.listener(move |_, _, _, cx| {
                            cx.open_url(&url);
                        })),
                    );
                }
                row.child(
                    Button::new("close", "Close")
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(DismissEvent))),
                )
                .into_any_element()
            }
        };

        v_flex()
            .key_context("LingCodeCloud")
            .track_focus(&self.focus_handle)
            .elevation_3(cx)
            .w(px(540.))
            .p_4()
            .gap_3()
            .child(Label::new(self.title.clone()).size(LabelSize::Large))
            .child(log)
            .child(footer)
    }
}
