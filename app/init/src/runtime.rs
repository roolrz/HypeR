// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Capability-scoped adapter from a validated manifest to `ProcessBuilder`.

use core::convert::Infallible;

use hyper_app::BootstrapError;
use hyper_app::manifest::{
    AuthorityDeclaration, AuthorityPolicy, CapabilityOperation, LaunchPlan, MAX_MANIFEST_BYTES,
    MAX_SERVICES, Manifest, Service, StartupPurposeDeclaration,
};
use hyper_app::supervision::{self, SupportError};
use hyper_app::{ManifestSource, ServiceGraphLauncher, bootstrap};
use hyper_os::bootfs::{BootFileRights, BootFs};
use hyper_os::channel;
use hyper_os::handle::{
    BootFsObject, ByteChannelObject, ConsoleObject, OwnedHandle, ProcessObject,
    ResourceDomainObject, Rights, RightsOffer, TaskFactoryObject, TaskGroupObject, TypedObject,
};
use hyper_os::startup::{self, Startup};
use hyper_os::task::ProcessBuilder;
use hyper_service::{
    console as console_contract, process as process_contract, session as session_contract,
    stdio as stdio_contract,
};

const MANIFEST_PATH: &str = "/etc/hyper/services.json";
const BOOTSTRAP_CONSOLE: &str = "bootstrap.console";
const CONSOLE_INPUT_CHANNEL: &str = "bootstrap.console-input-channel";
const CONSOLE_OUTPUT_CHANNEL: &str = "bootstrap.console-output-channel";
const SESSION_INPUT_CHANNEL: &str = "bootstrap.session-input-channel";
const SESSION_OUTPUT_CHANNEL: &str = "bootstrap.session-output-channel";
const SESSION_CLIENT_INPUT_CHANNEL: &str = "bootstrap.session-client-input-channel";
const SESSION_CLIENT_OUTPUT_CHANNEL: &str = "bootstrap.session-client-output-channel";
const SESSION_CLIENT_ERROR_CHANNEL: &str = "bootstrap.session-client-error-channel";
const SHELL_INPUT_CHANNEL: &str = "bootstrap.shell-input-channel";
const SHELL_OUTPUT_CHANNEL: &str = "bootstrap.shell-output-channel";
const SHELL_ERROR_CHANNEL: &str = "bootstrap.shell-error-channel";
const BOOTSTRAP_BOOT_FS: &str = "bootstrap.boot-fs";
const BOOTSTRAP_TASK_FACTORY: &str = "bootstrap.task-factory";
const BOOTSTRAP_TASK_GROUP: &str = "bootstrap.task-group";
const BOOTSTRAP_RESOURCE_DOMAIN: &str = "bootstrap.resource-domain";
const CONSOLE_INPUT_IMAGE: &str = "/svc/console-input";
const CONSOLE_OUTPUT_IMAGE: &str = "/svc/console-output";
const SESSION_IMAGE: &str = "/svc/session";
const SHELL_IMAGE: &str = "/bin/sh";

#[inline(never)]
pub(super) fn run(startup: &mut Startup<'_>) -> Result<Infallible, Error> {
    hyper_os::require_core_abi().map_err(|_| Error::OperatingSystem)?;
    let boot_fs = startup.take_boot_fs().map_err(|_| Error::OperatingSystem)?;
    let mut manifest_buffer = [0_u8; MAX_MANIFEST_BYTES];
    let source = LoadedManifest::load(&boot_fs, &mut manifest_buffer)?;
    let mut launcher = RuntimeLauncher::from_startup(startup, boot_fs)?;
    match bootstrap(&source, &mut launcher) {
        Ok(never) => match never {},
        Err(error) => {
            launcher.report_bootstrap_error(&error);
            Err(Error::Bootstrap)
        }
    }
}

pub(super) enum Error {
    Bootstrap,
    OperatingSystem,
    Source,
}

