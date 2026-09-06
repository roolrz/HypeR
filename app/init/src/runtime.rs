// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Capability-scoped adapter from a validated manifest to `ProcessBuilder`.

use core::convert::Infallible;

use hyper_app::BootstrapError;
use hyper_app::manifest::{
    AuthorityDeclaration, AuthorityPolicy, CapabilityOperation, LaunchPlan, MAX_MANIFEST_BYTES,
    MAX_SERVICES, Manifest, Service,
};
use hyper_app::supervision::{self, SupportError};
use hyper_app::{ManifestSource, ServiceGraphLauncher, bootstrap};
use hyper_os::bootfs::{BootFileRights, BootFs};
use hyper_os::channel;
use hyper_os::handle::{
    ByteChannelObject, ConsoleObject, OwnedHandle, ProcessObject, ResourceDomainObject, Rights,
    RightsOffer, TaskFactoryObject, TaskGroupObject, TypedObject,
};
use hyper_os::startup::{self, Startup};
use hyper_os::task::ProcessBuilder;

const MANIFEST_PATH: &str = "/etc/hyper/services.json";
const BOOTSTRAP_CONSOLE: &str = "bootstrap.console";
const CONSOLE_INPUT_CHANNEL: &str = "bootstrap.console-input-channel";
const CONSOLE_OUTPUT_CHANNEL: &str = "bootstrap.console-output-channel";
const SESSION_INPUT_CHANNEL: &str = "bootstrap.session-input-channel";
const SESSION_OUTPUT_CHANNEL: &str = "bootstrap.session-output-channel";
const CONSOLE_KIND: &str = "console";
const BYTE_CHANNEL_KIND: &str = "byte-channel";

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
    supervisors: [Option<OwnedHandle<ProcessObject>>; MAX_SERVICES],
}

impl RuntimeLauncher {
    fn from_startup(startup: &mut Startup<'_>, boot_fs: BootFs) -> Result<Self, Error> {
        let (console_input_channel, session_input_channel) =
            channel::create_pair().map_err(|_| Error::OperatingSystem)?;
        let (session_output_channel, console_output_channel) =
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
            match (capability.source(), capability.operation()) {
                (BOOTSTRAP_CONSOLE, CapabilityOperation::Duplicate)
                    if plan.capability_kind(service_index, capability_index)
                        == Some(ConsoleObject::KIND.as_raw()) =>
                {
                    builder
                        .add_handle_duplicate(
                            self.console.as_handle_ref(),
                            capability.purpose(),
                            RightsOffer::Exact(rights),
                        )
                        .map_err(|_| LaunchError::OperatingSystem)?;
                }
                (source, CapabilityOperation::Move)
                    if plan.capability_kind(service_index, capability_index)
                        == Some(ByteChannelObject::KIND.as_raw()) =>
                {
                    self.move_channel_into_builder(source, &builder, capability.purpose(), rights)?;
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
            | SESSION_OUTPUT_CHANNEL => Some(AuthorityDeclaration {
                provider: None,
                object_kind: ByteChannelObject::KIND.as_raw(),
                rights: byte_channel_rights().bits(),
                movable: true,
                duplicable: false,
                creatable: false,
            }),
            _ => None,
        }
    }

    fn object_kind(&self, name: &str) -> Option<u32> {
        match name {
            CONSOLE_KIND => Some(ConsoleObject::KIND.as_raw()),
            BYTE_CHANNEL_KIND => Some(ByteChannelObject::KIND.as_raw()),
            _ => None,
        }
    }

    fn right(&self, name: &str) -> Option<u64> {
        match name {
            "duplicate" => Some(Rights::DUPLICATE.bits()),
            "transfer" => Some(Rights::TRANSFER.bits()),
            "wait" => Some(Rights::WAIT.bits()),
            "inspect" => Some(Rights::INSPECT.bits()),
            "read" => Some(Rights::READ.bits()),
            "write" => Some(Rights::WRITE.bits()),
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
