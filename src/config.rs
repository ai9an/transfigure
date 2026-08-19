use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

use crate::{APP_NAME, CONFIG_VERSION, Error, Result};

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum ShellKind {
    #[default]
    Auto,
    Powershell,
    Pwsh,
    Sh,
}

impl std::fmt::Display for ShellKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            Self::Auto => "auto",
            Self::Powershell => "powershell",
            Self::Pwsh => "pwsh",
            Self::Sh => "sh",
        };
        formatter.write_str(value)
    }
}

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

#[derive(Debug, PartialEq, Eq)]
pub enum ArgumentTemplate<'a> {
    Literal(Cow<'a, str>),
    Positional(usize),
    All,
}

pub fn parse_argument_template(value: &str) -> Result<ArgumentTemplate<'_>> {
    if value == "{*}" {
        return Ok(ArgumentTemplate::All);
    }
    if value.len() >= 4 && value.starts_with("{{") && value.ends_with("}}") {
        let unescaped = &value[1..value.len() - 1];
        let inner = &unescaped[1..unescaped.len() - 1];
        if unescaped == "{*}"
            || (!inner.is_empty() && inner.chars().all(|character| character.is_ascii_digit()))
        {
            return Ok(ArgumentTemplate::Literal(Cow::Borrowed(unescaped)));
        }
    }
    if let Some(index) = value
        .strip_prefix('{')
        .and_then(|value| value.strip_suffix('}'))
        .filter(|value| {
            !value.is_empty() && value.chars().all(|character| character.is_ascii_digit())
        })
    {
        let index = index.parse::<usize>().map_err(|_| {
            Error::Message(format!(
                "placeholder '{value}' has an invalid argument number"
            ))
        })?;
        if index == 0 {
            return Err(Error::Message(
                "placeholder indexes start at {1}; {0} is invalid".into(),
            ));
        }
        return Ok(ArgumentTemplate::Positional(index - 1));
    }
    Ok(ArgumentTemplate::Literal(Cow::Borrowed(value)))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Shortcut {
    pub commands: Vec<CommandSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shell: Option<ShellKind>,
}

impl Shortcut {
    pub fn parse(tokens: Vec<String>, chain: bool, shell: Option<ShellKind>) -> Result<Self> {
        if tokens.is_empty() {
            return Err(Error::Message("a command definition is required".into()));
        }

        if !chain {
            return Ok(Self {
                commands: vec![command_from_tokens(tokens)?],
                shell,
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
        Ok(Self { commands, shell })
    }

    pub fn summary(&self) -> String {
        let commands = self
            .commands
            .iter()
            .map(CommandSpec::display)
            .collect::<Vec<_>>()
            .join(" -> ");
        match self.shell {
            Some(shell) => format!("[shell:{shell}] {commands}"),
            None => commands,
        }
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
        let mut store: Self =
            serde_json::from_str(&contents).map_err(|source| Error::InvalidConfig {
                path: path.to_owned(),
                source,
            })?;
        if store.version == 1 {
            store.version = CONFIG_VERSION;
        } else if store.version != CONFIG_VERSION {
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
            for command in &shortcut.commands {
                if !matches!(
                    parse_argument_template(&command.program)?,
                    ArgumentTemplate::Literal(_)
                ) {
                    return Err(Error::Message(format!(
                        "shortcut '{name}' uses a placeholder as a program; placeholders are allowed only in arguments"
                    )));
                }
                for argument in &command.args {
                    parse_argument_template(argument)?;
                }
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
    if !matches!(
        parse_argument_template(&program)?,
        ArgumentTemplate::Literal(_)
    ) {
        return Err(Error::Message(
            "placeholders are allowed only in command arguments, not the program name".into(),
        ));
    }
    for argument in &tokens {
        parse_argument_template(argument)?;
    }
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
        let shortcut = Shortcut::parse(
            vec!["tool".into(), "--then".into(), "value".into()],
            false,
            None,
        )
        .unwrap();
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
            None,
        )
        .unwrap();
        assert_eq!(shortcut.commands.len(), 2);
        assert_eq!(shortcut.commands[0].args, ["--then"]);
        assert_eq!(shortcut.commands[1].program, "second");
    }

    #[test]
    fn chain_requires_two_nonempty_steps() {
        assert!(Shortcut::parse(vec!["only".into()], true, None).is_err());
        assert!(Shortcut::parse(vec!["one".into(), "--then".into()], true, None).is_err());
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
                    shell: None,
                },
            )]),
        };
        assert!(store.validate().is_err());
    }

    #[test]
    fn parses_and_escapes_argument_placeholders() {
        assert_eq!(
            parse_argument_template("{1}").unwrap(),
            ArgumentTemplate::Positional(0)
        );
        assert_eq!(
            parse_argument_template("{*}").unwrap(),
            ArgumentTemplate::All
        );
        assert_eq!(
            parse_argument_template("{{1}}").unwrap(),
            ArgumentTemplate::Literal(Cow::Borrowed("{1}"))
        );
        assert!(parse_argument_template("{0}").is_err());
    }

    #[test]
    fn rejects_placeholder_programs() {
        let error = Shortcut::parse(vec!["{1}".into(), "value".into()], false, None)
            .unwrap_err()
            .to_string();
        assert!(error.contains("program"));
    }

    #[test]
    fn migrates_version_one_shortcuts_to_direct_mode() {
        let temp = tempfile::TempDir::new().unwrap();
        let path = temp.path().join("config.json");
        fs::write(
            &path,
            r#"{"version":1,"shortcuts":{"old":{"commands":[{"program":"tool","args":[]}]}}}"#,
        )
        .unwrap();
        let store = Store::load(&path).unwrap();
        assert_eq!(store.version, CONFIG_VERSION);
        assert_eq!(store.shortcuts["old"].shell, None);
    }
}
