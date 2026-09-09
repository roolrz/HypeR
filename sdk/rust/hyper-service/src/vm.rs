// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Startup and control-plane contracts for Native VM services.

use hyper_os::handle::{
    ByteChannelObject, CapabilityChannelObject, FileObject, Rights,
    VirtualMachineCreationLeaseObject,
};
use hyper_os::startup::StartupPurpose;

use crate::StartupContract;

pub const CREATION_LEASE_NAME: &str = "vm.creation-lease";
pub const CREATION_AUTHORITY_NAME: &str = "vm.creation-authority";
pub const IMAGE_NAME: &str = "vm.image";
pub const RUNTIME_IMAGE_NAME: &str = "vm.runtime-image";
pub const PROVISIONING_NAME: &str = "vm.provisioning";
pub const INSTANCE_CONTROL_NAME: &str = "vm.instance-control";
pub const CONSOLE_CONNECTION_NAME: &str = "vm.console-connection";
pub const MANAGER_CONNECTION_NAME: &str = "vm.manager-connection";
pub const CLIENT_CONTROL_NAME: &str = "vm.client-control";
pub const CLIENT_CAPABILITIES_NAME: &str = "vm.client-capabilities";

pub const CREATION_LEASE: StartupPurpose<VirtualMachineCreationLeaseObject> =
    StartupPurpose::new(0x8005_0001);
pub const IMAGE: StartupPurpose<FileObject> = StartupPurpose::new(0x8005_0002);
pub const RUNTIME_IMAGE: StartupPurpose<FileObject> = StartupPurpose::new(0x8005_0003);
pub const PROVISIONING: StartupPurpose<CapabilityChannelObject> = StartupPurpose::new(0x8005_0004);
pub const INSTANCE_CONTROL: StartupPurpose<ByteChannelObject> = StartupPurpose::new(0x8005_0005);
pub const CONSOLE_CONNECTION: StartupPurpose<CapabilityChannelObject> =
    StartupPurpose::new(0x8005_0006);
pub const MANAGER_CONNECTION: StartupPurpose<CapabilityChannelObject> =
    StartupPurpose::new(0x8005_0007);
pub const CLIENT_CONTROL: StartupPurpose<ByteChannelObject> = StartupPurpose::new(0x8005_0008);
pub const CLIENT_CAPABILITIES: StartupPurpose<CapabilityChannelObject> =
    StartupPurpose::new(0x8005_0009);

/// Rights retained while the manager forwards the guest image to a runtime.
pub const PROVISIONED_IMAGE_RIGHTS: Rights = Rights::READ
    .union(Rights::DUPLICATE)
    .union(Rights::TRANSFER);
/// Rights retained across the init-to-manager control-endpoint hop.
///
/// `TRANSFER` is transport authority only. The manager attenuates it away when
/// it installs the endpoint into the runtime process.
pub const PROVISIONED_INSTANCE_CONTROL_RIGHTS: Rights = Rights::WAIT
    .union(Rights::READ)
    .union(Rights::WRITE)
    .union(Rights::TRANSFER);
/// Rights visible to one VM runtime for its instance-control endpoint.
pub const INSTANCE_CONTROL_RIGHTS: Rights = Rights::WAIT.union(Rights::READ).union(Rights::WRITE);
pub const RUNTIME_CONSOLE_CONNECTION_RIGHTS: Rights = Rights::WAIT.union(Rights::READ);
pub const CONSOLE_SESSION_RIGHTS: Rights = Rights::WAIT
    .union(Rights::READ)
    .union(Rights::WRITE)
    .union(Rights::TRANSFER);
pub const MANAGER_CONNECTION_RIGHTS: Rights = Rights::DUPLICATE
    .union(Rights::TRANSFER)
    .union(Rights::WAIT)
    .union(Rights::READ)
    .union(Rights::WRITE);

pub const MANAGER_RUNTIME_IMAGE_CONTRACT: StartupContract =
    StartupContract::exact(RUNTIME_IMAGE_NAME, RUNTIME_IMAGE, Rights::EXECUTE);
