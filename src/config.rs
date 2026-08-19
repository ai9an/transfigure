use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

use crate::{APP_NAME, CONFIG_VERSION, Error, Result};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandSpec {
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
}

impl CommandSpec {
    pub fn display(&self) -> String {
        std::iter::once(&self.program)
            .chain(self.args.iter())
            .map(|value| quote(value))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Shortcut {
    pub commands: Vec<CommandSpec>,
}

impl Shortcut {
    pub fn parse(tokens: Vec<String>, chain: bool) -> Result<Self> {
        if tokens.is_empty() {
            return Err(Error::Message("a command definition is required".into()));
        }

        if !chain {
            return Ok(Self {
                commands: vec![command_from_tokens(tokens)?],
            });
        }

        let mut commands = Vec::new();
        let mut current = Vec::new();
        let mut iter = tokens.into_iter();
        while let Some(token) = iter.next() {
            match token.as_str() {
                "--then" => {
                    if current.is_empty() {
                        return Err(Error::Message(
                            "each chain step must contain a program".into(),
                        ));
                    }
                    commands.push(command_from_tokens(std::mem::take(&mut current))?);
                }
                "--literal" => {
                    let literal = iter.next().ok_or_else(|| {
                        Error::Message("`--literal` must be followed by a value".into())
                    })?;
                    current.push(literal);
                }
                _ => current.push(token),
            }
        }
        if current.is_empty() {
            return Err(Error::Message(
                "a chain cannot end with `--then` or an empty step".into(),
            ));
        }
        commands.push(command_from_tokens(current)?);
        if commands.len() < 2 {
            return Err(Error::Message(
                "`--chain` requires at least two commands separated by `--then`".into(),
            ));
        }
        Ok(Self { commands })
    }

    pub fn summary(&self) -> String {
        self.commands
            .iter()
            .map(CommandSpec::display)
            .collect::<Vec<_>>()
            .join(" -> ")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Store {
    pub version: u32,
    #[serde(default)]
    pub shortcuts: BTreeMap<String, Shortcut>,
}

impl Default for Store {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            shortcuts: BTreeMap::new(),
        }
    }
}

impl Store {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = fs::read_to_string(path).map_err(|source| Error::Read {
            path: path.to_owned(),
            source,
        })?;
        let store: Self =
            serde_json::from_str(&contents).map_err(|source| Error::InvalidConfig {
                path: path.to_owned(),
                source,
            })?;
        if store.version != CONFIG_VERSION {
            return Err(Error::Message(format!(
                "unsupported configuration version {} in {}; expected version {}",
                store.version,
                path.display(),
                CONFIG_VERSION
            )));
        }
        store.validate()?;
        Ok(store)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let parent = path.parent().ok_or_else(|| {
            Error::Message(format!(
                "configuration path {} has no parent",
                path.display()
            ))
        })?;
        fs::create_dir_all(parent).map_err(|source| Error::Write {
            path: parent.to_owned(),
            source,
        })?;
        let mut temporary = NamedTempFile::new_in(parent).map_err(|source| Error::Write {
            path: parent.to_owned(),
            source,
        })?;
        serde_json::to_writer_pretty(&mut temporary, self).map_err(|source| {
            Error::Message(format!("could not serialize configuration: {source}"))
        })?;
        use std::io::Write;
        temporary.write_all(b"\n").map_err(|source| Error::Write {
            path: path.to_owned(),
            source,
        })?;
        temporary.persist(path).map_err(|error| Error::Write {
            path: path.to_owned(),
            source: error.error,
        })?;
        Ok(())
    }

    pub fn find_name(&self, requested: &str) -> Option<&str> {
        self.shortcuts
            .keys()
            .find(|name| name.eq_ignore_ascii_case(requested))
            .map(String::as_str)
    }

    pub fn find(&self, requested: &str) -> Option<(&str, &Shortcut)> {
        self.shortcuts
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(requested))
            .map(|(name, shortcut)| (name.as_str(), shortcut))
    }