impl Error {
    pub(super) const fn diagnostic(&self) -> &'static [u8] {
        match self {
            Self::Bootstrap => b"HypeR init: bootstrap failed\n",
            Self::OperatingSystem => b"HypeR init: startup capability operation failed\n",
            Self::Source => b"HypeR init: invalid service manifest source\n",
        }
    }
}

struct LoadedManifest<'buffer> {
    bytes: &'buffer [u8],
}

impl<'buffer> LoadedManifest<'buffer> {
    #[inline(never)]
    fn load(
        boot_fs: &BootFs,
        buffer: &'buffer mut [u8; MAX_MANIFEST_BYTES],
    ) -> Result<Self, Error> {
        let file = boot_fs
            .open(MANIFEST_PATH, BootFileRights::READ)
            .map_err(|_| Error::OperatingSystem)?;
        let file_size = file.size().map_err(|_| Error::OperatingSystem)?;
        let length = usize::try_from(file_size)
            .ok()
            .filter(|length| *length <= MAX_MANIFEST_BYTES)
            .ok_or(Error::Source)?;
        let target = buffer.get_mut(..length).ok_or(Error::Source)?;
        file.read_exact(0, target)
            .map_err(|_| Error::OperatingSystem)?;
        core::str::from_utf8(target).map_err(|_| Error::Source)?;
        Ok(Self { bytes: target })
    }
}

#[derive(Clone, Copy)]
pub(super) enum SourceError {
    InvalidUtf8,
}

impl ManifestSource for LoadedManifest<'_> {
    type Error = SourceError;

    fn manifest(&self) -> Result<&str, Self::Error> {
        core::str::from_utf8(self.bytes).map_err(|_| SourceError::InvalidUtf8)
    }
}

struct RuntimeLauncher {
    boot_fs: BootFs,
    factory: OwnedHandle<TaskFactoryObject>,
    group: OwnedHandle<TaskGroupObject>,
    domain: OwnedHandle<ResourceDomainObject>,
    console: OwnedHandle<ConsoleObject>,
    console_input_channel: Option<OwnedHandle<ByteChannelObject>>,
    console_output_channel: Option<OwnedHandle<ByteChannelObject>>,
    session_input_channel: Option<OwnedHandle<ByteChannelObject>>,
    session_output_channel: Option<OwnedHandle<ByteChannelObject>>,
    session_client_input_channel: Option<OwnedHandle<ByteChannelObject>>,
    session_client_output_channel: Option<OwnedHandle<ByteChannelObject>>,
    session_client_error_channel: Option<OwnedHandle<ByteChannelObject>>,
    shell_input_channel: Option<OwnedHandle<ByteChannelObject>>,
    shell_output_channel: Option<OwnedHandle<ByteChannelObject>>,
    shell_error_channel: Option<OwnedHandle<ByteChannelObject>>,
    supervisors: [Option<OwnedHandle<ProcessObject>>; MAX_SERVICES],
}

impl RuntimeLauncher {
    fn from_startup(startup: &mut Startup<'_>, boot_fs: BootFs) -> Result<Self, Error> {
        let (console_input_channel, session_input_channel) =
            channel::create_pair().map_err(|_| Error::OperatingSystem)?;
        let (session_output_channel, console_output_channel) =
            channel::create_pair().map_err(|_| Error::OperatingSystem)?;
        let (session_client_input_channel, shell_input_channel) =
            channel::create_pair().map_err(|_| Error::OperatingSystem)?;
        let (shell_output_channel, session_client_output_channel) =
            channel::create_pair().map_err(|_| Error::OperatingSystem)?;
        let (shell_error_channel, session_client_error_channel) =
            channel::create_pair().map_err(|_| Error::OperatingSystem)?;
        Ok(Self {
            boot_fs,
            factory: startup
                .take(startup::TASK_FACTORY)
                .map_err(|_| Error::OperatingSystem)?,
            group: startup
                .take(startup::TASK_GROUP)
                .map_err(|_| Error::OperatingSystem)?,
            domain: startup
                .take(startup::RESOURCE_DOMAIN)
                .map_err(|_| Error::OperatingSystem)?,
            console: startup
                .take(startup::CONSOLE)
                .map_err(|_| Error::OperatingSystem)?,
            console_input_channel: Some(console_input_channel),
            console_output_channel: Some(console_output_channel),
            session_input_channel: Some(session_input_channel),
            session_output_channel: Some(session_output_channel),
            session_client_input_channel: Some(session_client_input_channel),
            session_client_output_channel: Some(session_client_output_channel),
            session_client_error_channel: Some(session_client_error_channel),
            shell_input_channel: Some(shell_input_channel),
            shell_output_channel: Some(shell_output_channel),
            shell_error_channel: Some(shell_error_channel),
            supervisors: core::array::from_fn(|_| None),
        })
    }

