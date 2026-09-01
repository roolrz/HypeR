// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! vCPU scheduler runner and local hardware-transition facade.

mod lifecycle;
mod runner;
mod transition;

use super::{active_vcpu, memory, registry, timer};

pub use crate::hal::vm::VcpuInterruptError;
#[allow(unused_imports)]
pub(crate) use lifecycle::{DetachedStopError, complete_detached_stop_if_requested};
pub use runner::RunError;
pub(super) use runner::create_thread;
pub use transition::HardwareTransitionError;
pub(crate) use transition::{activate, deactivate};
#[allow(unused_imports)]
pub(crate) use transition::{
    current_administrative_stop_requested, current_interrupt_reconcile_pending,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(dead_code)]
pub(crate) enum ReconcileObservationError {
    Active(active_vcpu::Error),
    Registry(registry::Error),
    Scheduler(crate::kernel::task::scheduler::Error),
}
