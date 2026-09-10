// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Bounded application-level fleet configuration and request protocol.
use serde::{Deserialize, Serialize};

pub const CONFIG_PATH: &str = "/etc/hyper/vms.json";
pub const MAX_DEFINITIONS: usize = 8;
pub const MAX_MESSAGE_BYTES: usize = 16 * 1024;
pub const MAX_CONFIG_BYTES: u64 = 64 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Definition {
    pub name: String,
    pub image: String,
    #[serde(default)]
    pub autostart: bool,
}
impl Definition {
    pub fn validate(&self) -> Result<(), String> {
        if self.name.is_empty()
            || self.name.len() > 32
            || !self
                .name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
            || !self.name.as_bytes()[0].is_ascii_alphanumeric()
        {
            return Err("VM names must start with a letter or digit and contain at most 32 letters, digits, '-', '_' or '.'".into());
        }
        if !self.image.starts_with('/')
            || self.image.len() > 512
            || self.image.chars().any(char::is_control)
            || self.image.split('/').any(|part| part == "..")
        {
            return Err("VM image must be an absolute path without parent components or control characters (at most 512 bytes)".into());
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub format: String,
    #[serde(rename = "virtual-machines")]
    pub machines: Vec<Definition>,
    #[serde(
        rename = "SPDX-FileCopyrightText",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub copyright: Option<String>,
    #[serde(
        rename = "SPDX-License-Identifier",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub license: Option<String>,
}
impl Config {
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        validate_definitions(&self.machines)?;
        serde_json::to_vec_pretty(self).map_err(|error| error.to_string())
    }

    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() as u64 > MAX_CONFIG_BYTES {
            return Err("VM configuration exceeds 64 KiB".into());
        }
        let config: Self = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
        if config.format != "hyper.vm-config" {
            return Err("expected format 'hyper.vm-config'".into());
        }
        validate_definitions(&config.machines)?;
        Ok(config)
    }
}

pub fn validate_definitions(definitions: &[Definition]) -> Result<(), String> {
    if definitions.len() > MAX_DEFINITIONS {
        return Err(format!(
            "at most {MAX_DEFINITIONS} VM definitions are supported"
        ));
    }
    for (index, definition) in definitions.iter().enumerate() {
        definition.validate()?;
        if definitions[..index]
            .iter()
            .any(|other| other.name == definition.name)
        {
            return Err(format!("duplicate VM name '{}'", definition.name));
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Action {
    Status,
    Start,
    Stop,
    Restart,
    Console,
    Delete,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "command", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Request {
    List,
    Control { name: String, action: Action },
    Create { definitions: Vec<Definition> },
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Stopped,
    Starting,
    Running,
    Stopping,
    Failed,
}
impl State {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Stopped => "stopped",
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Stopping => "stopping",
            Self::Failed => "failed",
        }
    }
}
impl std::fmt::Display for State {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.pad(self.as_str())
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Summary {
    pub name: String,
    pub state: State,
    pub image: String,
    pub autostart: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "result", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Response {
    Entries { machines: Vec<Summary> },
    Accepted,
    Error { message: String },
}

pub fn encode(value: &impl Serialize) -> Result<Vec<u8>, String> {
    let bytes = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    if bytes.len() > MAX_MESSAGE_BYTES {
        return Err("fleet message is too large".into());
    }
    Ok(bytes)
}
pub fn request(bytes: &[u8]) -> Result<Request, String> {
    serde_json::from_slice(bytes).map_err(|error| error.to_string())
}
pub fn response(bytes: &[u8]) -> Result<Response, String> {
    serde_json::from_slice(bytes).map_err(|error| error.to_string())
}

#[cfg(test)]
#[path = "../tests/fleet.rs"]
mod tests;