    fn validate(&self) -> Result<()> {
        let mut normalized_names = std::collections::HashSet::new();
        for (name, shortcut) in &self.shortcuts {
            validate_name(name)?;
            if !normalized_names.insert(name.to_ascii_lowercase()) {
                return Err(Error::Message(format!(
                    "configuration contains duplicate shortcut name '{name}'"
                )));
            }
            if shortcut.commands.is_empty()
                || shortcut
                    .commands
                    .iter()
                    .any(|command| command.program.is_empty())
            {
                return Err(Error::Message(format!(
                    "shortcut '{name}' has an invalid empty command"
                )));
            }
        }
        Ok(())
    }
}

pub fn validate_name(name: &str) -> Result<()> {
    let valid_length = (1..=64).contains(&name.len());
    let mut chars = name.chars();
    let valid_first = chars
        .next()
        .is_some_and(|character| character.is_ascii_alphanumeric());
    let valid_rest = chars
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.'));
    if !valid_length || !valid_first || !valid_rest || name.ends_with('.') {
        return Err(Error::Message(format!(
            "invalid shortcut name '{name}'; use 1-64 ASCII letters, numbers, '.', '-', or '_', starting with a letter or number"
        )));
    }

    let stem = name.split('.').next().unwrap_or(name);
    let uppercase = stem.to_ascii_uppercase();
    let windows_reserved = matches!(uppercase.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (uppercase.len() == 4
            && (uppercase.starts_with("COM") || uppercase.starts_with("LPT"))
            && uppercase.as_bytes()[3].is_ascii_digit()
            && uppercase.as_bytes()[3] != b'0');
    if name.eq_ignore_ascii_case(APP_NAME) || windows_reserved {
        return Err(Error::Message(format!(
            "shortcut name '{name}' is reserved"
        )));
    }
    Ok(())
}

fn command_from_tokens(mut tokens: Vec<String>) -> Result<CommandSpec> {
    if tokens.is_empty() || tokens[0].is_empty() {
        return Err(Error::Message("each command needs a program".into()));
    }
    let program = tokens.remove(0);
    Ok(CommandSpec {
        program,
        args: tokens,
    })
}

fn quote(value: &str) -> String {
    if !value.is_empty()
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "-_./:=+@".contains(character))
    {
        value.to_owned()
    } else {
        format!("{:?}", value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_single_command_without_treating_then_as_control() {
        let shortcut =
            Shortcut::parse(vec!["tool".into(), "--then".into(), "value".into()], false).unwrap();
        assert_eq!(shortcut.commands[0].args, ["--then", "value"]);
    }

    #[test]
    fn parses_chain_and_literal_control_token() {
        let shortcut = Shortcut::parse(
            vec![
                "first".into(),
                "--literal".into(),
                "--then".into(),
                "--then".into(),
                "second".into(),
            ],
            true,
        )
        .unwrap();
        assert_eq!(shortcut.commands.len(), 2);
        assert_eq!(shortcut.commands[0].args, ["--then"]);
        assert_eq!(shortcut.commands[1].program, "second");
    }

    #[test]
    fn chain_requires_two_nonempty_steps() {
        assert!(Shortcut::parse(vec!["only".into()], true).is_err());
        assert!(Shortcut::parse(vec!["one".into(), "--then".into()], true).is_err());
    }

    #[test]
    fn validates_portable_names() {
        for valid in ["download", "yt-dlp", "build_2", "tool.local"] {
            assert!(validate_name(valid).is_ok(), "{valid}");
        }
        for invalid in ["", "-bad", "two words", "transfigure", "CON", "LPT1"] {
            assert!(validate_name(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn rejects_invalid_loaded_shortcuts() {
        let store = Store {
            version: CONFIG_VERSION,
            shortcuts: BTreeMap::from([(
                "../escape".into(),
                Shortcut {
                    commands: vec![CommandSpec {
                        program: "tool".into(),
                        args: vec![],
                    }],
                },
            )]),
        };
        assert!(store.validate().is_err());
    }
}
