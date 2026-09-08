// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Fleet policy and supervision for Native virtual-machine runtimes.

#![no_std]
#![no_main]

use core::mem::MaybeUninit;
use core::time::Duration;

use hyper_os::capability_channel::{CapabilityChannel, CapabilityReceiveSlot};
use hyper_os::channel;
use hyper_os::fs::{Directory, File};
use hyper_os::handle::{
    ByteChannelObject, ConsoleObject, OwnedHandle, ProcessObject, ResourceDomainObject, Rights,
    RightsOffer, TaskGroupObject,
};
use hyper_os::startup::{self, Startup};
use hyper_os::task::{
    ProcessBuilder, ProcessTermination, create_resource_domain, create_task_group,
};
use hyper_os::wait::{ObjectSignals, WaitItem, wait_many};
use hyper_rt::ExitCode;
use hyper_service::process as process_contract;
use hyper_service::vm as vm_contract;

const RUNTIME_ARGUMENT: &str = "/svc/vm-runtime";
const INSTANCE_EXIT_GRACE: Duration = Duration::from_secs(2);

fn application_main(mut startup: Startup<'_>) -> ExitCode {
    if hyper_os::require_core_abi().is_err() {
        return ExitCode::FAILURE;
    }
    let manager = match FleetManager::from_startup(&mut startup) {
        Ok(manager) => manager,
        Err(_) => return ExitCode::FAILURE,
    };
    match manager.run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}

/// Long-lived fleet authority separated from each disposable VM instance.
struct FleetManager {
    runtime_image: File,
    libraries: Directory,
    factory: OwnedHandle<hyper_os::handle::TaskFactoryObject>,
    fleet_domain: OwnedHandle<ResourceDomainObject>,
    authority: OwnedHandle<hyper_os::handle::VirtualMachineCreationAuthorityObject>,
    provisioning: CapabilityChannel,
}

impl FleetManager {
    fn from_startup(startup: &mut Startup<'_>) -> hyper_os::Result<Self> {
        Ok(Self {
            runtime_image: File::from_handle(startup.take(vm_contract::RUNTIME_IMAGE)?),
            libraries: Directory::from_handle(
                startup.take(process_contract::CHILD_LIBRARY_DIRECTORY)?,
            ),
            factory: startup.take(startup::TASK_FACTORY)?,
            fleet_domain: startup.take(startup::RESOURCE_DOMAIN)?,
            authority: startup.take(startup::VIRTUAL_MACHINE_CREATION_AUTHORITY)?,
            provisioning: CapabilityChannel::from_handle(startup.take(vm_contract::PROVISIONING)?),
        })
    }

    /// Alternates between an empty fleet and one active instance.
    ///
    /// Each completed instance is fully retired before the next provisioning
    /// rendezvous. Extending the fleet therefore means storing several of the
    /// same capability-identified records, not changing the control protocol.
    fn run(&self) -> hyper_os::Result<()> {
        let mut state = ManagerState::Empty;
        loop {
            state = match state {
                ManagerState::Empty => {
                    let provision = self.receive_provision()?;
                    match self.launch_instance(provision) {
                        Ok(instance) => ManagerState::Active(instance),
                        Err(failure) => {
                            failure.publish();
                            ManagerState::Empty
                        }
                    }
                }
                ManagerState::Active(instance) => {
                    let completion = Self::supervise(instance)?;
                    completion.publish();
                    ManagerState::Empty
                }
            };
        }
    }

