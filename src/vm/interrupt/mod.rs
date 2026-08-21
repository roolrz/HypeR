//! Architecture-neutral virtual interrupt-controller services.

mod controller;
pub mod gicv3;

pub use controller::{
    Error, InterruptGroup, InterruptSnapshot, InterruptTrigger, ListEntry, ListState, VirtualCpuId,
    VirtualInterruptController, VirtualInterruptId,
};