pub const MANAGER_PROVISIONING_CONTRACT: StartupContract = StartupContract::exact(
    PROVISIONING_NAME,
    PROVISIONING,
    Rights::WAIT.union(Rights::READ),
);
pub const MANAGER_CREATION_AUTHORITY_CONTRACT: StartupContract = StartupContract::exact(
    CREATION_AUTHORITY_NAME,
    hyper_os::startup::VIRTUAL_MACHINE_CREATION_AUTHORITY,
    Rights::DERIVE.union(Rights::CREATE_VIRTUAL_MACHINE),
);
pub const MANAGER_CONNECTION_CONTRACT: StartupContract = StartupContract::exact(
    MANAGER_CONNECTION_NAME,
    MANAGER_CONNECTION,
    Rights::WAIT.union(Rights::READ),
);

pub const MANAGER_STARTUP_CONTRACTS: &[StartupContract] = &[
    MANAGER_RUNTIME_IMAGE_CONTRACT,
    MANAGER_PROVISIONING_CONTRACT,
    MANAGER_CREATION_AUTHORITY_CONTRACT,
    MANAGER_CONNECTION_CONTRACT,
];

pub const RUNTIME_IMAGE_CONTRACT: StartupContract =
    StartupContract::exact(IMAGE_NAME, IMAGE, Rights::READ);
pub const RUNTIME_CREATION_LEASE_CONTRACT: StartupContract = StartupContract::exact(
    CREATION_LEASE_NAME,
    CREATION_LEASE,
    Rights::CREATE_VIRTUAL_MACHINE,
);
pub const RUNTIME_INSTANCE_CONTROL_CONTRACT: StartupContract = StartupContract::exact(
    INSTANCE_CONTROL_NAME,
    INSTANCE_CONTROL,
    INSTANCE_CONTROL_RIGHTS,
);
pub const RUNTIME_CONSOLE_CONNECTION_CONTRACT: StartupContract = StartupContract::exact(
    CONSOLE_CONNECTION_NAME,
    CONSOLE_CONNECTION,
    RUNTIME_CONSOLE_CONNECTION_RIGHTS,
);

/// Complete startup vocabulary accepted by one per-VM runtime.
///
/// The connector receives authorized client byte channels from the manager.
pub const RUNTIME_STARTUP_CONTRACTS: &[StartupContract] = &[
    RUNTIME_IMAGE_CONTRACT,
    RUNTIME_CREATION_LEASE_CONTRACT,
    RUNTIME_INSTANCE_CONTROL_CONTRACT,
    RUNTIME_CONSOLE_CONNECTION_CONTRACT,
];

pub const CLIENT_CONTROL_CONTRACT: StartupContract = StartupContract::exact(
    CLIENT_CONTROL_NAME,
    CLIENT_CONTROL,
    Rights::WAIT.union(Rights::READ).union(Rights::WRITE),
);
pub const CLIENT_CAPABILITIES_CONTRACT: StartupContract = StartupContract::exact(
    CLIENT_CAPABILITIES_NAME,
    CLIENT_CAPABILITIES,
    Rights::WAIT.union(Rights::READ),
);
pub const CLIENT_STARTUP_CONTRACTS: &[StartupContract] = &[StartupContract::exact(
    MANAGER_CONNECTION_NAME,
    MANAGER_CONNECTION,
    Rights::WAIT.union(Rights::WRITE),
)];
pub const VMM_STARTUP_CONTRACTS: &[StartupContract] =
    &[CLIENT_CONTROL_CONTRACT, CLIENT_CAPABILITIES_CONTRACT];

pub const MESSAGE_BYTES: usize = 8;
const MESSAGE_MAGIC: [u8; 3] = *b"HVM";
const MESSAGE_VERSION: u8 = 1;
const KIND_PROVISION_REQUEST: u8 = 1;
const KIND_INSTANCE_COMMAND: u8 = 2;
const KIND_INSTANCE_STATUS: u8 = 3;
const KIND_INSTANCE_EVENT: u8 = 4;
const KIND_MANAGER_CONNECTION: u8 = 5;
const KIND_FLEET_COMMAND: u8 = 6;
const KIND_FLEET_RESPONSE: u8 = 7;
const KIND_FLEET_CAPABILITY: u8 = 8;

/// Capability-bearing request accepted by the fleet manager while empty.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProvisionRequest {
    LaunchInstance,
}

impl ProvisionRequest {
    #[must_use]
    pub const fn encode(self) -> [u8; MESSAGE_BYTES] {
        match self {
            Self::LaunchInstance => encode_message(KIND_PROVISION_REQUEST, 1, 0),
        }
    }