    #[inline(never)]
    fn receive_provision(&self) -> hyper_os::Result<Provision> {
        let mut bytes = [MaybeUninit::<u8>::uninit(); vm_contract::MESSAGE_BYTES];
        let mut slots = [
            CapabilityReceiveSlot::new::<hyper_os::handle::FileObject>(
                vm_contract::PROVISIONED_IMAGE_RIGHTS,
            ),
            CapabilityReceiveSlot::new::<ByteChannelObject>(
                vm_contract::PROVISIONED_INSTANCE_CONTROL_RIGHTS,
            ),
            CapabilityReceiveSlot::new::<ConsoleObject>(vm_contract::PROVISIONED_CONSOLE_RIGHTS),
        ];
        let message =
            self.provisioning
                .receive(hyper_os::DEADLINE_INFINITE, &mut bytes, &mut slots)?;
        let request = vm_contract::ProvisionRequest::decode(message.bytes())
            .ok_or(hyper_os::Error::InvalidResponse)?;
        if message.capability_count() != request.capability_count() {
            return Err(hyper_os::Error::InvalidResponse);
        }
        let image = slots
            .get_mut(0)
            .ok_or(hyper_os::Error::InvalidResponse)?
            .take::<hyper_os::handle::FileObject>()?
            .ok_or(hyper_os::Error::MissingHandle)?;
        let control = slots
            .get_mut(1)
            .ok_or(hyper_os::Error::InvalidResponse)?
            .take::<ByteChannelObject>()?
            .ok_or(hyper_os::Error::MissingHandle)?;
        let console = match request {
            vm_contract::ProvisionRequest::LaunchInstance { console: true } => Some(
                slots
                    .get_mut(2)
                    .ok_or(hyper_os::Error::InvalidResponse)?
                    .take::<ConsoleObject>()?
                    .ok_or(hyper_os::Error::MissingHandle)?,
            ),
            vm_contract::ProvisionRequest::LaunchInstance { console: false } => None,
        };
        Ok(Provision {
            image: Some(image),
            control,
            console,
        })
    }

    #[inline(never)]
    fn launch_instance(&self, mut provision: Provision) -> Result<VmInstance, LaunchFailure> {
        match self.launch_instance_inner(&mut provision) {
            Ok(parts) => Ok(VmInstance {
                _resource_domain: parts.domain,
                _task_group: parts.group,
                runtime: parts.runtime,
                runtime_control: Some(parts.runtime_control),
                client_control: Some(provision.control),
                tracker: vm_contract::InstanceTracker::new(),
                stop: vm_contract::InstanceStopState::new(),
                exit_deadline: None,
            }),
            Err(_) => Err(LaunchFailure {
                control: provision.control,
                reason: vm_contract::InstanceFailure::Runtime,
            }),
        }
    }

    fn launch_instance_inner(&self, provision: &mut Provision) -> hyper_os::Result<InstanceParts> {
        let domain = create_resource_domain(
            self.fleet_domain.as_handle_ref(),
            hyper_app::vm_policy::INITIAL_VM_LIMITS,
        )?;
        let group = create_task_group(self.factory.as_handle_ref(), domain.as_handle_ref())?;
        let lease = hyper_os::vm::derive_creation_lease(
            self.authority.as_handle_ref(),
            domain.as_handle_ref(),
        )?;
        let (manager_runtime, runtime_control) = channel::create_pair()?;
        let builder = ProcessBuilder::create(
            self.factory.as_handle_ref(),
            group.as_handle_ref(),
            domain.as_handle_ref(),
            self.runtime_image.as_handle_ref(),
        )?;
        // The manager receives an opaque image capability and must not infer
        // guest identity or policy from the image selected by init.
        builder.set_name("vm-runtime")?;
        builder.add_argument(RUNTIME_ARGUMENT)?;
        builder.add_handle_duplicate(
            self.libraries.as_handle_ref(),
            startup::DYNAMIC_LIBRARY_DIRECTORY.as_raw(),
            RightsOffer::Exact(Rights::READ.union(Rights::EXECUTE)),
        )?;
        let image = provision
            .image
            .take()
            .ok_or(hyper_os::Error::MissingHandle)?;
        if let Err(failure) = builder.add_handle_move(
            image,
            vm_contract::RUNTIME_IMAGE_CONTRACT.purpose(),
            RightsOffer::Exact(vm_contract::RUNTIME_IMAGE_CONTRACT.required_rights()),
        ) {
            let (error, image) = failure.into_parts();
            provision.image = Some(image);
            return Err(error);
        }
        if let Some(console) = provision.console.take()
            && let Err(failure) = builder.add_handle_move(
                console,
                vm_contract::RUNTIME_CONSOLE_OUTPUT_CONTRACT.purpose(),
                RightsOffer::Exact(vm_contract::RUNTIME_CONSOLE_OUTPUT_CONTRACT.required_rights()),
            )
        {
            let (error, console) = failure.into_parts();
            provision.console = Some(console);
            return Err(error);
        }
        builder
            .add_handle_move(
                lease,
                vm_contract::RUNTIME_CREATION_LEASE_CONTRACT.purpose(),
                RightsOffer::Exact(vm_contract::RUNTIME_CREATION_LEASE_CONTRACT.required_rights()),
            )
            .map_err(|failure| failure.error())?;
        builder
            .add_handle_move(
                runtime_control,
                vm_contract::RUNTIME_INSTANCE_CONTROL_CONTRACT.purpose(),
                RightsOffer::Exact(
                    vm_contract::RUNTIME_INSTANCE_CONTROL_CONTRACT.required_rights(),
                ),
            )
            .map_err(|failure| failure.error())?;
        builder.seal()?;
        let runtime = builder.start().map_err(|failure| failure.error())?;
        Ok(InstanceParts {
            domain,
            group,
            runtime,
            runtime_control: manager_runtime,
        })
    }

