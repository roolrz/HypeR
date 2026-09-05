// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Runtime acceptance probe for guest-to-host Fair preemption.
//!
//! Installation leaves one CPU0-affined Fair Thread ready while startup
//! creates the boot vCPU. Each yield transfers from the probe to that vCPU;
//! the probe can resume only after the physical timer takes the guest through
//! the private-stack IRQ-tail continuation. Repeating the handoff catches
//! one-shot context preservation mistakes without retaining a permanent test
//! workload after the marker is emitted.

use hyper::cpu::CpuIndex;

use crate::kernel::task::scheduler::{self, CpuMask};

const PREEMPTION_ROUNDS: usize = 3;

#[cfg(CONFIG_ARCH_AARCH64)]
const ARCHITECTURE: &str = "AArch64";
#[cfg(CONFIG_ARCH_RISCV64)]
const ARCHITECTURE: &str = "RISC-V";

pub(super) fn install() -> Result<(), scheduler::Error> {
    let probe = scheduler::kthread_create_with_affinity(
        "test/guest-irq-tail",
        run_probe,
        0,
        CpuMask::single(CpuIndex::BOOT),
    )?;
    scheduler::thread_ready(probe)?;
    Ok(())
}

extern "C" fn run_probe(_argument: usize) {
    for _ in 0..PREEMPTION_ROUNDS {
        if let Err(error) = scheduler::yield_now() {
            crate::pr_crit!("HypeR test: {ARCHITECTURE} IRQ-tail probe yield failed: {error:?}");
            crate::hal::cpu::halt()
        }
    }
    crate::println!("HypeR test: {ARCHITECTURE} IRQ-tail Fair vCPU preemption passed");
}
