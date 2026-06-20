//! LingCode Android tooling for the editor: detect the Android toolchain, build
//! and run Gradle projects, list/launch devices and emulators, and jump to the
//! Play Console for release.
//!
//! Like `lingcode_cloud`, the heavy lifting is shelling out to the standard
//! Android tools (`gradlew`, `adb`, `emulator`) and streaming their output into
//! a modal. This intentionally covers the build/run/deploy-prep essentials; the
//! deeper editor-integrated pieces from the macOS app (JDWP/DAP Kotlin debugger,
//! logcat / layout-inspector panels, run-destination toolbar, the native Play
//! Developer API upload flow) are larger follow-ups — see `LINGCODE-CHANGES.md`.

use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
};

use anyhow::{Context as _, Result, anyhow, bail};
use futures::{AsyncBufReadExt as _, AsyncReadExt as _, StreamExt as _};
use gpui::{
    App, AppContext as _, Context, DismissEvent, EventEmitter, FocusHandle, Focusable, Render,
    SharedString, Task, Window, actions,
};
use http_client::{AsyncBody, HttpClient, Method, Request as HttpRequest};
use serde::Deserialize;
use std::time::{SystemTime, UNIX_EPOCH};
use ui::prelude::*;
use util::process::Child;
use workspace::{AppState, DismissDecision, ModalView, Workspace};

actions!(
    lingcode_android,
    [
        /// Report the detected Android toolchain (SDK, JDK, adb, emulator, gradle).
        AndroidDoctor,
        /// Build the debug APK (`gradlew assembleDebug`).
        AndroidBuildDebug,
        /// Build the release App Bundle (`gradlew bundleRelease`).
        AndroidBuildBundle,
        /// Build, install, and start the app on the connected device or emulator.
        AndroidRun,
        /// List attached devices and running emulators (`adb devices -l`).
        AndroidListDevices,
        /// List installed Android Virtual Devices (`emulator -list-avds`).
        AndroidListEmulators,
        /// Launch the first available Android emulator.
        AndroidStartEmulator,
        /// Stream logcat from the connected device/emulator.
        AndroidLogcat,
        /// Dump the on-screen view hierarchy (uiautomator) from the device.
        AndroidLayoutInspector,
        /// Inspect the project's built APK/AAB (package, SDKs, permissions, size).
        AndroidAnalyzeApk,
        /// Compare two APK/AAB files (size delta + aapt2 badging diff).
        AndroidApkDiff,
        /// Build the release bundle and upload it to Google Play (Play Developer API).
        AndroidDeployToPlay,
        /// Open the Google Play Console in your browser.
        AndroidOpenPlayConsole,
    ]
);

const PLAY_CONSOLE_URL: &str = "https://play.google.com/console";

pub fn init(app_state: Arc<AppState>, cx: &mut App) {
    let http_client = app_state.client.http_client();
    cx.observe_new(
        move |workspace: &mut Workspace, _window, _cx: &mut Context<Workspace>| {
            let http_client = http_client.clone();
            workspace.register_action(move |workspace, _: &AndroidDeployToPlay, window, cx| {
                deploy_to_play(workspace, http_client.clone(), window, cx);
            });
            workspace.register_action(|workspace, _: &AndroidDoctor, window, cx| {
                open_task(workspace, AndroidAction::Doctor, window, cx);
            });
            workspace.register_action(|workspace, _: &AndroidBuildDebug, window, cx| {
                open_task(workspace, AndroidAction::BuildDebug, window, cx);
            });
            workspace.register_action(|workspace, _: &AndroidBuildBundle, window, cx| {
                open_task(workspace, AndroidAction::BuildBundle, window, cx);
            });
            workspace.register_action(|workspace, _: &AndroidRun, window, cx| {
                open_task(workspace, AndroidAction::Run, window, cx);
            });
            workspace.register_action(|workspace, _: &AndroidListDevices, window, cx| {
                open_task(workspace, AndroidAction::Devices, window, cx);
            });
            workspace.register_action(|workspace, _: &AndroidListEmulators, window, cx| {
                open_task(workspace, AndroidAction::Emulators, window, cx);
            });
            workspace.register_action(|workspace, _: &AndroidStartEmulator, window, cx| {
                start_emulator(workspace, window, cx);
            });
            workspace.register_action(|workspace, _: &AndroidLogcat, window, cx| {
                open_task(workspace, AndroidAction::Logcat, window, cx);
            });
            workspace.register_action(|workspace, _: &AndroidLayoutInspector, window, cx| {
                open_task(workspace, AndroidAction::LayoutInspector, window, cx);
            });
            workspace.register_action(|workspace, _: &AndroidAnalyzeApk, window, cx| {
                open_task(workspace, AndroidAction::AnalyzeApk, window, cx);
            });
            workspace.register_action(|workspace, _: &AndroidApkDiff, window, cx| {
                apk_diff(workspace, window, cx);
            });
            workspace.register_action(|_workspace, _: &AndroidOpenPlayConsole, _window, cx| {
                cx.open_url(PLAY_CONSOLE_URL);
            });
        },
    )
    .detach();
}