    #[must_use]
    pub fn decode(message: &[u8]) -> Option<Self> {
        let (value, detail) = decode_message(message, KIND_PROVISION_REQUEST)?;
        match (value, detail) {
            (1, 0) => Some(Self::LaunchInstance),
            _ => None,
        }
    }

    #[must_use]
    pub const fn capability_count(self) -> usize {
        match self {
            Self::LaunchInstance => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ManagerConnectionRequest;

impl ManagerConnectionRequest {
    pub const CAPABILITY_COUNT: usize = 2;

    #[must_use]
    pub const fn encode(self) -> [u8; MESSAGE_BYTES] {
        encode_message(KIND_MANAGER_CONNECTION, 1, 0)
    }

    #[must_use]
    pub fn decode(message: &[u8]) -> Option<Self> {
        matches!(
            decode_message(message, KIND_MANAGER_CONNECTION),
            Some((1, 0))
        )
        .then_some(Self)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum FleetCommand {
    List = 1,
    Status = 2,
    Start = 3,
    Stop = 4,
    Restart = 5,
    Console = 6,
}

impl FleetCommand {
    #[must_use]
    pub const fn encode(self) -> [u8; MESSAGE_BYTES] {
        encode_message(KIND_FLEET_COMMAND, self as u8, 0)
    }

    #[must_use]
    pub fn decode(message: &[u8]) -> Option<Self> {
        let (value, detail) = decode_message(message, KIND_FLEET_COMMAND)?;
        if detail != 0 {
            return None;
        }
        match value {
            1 => Some(Self::List),
            2 => Some(Self::Status),
            3 => Some(Self::Start),
            4 => Some(Self::Stop),
            5 => Some(Self::Restart),
            6 => Some(Self::Console),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum FleetState {
    Stopped = 1,
    Starting = 2,
    Running = 3,
    Stopping = 4,
    Failed = 5,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FleetResponse {
    State(FleetState),
    Accepted,
    Busy,
    Failed,
}

impl FleetResponse {
    #[must_use]
    pub const fn encode(self) -> [u8; MESSAGE_BYTES] {
        match self {
            Self::State(state) => encode_message(KIND_FLEET_RESPONSE, 1, state as u8),
            Self::Accepted => encode_message(KIND_FLEET_RESPONSE, 2, 0),
            Self::Busy => encode_message(KIND_FLEET_RESPONSE, 3, 0),
            Self::Failed => encode_message(KIND_FLEET_RESPONSE, 4, 0),
        }
    }

    #[must_use]
    pub fn decode(message: &[u8]) -> Option<Self> {
        let (value, detail) = decode_message(message, KIND_FLEET_RESPONSE)?;
        match (value, detail) {
            (1, 1) => Some(Self::State(FleetState::Stopped)),
            (1, 2) => Some(Self::State(FleetState::Starting)),
            (1, 3) => Some(Self::State(FleetState::Running)),
            (1, 4) => Some(Self::State(FleetState::Stopping)),
            (1, 5) => Some(Self::State(FleetState::Failed)),
            (2, 0) => Some(Self::Accepted),
            (3, 0) => Some(Self::Busy),
            (4, 0) => Some(Self::Failed),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConsoleCapability;

impl ConsoleCapability {
    #[must_use]
    pub const fn encode(self) -> [u8; MESSAGE_BYTES] {
        encode_message(KIND_FLEET_CAPABILITY, 1, 0)
    }

    #[must_use]
    pub fn decode(message: &[u8]) -> Option<Self> {
        matches!(decode_message(message, KIND_FLEET_CAPABILITY), Some((1, 0))).then_some(Self)
    }
}

/// Cooperative command sent from a fleet manager to one VM runtime.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum InstanceCommand {
    Stop = 1,
    AttachConsole = 2,
}

impl InstanceCommand {
    #[must_use]
    pub const fn encode(self) -> [u8; MESSAGE_BYTES] {
        encode_message(KIND_INSTANCE_COMMAND, self as u8, 0)
    }

    #[must_use]
    pub fn decode(message: &[u8]) -> Option<Self> {
        let (value, detail) = decode_message(message, KIND_INSTANCE_COMMAND)?;
        match (value, detail) {
            (1, 0) => Some(Self::Stop),
            (2, 0) => Some(Self::AttachConsole),
            _ => None,
        }
    }
}

/// Failure classification retained across the runtime/manager boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum InstanceFailure {
    Runtime = 1,
    InvalidImage = 2,
    GuestMemoryFault = 3,
    GuestMmio = 4,
    GuestSynchronous = 5,
    UnexpectedAdministrativeStop = 6,
    InvalidControlProtocol = 7,
    MissingTerminalStatus = 8,
    UnsupportedConfiguration = 9,
}

/// One manager action produced by an idempotent stop-state transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StopAction {
    None,
    SendCooperative,
    ForceProcess,
}

/// Stop escalation state retained independently of transport readiness.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InstanceStopState {
    phase: StopPhase,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StopPhase {
    Running,
    CooperativeRequested,
    Forced,
}

impl InstanceStopState {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            phase: StopPhase::Running,
        }
    }

    /// Begins graceful shutdown exactly once.
    pub fn request_cooperative(&mut self) -> StopAction {
        match self.phase {
            StopPhase::Running => {
                self.phase = StopPhase::CooperativeRequested;
                StopAction::SendCooperative
            }
            StopPhase::CooperativeRequested | StopPhase::Forced => StopAction::None,
        }
    }

    /// Escalates owner loss, protocol corruption, or an expired grace period.
    pub fn request_forced(&mut self) -> StopAction {
        match self.phase {
            StopPhase::Forced => StopAction::None,
            StopPhase::Running | StopPhase::CooperativeRequested => {
                self.phase = StopPhase::Forced;
                StopAction::ForceProcess
            }
        }
    }

    /// Escalates a cooperative request after its externally supplied grace
    /// deadline expires.
    ///
    /// The protocol deliberately does not synthesize time. A manager must call
    /// this transition only from a real monotonic-deadline observation.
    pub fn grace_period_expired(&mut self) -> StopAction {
        self.request_forced()
    }
}

impl Default for InstanceStopState {
    fn default() -> Self {
        Self::new()
    }
}

impl InstanceFailure {
    const fn from_wire(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Runtime),
            2 => Some(Self::InvalidImage),
            3 => Some(Self::GuestMemoryFault),
            4 => Some(Self::GuestMmio),
            5 => Some(Self::GuestSynchronous),
            6 => Some(Self::UnexpectedAdministrativeStop),
            7 => Some(Self::InvalidControlProtocol),
            8 => Some(Self::MissingTerminalStatus),
            9 => Some(Self::UnsupportedConfiguration),
            _ => None,
        }
    }
}

/// Coarse-grained construction and execution progress from one VM runtime.
///
/// This low-frequency control protocol never carries guest data. Status
/// validation is monotonic, so duplicate, skipped, or post-terminal messages
/// are rejected before they can alter fleet policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstanceStatus {
    ImageValidated,
    MemoryPrepared,
    Installed,
    Running,
    Stopped,
    Failed(InstanceFailure),
}

impl InstanceStatus {
    #[must_use]
    pub const fn encode(self) -> [u8; MESSAGE_BYTES] {
        match self {
            Self::ImageValidated => encode_message(KIND_INSTANCE_STATUS, 1, 0),
            Self::MemoryPrepared => encode_message(KIND_INSTANCE_STATUS, 2, 0),
            Self::Installed => encode_message(KIND_INSTANCE_STATUS, 3, 0),
            Self::Running => encode_message(KIND_INSTANCE_STATUS, 4, 0),
            Self::Stopped => encode_message(KIND_INSTANCE_STATUS, 5, 0),
            Self::Failed(reason) => encode_message(KIND_INSTANCE_STATUS, 6, reason as u8),
        }
    }

    #[must_use]
    pub fn decode(message: &[u8]) -> Option<Self> {
        let (value, detail) = decode_message(message, KIND_INSTANCE_STATUS)?;
        match (value, detail) {
            (1, 0) => Some(Self::ImageValidated),
            (2, 0) => Some(Self::MemoryPrepared),
            (3, 0) => Some(Self::Installed),
            (4, 0) => Some(Self::Running),
            (5, 0) => Some(Self::Stopped),
            (6, reason) => Some(Self::Failed(InstanceFailure::from_wire(reason)?)),
            _ => None,
        }
    }

    /// Validates the lifecycle sequence emitted by one runtime.
    #[must_use]
    pub const fn follows(self, previous: Option<Self>) -> bool {
        matches!(
            (previous, self),
            (None, Self::ImageValidated)
                | (None, Self::Failed(_))
                | (Some(Self::ImageValidated), Self::MemoryPrepared)
                | (Some(Self::ImageValidated), Self::Failed(_))
                | (Some(Self::MemoryPrepared), Self::Installed)
                | (Some(Self::MemoryPrepared), Self::Failed(_))
                | (Some(Self::Installed), Self::Running)
                | (Some(Self::Installed), Self::Failed(_))
                | (Some(Self::Running), Self::Stopped)
                | (Some(Self::Running), Self::Failed(_))
        )
    }

    #[must_use]
    pub const fn terminal(self) -> Option<InstanceEvent> {
        match self {
            Self::Stopped => Some(InstanceEvent::Stopped),
            Self::Failed(reason) => Some(InstanceEvent::Failed(reason)),
            Self::ImageValidated | Self::MemoryPrepared | Self::Installed | Self::Running => None,
        }
    }
}

/// Terminal instance state observable through the fleet control endpoint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstanceEvent {
    Stopped,
    Failed(InstanceFailure),
}

impl InstanceEvent {
    #[must_use]
    pub const fn encode(self) -> [u8; MESSAGE_BYTES] {
        match self {
            Self::Stopped => encode_message(KIND_INSTANCE_EVENT, 1, 0),
            Self::Failed(reason) => encode_message(KIND_INSTANCE_EVENT, 2, reason as u8),
        }
    }

    #[must_use]
    pub fn decode(message: &[u8]) -> Option<Self> {
        let (value, detail) = decode_message(message, KIND_INSTANCE_EVENT)?;
        match (value, detail) {
            (1, 0) => Some(Self::Stopped),
            (2, reason) => Some(Self::Failed(InstanceFailure::from_wire(reason)?)),
            _ => None,
        }
    }
}

/// Pure reducer used by the manager to validate one runtime status stream.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InstanceTracker {
    last: Option<InstanceStatus>,
    protocol_failed: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidStatusTransition;

impl InstanceTracker {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            last: None,
            protocol_failed: false,
        }
    }

    /// Records one decoded message. Invalid ordering permanently poisons the
    /// stream so a later plausible terminal record cannot hide corruption.
    pub fn observe(&mut self, status: InstanceStatus) -> Result<(), InvalidStatusTransition> {
        if self.protocol_failed || !status.follows(self.last) {
            self.protocol_failed = true;
            return Err(InvalidStatusTransition);
        }
        self.last = Some(status);
        Ok(())
    }

    /// Permanently marks a malformed wire record or transport transition.
    pub fn reject_protocol(&mut self) {
        self.protocol_failed = true;
    }

    /// Reports whether a valid terminal record has already been observed.
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        !self.protocol_failed
            && matches!(
                self.last,
                Some(InstanceStatus::Stopped | InstanceStatus::Failed(_))
            )
    }