    fn preflight(manifest: &Manifest<'_>) -> Result<(), LaunchError> {
        match supervision::validate(manifest) {
            Ok(()) => Ok(()),
            Err(SupportError::RestartPolicy) => Err(LaunchError::UnsupportedRestartPolicy),
            Err(SupportError::CriticalServiceCount) => {
                Err(LaunchError::UnsupportedSupervisionGraph)
            }
        }
    }

    fn launch_service(
        &mut self,
        service: &Service<'_>,
        service_index: usize,
        plan: &LaunchPlan,
    ) -> Result<OwnedHandle<ProcessObject>, LaunchError> {
        let executable = self
            .boot_fs
            .open(service.image(), BootFileRights::EXECUTE)
            .map_err(|_| LaunchError::OperatingSystem)?;
        let builder = ProcessBuilder::create(
            self.factory.as_handle_ref(),
            self.group.as_handle_ref(),
            self.domain.as_handle_ref(),
            executable.as_handle_ref(),
        )
        .map_err(|_| LaunchError::OperatingSystem)?;
        builder
            .set_name(service.name())
            .map_err(|_| LaunchError::OperatingSystem)?;
        builder
            .add_argument(service.image())
            .map_err(|_| LaunchError::OperatingSystem)?;

        for (capability_index, capability) in service.capabilities().enumerate() {
            let rights = plan
                .capability_rights(service_index, capability_index)
                .and_then(Rights::from_bits)
                .ok_or(LaunchError::InvalidPlan)?;
            let purpose = plan
                .capability_purpose(service_index, capability_index)
                .ok_or(LaunchError::InvalidPlan)?;
            match (capability.source(), capability.operation()) {
                (BOOTSTRAP_CONSOLE, CapabilityOperation::Duplicate)
                    if plan.capability_kind(service_index, capability_index)
                        == Some(ConsoleObject::KIND.as_raw()) =>
                {
                    builder
                        .add_handle_duplicate(
                            self.console.as_handle_ref(),
                            purpose,
                            RightsOffer::Exact(rights),
                        )
                        .map_err(|_| LaunchError::OperatingSystem)?;
                }
                (BOOTSTRAP_BOOT_FS, CapabilityOperation::Duplicate)
                    if plan.capability_kind(service_index, capability_index)
                        == Some(BootFsObject::KIND.as_raw()) =>
                {
                    builder
                        .add_handle_duplicate(
                            self.boot_fs.as_handle_ref(),
                            purpose,
                            RightsOffer::Exact(rights),
                        )
                        .map_err(|_| LaunchError::OperatingSystem)?;
                }
                (BOOTSTRAP_TASK_FACTORY, CapabilityOperation::Duplicate)
                    if plan.capability_kind(service_index, capability_index)
                        == Some(TaskFactoryObject::KIND.as_raw()) =>
                {
                    builder
                        .add_handle_duplicate(
                            self.factory.as_handle_ref(),
                            purpose,
                            RightsOffer::Exact(rights),
                        )
                        .map_err(|_| LaunchError::OperatingSystem)?;
                }
                (BOOTSTRAP_TASK_GROUP, CapabilityOperation::Duplicate)
                    if plan.capability_kind(service_index, capability_index)
                        == Some(TaskGroupObject::KIND.as_raw()) =>
                {
                    builder
                        .add_handle_duplicate(
                            self.group.as_handle_ref(),
                            purpose,
                            RightsOffer::Exact(rights),
                        )
                        .map_err(|_| LaunchError::OperatingSystem)?;
                }
                (BOOTSTRAP_RESOURCE_DOMAIN, CapabilityOperation::Duplicate)
                    if plan.capability_kind(service_index, capability_index)
                        == Some(ResourceDomainObject::KIND.as_raw()) =>
                {
                    builder
                        .add_handle_duplicate(
                            self.domain.as_handle_ref(),
                            purpose,
                            RightsOffer::Exact(rights),
                        )
                        .map_err(|_| LaunchError::OperatingSystem)?;
                }
                (source, CapabilityOperation::Move)
                    if plan.capability_kind(service_index, capability_index)
                        == Some(ByteChannelObject::KIND.as_raw()) =>
                {
                    self.move_channel_into_builder(source, &builder, purpose, rights)?;
                }
                _ => return Err(LaunchError::UnsupportedAuthority),
            }
        }

        builder.seal().map_err(|_| LaunchError::OperatingSystem)?;
        builder.start().map_err(|_| LaunchError::OperatingSystem)
    }

