// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Serialize};

/// A reusable declaration; identity belongs to its instantiated path, not file.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Component {
    #[serde(
        default,
        rename = "SPDX-FileCopyrightText",
        skip_serializing_if = "Option::is_none"
    )]
    pub copyright: Option<String>,
    #[serde(
        default,
        rename = "SPDX-License-Identifier",
        skip_serializing_if = "Option::is_none"
    )]
    pub license: Option<String>,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub program: Option<Program>,
    #[serde(default)]
    pub children: Vec<Child>,
    #[serde(default, rename = "use")]
    pub uses: Vec<Use>,
    #[serde(default)]
    pub capabilities: Vec<Capability>,
    #[serde(default)]
    pub expose: Vec<Expose>,
    #[serde(default)]
    pub offer: Vec<Offer>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Program {
    pub image: String,
    #[serde(default)]
    pub arguments: Vec<String>,
    #[serde(default)]
    pub startup: Startup,
    #[serde(default)]
    pub restart: Restart,
    #[serde(default = "timeout")]
    pub timeout_ms: u64,
    #[serde(default = "restart_delay")]
    pub restart_delay_ms: u64,
    #[serde(default = "restart_limit")]
    pub restart_limit: u32,
    #[serde(default)]
    pub resident: bool,
    /// Report termination and delegated worker failures directly to Console,
    /// independently of journal and terminal availability. This does not grant
    /// restart authority or turn a component failure into a manager failure.
    #[serde(default)]
    pub critical: bool,
    /// A trusted startup coordinator must install this instance's construction
    /// transaction. The manager never reconstructs consumed migration grants.
    #[serde(default)]
    pub prepared: bool,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Startup {
    #[default]
    Lazy,
    Eager,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Restart {
    #[default]
    Never,
    OnFailure,
}

const fn timeout() -> u64 {
    60_000
}
const fn restart_delay() -> u64 {
    1000
}
const fn restart_limit() -> u32 {
    3
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Child {
    pub name: String,
    pub component: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum CapabilityType {
    Protocol { protocol: String },
    Object { object_type: u32, rights: u64 },
}

impl CapabilityType {
    pub fn permits(&self, requested: &Self) -> bool {
        match (self, requested) {
            (Self::Protocol { protocol: a }, Self::Protocol { protocol: b }) => a == b,
            (
                Self::Object {
                    object_type: a,
                    rights,
                },
                Self::Object {
                    object_type: b,
                    rights: wanted,
                },
            ) => a == b && wanted & !rights == 0,
            _ => false,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Transfer {
    #[default]
    Duplicate,
    Move,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Capability {
    pub name: String,
    pub kind: CapabilityType,
    #[serde(default)]
    pub transfer: Transfer,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Use {
    pub name: String,
    pub kind: CapabilityType,
    /// Object capabilities are installed under this Native startup purpose.
    /// Protocol capabilities are connected by local name at runtime instead.
    #[serde(default)]
    pub purpose: Option<u32>,
}

/// Sources name only self, parent, or one immediate child (#name). A path is
/// never authority and cannot bypass an intermediate realm's expose/offer.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Expose {
    pub name: String,
    pub from: String,
    pub source: String,
    pub kind: CapabilityType,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Offer {
    pub name: String,
    pub to: String,
    pub from: String,
    pub source: String,
    pub kind: CapabilityType,
}
