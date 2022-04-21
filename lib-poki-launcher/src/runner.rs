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
use crate::config::Config;
use log::debug;
use nix::unistd::{getpid, setpgid};
use std::env::VarError;
use std::io::{self, ErrorKind};
use std::os::unix::process::CommandExt as _;
use std::process::{Command, Stdio};
use thiserror::Error;

use super::App;

/// Error from running the app
#[derive(Debug, Error)]
pub enum RunError {
    #[error("Execution failed with Exec line {exec}: {err}")]
    Exec {
        /// The exec string from the app
        exec: String,
        /// The error to propagate.
        err: io::Error,
    },
    #[error("Error getting value of $TERM: {0}")]
    TermVar(VarError),
    #[error("Could not determine what terminal program to use to launch this app, please set `term_cmd` in the config file")]
    CantFindTerm,
}

impl App {
    /// Run the app.
    pub fn run(&self, config: &Config) -> Result<(), RunError> {
        debug!("Exec: `{:?}`", self.exec);
        let mut command = if self.terminal {
            if let Some(term) = &config.term_cmd {
                let args: Vec<_> = term.split(' ').collect();
                let mut cmd = Command::new(args[0]);
                cmd.args(&args[1..]);
                cmd.args(&self.exec);
                cmd
            } else {
                let term = std::env::var("TERM").map_err(RunError::TermVar)?;
                let mut cmd = Command::new(term);
                cmd.arg("-e");
                cmd.args(&self.exec);
                cmd
            }
        } else {
            let mut cmd = Command::new(&self.exec[0]);
            cmd.args(&self.exec[1..]);
            cmd
        };
        debug!("Running command: `{:?}`", command);
        command
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        unsafe {
            command.pre_exec(|| {
                let pid = getpid();
                // TODO Hanle error here
                setpgid(pid, pid).expect("Failed to set pgid");
                Ok(())
            });
        }
        match command.spawn() {
            Ok(_) => Ok(()),
            Err(e) if config.term_cmd.is_none() && e.kind() == ErrorKind::NotFound => Err(RunError::CantFindTerm),
            Err(e) => Err(RunError::Exec {
                exec: format!("`{:?}`", command),
                err: e,
            })
        }
    }
}
