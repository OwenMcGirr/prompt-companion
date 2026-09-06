use crate::core::Draft;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    io::Write,
    path::{Path, PathBuf},
};
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub font_size: f64,
    pub button_height: f64,
    pub automatic: bool,
    pub floating: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            font_size: 22.,
            button_height: 86.,
            automatic: true,
            floating: true,
        }
    }
}
impl Settings {
    pub fn normalize(&mut self) {
        self.font_size = if self.font_size.is_finite() {
            self.font_size.clamp(18., 32.)
        } else {
            22.
        };
        self.button_height = if self.button_height.is_finite() {
            self.button_height.clamp(72., 120.)
        } else {
            86.
        };
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Saved {
    pub version: u32,
    pub selected_task_id: Option<String>,
    pub drafts: HashMap<String, Draft>,
    pub settings: Settings,
}
impl Default for Saved {
    fn default() -> Self {
        Self {
            version: 1,
            selected_task_id: None,
            drafts: HashMap::new(),
            settings: Settings::default(),
        }
    }
}
pub fn write_private(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path.parent().ok_or("Missing storage directory")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))
            .map_err(|e| e.to_string())?;
    }
    let mut f = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        f.as_file()
            .set_permissions(std::fs::Permissions::from_mode(0o600))
            .map_err(|e| e.to_string())?;
    }
    f.write_all(bytes)
        .and_then(|_| f.as_file().sync_all())
        .map_err(|e| e.to_string())?;
    f.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}
pub struct Store {
    pub dir: PathBuf,
    pub writable: bool,
}
impl Store {
    pub fn load(dir: PathBuf) -> (Self, Saved, Option<String>) {
        let mut store = Self {
            dir,
            writable: true,
        };
        match store.read() {
            Ok(saved) => (store, saved, None),
            Err(e) => {
                store.writable = false;
                (store,Saved::default(),Some(format!("Saved drafts could not be loaded: {e}. Existing files are untouched; new edits remain in memory.")))
            }
        }
    }
    fn read(&self) -> Result<Saved, String> {
        let path = self.dir.join("state.json");
        let mut saved: Saved = if path.exists() {
            serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?
        } else {
            Saved::default()
        };
        if saved.version != 1 {
            return Err("unsupported storage version".into());
        }
        saved.settings.normalize();
        for d in saved.drafts.values_mut() {
            *d = d.normalized();
        }
        self.save(&saved)?;
        Ok(saved)
    }
    pub fn save(&self, saved: &Saved) -> Result<(), String> {
        if !self.writable {
            return Err("Storage is unavailable; use Copy Prompt before quitting.".into());
        }
        write_private(
            &self.dir.join("state.json"),
            &serde_json::to_vec(saved).map_err(|e| e.to_string())?,
        )
    }
}
