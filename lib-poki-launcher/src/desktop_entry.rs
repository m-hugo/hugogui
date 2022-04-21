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
use super::App;
use freedesktop_entry_parser as fdep;
use std::path::{Path, PathBuf};
use std::str::ParseBoolError;
use thiserror::Error;

/// Error from paring a desktop entry
#[derive(Debug, Error)]
pub enum EntryParseError {
    /// Desktop file is missing the 'Desktop Entry' section.
    #[error("Desktop file {file_path} is missing 'Desktop Entry' section")]
    MissingSection { file_path: PathBuf },
    /// Desktop file is missing the 'Name' parameter.
    #[error("Desktop file {file_path} is missing the 'Name' parameter")]
    MissingName { file_path: PathBuf },
    /// Desktop file is missing the 'Exec' parameter.
    #[error("Desktop file {file_path} is missing the 'Exec' parameter")]
    MissingExec { file_path: PathBuf },
    /// Desktop file is missing the 'Icon' parameter.
    #[error("Desktop file {file_path} is missing the 'Icon' parameter")]
    MissingIcon { file_path: PathBuf },
    /// Failed to parse desktop file.
    #[error("Failed to parse desktop file {file_path}: {err}")]
    InvalidDesktopFile {
        file_path: PathBuf,
        err: std::io::Error,
    },
    /// A property had an invalid value.
    /// This is returned if NoDisplay or Hidden are set to a value that isn't
    /// `true` or `false`.
    #[error(
        "In entry {file_path} property {name} has an invalid value {value}"
    )]
    InvalidPropVal {
        file_path: PathBuf,
        name: String,
        value: String,
    },
    /// A property had a value with an invalid escape sequence.
    #[error(
        "In entry {file_path} property {name} has an invalid sequence \\{value}"
    )]
    InvalidEscape {
        file_path: PathBuf,
        name: String,
        value: char,
    },
}

fn prop_is_true(item: Option<&str>) -> Result<bool, ParseBoolError> {
    match item {
        Some(text) => Ok(text.parse()?),
        None => Ok(false),
    }
}

fn unescape_string(s: &str) -> String {
    let mut unescaped = String::with_capacity(s.len());
    let mut iter = s.chars();
    while let Some(c) = iter.next() {
        if c == '\\' {
            match iter.next() {
                Some('\\') => unescaped.push('\\'),
                Some('n') => unescaped.push('\n'),
                Some('s') => unescaped.push(' '),
                Some('t') => unescaped.push('\t'),
                Some('r') => unescaped.push('\r'),
                Some(other) => {
                    unescaped.push('\\');
                    unescaped.push(other);
                }
                None => {}
            }
        } else {
            unescaped.push(c);
        }
    }
    unescaped
}

fn parse_exec(s: &str, name: &str, icon: &str, file_path: &str) -> Vec<String> {
    let mut output = Vec::new();
    let mut iter = s.chars();
    let mut in_quote = false;
    let mut part = String::new();
    fn push(v: &mut Vec<String>, item: String) {
        if !item.is_empty() {
            v.push(item);
        }
    }
    while let Some(c) = iter.next() {
        if in_quote {
            if c == '"' {
                push(&mut output, part);
                part = String::new();
                in_quote = false;
            } else if c == '\\' {
                match iter.next() {
                    Some('"') => part.push('"'),
                    Some('`') => part.push('`'),
                    Some('$') => part.push('$'),
                    Some('\\') => part.push('\\'),
                    Some(other) => {
                        part.push('\\');
                        part.push(other);
                    }
                    None => {}
                }
            } else {
                part.push(c);
            }
        } else if c == '"' {
            in_quote = true;
        } else if c == ' ' {
            push(&mut output, part);
            part = String::new();
        } else if c == '%' {
            match iter.next() {
                Some('%') => part.push('%'),
                Some('i') => {
                    if !icon.is_empty() {
                        push(&mut output, part);
                        output.push("--icon".to_owned());
                        output.push(icon.to_owned());
                        part = String::new();
                    }
                }
                Some('c') => {
                    push(&mut output, part);
                    output.push(name.to_owned());
                    part = String::new();
                }
                Some('k') => {
                    push(&mut output, part);
                    output.push(file_path.to_owned());
                    part = String::new();
                }
                Some(_) | None => {}
            }
        } else {
            part.push(c);
        }
    }
    push(&mut output, part);
    output
}

