// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Bounded application-level fleet configuration and request protocol.
pub use crate::affinity::Affinity;
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
    pub configuration: Configuration,
    #[serde(default)]
    pub autostart: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disk: Option<Disk>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<Network>,
}
/// Explicit per-instance policy, stored in JSON rather than the image bundle.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Configuration {
    #[serde(rename = "memory-bytes")]
    pub memory_bytes: u64,
    pub vcpus: u32,
    pub bootargs: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub affinity: Vec<Affinity>,
}
impl Configuration {
    pub fn image_configuration(&self) -> Result<hyper_vm_image::Configuration, String> {
        use hyper_vm_image::BootArguments;
        // Architecture-specific limits are checked after reading the ITB.
        if !(1..=8).contains(&self.vcpus) {
            return Err("VM requires 1..=8 vCPUs".into());
        }
        crate::affinity::masks(&self.affinity, self.vcpus)?;
        if self.memory_bytes < 64 * 1024 * 1024 || !self.memory_bytes.is_power_of_two() {
            return Err("VM memory-bytes must be a power of two of at least 64 MiB".into());
        }
        let boot_arguments = BootArguments::new(&self.bootargs)
            .map_err(|_| "VM bootargs must contain at most 2048 bytes and no NUL".to_string())?;
        Ok(hyper_vm_image::Configuration {
            memory_size: self.memory_bytes,
            vcpu_count: self.vcpus,
            boot_arguments,
        })
    }

    /// Raw arguments avoid JSON escaping expanding the bounded kernel cmdline.
    pub fn runtime_arguments(&self) -> Result<Vec<String>, String> {
        self.image_configuration()?;
        let mut args = vec![
            self.memory_bytes.to_string(),
            self.vcpus.to_string(),
            self.bootargs.clone(),
        ];
        // One bounded argument per vCPU stays below the 4096-byte argument limit,
        // even when every supported host CPU is listed explicitly.
        for entry in &self.affinity {
            args.push(serde_json::to_string(entry).map_err(|error| error.to_string())?);
        }
        Ok(args)
    }

    pub fn from_runtime_arguments(args: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut args = args.into_iter();
        let mut next = || {
            args.next()
                .ok_or_else(|| "missing VM configuration argument".to_string())
        };
        let config = Self {
            memory_bytes: next()?.parse().map_err(|_| "invalid VM memory")?,
            vcpus: next()?.parse().map_err(|_| "invalid VM CPU count")?,
            bootargs: next()?,
            affinity: args
                .map(|value| serde_json::from_str(&value))
                .collect::<Result<_, _>>()
                .map_err(|error| error.to_string())?,
        };
        config.image_configuration()?;
        Ok(config)
    }
}

/// One exclusive volume authorized by the board's I/O client table.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Disk {
    pub client: u32,
    pub volume: String,
}

/// A board-authorized network endpoint; client zero is reserved for Native.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Network {
    pub client: u32,
    pub network: String,
    pub mac: String,
}

impl Network {
    pub fn mac_bytes(&self) -> Result<[u8; 6], String> {
        hyper_service::io::parse_mac(&self.mac).ok_or_else(|| {
            "network MAC must be a canonical locally administered unicast address".into()
        })
    }

    pub fn validate(&self) -> Result<(), String> {
        if !(1..=127).contains(&self.client) || !hyper_service::io::valid_name(&self.network) {
            return Err("network requires a guest client and a valid network name".into());
        }
        self.mac_bytes()?;
        Ok(())
    }
}
impl Disk {
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=127).contains(&self.client) || !hyper_service::io::valid_volume(&self.volume) {
            return Err("disk requires a client index from 1 to 127 and a volume name of 1 to 32 letters, digits, '-' or '_'".into());
        }
        Ok(())
    }
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
        self.configuration.image_configuration()?;
        if let Some(disk) = &self.disk {
            disk.validate()?;
        }
        if let Some(network) = &self.network {
            network.validate()?;
            if self
                .disk
                .as_ref()
                .is_some_and(|disk| disk.client != network.client)
            {
                return Err("disk and network must share one I/O client".into());
            }
        }
        Ok(())
    }

    pub fn io_connection(&self) -> Result<Option<hyper_service::io::Connection<'_>>, String> {
        self.validate()?;
        let client = self
            .disk
            .as_ref()
            .map(|disk| disk.client)
            .or_else(|| self.network.as_ref().map(|network| network.client));
        client
            .map(|client| {
                Ok(hyper_service::io::Connection {
                    client,
                    volume: self.disk.as_ref().map(|disk| disk.volume.as_str()),
                    network: self
                        .network
                        .as_ref()
                        .map(|network| network.network.as_str()),
                    mac: self
                        .network
                        .as_ref()
                        .map(Network::mac_bytes)
                        .transpose()?
                        .unwrap_or([0; 6]),
                })
            })
            .transpose()
    }

    /// Reject authority collisions both within a fleet and during later creates.
    pub fn check_conflicts(&self, other: &Self) -> Result<(), String> {
        if self.name == other.name {
            return Err(format!("duplicate VM name '{}'", self.name));
        }
        if let (Some(connection), Some(previous)) = (self.io_connection()?, other.io_connection()?)
            && connection.client == previous.client
        {
            return Err(format!(
                "I/O client {} is already assigned",
                connection.client
            ));
        }
        if let (Some(disk), Some(previous)) = (&self.disk, &other.disk)
            && disk.volume == previous.volume
        {
            return Err(format!("duplicate exclusive disk volume '{}'", disk.volume));
        }
        if let (Some(network), Some(previous)) = (&self.network, &other.network)
            && network.mac == previous.mac
        {
            return Err(format!("network MAC '{}' is already assigned", network.mac));
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
        for other in &definitions[..index] {
            definition.check_conflicts(other)?;
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
    Affinity {
        name: String,
        vcpu: u32,
        affinity_words: Vec<u64>,
    },
    Control {
        name: String,
        action: Action,
    },
    Create {
        definitions: Vec<Definition>,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Unavailable,
    Stopped,
    Starting,
    Running,
    Stopping,
    Failed,
}
impl State {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unavailable => "unavailable",
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub placement: Vec<VcpuPlacement>,
    #[serde(default)]
    pub read_only: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vcpus: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resident_memory_bytes: Option<u64>,
    pub name: String,
    pub state: State,
    pub image: String,
    pub autostart: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disk: Option<Disk>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<Network>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct VcpuPlacement {
    pub vcpu: u32,
    pub host_cpu: Option<u32>,
    pub pending_host_cpu: Option<u32>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "result", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Response {
    Entries { machines: Vec<Summary> },
    Accepted,
    AffinityAccepted { vcpu: u32 },
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
