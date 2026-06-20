//! Magic Install: detect the project's package manager(s) from marker files and
//! run their install commands, streaming output into a modal.
//!
//! Native port of the macOS LingCode app's `MagicInstallService`. Unlike the
//! `lingcode_cloud` / `lingcode_android` actions, this does NOT shell out to the
//! `lingcode` CLI — detection is just "does marker file X exist" and the install
//! command is a plain subprocess, so keeping it native means Magic Install works
//! even when the CLI isn't on PATH. The modal-streaming shape mirrors
//! `lingcode_cloud::CloudModal`.

use std::{path::PathBuf, process::Stdio, sync::Arc};

use anyhow::{Context as _, Result};
use futures::{AsyncBufReadExt as _, StreamExt as _};
use gpui::{
    App, AppContext as _, Context, DismissEvent, EventEmitter, FocusHandle, Focusable, Render,
    SharedString, Task, Window, actions,
};
use ui::prelude::*;
use util::process::Child;
use workspace::{AppState, DismissDecision, ModalView, Workspace};

actions!(
    lingcode_install,
    [
        /// Detect the project's package manager(s) and install dependencies.
        MagicInstall,
    ]
);

pub fn init(_: Arc<AppState>, cx: &mut App) {
    cx.observe_new(
        |workspace: &mut Workspace, _window, _cx: &mut Context<Workspace>| {
            workspace.register_action(|workspace, _: &MagicInstall, window, cx| {
                let cwd = workspace
                    .project()
                    .read(cx)
                    .visible_worktrees(cx)
                    .next()
                    .map(|wt| wt.read(cx).abs_path().to_path_buf());
                let Some(cwd) = cwd else {
                    log::error!("Magic Install: no open project");
                    return;
                };
                workspace.toggle_modal(window, cx, move |_window, cx| InstallModal::new(cwd, cx));
            });
        },
    )
    .detach();
}

/// A single package manager: the marker files that imply it, markers that
/// suppress it (e.g. plain `npm` is suppressed when a `yarn.lock` is present so
/// we don't run both), and the install command to run.
struct PackageManager {
    name: &'static str,
    markers: &'static [&'static str],
    suppressed_by: &'static [&'static str],
    program: &'static str,
    args: &'static [&'static str],
}

/// Detection table, ported from the macOS MagicInstallService. Ordered roughly
/// by ecosystem; suppression keeps lockfile-specific managers from doubling up
/// with their generic counterparts.
const MANAGERS: &[PackageManager] = &[
    // ── JavaScript / TypeScript ──
    PackageManager {
        name: "pnpm",
        markers: &["pnpm-lock.yaml"],
        suppressed_by: &[],
        program: "pnpm",
        args: &["install"],
    },
    PackageManager {
        name: "yarn",
        markers: &["yarn.lock"],
        suppressed_by: &[],
        program: "yarn",
        args: &["install"],
    },
    PackageManager {
        name: "bun",
        markers: &["bun.lockb", "bun.lock"],
        suppressed_by: &[],
        program: "bun",
        args: &["install"],
    },
    PackageManager {
        name: "npm",
        markers: &["package.json"],
        suppressed_by: &["yarn.lock", "pnpm-lock.yaml", "bun.lockb", "bun.lock"],
        program: "npm",
        args: &["install"],
    },
    // ── Rust ──
    PackageManager {
        name: "cargo",
        markers: &["Cargo.toml"],
        suppressed_by: &[],
        program: "cargo",
        args: &["fetch"],
    },
    // ── Python ──
    PackageManager {
        name: "poetry",
        markers: &["poetry.lock"],
        suppressed_by: &[],
        program: "poetry",
        args: &["install"],
    },
    PackageManager {
        name: "pipenv",
        markers: &["Pipfile"],
        suppressed_by: &[],
        program: "pipenv",
        args: &["install"],
    },
    PackageManager {
        name: "pip",
        markers: &["requirements.txt"],
        suppressed_by: &["poetry.lock", "Pipfile"],
        program: "python",
        args: &["-m", "pip", "install", "-r", "requirements.txt"],
    },
    // ── Go ──
    PackageManager {
        name: "go",
        markers: &["go.mod"],
        suppressed_by: &[],
        program: "go",
        args: &["mod", "download"],
    },
    // ── Ruby ──
    PackageManager {
        name: "bundler",
        markers: &["Gemfile"],
        suppressed_by: &[],
        program: "bundle",
        args: &["install"],
    },
    // ── PHP ──
    PackageManager {
        name: "composer",
        markers: &["composer.json"],
        suppressed_by: &[],
        program: "composer",
        args: &["install"],
    },
    // ── .NET ──
    PackageManager {
        name: "dotnet",
        markers: &["global.json", "paket.dependencies"],
        suppressed_by: &[],
        program: "dotnet",
        args: &["restore"],
    },
    // ── JVM ──
    PackageManager {
        name: "maven",
        markers: &["pom.xml"],
        suppressed_by: &[],
        program: "mvn",
        args: &["install", "-q"],
    },
    // ── Swift ──
    PackageManager {
        name: "swift",
        markers: &["Package.swift"],
        suppressed_by: &[],
        program: "swift",
        args: &["package", "resolve"],
    },
];

enum Status {
    Running,
    Done,
    Error(SharedString),
}

pub struct InstallModal {
    focus_handle: FocusHandle,
    lines: Vec<SharedString>,
    status: Status,
    _task: Task<()>,
}

