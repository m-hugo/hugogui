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
/// Application configuration
mod config;
/// Interact with the app database
mod db;
/// Parse desktop entries
mod desktop_entry;
/// Run an app
mod runner;
/// Scan for desktop entries
mod scan;
pub mod hot_reload;

use directories_next::ProjectDirs;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::{
    cmp::{Eq, Ord, Ordering, PartialEq, PartialOrd},
    path::PathBuf,
};
use uuid::Uuid;

pub use crate::config::Config;
pub use crate::db::AppsDB;

/// Custom error types
pub mod error {
    pub use crate::db::AppDBError;
    pub use crate::desktop_entry::EntryParseError;
    pub use crate::runner::RunError;
    pub use crate::scan::ScanError;
}

lazy_static! {
    /// Object to get paths to various dir used by this app
    ///
    /// Namely:
    /// - config_dir
    /// - data_local_dir
    static ref DIRS: ProjectDirs =
        ProjectDirs::from("dev", "Ben Aaron Goldberg", "Poki-Launcher")
            .unwrap();
    /// Path to the DB file
    pub static ref DB_PATH: PathBuf = {
        let data_dir = DIRS.data_dir();
        let mut db_path = data_dir.to_path_buf();
        db_path.push("apps.db");
        db_path
    };
    /// Path to the config file
    pub static ref CFG_PATH: PathBuf = {
        let config_dir = DIRS.config_dir();
        config_dir.join("poki-launcher.hjson")
    };
}

/// An app on your machine.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct App {
    /// Display name of the app.
    pub name: String,
    /// The exec string used to run the app.
    pub exec: Vec<String>,
    /// Score of the app of the ranking algo.
    score: f32,
    /// Uuid used to uniquely identify this app.
    /// This is saved to find the app later when the list changes.
    pub uuid: String,
    /// Icon name for this app.
    /// The icon name has to be looked up in the system's icon
    /// theme to get a file path.
    pub icon: String,
    /// If true, launch in terminal
    pub(crate) terminal: bool,
}

impl App {
    /// Create a new app.
    pub fn new(
        name: String,
        icon: String,
        exec: Vec<String>,
        terminal: bool,
    ) -> App {
        App {
            name,
            icon,
            exec,
            uuid: Uuid::new_v4().to_string(),
            score: 0.0,
            terminal,
        }
    }

    /// Set this app's name, icon, and exec to the values of the other app.
    pub fn merge(&mut self, other: &App) {
        self.name = other.name.clone();
        self.icon = other.icon.clone();
        self.exec = other.exec.clone();
    }
}

impl PartialEq for App {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.exec == other.exec
            && self.icon == other.icon
    }
}

impl Eq for App {}

impl Ord for App {
    fn cmp(&self, other: &Self) -> Ordering {
        self.name
            .cmp(&other.name)
            .then(self.exec.cmp(&other.exec))
            .then(self.icon.cmp(&other.icon))
    }
}

impl PartialOrd for App {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for App {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}