    fn supervise(mut instance: VmInstance) -> hyper_os::Result<InstanceCompletion> {
        loop {
            let selected = {
                let process_wait = WaitItem::new(
                    instance.runtime.as_handle_ref(),
                    ObjectSignals::<ProcessObject>::TERMINATED,
                );
                let mut waits = [process_wait; 3];
                let mut sources = [InstanceWait::Process; 3];
                let mut count = 1usize;
                if let Some(control) = instance.client_control.as_ref() {
                    *waits
                        .get_mut(count)
                        .ok_or(hyper_os::Error::InvalidResponse)? = WaitItem::new(
                        control.as_handle_ref(),
                        ObjectSignals::<ByteChannelObject>::READABLE
                            .union(ObjectSignals::<ByteChannelObject>::PEER_CLOSED),
                    );
                    *sources
                        .get_mut(count)
                        .ok_or(hyper_os::Error::InvalidResponse)? = InstanceWait::ClientControl;
                    count += 1;
                }
                if let Some(control) = instance.runtime_control.as_ref() {
                    *waits
                        .get_mut(count)
                        .ok_or(hyper_os::Error::InvalidResponse)? = WaitItem::new(
                        control.as_handle_ref(),
                        ObjectSignals::<ByteChannelObject>::READABLE
                            .union(ObjectSignals::<ByteChannelObject>::PEER_CLOSED),
                    );
                    *sources
                        .get_mut(count)
                        .ok_or(hyper_os::Error::InvalidResponse)? = InstanceWait::RuntimeControl;
                    count += 1;
                }
                let deadline = instance.exit_deadline.map_or(
                    hyper_os::DEADLINE_INFINITE,
                    hyper_os::time::FiniteDeadline::as_raw,
                );
                let observation = match wait_many(
                    waits.get(..count).ok_or(hyper_os::Error::InvalidResponse)?,
                    deadline,
                ) {
                    Ok(observation) => observation,
                    Err(hyper_os::Error::Status(hyper_os::Status::TIMED_OUT)) => {
                        instance.grace_period_expired();
                        continue;
                    }
                    Err(error) => return Err(error),
                };
                (
                    *sources
                        .get(observation.index)
                        .ok_or(hyper_os::Error::InvalidResponse)?,
                    observation.observed,
                )
            };
            match selected {
                (InstanceWait::Process, _) => return Self::finish_instance(instance),
                (InstanceWait::ClientControl, observed)
                    if ObjectSignals::<ByteChannelObject>::READABLE.is_present_in(observed) =>
                {
                    if instance.receive_client_command().is_err() {
                        drop(instance.client_control.take());
                        instance.force_stop();
                    } else {
                        instance.request_cooperative_stop()?;
                    }
                }
                (InstanceWait::ClientControl, _) => {
                    drop(instance.client_control.take());
                    instance.force_stop();
                }
                (InstanceWait::RuntimeControl, observed)
                    if ObjectSignals::<ByteChannelObject>::READABLE.is_present_in(observed) =>
                {
                    match instance.receive_runtime_status() {
                        Ok(true) => instance.arm_exit_deadline()?,
                        Ok(false) => {}
                        Err(_) => {
                            instance.tracker.reject_protocol();
                            drop(instance.runtime_control.take());
                            instance.force_stop();
                        }
                    }
                }
                (InstanceWait::RuntimeControl, _) => {
                    drop(instance.runtime_control.take());
                    if !instance.tracker.is_terminal() {
                        instance.tracker.reject_protocol();
                        instance.force_stop();
                    }
                }
            }
        }
    }