    #[must_use]
    pub const fn last_status(&self) -> Option<InstanceStatus> {
        self.last
    }

    /// Combines the validated terminal record with process-exit status.
    #[must_use]
    pub const fn finish(self, process_succeeded: bool) -> InstanceEvent {
        if self.protocol_failed {
            return InstanceEvent::Failed(InstanceFailure::InvalidControlProtocol);
        }
        match (process_succeeded, self.last) {
            (true, Some(status)) => match status.terminal() {
                Some(event) => event,
                None => InstanceEvent::Failed(InstanceFailure::MissingTerminalStatus),
            },
            (true, None) => InstanceEvent::Failed(InstanceFailure::MissingTerminalStatus),
            (false, Some(InstanceStatus::Failed(reason))) => InstanceEvent::Failed(reason),
            (false, _) => InstanceEvent::Failed(InstanceFailure::Runtime),
        }
    }
}

impl Default for InstanceTracker {
    fn default() -> Self {
        Self::new()
    }
}

const fn encode_message(kind: u8, value: u8, detail: u8) -> [u8; MESSAGE_BYTES] {
    [
        MESSAGE_MAGIC[0],
        MESSAGE_MAGIC[1],
        MESSAGE_MAGIC[2],
        MESSAGE_VERSION,
        kind,
        value,
        detail,
        0,
    ]
}

