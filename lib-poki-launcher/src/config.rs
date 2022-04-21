/***
 * This file is part of Poki Launcher.
 *
 * Poki Launcher is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * Poki Launcher is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with Poki Launcher.  If not, see <https://www.gnu.org/licenses/>.
 */
use crate::DIRS;
use serde::{Deserialize, Serialize};
use shellexpand::LookupError;
use std::default::Default;
use std::env::VarError;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// User defended app setting
///
/// These are the setting that can be overridden in Poki Launcher's config file
/// located at `~/.config/poki-launcher/poki-launcher.hjson`. When no config
/// file is present the default values are used. Only values the user wishes to
/// override need to be specified.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// The list of directories to search for desktop entries in.
    ///
    /// Default:
    /// - /usr/share/applications
    /// - ~/.local/share/applications/
    /// - /var/lib/snapd/desktop/applications
    /// - /var/lib/flatpak/exports/share/applications
    pub app_paths: Vec<PathBuf>,
    /// Command to use to run terminal apps
    ///
    /// Default: None which uses "$TERM -e {}"
    pub term_cmd: Option<String>,
    /// Frecency half life
    ///
    /// Warning: Changing the half life will reset the app scores
    pub half_life: f32,

    /// Hight of the launcher window pxs
    ///
    /// Default: 500
    pub window_height: i32,
    /// Width of the launcher window in pxs
    ///
    /// Default: 500
    pub window_width: i32,
    /// Launcher window background color
    ///
    /// Default: #282a36
    pub background_color: String,
    /// Launcher window border color
    ///
    /// Default: #2e303b
    pub border_color: String,
    /// Launcher input box background color
    ///
    /// Default: #44475a
    pub input_box_color: String,
    /// Launcher input box text color
    ///
    /// Default: #f8f8f2
    pub input_text_color: String,
    /// Launcher app list selected app background color
    ///
    /// Default: #44475a
    pub selected_app_color: String,
    /// Launcher app list text color
    ///
    /// Default: #f8f8f2
    pub app_text_color: String,
    /// Launcher app list separator color
    ///
    /// Default: #bd93f9
    pub app_separator_color: String,
    /// Launcher input box font size
    ///
    /// Default: 13
    pub input_font_size: i32,
    /// Launcher app list font size
    ///
    /// Default: 20
    pub app_font_size: i32,
    /// Ratio between launcher input box height and total height
    ///
    /// Default: 0.1
    /// Ex. 0.1 == 10% of total window height
    pub input_box_ratio: f32,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            app_paths: vec![
                "/usr/share/applications".into(),
                "~/.local/share/applications/".into(),
                "/var/lib/snapd/desktop/applications".into(),
                "/var/lib/flatpak/exports/share/applications".into(),
            ],
            term_cmd: None,
            // Default half life of 7 days
            half_life: 7.0,

            window_height: 500,
            window_width: 500,

            background_color: "#282a36".into(),
            border_color: "#2e303b".into(),
            input_box_color: "#44475a".into(),
            input_text_color: "#f8f8f2".into(),
            selected_app_color: "#44475a".into(),
            app_text_color: "#f8f8f2".into(),
            app_separator_color: "#bd93f9".into(),

            input_font_size: 13,
            app_font_size: 20,
            input_box_ratio: 0.1,
        }
    }
}

impl Config {
    /// Load the app config.
    pub fn load() -> Result<Config, ConfigError> {
        let mut cfg = config::Config::default();
        let config_dir = DIRS.config_dir();
        let file_path = config_dir.join("poki-launcher.hjson");

        let mut config = if file_path.as_path().exists() {
            cfg.merge(config::File::with_name(file_path.to_str().unwrap()))?;
            cfg.try_into()?
        } else {
            Self::default()
        };

        let mut expanded_paths = Vec::with_capacity(config.app_paths.len());
        for path in config.app_paths.into_iter() {
            match path.to_str() {
                Some(s) => {
                    let expanded = shellexpand::full(s).map_err(|e| {
                        ConfigError::ExpandPath(s.to_owned(), e)
                    })?;
                    expanded_paths.push(Path::new(&*expanded).to_owned());
                }
                None => expanded_paths.push(path),
            }
        }
        config.app_paths = expanded_paths;
        const DAYS_TO_SECS: f32 = 24. * 60. * 60.;
        config.half_life *= DAYS_TO_SECS;

        Ok(config)
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Error loading config: {0}")]
    Parse(#[from] config::ConfigError),
    #[error("Error expanding app_path value `{0}`: {1}")]
    ExpandPath(String, LookupError<VarError>),
}