    fn finish_instance(mut instance: VmInstance) -> hyper_os::Result<InstanceCompletion> {
        instance
            .runtime
            .as_process_supervisor()
            .wait_terminated(hyper_os::DEADLINE_INFINITE)?;
        instance.drain_runtime_statuses()?;
        let process_succeeded = matches!(
            instance.runtime.as_process_supervisor().info()?.terminal,
            Some(ProcessTermination::ProcessExited { status: 0 })
        );
        let event = instance.tracker.finish(process_succeeded);
        let control = instance.client_control.take();
        drop(instance);
        Ok(InstanceCompletion { control, event })
    }
}

/// Explicit single-instance policy state for the initial manager.
///
/// A future multi-instance manager can replace `Active` with a bounded or
/// allocated collection without changing the provisioning or per-instance
/// control contracts.
enum ManagerState {
    Empty,
    Active(VmInstance),
}

#[derive(Clone, Copy)]
enum InstanceWait {
    Process,
    ClientControl,
    RuntimeControl,
}

struct Provision {
    image: Option<OwnedHandle<hyper_os::handle::FileObject>>,
    control: OwnedHandle<ByteChannelObject>,
    console: Option<OwnedHandle<ConsoleObject>>,
}

struct InstanceParts {
    domain: OwnedHandle<ResourceDomainObject>,
    group: OwnedHandle<TaskGroupObject>,
    runtime: OwnedHandle<ProcessObject>,
    runtime_control: OwnedHandle<ByteChannelObject>,
}

struct LaunchFailure {
    control: OwnedHandle<ByteChannelObject>,
    reason: vm_contract::InstanceFailure,
}

impl LaunchFailure {
    fn publish(self) {
        let _ = self
            .control
            .as_byte_channel()
            .send(&vm_contract::InstanceEvent::Failed(self.reason).encode());
    }
}

struct InstanceCompletion {
    control: Option<OwnedHandle<ByteChannelObject>>,
    event: vm_contract::InstanceEvent,
}

impl InstanceCompletion {
    fn publish(self) {
        if let Some(control) = self.control {
            let _ = control.as_byte_channel().send(&self.event.encode());
        }
    }
}

/// Authority retained by the manager for one isolated VM service group.
struct VmInstance {
    _resource_domain: OwnedHandle<ResourceDomainObject>,
    _task_group: OwnedHandle<TaskGroupObject>,
    runtime: OwnedHandle<ProcessObject>,
    runtime_control: Option<OwnedHandle<ByteChannelObject>>,
    client_control: Option<OwnedHandle<ByteChannelObject>>,
    tracker: vm_contract::InstanceTracker,
    stop: vm_contract::InstanceStopState,
    exit_deadline: Option<hyper_os::time::FiniteDeadline>,
}

