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
use std::fs::File;
use std::io;
use std::io::Write as _;
use std::process;
use std::time::SystemTime;
use std::{cmp::Ordering, fs::create_dir, path::PathBuf};

use crate::{
    config::Config,
    scan::{scan_desktop_entries, ScanError},
};
use file_locker::FileLock;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use log::*;
use rmp_serde as rmp;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{App, DB_PATH, DIRS};

/// An apps database.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppsDB {
    /// The list of apps.
    pub apps: Vec<App>,
    /// The reference time used in the ranking calculations.
    reference_time: f64,
    /// The half life of the app launches
    half_life: f32,
    /// App config
    #[serde(skip_serializing, skip_deserializing)]
    pub config: Config,
}

/// Main methods used to manage thr AppsDB
impl AppsDB {
    /// Initialize the AppsDB
    ///
    /// Load the DB and rescan if it exists or initialize a new one if it
    /// doesn't. On successful initialization, returns an AppsDB and a list of
    /// scan errors that where encountered while scanning for apps. Scan errors
    /// can generally be ignored but you might want to log them. If the half
    /// life in the config and the half life in the AppsDB on load, the AppsDB
    /// will be reinitialized.
    pub fn init(
        config: Config,
    ) -> Result<(AppsDB, Vec<ScanError>), AppDBError> {
        let (apps_db, errors) = if Self::exists() {
            let mut apps_db = Self::load(config.clone())?;
            if (config.half_life - apps_db.half_life).abs() < f32::EPSILON {
                let errors = apps_db.rescan_desktop_entries()?;
                (apps_db, errors)
            } else {
                info!(
                    "Resetting AppsDB due to altered half life {} => {}",
                    apps_db.half_life, config.half_life
                );
                Self::from_desktop_entries(config)
            }
        } else {
            Self::from_desktop_entries(config)
        };
        apps_db.save()?;
        Ok((apps_db, errors))
    }

    /// This ranks the apps both by frecency score and fuzzy search.
    pub fn get_ranked_list(
        &self,
        search: Option<&str>,
        num_items: Option<usize>,
    ) -> Vec<App> {
        let matcher = SkimMatcherV2::default();
        let iter = self.apps.iter();
        let mut app_list = match search {
            Some(search) => iter
                .filter_map(|app| {
                    match matcher.fuzzy_match(&app.name, search) {
                        Some(score) if score > 0 => {
                            let mut app = app.clone();
                            app.score =
                                self.get_frecency(&app) + score as f32 / 100.;
                            Some(app)
                        }
                        _ => None,
                    }
                })
                .collect::<Vec<App>>(),
            None => iter.cloned().collect::<Vec<App>>(),
        };
        app_list.sort_unstable_by(|left, right| {
            right.score.partial_cmp(&left.score).unwrap()
        });
        if let Some(n) = num_items {
            app_list = app_list.into_iter().take(n).collect();
        }
        app_list
    }

    /// Increment to score for app `to_update` by 1 launch and save to DB.
    pub fn update(&mut self, to_update: &App) -> Result<(), AppDBError> {
        self.update_score(&to_update.uuid, 1.0);
        self.sort();
        self.save()
    }

    /// Update self with new desktop entries.
    ///
    /// Scan the desktop entries again then merge the new list into self with
    /// `AppsDB.merge` then saves those changes. Returns a list of scan errors
    /// on success and an AppDBError id saving the new DB failed. Scan errors
    /// can generally be ignored.
    pub fn rescan_desktop_entries(
        &mut self,
    ) -> Result<Vec<ScanError>, AppDBError> {
        let (apps, errors) = scan_desktop_entries(&self.config.app_paths);
        self.merge_new_entries(apps);
        self.save()?;
        Ok(errors)
    }
}

