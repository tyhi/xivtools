use crate::task::Task;
use anyhow::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Options {
    // Stored as i32 because imgui doesn't bind to unsigned ints.
    #[serde(default)]
    pub gear: [i32; xiv::JOB_CNT],
    #[serde(default)]
    pub specialist: [bool; xiv::JOB_CNT],
    #[serde(default)]
    pub should_clear_window_on_craft: bool,
    #[serde(default)]
    pub remove_finished_tasks: bool,
    #[serde(default)]
    pub use_trial_synthesis: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            gear: [0; xiv::JOB_CNT],
            specialist: [false; xiv::JOB_CNT],
            use_trial_synthesis: false,
            should_clear_window_on_craft: true,
            remove_finished_tasks: true,
        }
    }
}

// Placeholder.
#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize, Default)]
pub struct Macro {}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub options: Options,
    #[serde(default)]
    pub tasks: Vec<Task>,
    #[serde(default)]
    pub macros: Vec<Macro>,
}

pub const DEFAULT_CONFIG_FILE: &str = "config.json";

pub fn get_config(path: Option<&Path>) -> Config {
    let p = path.unwrap_or_else(|| Path::new(DEFAULT_CONFIG_FILE));
    read_config(p).unwrap_or_else(|e| {
        log::info!(
            "Unable to open {} ({}), creating a new default config.",
            p.display(),
            e.to_string()
        );
        Config::default()
    })
}

// Reads the config from disk. Returns a default config if one cannot be found.
pub fn read_config(path: &Path) -> Result<Config, Error> {
    Ok(serde_json::from_str::<Config>(&std::fs::read_to_string(
        path,
    )?)?)
}

// Writes |cfg| to disk.
pub fn write_config(path: Option<&Path>, cfg: &Config) -> Result<(), Error> {
    let p = path.unwrap_or_else(|| Path::new(DEFAULT_CONFIG_FILE));
    std::fs::write(p, serde_json::to_string_pretty(&cfg)?.as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() -> Result<(), Error> {
        assert_eq!(
            get_config(Some(&Path::new("config.json.test"))),
            Config::default()
        );
        Ok(())
    }
}