    fn move_channel_into_builder(
        &mut self,
        source: &str,
        builder: &ProcessBuilder,
        purpose: u32,
        rights: Rights,
    ) -> Result<(), LaunchError> {
        let slot = match source {
            CONSOLE_INPUT_CHANNEL => &mut self.console_input_channel,
            CONSOLE_OUTPUT_CHANNEL => &mut self.console_output_channel,
            SESSION_INPUT_CHANNEL => &mut self.session_input_channel,
            SESSION_OUTPUT_CHANNEL => &mut self.session_output_channel,
            SESSION_CLIENT_INPUT_CHANNEL => &mut self.session_client_input_channel,
            SESSION_CLIENT_OUTPUT_CHANNEL => &mut self.session_client_output_channel,
            SESSION_CLIENT_ERROR_CHANNEL => &mut self.session_client_error_channel,
            SHELL_INPUT_CHANNEL => &mut self.shell_input_channel,
            SHELL_OUTPUT_CHANNEL => &mut self.shell_output_channel,
            SHELL_ERROR_CHANNEL => &mut self.shell_error_channel,
            _ => return Err(LaunchError::UnsupportedAuthority),
        };
        let channel = slot.take().ok_or(LaunchError::AuthorityConsumed)?;
        match builder.add_handle_move(channel, purpose, RightsOffer::Exact(rights)) {
            Ok(()) => Ok(()),
            Err(failure) => {
                let (_, channel) = failure.into_parts();
                *slot = Some(channel);
                Err(LaunchError::OperatingSystem)
            }
        }
    }

    fn start_initial_graph(
        &mut self,
        manifest: &Manifest<'_>,
        plan: &LaunchPlan,
    ) -> Result<(), LaunchError> {
        for position in 0..plan.service_count() {
            let service_index = plan
                .service_index(position)
                .ok_or(LaunchError::InvalidPlan)?;
            let service = manifest
                .service(service_index)
                .ok_or(LaunchError::InvalidPlan)?;
            if self
                .supervisors
                .get(service_index)
                .ok_or(LaunchError::InvalidPlan)?
                .is_some()
            {
                return Err(LaunchError::InvalidPlan);
            }
            let supervisor = self.launch_service(service, service_index, plan)?;
            let slot = self
                .supervisors
                .get_mut(service_index)
                .ok_or(LaunchError::InvalidPlan)?;
            *slot = Some(supervisor);
        }
        Ok(())
    }

