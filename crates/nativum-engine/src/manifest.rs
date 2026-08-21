//! `app.json` and the three-files-of-truth load path.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use nativum_core::{Appearance, Error, Result};

use crate::core_json::JsonCore;

/// Window size and title.
#[derive(Clone, Debug, Deserialize)]
pub struct WindowSpec {
    /// Title bar / preview heading.
    #[serde(default = "default_title")]
    pub title: String,
    /// Logical width.
    #[serde(default = "default_width")]
    pub width: f32,
    /// Logical height.
    #[serde(default = "default_height")]
    pub height: f32,
}

fn default_title() -> String {
    "nativum".to_string()
}
fn default_width() -> f32 {
    400.0
}
fn default_height() -> f32 {
    320.0
}

impl Default for WindowSpec {
    fn default() -> Self {
        Self {
            title: default_title(),
            width: default_width(),
            height: default_height(),
        }
    }
}

/// App manifest. Fields we don't use yet are ignored.
#[derive(Clone, Debug, Deserialize)]
pub struct Manifest {
    /// Package name.
    #[serde(default)]
    pub name: String,
    /// Display name.
    #[serde(default)]
    pub display_name: String,
    /// Version.
    #[serde(default = "default_version")]
    pub version: String,
    /// Window.
    #[serde(default)]
    pub window: WindowSpec,
    /// View path, relative to the app directory.
    #[serde(default = "default_view")]
    pub view: String,
    /// JSON core path.
    #[serde(default = "default_core")]
    pub core: String,
    /// `"light"` / `"dark"` / `"system"` (system currently means light).
    #[serde(default)]
    pub appearance: String,
}

fn default_version() -> String {
    "0.1.0".to_string()
}
fn default_view() -> String {
    "src/app.native".to_string()
}
fn default_core() -> String {
    "src/core.json".to_string()
}

impl Manifest {
    /// Resolve appearance.
    pub fn appearance(&self) -> Appearance {
        match self.appearance.as_str() {
            "dark" => Appearance::Dark,
            _ => Appearance::Light,
        }
    }
}

/// An app directory after loading.
pub struct LoadedApp {
    /// Directory.
    pub dir: PathBuf,
    /// Manifest.
    pub manifest: Manifest,
    /// Markup source.
    pub view_source: String,
    /// JSON core.
    pub core: JsonCore,
}

/// Load `app.json` + view + core from `dir`.
pub fn load_app_dir(dir: &Path) -> Result<LoadedApp> {
    let manifest_path = if dir.join("app.json").exists() {
        dir.join("app.json")
    } else {
        return Err(Error::Config(format!("no app.json in {}", dir.display())));
    };
    let text = std::fs::read_to_string(&manifest_path)?;
    let mut manifest: Manifest = serde_json::from_str(&text)?;
    if manifest.name.is_empty() {
        manifest.name = dir
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("app")
            .to_string();
    }
    if (manifest.window.title.is_empty() || manifest.window.title == "nativum")
        && !manifest.display_name.is_empty()
    {
        manifest.window.title = manifest.display_name.clone();
    }
    let view_path = dir.join(&manifest.view);
    let view_source = std::fs::read_to_string(&view_path)
        .map_err(|e| Error::Config(format!("cannot read {}: {e}", view_path.display())))?;
    let core_path = dir.join(&manifest.core);
    let core = JsonCore::load(&core_path)?;
    Ok(LoadedApp {
        dir: dir.to_path_buf(),
        manifest,
        view_source,
        core,
    })
}