#[derive(Clone, Copy)]
enum AndroidAction {
    Doctor,
    BuildDebug,
    BuildBundle,
    Run,
    Devices,
    Emulators,
    Logcat,
    LayoutInspector,
    AnalyzeApk,
}

impl AndroidAction {
    fn title(self) -> &'static str {
        match self {
            AndroidAction::Doctor => "Android Toolchain",
            AndroidAction::BuildDebug => "Build Debug APK",
            AndroidAction::BuildBundle => "Build Release Bundle",
            AndroidAction::Run => "Run on Device / Emulator",
            AndroidAction::Devices => "Android Devices",
            AndroidAction::Emulators => "Android Emulators",
            AndroidAction::Logcat => "Logcat",
            AndroidAction::LayoutInspector => "Layout Inspector",
            AndroidAction::AnalyzeApk => "Analyze APK / AAB",
        }
    }

    fn needs_project(self) -> bool {
        matches!(
            self,
            AndroidAction::BuildDebug
                | AndroidAction::BuildBundle
                | AndroidAction::Run
                | AndroidAction::AnalyzeApk
        )
    }
}

// --- Toolchain discovery -----------------------------------------------------

fn home_dir() -> Option<PathBuf> {
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var(var).ok().map(PathBuf::from)
}

fn exe_name(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

fn android_sdk() -> Option<PathBuf> {
    for var in ["ANDROID_HOME", "ANDROID_SDK_ROOT"] {
        if let Ok(value) = std::env::var(var) {
            let path = PathBuf::from(value);
            if path.is_dir() {
                return Some(path);
            }
        }
    }
    let default = if cfg!(windows) {
        std::env::var("LOCALAPPDATA")
            .ok()
            .map(|local| PathBuf::from(local).join("Android").join("Sdk"))
    } else if cfg!(target_os = "macos") {
        home_dir().map(|home| home.join("Library").join("Android").join("sdk"))
    } else {
        home_dir().map(|home| home.join("Android").join("Sdk"))
    };
    default.filter(|path| path.is_dir())
}

fn sdk_tool(sdk: Option<&Path>, subdir: &str, name: &str) -> Option<PathBuf> {
    if let Some(sdk) = sdk {
        let candidate = sdk.join(subdir).join(exe_name(name));
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    which::which(name).ok()
}

fn adb_path(sdk: Option<&Path>) -> Option<PathBuf> {
    sdk_tool(sdk, "platform-tools", "adb")
}

fn emulator_path(sdk: Option<&Path>) -> Option<PathBuf> {
    sdk_tool(sdk, "emulator", "emulator")
}

fn java_home() -> Option<PathBuf> {
    std::env::var("JAVA_HOME")
        .ok()
        .map(PathBuf::from)
        .filter(|path| path.is_dir())
}

/// Build the `(program, args)` to invoke Gradle for `tasks` in `cwd`. Prefers the
/// project's `gradlew` wrapper; on Windows the `.bat` wrapper is run via `cmd /C`
/// (CreateProcess can't execute a batch file directly).
fn gradle_command(cwd: &Path, tasks: &[&str]) -> Option<(PathBuf, Vec<String>)> {
    let wrapper = cwd.join(if cfg!(windows) { "gradlew.bat" } else { "gradlew" });
    if wrapper.is_file() {
        if cfg!(windows) {
            let mut args = vec!["/C".to_string(), wrapper.to_string_lossy().into_owned()];
            args.extend(tasks.iter().map(|task| task.to_string()));
            return Some((PathBuf::from("cmd"), args));
        }
        return Some((wrapper, tasks.iter().map(|task| task.to_string()).collect()));
    }
    which::which("gradle")
        .ok()
        .map(|gradle| (gradle, tasks.iter().map(|task| task.to_string()).collect()))
}

fn doctor_report() -> Vec<SharedString> {
    let sdk = android_sdk();
    let mut lines: Vec<SharedString> = Vec::new();

    lines.push(match &sdk {
        Some(path) => format!("Android SDK: {}", path.display()).into(),
        None => "Android SDK: NOT FOUND — set ANDROID_HOME or install the SDK".into(),
    });
    lines.push(match java_home() {
        Some(path) => format!("JDK (JAVA_HOME): {}", path.display()).into(),
        None => "JDK: JAVA_HOME not set — install JDK 17+ and set JAVA_HOME".into(),
    });
    lines.push(match adb_path(sdk.as_deref()) {
        Some(path) => format!("adb: {}", path.display()).into(),
        None => "adb: NOT FOUND — expected <sdk>/platform-tools/adb".into(),
    });
    lines.push(match emulator_path(sdk.as_deref()) {
        Some(path) => format!("emulator: {}", path.display()).into(),
        None => "emulator: NOT FOUND — expected <sdk>/emulator/emulator".into(),
    });
    lines.push(match which::which("gradle") {
        Ok(path) => format!("gradle (system): {}", path.display()).into(),
        Err(_) => "gradle (system): not on PATH (projects with gradlew don't need it)".into(),
    });
    lines
}

// --- Job model ---------------------------------------------------------------

enum AndroidJob {
    /// Spawn a process and stream its output.
    Command {
        program: PathBuf,
        args: Vec<String>,
        cwd: PathBuf,
    },
    /// No subprocess — just display these lines (toolchain report or an error).
    Report(Vec<SharedString>),
    /// Build the release bundle, then upload it to Google Play.
    PlayDeploy {
        cwd: PathBuf,
        http_client: Arc<dyn HttpClient>,
        config: PlayConfig,
    },
    /// Stream `adb logcat` from the first connected device.
    Logcat { adb: PathBuf },
    /// Dump and display the on-screen view hierarchy.
    LayoutInspector { adb: PathBuf },
    /// Inspect the project's built APK/AAB.
    AnalyzeApk { cwd: PathBuf, sdk: Option<PathBuf> },
    /// Diff two APK/AAB artifacts (size + aapt2 badging).
    ApkDiff {
        a: PathBuf,
        b: PathBuf,
        sdk: Option<PathBuf>,
    },
}

/// Release config, read from `<project>/.lingcode/play-deploy.json`.
#[derive(Clone, Deserialize)]
struct PlayConfig {
    /// Path to the Google Cloud service-account JSON key (with Play access).
    service_account_json_path: String,
    /// The app's application id, e.g. "com.example.app".
    package_name: String,
    /// Release track: "internal" (default), "alpha", "beta", or "production".
    #[serde(default = "default_track")]
    track: String,
    /// Bump `versionCode` in build.gradle before building.
    #[serde(default)]
    auto_bump_version_code: bool,
    /// Optional explicit path to the built `.aab` (otherwise auto-located).
    #[serde(default)]
    aab_path: Option<String>,
}

fn default_track() -> String {
    "internal".to_string()
}

fn project_root(workspace: &Workspace, cx: &Context<Workspace>) -> Option<PathBuf> {
    workspace
        .project()
        .read(cx)
        .visible_worktrees(cx)
        .next()
        .map(|worktree| worktree.read(cx).abs_path().to_path_buf())
}

fn build_job(action: AndroidAction, cwd: Option<PathBuf>) -> AndroidJob {
    if action.needs_project() && cwd.is_none() {
        return AndroidJob::Report(vec!["Open an Android project first.".into()]);
    }
    let sdk = android_sdk();
    match action {
        AndroidAction::Doctor => AndroidJob::Report(doctor_report()),
        AndroidAction::BuildDebug | AndroidAction::BuildBundle | AndroidAction::Run => {
            let cwd = cwd.expect("checked by needs_project");
            let tasks: &[&str] = match action {
                AndroidAction::BuildDebug => &["assembleDebug"],
                AndroidAction::BuildBundle => &["bundleRelease"],
                AndroidAction::Run => &["installDebug"],
                _ => unreachable!(),
            };
            match gradle_command(&cwd, tasks) {
                Some((program, args)) => AndroidJob::Command { program, args, cwd },
                None => AndroidJob::Report(vec![
                    "No Gradle wrapper (gradlew) found in this project and no system `gradle` on PATH.".into(),
                ]),
            }
        }
        AndroidAction::Devices => match adb_path(sdk.as_deref()) {
            Some(program) => AndroidJob::Command {
                program,
                args: vec!["devices".into(), "-l".into()],
                cwd: cwd.or_else(home_dir).unwrap_or_else(|| PathBuf::from(".")),
            },
            None => AndroidJob::Report(vec![
                "adb not found — install the Android SDK platform-tools.".into(),
            ]),
        },
        AndroidAction::Emulators => match emulator_path(sdk.as_deref()) {
            Some(program) => AndroidJob::Command {
                program,
                args: vec!["-list-avds".into()],
                cwd: cwd.or_else(home_dir).unwrap_or_else(|| PathBuf::from(".")),
            },
            None => AndroidJob::Report(vec![
                "emulator not found — install the Android SDK emulator package.".into(),
            ]),
        },
        AndroidAction::Logcat => match adb_path(sdk.as_deref()) {
            Some(adb) => AndroidJob::Logcat { adb },
            None => AndroidJob::Report(vec!["adb not found.".into()]),
        },
        AndroidAction::LayoutInspector => match adb_path(sdk.as_deref()) {
            Some(adb) => AndroidJob::LayoutInspector { adb },
            None => AndroidJob::Report(vec!["adb not found.".into()]),
        },
        AndroidAction::AnalyzeApk => AndroidJob::AnalyzeApk {
            cwd: cwd.expect("checked by needs_project"),
            sdk,
        },
    }
}

fn open_task(
    workspace: &mut Workspace,
    action: AndroidAction,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let cwd = project_root(workspace, cx);
    let job = build_job(action, cwd);
    let title: SharedString = action.title().into();
    workspace.toggle_modal(window, cx, move |_window, cx| {
        AndroidModal::new(title, job, cx)
    });
}

const PLAY_CONFIG_REL: &str = ".lingcode/play-deploy.json";

fn play_config_sample() -> Vec<SharedString> {
    vec![
        format!("No release config found at {PLAY_CONFIG_REL}.").into(),
        "Create it with your Play service-account key, for example:".into(),
        "{".into(),
        "  \"service_account_json_path\": \"C:/keys/play-service-account.json\",".into(),
        "  \"package_name\": \"com.example.app\",".into(),
        "  \"track\": \"internal\",".into(),
        "  \"auto_bump_version_code\": true".into(),
        "}".into(),
        "".into(),
        "The release bundle must be signed by your project's signingConfigs.".into(),
    ]
}

fn deploy_to_play(
    workspace: &mut Workspace,
    http_client: Arc<dyn HttpClient>,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let title: SharedString = "Deploy to Google Play".into();
    let Some(cwd) = project_root(workspace, cx) else {
        let job = AndroidJob::Report(vec!["Open an Android project first.".into()]);
        workspace.toggle_modal(window, cx, move |_window, cx| {
            AndroidModal::new(title.clone(), job, cx)
        });
        return;
    };

    let config_path = cwd.join(PLAY_CONFIG_REL);
    let job = match std::fs::read_to_string(&config_path)
        .ok()
        .and_then(|raw| serde_json::from_str::<PlayConfig>(&raw).ok())
    {
        Some(config) => AndroidJob::PlayDeploy {
            cwd,
            http_client,
            config,
        },
        None => AndroidJob::Report(play_config_sample()),
    };
    workspace.toggle_modal(window, cx, move |_window, cx| {
        AndroidModal::new(title.clone(), job, cx)
    });
}

/// Launch the first installed AVD, detached (the emulator process outlives the
/// IDE action). Shows an informational modal.
fn start_emulator(workspace: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
    let Some(emulator) = emulator_path(android_sdk().as_deref()) else {
        let job = AndroidJob::Report(vec![
            "emulator not found — install the Android SDK emulator package.".into(),
        ]);
        workspace.toggle_modal(window, cx, move |_window, cx| {
            AndroidModal::new("Android Emulators".into(), job, cx)
        });
        return;
    };

    cx.background_spawn(async move {
        let listed = util::command::new_std_command(&emulator)
            .arg("-list-avds")
            .output();
        let Some(name) = listed.ok().and_then(|out| {
            String::from_utf8_lossy(&out.stdout)
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty())
                .map(str::to_string)
        }) else {
            log::error!("LingCode Android: no AVDs found to launch");
            return;
        };
        if let Err(err) = util::command::new_std_command(&emulator)
            .arg(format!("@{name}"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            log::error!("LingCode Android: failed to launch emulator @{name}: {err}");
        }
    })
    .detach();

    let job = AndroidJob::Report(vec![
        "Launching the first available emulator…".into(),
        "It will open in a new window shortly.".into(),
    ]);
    workspace.toggle_modal(window, cx, move |_window, cx| {
        AndroidModal::new("Android Emulators".into(), job, cx)
    });
}

// --- Modal -------------------------------------------------------------------

enum Status {
    Running,
    Done,
    Error(SharedString),
}

pub struct AndroidModal {
    focus_handle: FocusHandle,
    title: SharedString,
    lines: Vec<SharedString>,
    status: Status,
    _task: Option<Task<()>>,
}

impl AndroidModal {
    fn new(title: SharedString, job: AndroidJob, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        if let AndroidJob::Report(lines) = job {
            return Self {
                focus_handle,
                title,
                lines,
                status: Status::Done,
                _task: None,
            };
        }

        let run = cx.spawn(async move |this, cx| {
            let result = match job {
                AndroidJob::Command { program, args, cwd } => {
                    run_android_command(program, args, cwd, this.clone(), cx).await
                }
                AndroidJob::PlayDeploy {
                    cwd,
                    http_client,
                    config,
                } => run_play_deploy(cwd, http_client, config, this.clone(), cx).await,
                AndroidJob::Logcat { adb } => run_logcat(adb, this.clone(), cx).await,
                AndroidJob::LayoutInspector { adb } => {
                    run_layout_inspector(adb, this.clone(), cx).await
                }
                AndroidJob::AnalyzeApk { cwd, sdk } => {
                    run_analyze_apk(cwd, sdk, this.clone(), cx).await
                }
                AndroidJob::ApkDiff { a, b, sdk } => {
                    run_apk_diff(a, b, sdk, this.clone(), cx).await
                }
                AndroidJob::Report(_) => Ok(()),
            };
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
            title,
            lines: Vec::new(),
            status: Status::Running,
            _task: Some(run),
        }
    }

    fn push_line(&mut self, line: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.lines.push(line.into());
        if self.lines.len() > 400 {
            self.lines.remove(0);
        }
        cx.notify();
    }
}

/// Spawn `program args` in `cwd`, streaming stdout into the modal. Returns true
/// on success; on failure the trailing stderr is pushed as a line.
async fn stream_process(
    program: &Path,
    args: &[String],
    cwd: &Path,
    this: &gpui::WeakEntity<AndroidModal>,
    cx: &mut gpui::AsyncApp,
) -> Result<bool> {
    let mut command = util::command::new_std_command(program);
    command.args(args);
    command.current_dir(cwd);

    let mut child = Child::spawn(command, Stdio::null(), Stdio::piped(), Stdio::piped())
        .with_context(|| format!("failed to launch `{}`", program.display()))?;
    let stdout = child.stdout.take().context("failed to capture stdout")?;
    let stderr = child.stderr.take().context("failed to capture stderr")?;

    let mut lines = futures::io::BufReader::new(stdout).lines();
    while let Some(line) = lines.next().await {
        let line = match line {
            Ok(line) => line,
            Err(err) => {
                log::warn!("LingCode Android: stdout read error: {err}");
                break;
            }
        };
        if line.is_empty() {
            continue;
        }
        this.update(cx, |modal, cx| modal.push_line(line, cx)).ok();
    }

    let status = child.status().await.context("failed to await process exit")?;
    if !status.success() {
        let mut err = String::new();
        let _ = futures::io::BufReader::new(stderr)
            .read_to_string(&mut err)
            .await;
        let message = err.trim();
        if !message.is_empty() {
            this.update(cx, |modal, cx| modal.push_line(message.to_string(), cx))
                .ok();
        }
    }
    Ok(status.success())
}

async fn run_android_command(
    program: PathBuf,
    args: Vec<String>,
    cwd: PathBuf,
    this: gpui::WeakEntity<AndroidModal>,
    cx: &mut gpui::AsyncApp,
) -> Result<()> {
    let ok = stream_process(&program, &args, &cwd, &this, cx).await?;
    if !ok {
        bail!("`{}` exited with a non-zero status", program.display());
    }
    Ok(())
}

// --- Google Play deploy ------------------------------------------------------

async fn push(this: &gpui::WeakEntity<AndroidModal>, cx: &mut gpui::AsyncApp, line: impl Into<SharedString>) {
    this.update(cx, |modal, cx| modal.push_line(line, cx)).ok();
}

/// Build the signed release bundle and upload it to Google Play via the Play
/// Developer API (mirrors the macOS GooglePlayDeployService flow).
async fn run_play_deploy(
    cwd: PathBuf,
    http_client: Arc<dyn HttpClient>,
    config: PlayConfig,
    this: gpui::WeakEntity<AndroidModal>,
    cx: &mut gpui::AsyncApp,
) -> Result<()> {
    // 1. Optional versionCode bump.
    if config.auto_bump_version_code {
        match bump_version_code(&cwd) {
            Ok(Some((old, new))) => push(&this, cx, format!("Bumped versionCode {old} → {new}")).await,
            Ok(None) => push(&this, cx, "No versionCode found to bump (skipping).").await,
            Err(err) => push(&this, cx, format!("versionCode bump skipped: {err}")).await,
        }
    }

    // 2. Build the release bundle.
    push(&this, cx, "Building release bundle…").await;
    let Some((program, args)) = gradle_command(&cwd, &["bundleRelease"]) else {
        bail!("No Gradle wrapper (gradlew) and no system `gradle` on PATH.");
    };
    if !stream_process(&program, &args, &cwd, &this, cx).await? {
        bail!("Release build failed.");
    }

    // 3. Locate the .aab.
    let aab = locate_aab(&cwd, config.aab_path.as_deref())
        .context("could not find the built .aab (set \"aab_path\" in the config)")?;
    push(&this, cx, format!("Built {}", aab.display())).await;
    let aab_bytes = std::fs::read(&aab).with_context(|| format!("reading {}", aab.display()))?;

    // 4. Authenticate (service account JWT → OAuth2 access token).
    push(&this, cx, "Authenticating with Google Play…").await;
    let sa = ServiceAccount::load(&config.service_account_json_path)?;
    let token = sa.access_token(http_client.as_ref()).await?;

    // 5. Create an edit.
    let pkg = &config.package_name;
    let base = format!("https://androidpublisher.googleapis.com/androidpublisher/v3/applications/{pkg}");
    push(&this, cx, "Creating Play edit…").await;
    let edit: serde_json::Value =
        api_json(http_client.as_ref(), Method::POST, &format!("{base}/edits"), &token, None, AsyncBody::from("")).await?;
    let edit_id = edit
        .get("id")
        .and_then(|v| v.as_str())
        .context("Play API did not return an edit id")?
        .to_string();

    // 6. Upload the bundle.
    push(&this, cx, "Uploading bundle…").await;
    let upload_url = format!(
        "https://androidpublisher.googleapis.com/upload/androidpublisher/v3/applications/{pkg}/edits/{edit_id}/bundles?uploadType=media"
    );
    let uploaded: serde_json::Value = api_json(
        http_client.as_ref(),
        Method::POST,
        &upload_url,
        &token,
        Some("application/octet-stream"),
        AsyncBody::from(aab_bytes),
    )
    .await?;
    let version_code = uploaded
        .get("versionCode")
        .and_then(|v| v.as_i64())
        .context("Play API did not return a versionCode")?;
    push(&this, cx, format!("Uploaded versionCode {version_code}")).await;

    // 7. Assign to the chosen track.
    let track = &config.track;
    push(&this, cx, format!("Assigning to '{track}' track…")).await;
    let track_body = serde_json::json!({
        "track": track,
        "releases": [{ "status": "completed", "versionCodes": [version_code.to_string()] }]
    });
    let _: serde_json::Value = api_json(
        http_client.as_ref(),
        Method::PUT,
        &format!("{base}/edits/{edit_id}/tracks/{track}"),
        &token,
        Some("application/json"),
        AsyncBody::from(serde_json::to_string(&track_body)?),
    )
    .await?;

    // 8. Commit.
    push(&this, cx, "Committing…").await;
    let _: serde_json::Value = api_json(
        http_client.as_ref(),
        Method::POST,
        &format!("{base}/edits/{edit_id}:commit"),
        &token,
        None,
        AsyncBody::from(""),
    )
    .await?;

    push(&this, cx, format!("Done — versionCode {version_code} released to '{track}'.")).await;
    Ok(())
}

#[derive(Deserialize)]
struct ServiceAccount {
    client_email: String,
    private_key: String,
    #[serde(default = "default_token_uri")]
    token_uri: String,
}

fn default_token_uri() -> String {
    "https://oauth2.googleapis.com/token".to_string()
}

impl ServiceAccount {
    fn load(path: &str) -> Result<Self> {
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("reading service account JSON at {path}"))?;
        serde_json::from_str(&raw).context("parsing service account JSON")
    }

    async fn access_token(&self, client: &dyn HttpClient) -> Result<String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0) as i64;
        let claims = serde_json::json!({
            "iss": self.client_email,
            "scope": "https://www.googleapis.com/auth/androidpublisher",
            "aud": self.token_uri,
            "iat": now,
            "exp": now + 3600,
        });
        let key = jsonwebtoken::EncodingKey::from_rsa_pem(self.private_key.as_bytes())
            .context("invalid service account private key")?;
        let jwt = jsonwebtoken::encode(
            &jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256),
            &claims,
            &key,
        )
        .context("signing the auth JWT")?;

        let body = format!(
            "grant_type=urn:ietf:params:oauth:grant-type:jwt-bearer&assertion={jwt}"
        );
        let request = HttpRequest::builder()
            .method(Method::POST)
            .uri(&self.token_uri)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(AsyncBody::from(body))?;
        let mut response = client.send(request).await?;
        let mut text = String::new();
        response.body_mut().read_to_string(&mut text).await?;
        if !response.status().is_success() {
            bail!("Google OAuth failed ({}): {text}", response.status());
        }
        let value: serde_json::Value = serde_json::from_str(&text)?;
        value
            .get("access_token")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow!("OAuth response had no access_token"))
    }
}