    fn supervise(&mut self, manifest: &Manifest<'_>) -> Result<Infallible, LaunchError> {
        let critical =
            supervision::critical_service_index(manifest).ok_or(LaunchError::InvalidPlan)?;
        let supervisor = self
            .supervisors
            .get(critical)
            .and_then(Option::as_ref)
            .ok_or(LaunchError::InvalidPlan)?;
        supervisor
            .as_process_supervisor()
            .wait_terminated(hyper_os::DEADLINE_INFINITE)
            .map_err(|_| LaunchError::OperatingSystem)?;
        drop(
            self.supervisors
                .get_mut(critical)
                .ok_or(LaunchError::InvalidPlan)?
                .take(),
        );
        Err(LaunchError::CriticalServiceTerminated)
    }

    fn request_service_stop(&self) -> Result<(), LaunchError> {
        let mut failed = false;
        for supervisor in self.supervisors.iter().filter_map(Option::as_ref) {
            if supervisor.as_process_supervisor().request_stop().is_err() {
                failed = true;
            }
        }
        if failed {
            Err(LaunchError::StopRollbackFailed)
        } else {
            Ok(())
        }
    }

    fn report_bootstrap_error(&self, error: &BootstrapError<SourceError, LaunchError>) {
        let message = match error {
            BootstrapError::Source(_) => b"HypeR init: manifest source failed\n".as_slice(),
            BootstrapError::Parse(_) => b"HypeR init: manifest parse failed\n".as_slice(),
            BootstrapError::Validate(_) => b"HypeR init: manifest validation failed\n".as_slice(),
            BootstrapError::Launch(error) => error.diagnostic(),
        };
        let _ = self.console.as_emergency_console().write_all(message);
    }
}

