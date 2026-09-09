// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Capability-scoped adapter from a validated manifest to `ProcessBuilder`.

mod authority;
mod launcher;
mod policy;
mod provision;
mod report;
mod supervisor;

use authority::AuthorityInventory;
use launcher::ServiceLauncher;
use policy::BootstrapPolicy;
use provision::InitialVmProvisioner;
use supervisor::SupervisorSet;

use std::convert::Infallible;

use hyper_init::BootstrapError;
use hyper_init::manifest::{LaunchPlan, MAX_MANIFEST_BYTES, Manifest};
use hyper_init::supervision::{self, SupportError};
use hyper_init::{ManifestSource, ServiceGraphLauncher, bootstrap};
use hyper_os::capability_channel::CapabilityChannel;
use hyper_os::channel;
use hyper_os::fs::{Directory, FileRights};
use hyper_os::startup::{self, Startup};
use hyper_os::task::{create_resource_domain, create_task_group};
use hyper_service::vm as vm_contract;
use hyper_vm_policy::INITIAL_VM_FLEET_LIMITS;

const MANIFEST_PATH: &str = "/etc/hyper/services.json";

#[inline(never)]
pub(super) fn run(startup: &mut Startup<'_>) -> Result<Infallible, Error> {
    hyper_os::require_core_abi().map_err(|_| Error::OperatingSystem)?;
    let root_directory = startup
        .take_root_directory()
        .map_err(|_| Error::OperatingSystem)?;
    let source = LoadedManifest::load(&root_directory)?;
    let policy = BootstrapPolicy;
    let mut runtime = Runtime::from_startup(startup, root_directory)?;
    match bootstrap(&source, &policy, &mut runtime) {
        Ok(never) => match never {},
        Err(error) => {
            runtime.report_bootstrap_error(&error);
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

struct LoadedManifest(String);

impl LoadedManifest {
    fn load(root_directory: &Directory) -> Result<Self, Error> {
        let file = root_directory
            .open(MANIFEST_PATH, FileRights::READ)
            .map_err(|_| Error::OperatingSystem)?;
        let length = usize::try_from(file.size().map_err(|_| Error::OperatingSystem)?)
            .ok()
            .filter(|length| *length <= MAX_MANIFEST_BYTES)
            .ok_or(Error::Source)?;
        let mut bytes = vec![0; length];
        file.read_exact_at(0, &mut bytes)
            .map_err(|_| Error::OperatingSystem)?;
        String::from_utf8(bytes)
            .map(Self)
            .map_err(|_| Error::Source)
    }
}

type SourceError = std::convert::Infallible;

impl ManifestSource for LoadedManifest {
    type Error = SourceError;
    fn manifest(&self) -> Result<&str, Self::Error> {
        Ok(&self.0)
    }
}

/// Top-level init state. Subobjects own disjoint authority and lifecycle roles.
struct Runtime {
    launcher: ServiceLauncher,
    provisioner: InitialVmProvisioner,
    supervisors: SupervisorSet,
}

impl Runtime {
    fn from_startup(startup: &mut Startup<'_>, root_directory: Directory) -> Result<Self, Error> {
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
        let (init_vm_provisioning_channel, vm_provisioning_channel) =
            CapabilityChannel::create().map_err(|_| Error::OperatingSystem)?;
        let (vm_client_connection_channel, vm_manager_connection_channel) =
            CapabilityChannel::create().map_err(|_| Error::OperatingSystem)?;
        let (vm_instance_control_channel, manager_vm_instance_control_channel) =
            channel::create_pair().map_err(|_| Error::OperatingSystem)?;
        let factory = startup
            .take(startup::TASK_FACTORY)
            .map_err(|_| Error::OperatingSystem)?;
        let group = startup
            .take(startup::TASK_GROUP)
            .map_err(|_| Error::OperatingSystem)?;
        let domain = startup
            .take(startup::RESOURCE_DOMAIN)
            .map_err(|_| Error::OperatingSystem)?;
        let vm_fleet_domain =
            create_resource_domain(domain.as_handle_ref(), INITIAL_VM_FLEET_LIMITS)
                .map_err(|_| Error::OperatingSystem)?;
        let vm_fleet_group =
            create_task_group(factory.as_handle_ref(), vm_fleet_domain.as_handle_ref())
                .map_err(|_| Error::OperatingSystem)?;
        let authorities = AuthorityInventory {
            root_directory,
            library_directory: Directory::from_handle(
                startup
                    .take(startup::DYNAMIC_LIBRARY_DIRECTORY)
                    .map_err(|_| Error::OperatingSystem)?,
            ),
            factory,
            group,
            domain,
            vm_fleet_group,
            vm_fleet_domain,
            task_inspector: startup
                .take(startup::TASK_INSPECTOR)
                .map_err(|_| Error::OperatingSystem)?,
            object_inspector: startup
                .take(startup::OBJECT_INSPECTOR)
                .map_err(|_| Error::OperatingSystem)?,
            memory_inspector: startup
                .take(startup::MEMORY_INSPECTOR)
                .map_err(|_| Error::OperatingSystem)?,
            cpu_inspector: startup
                .take(startup::CPU_INSPECTOR)
                .map_err(|_| Error::OperatingSystem)?,
            vm_authority: startup
                .take(startup::VIRTUAL_MACHINE_CREATION_AUTHORITY)
                .map_err(|_| Error::OperatingSystem)?,
            console: hyper_rt::process::console().map_err(|_| Error::OperatingSystem)?,
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
            vm_provisioning_channel: Some(vm_provisioning_channel.into_handle()),
            vm_client_connection_channel: vm_client_connection_channel.into_handle(),
            vm_manager_connection_channel: Some(vm_manager_connection_channel.into_handle()),
        };
        Ok(Self {
            launcher: ServiceLauncher { authorities },
            provisioner: InitialVmProvisioner {
                init_vm_provisioning_channel,
                vm_instance_control_channel: Some(vm_instance_control_channel),
                manager_vm_instance_control_channel: Some(manager_vm_instance_control_channel),
            },
            supervisors: SupervisorSet {
                processes: std::array::from_fn(|_| None),
            },
        })
    }

    fn preflight(manifest: &Manifest<'_>) -> Result<(), LaunchError> {
        match supervision::validate(manifest) {
            Ok(()) => Ok(()),
            Err(SupportError::RestartPolicy) => Err(LaunchError::UnsupportedRestartPolicy),
            Err(SupportError::MissingCriticalService) => {
                Err(LaunchError::UnsupportedSupervisionGraph)
            }
        }
    }
}

impl Runtime {
    fn report_bootstrap_error(&self, error: &BootstrapError<SourceError, LaunchError>) {
        let message = match error {
            BootstrapError::Source(_) => b"HypeR init: manifest source failed\n".as_slice(),
            BootstrapError::Parse(_) => b"HypeR init: manifest parse failed\n".as_slice(),
            BootstrapError::Validate(_) => b"HypeR init: manifest validation failed\n".as_slice(),
            BootstrapError::Launch(error) => error.diagnostic(),
        };
        let _ = self
            .launcher
            .authorities
            .console
            .as_emergency_console()
            .write_all(message);
    }
}

impl ServiceGraphLauncher for Runtime {
    type Error = LaunchError;

    fn launch(
        &mut self,
        manifest: &Manifest<'_>,
        plan: &LaunchPlan<'_>,
    ) -> Result<Infallible, Self::Error> {
        self.launch_validated_graph(manifest, plan)
    }
}

impl Runtime {
    #[inline(never)]
    fn launch_validated_graph(
        &mut self,
        manifest: &Manifest<'_>,
        plan: &LaunchPlan<'_>,
    ) -> Result<Infallible, LaunchError> {
        Self::preflight(manifest)?;
        let vm_manager_index = plan
            .unique_service_for_purpose(vm_contract::PROVISIONING.as_raw())
            .ok_or(LaunchError::InvalidPlan)?;
        let initial_vm_image = plan.initial_vm_image().ok_or(LaunchError::InvalidPlan)?;
        let result = (|| {
            self.launcher.start_initial_graph(
                manifest,
                plan,
                vm_manager_index,
                &mut self.supervisors,
            )?;
            self.provisioner.provision_initial_vm(
                manifest,
                vm_manager_index,
                initial_vm_image,
                &self.launcher.authorities.root_directory,
                self.launcher.authorities.console,
                &mut self.supervisors,
            )?;
            self.supervisors.supervise(
                manifest,
                &mut self.provisioner.vm_instance_control_channel,
                self.launcher.authorities.console,
            )
        })();
        match result {
            Ok(never) => match never {},
            Err(error) => {
                if let Err(stop_error) = self.supervisors.request_service_stop() {
                    let _ = self
                        .launcher
                        .authorities
                        .console
                        .as_emergency_console()
                        .write_all(stop_error.diagnostic());
                }
                Err(error)
            }
        }
    }
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
    VmInstanceFailed,
    VmInstanceProtocol,
    VmManagerTerminated,
    VmProvisioningClosed,
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
            Self::VmInstanceFailed => b"HypeR init: initial VM failed\n",
            Self::VmInstanceProtocol => b"HypeR init: initial VM protocol failed\n",
            Self::VmManagerTerminated => b"HypeR init: VM manager terminated before provisioning\n",
            Self::VmProvisioningClosed => b"HypeR init: VM provisioning channel closed\n",
        }
    }

    const fn reason(&self) -> &'static [u8] {
        match self {
            Self::AuthorityConsumed => b"authority already consumed",
            Self::CriticalServiceTerminated => b"critical service terminated",
            Self::InvalidPlan => b"invalid launch plan",
            Self::OperatingSystem => b"kernel operation rejected",
            Self::StopRollbackFailed => b"rollback failed",
            Self::UnsupportedAuthority => b"unsupported authority",
            Self::UnsupportedRestartPolicy => b"unsupported restart policy",
            Self::UnsupportedSupervisionGraph => b"unsupported supervision graph",
            Self::VmInstanceFailed => b"initial VM failed",
            Self::VmInstanceProtocol => b"initial VM protocol failed",
            Self::VmManagerTerminated => b"VM manager terminated",
            Self::VmProvisioningClosed => b"VM provisioning channel closed",
        }
    }
}