/// Other lower level methods that may be required sometimes
impl AppsDB {
    /// Create a new app.
    pub fn new(config: Config, apps: Vec<App>) -> Self {
        AppsDB {
            apps,
            reference_time: current_time_secs(),
            half_life: config.half_life,
            config,
        }
    }

    /// Create an `AppsDB` from the desktop entries.
    ///
    /// Returns and scan errors encountered while finding apps in addition
    /// to the AppsDB.
    pub fn from_desktop_entries(config: Config) -> (AppsDB, Vec<ScanError>) {
        let (apps, errors) = scan_desktop_entries(&config.app_paths);
        (AppsDB::new(config, apps), errors)
    }

    /// Load database file.
    pub fn load(config: Config) -> Result<AppsDB, AppDBError> {
        let lock = FileLock::lock(&*DB_PATH, true, false).map_err(|err| {
            AppDBError::FileOpen {
                file_path: DB_PATH.to_owned(),
                err,
            }
        })?;
        let mut apps: AppsDB =
            rmp::from_read(&lock.file).map_err(|err| AppDBError::ParseDB {
                file_path: DB_PATH.to_owned(),
                err,
            })?;
        apps.config = config;
        Ok(apps)
    }

    /// Save database file.
    pub fn save(&self) -> Result<(), AppDBError> {
        let data_dir = DIRS.data_dir();
        if !data_dir.exists() {
            create_dir(&data_dir).map_err(|err| AppDBError::DirCreate {
                dir_path: data_dir.to_owned(),
                err,
            })?;
        }
        let buf = rmp::to_vec(&self).expect("Failed to encode apps db");
        if DB_PATH.exists() {
            let mut lock =
                FileLock::lock(&*DB_PATH, true, true).map_err(|err| {
                    AppDBError::FileOpen {
                        file_path: DB_PATH.to_owned(),
                        err,
                    }
                })?;
            lock.file
                .write_all(&buf)
                .map_err(|err| AppDBError::FileWrite {
                    file_path: DB_PATH.to_owned(),
                    err,
                })?;
        } else {
            let mut file = File::create(&*DB_PATH).map_err(|err| {
                AppDBError::FileCreate {
                    file_path: DB_PATH.to_owned(),
                    err,
                }
            })?;
            file.write_all(&buf).map_err(|err| AppDBError::FileWrite {
                file_path: DB_PATH.to_owned(),
                err,
            })?;
        }
        Ok(())
    }

    /// Sort the apps database by score.
    fn sort(&mut self) {
        self.apps.sort_unstable_by(|left, right| {
            left.score
                .partial_cmp(&right.score)
                .unwrap_or(Ordering::Less)
        });
    }

    /// Seconds elapsed since the reference time.
    fn secs_elapsed(&self) -> f32 {
        (current_time_secs() - self.reference_time) as f32
    }

    /// Update the score of an app.
    ///
    /// Note: Does not save the DB change.
    ///
    /// # Arguments
    ///
    /// * `uuid` - The uuid of the app to update.
    /// * `weight` - The amount to update to score by.
    pub fn update_score(&mut self, uuid: &str, weight: f32) {
        let elapsed = self.secs_elapsed();
        self.apps
            .iter_mut()
            .find(|app| app.uuid == *uuid)
            .unwrap()
            .update_frecency(weight, elapsed, self.half_life);
    }

    /// Merge the apps from a re-scan into the database.
    ///
    /// * Apps in `self` that are not in `apps_to_merge` will be removed from `self`
    /// * Apps in `apps_to_merge` not in `self` will be added to `self`
    pub fn merge_new_entries(&mut self, mut apps_to_merge: Vec<App>) {
        let apps = std::mem::take(&mut self.apps);
        self.apps = apps
            .into_iter()
            .filter(|app| apps_to_merge.contains(app))
            .collect();
        apps_to_merge = apps_to_merge
            .into_iter()
            .filter(|app| !self.apps.contains(app))
            .collect();
        self.apps.extend(apps_to_merge);
    }