impl AuthorityPolicy for RuntimeLauncher {
    fn authority<'policy>(&'policy self, source: &str) -> Option<AuthorityDeclaration<'policy>> {
        match source {
            BOOTSTRAP_CONSOLE => Some(AuthorityDeclaration {
                provider: None,
                object_kind: ConsoleObject::KIND.as_raw(),
                rights: console_rights().bits(),
                movable: false,
                duplicable: true,
                creatable: false,
            }),
            CONSOLE_INPUT_CHANNEL
            | CONSOLE_OUTPUT_CHANNEL
            | SESSION_INPUT_CHANNEL
            | SESSION_OUTPUT_CHANNEL
            | SESSION_CLIENT_INPUT_CHANNEL
            | SESSION_CLIENT_OUTPUT_CHANNEL
            | SESSION_CLIENT_ERROR_CHANNEL
            | SHELL_INPUT_CHANNEL
            | SHELL_OUTPUT_CHANNEL
            | SHELL_ERROR_CHANNEL => Some(AuthorityDeclaration {
                provider: None,
                object_kind: ByteChannelObject::KIND.as_raw(),
                rights: byte_channel_rights().bits(),
                movable: true,
                duplicable: false,
                creatable: false,
            }),
            BOOTSTRAP_BOOT_FS => Some(AuthorityDeclaration {
                provider: None,
                object_kind: BootFsObject::KIND.as_raw(),
                rights: boot_fs_rights().bits(),
                movable: false,
                duplicable: true,
                creatable: false,
            }),
            BOOTSTRAP_TASK_FACTORY => Some(AuthorityDeclaration {
                provider: None,
                object_kind: TaskFactoryObject::KIND.as_raw(),
                rights: task_factory_rights().bits(),
                movable: false,
                duplicable: true,
                creatable: false,
            }),
            BOOTSTRAP_TASK_GROUP => Some(AuthorityDeclaration {
                provider: None,
                object_kind: TaskGroupObject::KIND.as_raw(),
                rights: task_group_rights().bits(),
                movable: false,
                duplicable: true,
                creatable: false,
            }),
            BOOTSTRAP_RESOURCE_DOMAIN => Some(AuthorityDeclaration {
                provider: None,
                object_kind: ResourceDomainObject::KIND.as_raw(),
                rights: resource_domain_rights().bits(),
                movable: false,
                duplicable: true,
                creatable: false,
            }),
            _ => None,
        }
    }

    fn startup_purpose(&self, image: &str, name: &str) -> Option<StartupPurposeDeclaration> {
        let (value, object_kind) = match (image, name) {
            (CONSOLE_INPUT_IMAGE | CONSOLE_OUTPUT_IMAGE, console_contract::SYSTEM_CONSOLE_NAME) => {
                (
                    console_contract::SYSTEM_CONSOLE.as_raw(),
                    ConsoleObject::KIND.as_raw(),
                )
            }
            (CONSOLE_INPUT_IMAGE | CONSOLE_OUTPUT_IMAGE, console_contract::DATA_CHANNEL_NAME) => (
                console_contract::DATA_CHANNEL.as_raw(),
                ByteChannelObject::KIND.as_raw(),
            ),
            (SESSION_IMAGE, session_contract::CONSOLE_INPUT_NAME) => (
                session_contract::CONSOLE_INPUT.as_raw(),
                ByteChannelObject::KIND.as_raw(),
            ),
            (SESSION_IMAGE, session_contract::CONSOLE_OUTPUT_NAME) => (
                session_contract::CONSOLE_OUTPUT.as_raw(),
                ByteChannelObject::KIND.as_raw(),
            ),
            (SESSION_IMAGE, session_contract::CLIENT_INPUT_NAME) => (
                session_contract::CLIENT_INPUT.as_raw(),
                ByteChannelObject::KIND.as_raw(),
            ),
            (SESSION_IMAGE, session_contract::CLIENT_OUTPUT_NAME) => (
                session_contract::CLIENT_OUTPUT.as_raw(),
                ByteChannelObject::KIND.as_raw(),
            ),
            (SESSION_IMAGE, session_contract::CLIENT_ERROR_NAME) => (
                session_contract::CLIENT_ERROR.as_raw(),
                ByteChannelObject::KIND.as_raw(),
            ),
            (SHELL_IMAGE, stdio_contract::STANDARD_INPUT_NAME) => (
                stdio_contract::STANDARD_INPUT.as_raw(),
                ByteChannelObject::KIND.as_raw(),
            ),
            (SHELL_IMAGE, stdio_contract::STANDARD_OUTPUT_NAME) => (
                stdio_contract::STANDARD_OUTPUT.as_raw(),
                ByteChannelObject::KIND.as_raw(),
            ),
            (SHELL_IMAGE, stdio_contract::STANDARD_ERROR_NAME) => (
                stdio_contract::STANDARD_ERROR.as_raw(),
                ByteChannelObject::KIND.as_raw(),
            ),
            (SHELL_IMAGE, process_contract::BOOT_FS_NAME) => {
                (startup::BOOT_FS.as_raw(), BootFsObject::KIND.as_raw())
            }
            (SHELL_IMAGE, process_contract::TASK_FACTORY_NAME) => (
                startup::TASK_FACTORY.as_raw(),
                TaskFactoryObject::KIND.as_raw(),
            ),
            (SHELL_IMAGE, process_contract::TASK_GROUP_NAME) => {
                (startup::TASK_GROUP.as_raw(), TaskGroupObject::KIND.as_raw())
            }
            (SHELL_IMAGE, process_contract::RESOURCE_DOMAIN_NAME) => (
                startup::RESOURCE_DOMAIN.as_raw(),
                ResourceDomainObject::KIND.as_raw(),
            ),
            (SHELL_IMAGE, _) => return None,
            (_, _) => return None,
        };
        Some(StartupPurposeDeclaration { value, object_kind })
    }

    fn right(&self, name: &str) -> Option<u64> {
        match name {
            "duplicate" => Some(Rights::DUPLICATE.bits()),
            "transfer" => Some(Rights::TRANSFER.bits()),
            "wait" => Some(Rights::WAIT.bits()),
            "inspect" => Some(Rights::INSPECT.bits()),
            "read" => Some(Rights::READ.bits()),
            "write" => Some(Rights::WRITE.bits()),
            "create-process" => Some(Rights::CREATE_PROCESS.bits()),
            "attach-process" => Some(Rights::TASK_GROUP_ATTACH_PROCESS.bits()),
            "sponsor" => Some(Rights::RESOURCE_DOMAIN_SPONSOR.bits()),
            _ => None,
        }
    }
}