fn decode_message(message: &[u8], expected_kind: u8) -> Option<(u8, u8)> {
    let bytes: &[u8; MESSAGE_BYTES] = message.try_into().ok()?;
    if bytes[..3] != MESSAGE_MAGIC
        || bytes[3] != MESSAGE_VERSION
        || bytes[4] != expected_kind
        || bytes[7] != 0
    {
        return None;
    }
    Some((bytes[5], bytes[6]))
}

#[cfg(test)]
mod tests {
    use super::{
        FleetCommand, FleetResponse, FleetState, INSTANCE_CONTROL_RIGHTS, InstanceCommand,
        InstanceEvent, InstanceFailure, InstanceStatus, InstanceStopState, InstanceTracker,
        InvalidStatusTransition, ManagerConnectionRequest, PROVISIONED_IMAGE_RIGHTS,
        PROVISIONED_INSTANCE_CONTROL_RIGHTS, ProvisionRequest, RUNTIME_CONSOLE_CONNECTION_CONTRACT,
        RUNTIME_CREATION_LEASE_CONTRACT, RUNTIME_IMAGE_CONTRACT, RUNTIME_INSTANCE_CONTROL_CONTRACT,
        RUNTIME_STARTUP_CONTRACTS, StopAction,
    };
    use hyper_os::handle::Rights;