/// Parse a desktop entry
///
/// # Arguments
///
/// * `path` - Path to the desktop entry
///
/// # Return
///
/// Returns `Ok(None)` if the app should not be listed.
///
/// # Example
///
/// Parse a list of desktop entries, separating successes from failures,
/// then removing apps that shouldn't be displayed (None) from the successes.
/// ```ignore
/// use std::path::Path;
///
/// let entries = vec![Path::new("./firefox.desktop"), Path::new("./chrome.desktop")];
/// let (apps, errors): (Vec<_>, Vec<_>) = entries
///     .into_iter()
///     .map(|path| parse_desktop_file(&path))
///     .partition(Result::is_ok);
/// let mut apps: Vec<_> = apps
///     .into_iter()
///     .map(Result::unwrap)
///     .filter_map(|x| x)
///     .collect();
/// ```
pub fn parse_desktop_file(
    path: impl AsRef<Path>,
) -> Result<Option<App>, EntryParseError> {
    let path = path.as_ref();
    let file = fdep::parse_entry(&path).map_err(|err| {
        EntryParseError::InvalidDesktopFile {
            file_path: path.to_owned(),
            err,
        }
    })?;
    if !file.has_section("Desktop Entry") {
        return Err(EntryParseError::MissingSection {
            file_path: path.to_owned(),
        });
    }
    let section = file.section("Desktop Entry");
    let not_display =
        prop_is_true(section.attr("NoDisplay")).map_err(|_| {
            EntryParseError::InvalidPropVal {
                file_path: path.to_owned(),
                name: "NoDisplay".into(),
                value: section.attr("NoDisplay").unwrap().to_owned(),
            }
        })?;
    let hidden = prop_is_true(section.attr("Hidden")).map_err(|_| {
        EntryParseError::InvalidPropVal {
            file_path: path.to_owned(),
            name: "Hidden".into(),
            value: section.attr("Hidden").unwrap().to_owned(),
        }
    })?;
    if not_display || hidden {
        return Ok(None);
    }
    let name = unescape_string(section.attr("Name").ok_or(
        EntryParseError::MissingName {
            file_path: path.to_owned(),
        },
    )?);
    let icon = section.attr("Icon").unwrap_or("");
    let exec_str =
        section.attr("Exec").ok_or(EntryParseError::MissingExec {
            file_path: path.to_owned(),
        })?;
    let exec =
        parse_exec(exec_str, &name, icon, path.to_string_lossy().as_ref());
    let terminal = {
        if let Some(value) = section.attr("Terminal") {
            value.parse().map_err(|_| EntryParseError::InvalidPropVal {
                file_path: path.to_owned(),
                name: "Terminal".into(),
                value: section.attr("Terminal").unwrap().to_owned(),
            })?
        } else {
            false
        }
    };
    Ok(Some(App::new(
        name,
        icon.to_owned(),
        exec,
        terminal,
    )))
}

#[cfg(test)]
mod test {
    use super::*;

    fn ovec(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_owned()).collect()
    }

    mod parse_exec {
        use super::*;

        #[test]
        fn basic() {
            let exec = "/usr/bin/cat --flag";
            let expected = ovec(&["/usr/bin/cat", "--flag"]);
            assert_eq!(
                parse_exec(
                    &unescape_string(exec),
                    "cat",
                    "cat",
                    "/cat.desktop"
                ),
                expected
            );
        }

        #[test]
        fn quoted() {
            let exec = "\"/usr/bin/cat\" --flag";
            let expected = ovec(&["/usr/bin/cat", "--flag"]);
            assert_eq!(
                parse_exec(
                    &unescape_string(exec),
                    "cat",
                    "cat",
                    "/cat.desktop"
                ),
                expected
            );
        }

        #[test]
        fn args() {
            let exec = "\"/usr/bin/cat\" --flag %k %i %c %f %%";
            let expected = ovec(&[
                "/usr/bin/cat",
                "--flag",
                "/cat.desktop",
                "--icon",
                "cat",
                "cat",
                "%",
            ]);
            assert_eq!(
                parse_exec(
                    &unescape_string(exec),
                    "cat",
                    "cat",
                    "/cat.desktop"
                ),
                expected
            );
        }

        #[test]
        fn complex() {
            let exec = r#""/usr/bin folder/cat" --flag "a very weird \\\\ \" string \\$ <>`" "#;

            let first_pass = r#""/usr/bin folder/cat" --flag "a very weird \\ \" string \$ <>`" "#;
            assert_eq!(unescape_string(exec), first_pass);
            let exec = unescape_string(exec);
            let expected = ovec(&[
                "/usr/bin folder/cat",
                "--flag",
                r#"a very weird \ " string $ <>`"#,
            ]);
            assert_eq!(
                parse_exec(&exec, "cat", "cat", "/cat icon.png"),
                expected
            );
        }
    }

    mod parse_desktop_file {
        use crate::App;
        use std::env::temp_dir;
        use std::fs::{remove_file, File};
        use std::io::prelude::*;

        use super::*;

        #[test]
        fn vaild_file_exist() {
            let path = temp_dir().join("./test.desktop");
            let mut file = File::create(&path).unwrap();
            file.write_all(
                b"[Desktop Entry]
 Name=Test
 Icon=testicon
 Exec=/usr/bin/test --with-flag %f",
            )
            .unwrap();
            let app = parse_desktop_file(&path).unwrap().unwrap();
            let other_app = App::new(
                "Test".to_owned(),
                "testicon".to_owned(),
                ovec(&["/usr/bin/test", "--with-flag"]),
                false,
            );
            // Note, apps will have different uuids but Eq doesn't consider them
            assert_eq!(app, other_app);
            remove_file(&path).unwrap();
        }

        #[test]
        fn file_with_args() {
            let path = temp_dir().join("./test2.desktop");
            let mut file = File::create(&path).unwrap();
            file.write_all(
                b"[Desktop Entry]
 Name=Test
 Icon=testicon
 Exec=/usr/bin/test --with-flag %c %i %k %f",
            )
            .unwrap();
            let app = parse_desktop_file(&path).unwrap().unwrap();
            let other_app = App::new(
                "Test".to_owned(),
                "testicon".to_owned(),
                ovec(&[
                    "/usr/bin/test",
                    "--with-flag",
                    "Test",
                    "--icon",
                    "testicon",
                    path.to_string_lossy().as_ref(),
                ]),
                false,
            );
            // Note, apps will have different uuids but Eq doesn't consider them
            assert_eq!(app, other_app);
            remove_file(&path).unwrap();
        }
    }
}