    /// DB file exists
    pub fn exists() -> bool {
        DB_PATH.exists()
    }

    /// Get app frecency
    fn get_frecency(&self, app: &App) -> f32 {
        app.get_frecency(self.secs_elapsed(), self.half_life)
    }
}

impl App {
    /// Get app frecency
    fn get_frecency(&self, elapsed: f32, half_life: f32) -> f32 {
        self.score / 2.0f32.powf(elapsed / half_life)
    }

    /// Set app frecency
    fn set_frecency(&mut self, new: f32, elapsed: f32, half_life: f32) {
        self.score = new * 2.0f32.powf(elapsed / half_life);
    }

    /// Update app frecency
    fn update_frecency(&mut self, weight: f32, elapsed: f32, half_life: f32) {
        self.set_frecency(
            self.get_frecency(elapsed, half_life) + weight,
            elapsed,
            half_life,
        );
    }
}

/// Return the current time in seconds as a float
pub fn current_time_secs() -> f64 {
    match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
        Ok(n) => {
            (u128::from(n.as_secs()) * 1000 + u128::from(n.subsec_millis()))
                as f64
                / 1000.0
        }
        Err(e) => {
            error!("invalid system time: {}", e);
            process::exit(1);
        }
    }
}

/// Error from working with the AppsDB
#[derive(Debug, Error)]
pub enum AppDBError {
    /// Error opening DB file
    #[error("Failed to open apps database file {file_path}: {err}")]
    FileOpen { file_path: PathBuf, err: io::Error },
    /// Error creating DB file
    #[error("Failed to create apps database file {file_path}: {err}")]
    FileCreate { file_path: PathBuf, err: io::Error },
    /// Error creating DB directory
    #[error("Failed to create directory for apps database {dir_path}: {err}")]
    DirCreate { dir_path: PathBuf, err: io::Error },
    #[error("Failed to write to apps database file {file_path}: {err}")]
    /// Error writing to DB file
    FileWrite { file_path: PathBuf, err: io::Error },
    /// Error parsing DB file
    #[error("Couldn't parse apps database file {file_path}: {err}")]
    ParseDB {
        file_path: PathBuf,
        err: rmp_serde::decode::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_new_entries_identical() {
        let apps = vec![
            App::new(
                "Test1".to_owned(),
                "icon".to_owned(),
                vec!["/bin/test".to_owned()],
                false,
            ),
            App::new(
                "Test2".to_owned(),
                "icon".to_owned(),
                vec!["/bin/test".to_owned()],
                false,
            ),
        ];
        let mut apps_db = AppsDB::new(Config::default(), apps.clone());
        apps_db.merge_new_entries(apps.clone());
        assert_eq!(apps, apps_db.apps);
    }

    #[test]
    fn merge_new_entries_remove() {
        let mut apps = vec![
            App::new(
                "Test1".to_owned(),
                "icon".to_owned(),
                vec!["/bin/test".to_owned()],
                false,
            ),
            App::new(
                "Test2".to_owned(),
                "icon".to_owned(),
                vec!["/bin/test".to_owned()],
                false,
            ),
        ];
        let mut apps_db = AppsDB::new(Config::default(), apps.clone());
        apps.remove(0);
        apps_db.merge_new_entries(apps.clone());
        assert_eq!(apps, apps_db.apps);
    }

    #[test]
    fn merge_new_entries_add() {
        let mut apps = vec![App::new(
            "Test1".to_owned(),
            "icon".to_owned(),
            vec!["/bin/test".to_owned()],
            false,
        )];
        let mut apps_db = AppsDB::new(Config::default(), apps.clone());
        apps.push(App::new(
            "Test2".to_owned(),
            "icon".to_owned(),
            vec!["/bin/test".to_owned()],
            false,
        ));
        apps_db.merge_new_entries(apps.clone());
        assert_eq!(apps, apps_db.apps);
    }
}