impl ServiceGraphLauncher for RuntimeLauncher {
    type Error = LaunchError;

    fn launch(
        &mut self,
        manifest: &Manifest<'_>,
        plan: &LaunchPlan,
    ) -> Result<Infallible, Self::Error> {
        self.launch_validated_graph(manifest, plan)
    }
}

impl RuntimeLauncher {
    #[inline(never)]
    fn launch_validated_graph(
        &mut self,
        manifest: &Manifest<'_>,
        plan: &LaunchPlan,
    ) -> Result<Infallible, LaunchError> {
        Self::preflight(manifest)?;
        let result = self
            .start_initial_graph(manifest, plan)
            .and_then(|()| self.supervise(manifest));
        match result {
            Ok(never) => match never {},
            Err(error) => {
                if let Err(stop_error) = self.request_service_stop() {
                    let _ = self
                        .console
                        .as_emergency_console()
                        .write_all(stop_error.diagnostic());
                }
                Err(error)
            }
        }
    }
}

fn console_rights() -> Rights {
    Rights::DUPLICATE
        .union(Rights::TRANSFER)
        .union(Rights::WAIT)
        .union(Rights::INSPECT)
        .union(Rights::READ)
        .union(Rights::WRITE)
}

fn byte_channel_rights() -> Rights {
    Rights::TRANSFER
        .union(Rights::WAIT)
        .union(Rights::INSPECT)
        .union(Rights::READ)
        .union(Rights::WRITE)
}

fn boot_fs_rights() -> Rights {
    Rights::DUPLICATE
        .union(Rights::TRANSFER)
        .union(Rights::INSPECT)
        .union(Rights::READ)
}

fn task_factory_rights() -> Rights {
    Rights::DUPLICATE
        .union(Rights::TRANSFER)
        .union(Rights::INSPECT)
        .union(Rights::CREATE_PROCESS)
}

fn task_group_rights() -> Rights {
    Rights::DUPLICATE
        .union(Rights::TRANSFER)
        .union(Rights::INSPECT)
        .union(Rights::REQUEST_STOP)
        .union(Rights::TASK_GROUP_ATTACH_PROCESS)
}

fn resource_domain_rights() -> Rights {
    Rights::DUPLICATE
        .union(Rights::TRANSFER)
        .union(Rights::INSPECT)
        .union(Rights::RESOURCE_DOMAIN_SPONSOR)
}

pub(super) enum LaunchError {
    AuthorityConsumed,
    CriticalServiceTerminated,
    InvalidPlan,
    OperatingSystem,
    StopRollbackFailed,
    UnsupportedAuthority,
    UnsupportedRestartPolicy,
    UnsupportedSupervisionGraph,
}

impl LaunchError {
    const fn diagnostic(&self) -> &'static [u8] {
        match self {
            Self::AuthorityConsumed => b"HypeR init: service authority was already consumed\n",
            Self::CriticalServiceTerminated => b"HypeR init: critical service terminated\n",
            Self::InvalidPlan => b"HypeR init: invalid launch plan\n",
            Self::OperatingSystem => b"HypeR init: service launch operation failed\n",
            Self::StopRollbackFailed => b"HypeR init: service rollback failed\n",
            Self::UnsupportedAuthority => b"HypeR init: unsupported service authority\n",
            Self::UnsupportedRestartPolicy => b"HypeR init: unsupported restart policy\n",
            Self::UnsupportedSupervisionGraph => {
                b"HypeR init: unsupported critical-service supervision graph\n"
            }
        }
    }
}