impl InstallModal {
    fn new(cwd: PathBuf, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        let run = cx.spawn(async move |this, cx| {
            let result = run_install(cwd, this.clone(), cx).await;
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
            lines: Vec::new(),
            status: Status::Running,
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

/// Detect package managers in `cwd` and run each one's install command, streaming
/// stdout into the modal. Returns Err only on spawn/transport failure; a failed
/// install command is recorded as a line and reflected in the final status.
async fn run_install(
    cwd: PathBuf,
    this: gpui::WeakEntity<InstallModal>,
    cx: &mut gpui::AsyncApp,
) -> Result<()> {
    let present: Vec<&PackageManager> = MANAGERS
        .iter()
        .filter(|m| m.markers.iter().any(|f| cwd.join(f).exists()))
        .filter(|m| !m.suppressed_by.iter().any(|f| cwd.join(f).exists()))
        .collect();

    if present.is_empty() {
        this.update(cx, |modal, cx| {
            modal.push_line("No recognized package manager in this project.", cx);
        })
        .ok();
        return Ok(());
    }

    this.update(cx, |modal, cx| {
        let names: Vec<&str> = present.iter().map(|m| m.name).collect();
        modal.push_line(format!("Detected: {}", names.join(", ")), cx);
    })
    .ok();

    let mut any_failed = false;
    for pm in present {
        this.update(cx, |modal, cx| {
            modal.push_line(format!("→ {} {}", pm.program, pm.args.join(" ")), cx)
        })
        .ok();

        // `which` resolves the real path (handling Windows `.cmd`/`.bat`
        // wrappers); fall back to the bare name so the spawn error is clear.
        let program = which::which(pm.program).unwrap_or_else(|_| PathBuf::from(pm.program));
        let mut command = util::command::new_std_command(&program);
        command.args(pm.args);
        command.current_dir(&cwd);

        let mut child = match Child::spawn(command, Stdio::null(), Stdio::piped(), Stdio::piped()) {
            Ok(child) => child,
            Err(err) => {
                any_failed = true;
                this.update(cx, |modal, cx| {
                    modal.push_line(format!("  {} not found ({err})", pm.program), cx)
                })
                .ok();
                continue;
            }
        };
        let stdout = child.stdout.take().context("failed to capture stdout")?;

        let mut reader = futures::io::BufReader::new(stdout).lines();
        while let Some(line) = reader.next().await {
            let line = match line {
                Ok(line) => line,
                Err(err) => {
                    log::warn!("Magic Install: stdout read error: {err}");
                    break;
                }
            };
            if line.is_empty() {
                continue;
            }
            this.update(cx, |modal, cx| modal.push_line(format!("  {line}"), cx))
                .ok();
        }

        let status = child
            .status()
            .await
            .context("failed to await install exit")?;
        if status.success() {
            this.update(cx, |modal, cx| {
                modal.push_line(format!("{} ✓", pm.name), cx)
            })
            .ok();
        } else {
            any_failed = true;
            this.update(cx, |modal, cx| {
                modal.push_line(format!("{} failed ({status})", pm.name), cx)
            })
            .ok();
        }
    }

    if any_failed {
        this.update(cx, |modal, cx| {
            modal.status = Status::Error("One or more installs failed.".into());
            cx.notify();
        })
        .ok();
    }
    Ok(())
}

impl ModalView for InstallModal {
    fn on_before_dismiss(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> DismissDecision {
        DismissDecision::Dismiss(true)
    }
}

impl Focusable for InstallModal {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EventEmitter<DismissEvent> for InstallModal {}

impl Render for InstallModal {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let log = v_flex().gap_0p5().children(self.lines.iter().map(|line| {
            Label::new(line.clone())
                .size(LabelSize::Small)
                .color(Color::Muted)
        }));

        let footer = match &self.status {
            Status::Running => h_flex()
                .child(Label::new("Installing…").color(Color::Muted))
                .into_any_element(),
            Status::Error(message) => h_flex()
                .gap_2()
                .child(Label::new(message.clone()).color(Color::Error))
                .child(
                    Button::new("close", "Close")
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(DismissEvent))),
                )
                .into_any_element(),
            Status::Done => h_flex()
                .gap_2()
                .child(Label::new("Done").color(Color::Success))
                .child(
                    Button::new("close", "Close")
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(DismissEvent))),
                )
                .into_any_element(),
        };

        v_flex()
            .key_context("LingCodeInstall")
            .track_focus(&self.focus_handle)
            .elevation_3(cx)
            .w(px(540.))
            .p_4()
            .gap_3()
            .child(Label::new("Install Dependencies").size(LabelSize::Large))
            .child(log)
            .child(footer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Build the same detected set `run_install` computes, without spawning.
    fn detect(cwd: &std::path::Path) -> Vec<&'static str> {
        MANAGERS
            .iter()
            .filter(|m| m.markers.iter().any(|f| cwd.join(f).exists()))
            .filter(|m| !m.suppressed_by.iter().any(|f| cwd.join(f).exists()))
            .map(|m| m.name)
            .collect()
    }

    #[test]
    fn npm_suppressed_by_yarn_lock() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("package.json"), "{}").unwrap();
        fs::write(dir.path().join("yarn.lock"), "").unwrap();
        let detected = detect(dir.path());
        assert!(detected.contains(&"yarn"));
        assert!(!detected.contains(&"npm"));
    }

    #[test]
    fn plain_npm_when_no_lockfile() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("package.json"), "{}").unwrap();
        assert_eq!(detect(dir.path()), vec!["npm"]);
    }

    #[test]
    fn multiple_ecosystems_detected() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "").unwrap();
        fs::write(dir.path().join("go.mod"), "").unwrap();
        let detected = detect(dir.path());
        assert!(detected.contains(&"cargo"));
        assert!(detected.contains(&"go"));
    }

    #[test]
    fn empty_project_detects_nothing() {
        let dir = tempfile::tempdir().unwrap();
        assert!(detect(dir.path()).is_empty());
    }
}
