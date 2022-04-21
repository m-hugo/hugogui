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
use crate::desktop_entry::{parse_desktop_file, EntryParseError};
use crate::App;
use std::{env::VarError, path::PathBuf};
use thiserror::Error;
use walkdir::WalkDir;

/// Error from scanning for desktop entries
///
/// These can generally be ignored but you might want to log them.
#[derive(Debug, Error)]
pub enum ScanError {
    /// Failed to scan the directory for some reason (ex. it doesn't exist).
    #[error("Failed to scan directory {dir} for desktop entries: {err}")]
    ScanDirectory { dir: String, err: walkdir::Error },
    /// Paring the entry failed.
    #[error("Parse error: {err}")]
    ParseEntry { err: EntryParseError },
    /// Path expansion failed.
    #[error("Failed to expand path {path}: {err}")]
    PathExpand {
        path: String,
        err: shellexpand::LookupError<VarError>,
    },
}

/// Get a list of desktop entries from a list of directories to search.
pub fn desktop_entires(paths: &[PathBuf]) -> (Vec<PathBuf>, Vec<ScanError>) {
    let mut files = Vec::new();
    let mut errors = Vec::new();
    for path in paths {
        for entry in WalkDir::new(&path) {
            match entry {
                Ok(entry) => {
                    if entry.file_name().to_str().unwrap().contains(".desktop")
                    {
                        files.push(entry.path().to_owned())
                    }
                }
                Err(err) => {
                    errors.push(ScanError::ScanDirectory {
                        dir: path.display().to_string(),
                        err,
                    });
                    continue;
                }
            }
        }
    }
    (files, errors)
}

/// Get a list of apps for a list of paths to search.
pub fn scan_desktop_entries(paths: &[PathBuf]) -> (Vec<App>, Vec<ScanError>) {
    let (entries, mut errors) = desktop_entires(paths);
    let (apps, errs): (Vec<_>, Vec<_>) = entries
        .into_iter()
        .map(|path| {
            parse_desktop_file(&path)
                .map_err(|err| ScanError::ParseEntry { err })
        })
        .partition(Result::is_ok);
    let mut apps: Vec<_> =
        apps.into_iter().map(Result::unwrap).flatten().collect();
    apps.sort_unstable();
    apps.dedup();
    errors.extend(errs.into_iter().map(Result::unwrap_err).collect::<Vec<_>>());
    (apps, errors)
}