/// Send an authenticated Play API request and parse the JSON response.
async fn api_json(
    client: &dyn HttpClient,
    method: Method,
    url: &str,
    token: &str,
    content_type: Option<&str>,
    body: AsyncBody,
) -> Result<serde_json::Value> {
    let mut builder = HttpRequest::builder()
        .method(method)
        .uri(url)
        .header("Authorization", format!("Bearer {token}"));
    if let Some(content_type) = content_type {
        builder = builder.header("Content-Type", content_type);
    }
    let request = builder.body(body)?;
    let mut response = client.send(request).await?;
    let mut text = String::new();
    response.body_mut().read_to_string(&mut text).await?;
    if !response.status().is_success() {
        bail!("Play API {url} failed ({}): {text}", response.status());
    }
    if text.trim().is_empty() {
        return Ok(serde_json::Value::Null);
    }
    serde_json::from_str(&text).with_context(|| format!("parsing Play API response from {url}"))
}

/// Find the built release `.aab`, honoring an explicit override path.
fn locate_aab(cwd: &Path, explicit: Option<&str>) -> Option<PathBuf> {
    if let Some(path) = explicit {
        let p = PathBuf::from(path);
        let p = if p.is_absolute() { p } else { cwd.join(p) };
        return p.is_file().then_some(p);
    }
    for rel in [
        "app/build/outputs/bundle/release/app-release.aab",
        "build/outputs/bundle/release/app-release.aab",
    ] {
        let candidate = cwd.join(rel);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Best-effort `versionCode N` (or `versionCode = N`) increment in the app's
/// build.gradle[.kts]. Returns the old/new values, or None if not found.
fn bump_version_code(cwd: &Path) -> Result<Option<(u64, u64)>> {
    for rel in ["app/build.gradle", "app/build.gradle.kts", "build.gradle", "build.gradle.kts"] {
        let path = cwd.join(rel);
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Some(idx) = text.find("versionCode") else {
            continue;
        };
        // Scan past "versionCode", an optional '=', and whitespace to the integer.
        let after = &text[idx + "versionCode".len()..];
        let digits_start_rel = after.find(|c: char| c.is_ascii_digit());
        let Some(ds) = digits_start_rel else { continue };
        let digits: String = after[ds..]
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        let Ok(old) = digits.parse::<u64>() else {
            continue;
        };
        let new = old + 1;
        let abs_digits_start = idx + "versionCode".len() + ds;
        let mut updated = String::with_capacity(text.len() + 1);
        updated.push_str(&text[..abs_digits_start]);
        updated.push_str(&new.to_string());
        updated.push_str(&text[abs_digits_start + digits.len()..]);
        std::fs::write(&path, updated).with_context(|| format!("writing {}", path.display()))?;
        return Ok(Some((old, new)));
    }
    Ok(None)
}

// --- Logcat / layout inspector / APK analyzer --------------------------------

/// Spawn `program args` and read all of stdout to a string.
async fn capture_output(program: &Path, args: &[&str]) -> Result<String> {
    let mut command = util::command::new_std_command(program);
    command.args(args);
    let mut child = Child::spawn(command, Stdio::null(), Stdio::piped(), Stdio::null())
        .with_context(|| format!("failed to launch `{}`", program.display()))?;
    let stdout = child.stdout.take().context("failed to capture stdout")?;
    let mut out = String::new();
    futures::io::BufReader::new(stdout)
        .read_to_string(&mut out)
        .await?;
    let _ = child.status().await;
    Ok(out)
}

/// Return the serial of the first device/emulator in the `device` state.
async fn first_device_serial(adb: &Path) -> Option<String> {
    let out = capture_output(adb, &["devices"]).await.ok()?;
    out.lines().skip(1).find_map(|line| {
        let mut parts = line.split_whitespace();
        let serial = parts.next()?;
        let state = parts.next()?;
        (state == "device").then(|| serial.to_string())
    })
}

async fn run_logcat(
    adb: PathBuf,
    this: gpui::WeakEntity<AndroidModal>,
    cx: &mut gpui::AsyncApp,
) -> Result<()> {
    let Some(serial) = first_device_serial(&adb).await else {
        bail!("No connected device or emulator (start one first).");
    };
    push(&this, cx, format!("Streaming logcat from {serial} (close to stop)…")).await;
    let cwd = home_dir().unwrap_or_else(|| PathBuf::from("."));
    let args = vec![
        "-s".to_string(),
        serial,
        "logcat".to_string(),
        "-v".to_string(),
        "threadtime".to_string(),
    ];
    stream_process(&adb, &args, &cwd, &this, cx).await?;
    Ok(())
}

async fn run_layout_inspector(
    adb: PathBuf,
    this: gpui::WeakEntity<AndroidModal>,
    cx: &mut gpui::AsyncApp,
) -> Result<()> {
    let Some(serial) = first_device_serial(&adb).await else {
        bail!("No connected device or emulator (start one first).");
    };
    push(&this, cx, "Dumping view hierarchy…").await;
    let remote = "/sdcard/lingcode_layout.xml";
    capture_output(
        &adb,
        &["-s", serial.as_str(), "shell", "uiautomator", "dump", remote],
    )
    .await?;
    let xml = capture_output(&adb, &["-s", serial.as_str(), "shell", "cat", remote]).await?;
    if xml.trim().is_empty() {
        bail!("uiautomator returned no hierarchy (is the screen on?).");
    }
    // The dump is one long line; chunk it so it's readable in the modal.
    for chunk in xml.trim().as_bytes().chunks(200) {
        push(&this, cx, String::from_utf8_lossy(chunk).into_owned()).await;
    }
    Ok(())
}

/// Find `aapt2` under `<sdk>/build-tools/<version>/`.
fn find_aapt2(sdk: Option<&Path>) -> Option<PathBuf> {
    let sdk = sdk?;
    let build_tools = sdk.join("build-tools");
    let mut versions: Vec<PathBuf> = std::fs::read_dir(&build_tools)
        .ok()?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .collect();
    versions.sort();
    for dir in versions.into_iter().rev() {
        let candidate = dir.join(exe_name("aapt2"));
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Pick two APK/AAB files via the native dialog, then open a modal diffing them.
fn apk_diff(workspace: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
    let _ = workspace;
    let sdk = android_sdk();
    let paths_receiver = cx.prompt_for_paths(gpui::PathPromptOptions {
        files: true,
        directories: false,
        multiple: true,
        prompt: Some("Select two APK/AAB files to compare".into()),
    });
    let workspace = cx.weak_entity();
    window
        .spawn(cx, async move |cx| {
            let paths = match paths_receiver.await {
                Ok(Ok(Some(paths))) => paths,
                _ => return,
            };
            workspace
                .update_in(cx, |workspace, window, cx| {
                    let job = if paths.len() == 2 {
                        AndroidJob::ApkDiff {
                            a: paths[0].clone(),
                            b: paths[1].clone(),
                            sdk,
                        }
                    } else {
                        AndroidJob::Report(vec![
                            "Select exactly two APK/AAB files to compare.".into(),
                        ])
                    };
                    workspace.toggle_modal(window, cx, move |_window, cx| {
                        AndroidModal::new("APK / AAB Diff".into(), job, cx)
                    });
                })
                .ok();
        })
        .detach();
}

async fn run_apk_diff(
    a: PathBuf,
    b: PathBuf,
    sdk: Option<PathBuf>,
    this: gpui::WeakEntity<AndroidModal>,
    cx: &mut gpui::AsyncApp,
) -> Result<()> {
    let size_a = std::fs::metadata(&a).map(|m| m.len()).unwrap_or(0);
    let size_b = std::fs::metadata(&b).map(|m| m.len()).unwrap_or(0);
    push(&this, cx, format!("A: {}", a.display())).await;
    push(&this, cx, format!("B: {}", b.display())).await;
    push(&this, cx, format!("Size A: {} KB", size_a / 1024)).await;
    push(&this, cx, format!("Size B: {} KB", size_b / 1024)).await;
    push(
        &this,
        cx,
        format!(
            "Size delta (B - A): {} KB",
            (size_b as i64 - size_a as i64) / 1024
        ),
    )
    .await;
    push(&this, cx, String::new()).await;

    let Some(aapt2) = find_aapt2(sdk.as_deref()) else {
        push(
            &this,
            cx,
            "aapt2 not found (install Android SDK build-tools) — size diff only.".to_string(),
        )
        .await;
        return Ok(());
    };

    let a_str = a.to_string_lossy();
    let badging_a = capture_output(&aapt2, &["dump", "badging", a_str.as_ref()])
        .await
        .unwrap_or_default();
    let b_str = b.to_string_lossy();
    let badging_b = capture_output(&aapt2, &["dump", "badging", b_str.as_ref()])
        .await
        .unwrap_or_default();

    let lines_a: std::collections::BTreeSet<&str> = badging_a.lines().collect();
    let lines_b: std::collections::BTreeSet<&str> = badging_b.lines().collect();

    push(&this, cx, "Only in A (removed):".to_string()).await;
    let mut removed = 0;
    for line in lines_a.difference(&lines_b).take(60) {
        push(&this, cx, format!("- {}", line)).await;
        removed += 1;
    }
    if removed == 0 {
        push(&this, cx, "  (none)".to_string()).await;
    }

    push(&this, cx, String::new()).await;
    push(&this, cx, "Only in B (added):".to_string()).await;
    let mut added = 0;
    for line in lines_b.difference(&lines_a).take(60) {
        push(&this, cx, format!("+ {}", line)).await;
        added += 1;
    }
    if added == 0 {
        push(&this, cx, "  (none)".to_string()).await;
    }

    Ok(())
}

async fn run_analyze_apk(
    cwd: PathBuf,
    sdk: Option<PathBuf>,
    this: gpui::WeakEntity<AndroidModal>,
    cx: &mut gpui::AsyncApp,
) -> Result<()> {
    let artifact = [
        "app/build/outputs/apk/debug/app-debug.apk",
        "app/build/outputs/bundle/release/app-release.aab",
        "app/build/outputs/apk/release/app-release.apk",
        "build/outputs/apk/debug/app-debug.apk",
    ]
    .iter()
    .map(|rel| cwd.join(rel))
    .find(|path| path.is_file());

    let Some(artifact) = artifact else {
        bail!("No built APK/AAB found — build the app first.");
    };
    push(&this, cx, format!("Analyzing {}", artifact.display())).await;
    if let Ok(meta) = std::fs::metadata(&artifact) {
        push(&this, cx, format!("Size: {} KB", meta.len() / 1024)).await;
    }

    match find_aapt2(sdk.as_deref()) {
        Some(aapt2) => {
            let artifact_str = artifact.to_string_lossy();
            let badging =
                capture_output(&aapt2, &["dump", "badging", artifact_str.as_ref()]).await?;
            for line in badging.lines().take(80) {
                push(&this, cx, line.to_string()).await;
            }
        }
        None => {
            push(
                &this,
                cx,
                "aapt2 not found (install Android SDK build-tools) — size only.".to_string(),
            )
            .await;
        }
    }
    Ok(())
}

impl ModalView for AndroidModal {
    fn on_before_dismiss(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> DismissDecision {
        DismissDecision::Dismiss(true)
    }
}

impl Focusable for AndroidModal {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EventEmitter<DismissEvent> for AndroidModal {}

impl Render for AndroidModal {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let log = v_flex().gap_0p5().children(
            self.lines.iter().map(|line| {
                Label::new(line.clone())
                    .size(LabelSize::Small)
                    .color(Color::Muted)
            }),
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
            Status::Done => h_flex()
                .child(
                    Button::new("close", "Close")
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(DismissEvent))),
                )
                .into_any_element(),
        };

        v_flex()
            .key_context("LingCodeAndroid")
            .track_focus(&self.focus_handle)
            .elevation_3(cx)
            .w(px(560.))
            .p_4()
            .gap_3()
            .child(Label::new(self.title.clone()).size(LabelSize::Large))
            .child(log)
            .child(footer)
    }
}