impl VmInstance {
    fn client_control(&self) -> hyper_os::Result<&OwnedHandle<ByteChannelObject>> {
        self.client_control
            .as_ref()
            .ok_or(hyper_os::Error::MissingHandle)
    }

    fn receive_client_command(&self) -> hyper_os::Result<()> {
        let mut message = [0u8; vm_contract::MESSAGE_BYTES];
        let length = self
            .client_control()?
            .as_byte_channel()
            .receive(&mut message)?;
        if vm_contract::InstanceCommand::decode(
            message
                .get(..length)
                .ok_or(hyper_os::Error::InvalidResponse)?,
        ) == Some(vm_contract::InstanceCommand::Stop)
        {
            Ok(())
        } else {
            Err(hyper_os::Error::InvalidResponse)
        }
    }

    fn receive_runtime_status(&mut self) -> hyper_os::Result<bool> {
        let mut message = [0u8; vm_contract::MESSAGE_BYTES];
        let length = self
            .runtime_control
            .as_ref()
            .ok_or(hyper_os::Error::MissingHandle)?
            .as_byte_channel()
            .receive(&mut message)?;
        let status = vm_contract::InstanceStatus::decode(
            message
                .get(..length)
                .ok_or(hyper_os::Error::InvalidResponse)?,
        )
        .ok_or(hyper_os::Error::InvalidResponse)?;
        self.tracker
            .observe(status)
            .map_err(|_| hyper_os::Error::InvalidResponse)?;
        Ok(self.tracker.is_terminal())
    }

    fn drain_runtime_statuses(&mut self) -> hyper_os::Result<()> {
        let Some(control) = self.runtime_control.as_ref() else {
            return Ok(());
        };
        loop {
            let mut message = [0u8; vm_contract::MESSAGE_BYTES];
            match control.as_byte_channel().try_receive(&mut message) {
                Ok(length) => {
                    let status = message
                        .get(..length)
                        .and_then(vm_contract::InstanceStatus::decode);
                    match status {
                        Some(status) if self.tracker.observe(status).is_ok() => {}
                        Some(_) | None => self.tracker.reject_protocol(),
                    }
                }
                Err(hyper_os::Error::Status(hyper_os::Status::WOULD_BLOCK))
                | Err(hyper_os::Error::Status(hyper_os::Status::PEER_CLOSED)) => return Ok(()),
                Err(error) => return Err(error),
            }
        }
    }

    fn request_cooperative_stop(&mut self) -> hyper_os::Result<()> {
        if self.stop.request_cooperative() != vm_contract::StopAction::SendCooperative {
            return Ok(());
        }
        match self
            .runtime_control
            .as_ref()
            .ok_or(hyper_os::Error::MissingHandle)?
            .as_byte_channel()
            .send(&vm_contract::InstanceCommand::Stop.encode())
        {
            Ok(()) => self.arm_exit_deadline(),
            Err(hyper_os::Error::Status(hyper_os::Status::PEER_CLOSED)) => {
                self.force_stop();
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    fn force_stop(&mut self) {
        self.exit_deadline = None;
        if self.stop.request_forced() == vm_contract::StopAction::ForceProcess {
            let _ = self.runtime.as_process_supervisor().request_stop();
        }
    }

    fn arm_exit_deadline(&mut self) -> hyper_os::Result<()> {
        if self.exit_deadline.is_none() {
            self.exit_deadline = Some(hyper_os::time::deadline_after(INSTANCE_EXIT_GRACE)?);
        }
        Ok(())
    }

    fn grace_period_expired(&mut self) {
        if self.tracker.is_terminal() {
            self.tracker.reject_protocol();
        }
        self.exit_deadline = None;
        if self.stop.grace_period_expired() == vm_contract::StopAction::ForceProcess {
            let _ = self.runtime.as_process_supervisor().request_stop();
        }
    }
}

hyper_rt::entry!(application_main);