    #[test]
    fn provisioned_capabilities_retain_only_required_forwarding_authority() {
        assert!(PROVISIONED_IMAGE_RIGHTS.contains(Rights::TRANSFER));
        assert!(PROVISIONED_INSTANCE_CONTROL_RIGHTS.contains(Rights::TRANSFER));
        assert!(!INSTANCE_CONTROL_RIGHTS.contains(Rights::TRANSFER));
        assert!(PROVISIONED_INSTANCE_CONTROL_RIGHTS.contains(INSTANCE_CONTROL_RIGHTS));
    }

    #[test]
    fn runtime_startup_contract_is_complete_exact_and_unambiguous() {
        assert_eq!(
            RUNTIME_STARTUP_CONTRACTS,
            &[
                RUNTIME_IMAGE_CONTRACT,
                RUNTIME_CREATION_LEASE_CONTRACT,
                RUNTIME_INSTANCE_CONTROL_CONTRACT,
                RUNTIME_CONSOLE_CONNECTION_CONTRACT,
            ]
        );
        for (index, contract) in RUNTIME_STARTUP_CONTRACTS.iter().enumerate() {
            assert_eq!(contract.required_rights(), contract.allowed_rights());
            for other in RUNTIME_STARTUP_CONTRACTS.iter().skip(index + 1) {
                assert_ne!(contract.name(), other.name());
                assert_ne!(contract.purpose(), other.purpose());
            }
        }
    }

    #[test]
    fn wire_values_are_explicit_and_stable() {
        assert_eq!(
            ProvisionRequest::LaunchInstance.encode(),
            [b'H', b'V', b'M', 1, 1, 1, 0, 0]
        );
        assert_eq!(
            InstanceCommand::Stop.encode(),
            [b'H', b'V', b'M', 1, 2, 1, 0, 0]
        );
        assert_eq!(
            InstanceStatus::Running.encode(),
            [b'H', b'V', b'M', 1, 3, 4, 0, 0]
        );
        assert_eq!(
            InstanceStatus::Failed(InstanceFailure::GuestMemoryFault).encode(),
            [b'H', b'V', b'M', 1, 3, 6, 3, 0]
        );
        assert_eq!(
            InstanceEvent::Failed(InstanceFailure::GuestMmio).encode(),
            [b'H', b'V', b'M', 1, 4, 2, 4, 0]
        );
    }

    #[test]
    fn message_kinds_cannot_be_confused() {
        let command = ProvisionRequest::LaunchInstance.encode();
        assert_eq!(
            ProvisionRequest::decode(&command),
            Some(ProvisionRequest::LaunchInstance)
        );
        assert_eq!(InstanceCommand::decode(&command), None);
        assert_eq!(InstanceStatus::decode(&command), None);
        assert_eq!(InstanceEvent::decode(&command), None);
        assert_eq!(FleetCommand::decode(&command), None);
        assert_eq!(FleetResponse::decode(&command), None);
    }

    #[test]
    fn fleet_protocol_round_trips_every_command_and_state() {
        assert_eq!(
            ManagerConnectionRequest::decode(&ManagerConnectionRequest.encode()),
            Some(ManagerConnectionRequest)
        );
        for command in [
            FleetCommand::List,
            FleetCommand::Status,
            FleetCommand::Start,
            FleetCommand::Stop,
            FleetCommand::Restart,
            FleetCommand::Console,
        ] {
            assert_eq!(FleetCommand::decode(&command.encode()), Some(command));
        }
        for response in [
            FleetResponse::State(FleetState::Stopped),
            FleetResponse::State(FleetState::Starting),
            FleetResponse::State(FleetState::Running),
            FleetResponse::State(FleetState::Stopping),
            FleetResponse::State(FleetState::Failed),
            FleetResponse::Accepted,
            FleetResponse::Busy,
            FleetResponse::Failed,
        ] {
            assert_eq!(FleetResponse::decode(&response.encode()), Some(response));
        }
    }

