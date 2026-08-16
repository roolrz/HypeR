//! Architecture-independent hardware interface descriptions.
//!
//! This layer contains register layouts and protocol constants only. Physical
//! drivers and virtual-device models may both depend on it, but it owns no
//! device state, probing, or kernel policy.

pub mod pl011;
