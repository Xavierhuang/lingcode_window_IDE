//! "New from Template" — scaffold a starter project from an embedded template
//! and open it as a workspace.
//!
//! Templates are compiled into the binary via `include_str!` (no network),
//! mirroring the offline-first approach of the other LingCode crates. The flow
//! is: pick a template (prompt) → pick a parent folder (system path picker) →
//! write the files into a fresh, non-colliding project directory → open it.
//!
//! To add a template: drop its files under `templates/<dir>/`, list them in a
//! `&[TemplateFile]` const with `template_file!`, and add a `Template` entry to
//! `TEMPLATES`.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use fs::Fs;
use gpui::{App, Context, PathPromptOptions, PromptLevel, Window};
use workspace::{AppState, NewFromTemplate, OpenMode, Workspace};

/// One file inside a template: a path relative to the project root and its contents.
struct TemplateFile {
    path: &'static str,
    contents: &'static str,
}

/// A starter project template.
struct Template {
    /// Label shown in the picker.
    label: &'static str,
    /// Default directory name created for a new project.
    slug: &'static str,
    files: &'static [TemplateFile],
}

/// Embeds `templates/<dir>/<path>` and records it under the relative `<path>`.
macro_rules! template_file {
    ($dir:literal, $path:literal) => {
        TemplateFile {
            path: $path,
            contents: include_str!(concat!("../templates/", $dir, "/", $path)),
        }
    };
}

const PYTHON_FILES: &[TemplateFile] = &[
    template_file!("python", "main.py"),
    template_file!("python", "requirements.txt"),
    template_file!("python", "tests/test_main.py"),
    template_file!("python", ".gitignore"),
    template_file!("python", "README.md"),
];

const WEB_GAME_FILES: &[TemplateFile] = &[
    template_file!("web-game", "index.html"),
    template_file!("web-game", "game.js"),
    template_file!("web-game", "style.css"),
    template_file!("web-game", "README.md"),
];

const ANDROID_FILES: &[TemplateFile] = &[
    template_file!("android", "settings.gradle.kts"),
    template_file!("android", "build.gradle.kts"),
    template_file!("android", "gradle.properties"),
    template_file!("android", "app/build.gradle.kts"),
    template_file!("android", "app/src/main/AndroidManifest.xml"),
    template_file!("android", "app/src/main/kotlin/dev/lingcode/app/MainActivity.kt"),
    template_file!("android", "app/src/main/res/layout/activity_main.xml"),
    template_file!("android", "app/src/main/res/values/strings.xml"),
    template_file!("android", ".gitignore"),
    template_file!("android", "README.md"),
];

const NODE_TS_FILES: &[TemplateFile] = &[
    template_file!("node-ts", "package.json"),
    template_file!("node-ts", "tsconfig.json"),
    template_file!("node-ts", "src/index.ts"),
    template_file!("node-ts", ".gitignore"),
    template_file!("node-ts", "README.md"),
];

const TEMPLATES: &[Template] = &[
    Template {
        label: "Python app",
        slug: "python-app",
        files: PYTHON_FILES,
    },
    Template {
        label: "Web / HTML5 game",
        slug: "web-game",
        files: WEB_GAME_FILES,
    },
    Template {
        label: "Android app",
        slug: "android-app",
        files: ANDROID_FILES,
    },
    Template {
        label: "Node / TypeScript",
        slug: "node-ts-app",
        files: NODE_TS_FILES,
    },
];

pub fn init(_app_state: Arc<AppState>, cx: &mut App) {
    cx.observe_new(|workspace: &mut Workspace, _window, _cx: &mut Context<Workspace>| {
        workspace.register_action(|workspace, _: &NewFromTemplate, window, cx| {
            new_from_template(workspace, window, cx);
        });
    })
    .detach();
}

fn new_from_template(workspace: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
    let mut answers: Vec<&str> = TEMPLATES.iter().map(|t| t.label).collect();
    answers.push("Cancel");
    let selection = window.prompt(
        PromptLevel::Info,
        "New Project from Template",
        Some("Choose a starter template, then pick a folder to create it in."),
        &answers,
        cx,
    );
    let fs = workspace.app_state().fs.clone();

    cx.spawn_in(window, async move |workspace, cx| {
        // Dismissed prompt, or the trailing "Cancel" entry (out of range), bails out.
        let Ok(index) = selection.await else {
            return Ok(());
        };
        let Some(template) = TEMPLATES.get(index) else {
            return Ok(());
        };

        // Ask for the parent directory to create the project in.
        let paths_rx = workspace.update_in(cx, |_workspace, _window, cx| {
            cx.prompt_for_paths(PathPromptOptions {
                files: false,
                directories: true,
                multiple: false,
                prompt: Some("Create project in".into()),
            })
        })?;
        let parent = match paths_rx.await {
            Ok(Ok(Some(paths))) => match paths.into_iter().next() {
                Some(path) => path,
                None => return Ok(()),
            },
            // Cancelled, or the picker errored — nothing to do.
            _ => return Ok(()),
        };

        // Scaffold into a fresh, non-colliding project directory.
        let target = unique_dir(fs.as_ref(), &parent, template.slug).await;
        for file in template.files {
            let path = target.join(file.path);
            if let Some(dir) = path.parent() {
                fs.create_dir(dir).await?;
            }
            fs.write(&path, file.contents.as_bytes()).await?;
        }

        workspace
            .update_in(cx, |workspace, window, cx| {
                workspace.open_workspace_for_paths(OpenMode::Activate, vec![target], window, cx)
            })?
            .await?;
        anyhow::Ok(())
    })
    .detach_and_log_err(cx);
}

/// Returns `parent/slug`, appending `-1`, `-2`, … until the path is unused.
async fn unique_dir(fs: &dyn Fs, parent: &Path, slug: &str) -> PathBuf {
    let mut candidate = parent.join(slug);
    let mut n = 1;
    while exists(fs, &candidate).await {
        candidate = parent.join(format!("{slug}-{n}"));
        n += 1;
    }
    candidate
}

async fn exists(fs: &dyn Fs, path: &Path) -> bool {
    fs.is_dir(path).await || fs.is_file(path).await
}