    #[test]
    fn lifecycle_reducer_accepts_only_complete_monotonic_paths() {
        let mut previous = None;
        for status in [
            InstanceStatus::ImageValidated,
            InstanceStatus::MemoryPrepared,
            InstanceStatus::Installed,
            InstanceStatus::Running,
            InstanceStatus::Stopped,
        ] {
            assert!(status.follows(previous));
            previous = Some(status);
        }
        assert_eq!(
            previous.and_then(InstanceStatus::terminal),
            Some(InstanceEvent::Stopped)
        );
        assert!(!InstanceStatus::Running.follows(Some(InstanceStatus::ImageValidated)));
        assert!(!InstanceStatus::Stopped.follows(Some(InstanceStatus::Stopped)));
    }

    #[test]
    fn failure_is_terminal_at_every_nonterminal_stage() {
        for previous in [
            None,
            Some(InstanceStatus::ImageValidated),
            Some(InstanceStatus::MemoryPrepared),
            Some(InstanceStatus::Installed),
            Some(InstanceStatus::Running),
        ] {
            let failed = InstanceStatus::Failed(InstanceFailure::Runtime);
            assert!(failed.follows(previous));
            assert_eq!(
                failed.terminal(),
                Some(InstanceEvent::Failed(InstanceFailure::Runtime))
            );
        }
    }

    #[test]
    fn malformed_and_reserved_messages_are_rejected() {
        let mut message = InstanceStatus::Running.encode();
        message[7] = 1;
        assert_eq!(InstanceStatus::decode(&message), None);
        assert_eq!(InstanceStatus::decode(&message[..7]), None);
    }

    #[test]
    fn provisioning_flags_determine_the_atomic_capability_shape() {
        let request = ProvisionRequest::LaunchInstance;
        assert_eq!(ProvisionRequest::decode(&request.encode()), Some(request));
        assert_eq!(request.capability_count(), 2);
    }

    #[test]
    fn stop_escalation_is_idempotent_and_peer_close_can_force() {
        let mut state = InstanceStopState::new();
        assert_eq!(state.request_cooperative(), StopAction::SendCooperative);
        assert_eq!(state.request_cooperative(), StopAction::None);
        assert_eq!(state.grace_period_expired(), StopAction::ForceProcess);
        assert_eq!(state.request_forced(), StopAction::None);

        let mut owner_lost = InstanceStopState::new();
        assert_eq!(owner_lost.request_forced(), StopAction::ForceProcess);
        assert_eq!(owner_lost.request_cooperative(), StopAction::None);
    }

    #[test]
    fn runtime_finalization_cannot_hide_missing_or_invalid_status() {
        let mut clean = InstanceTracker::new();
        for status in [
            InstanceStatus::ImageValidated,
            InstanceStatus::MemoryPrepared,
            InstanceStatus::Installed,
            InstanceStatus::Running,
            InstanceStatus::Stopped,
        ] {
            assert_eq!(clean.observe(status), Ok(()));
        }
        assert!(clean.is_terminal());
        assert_eq!(clean.finish(true), InstanceEvent::Stopped);

        let missing = InstanceTracker::new();
        assert!(!missing.is_terminal());
        assert_eq!(
            missing.finish(true),
            InstanceEvent::Failed(InstanceFailure::MissingTerminalStatus)
        );

        let mut invalid = InstanceTracker::new();
        assert_eq!(
            invalid.observe(InstanceStatus::Running),
            Err(InvalidStatusTransition)
        );
        assert_eq!(
            invalid.finish(true),
            InstanceEvent::Failed(InstanceFailure::InvalidControlProtocol)
        );
    }

    #[test]
    fn runtime_failure_reason_survives_process_failure() {
        let mut tracker = InstanceTracker::new();
        assert_eq!(
            tracker.observe(InstanceStatus::Failed(InstanceFailure::InvalidImage)),
            Ok(())
        );
        assert_eq!(
            tracker.finish(false),
            InstanceEvent::Failed(InstanceFailure::InvalidImage)
        );
    }
}
