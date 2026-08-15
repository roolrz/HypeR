//! Exception, interrupt-domain, and kernel-timer policy.

pub(crate) mod exception;
pub(crate) mod interrupt;
pub(crate) mod timer;
pub(crate) mod vgic;

/// Initializes the root interrupt controller and kernel IRQ domain.
pub(crate) fn initialize_controller(boot: &mut super::boot::Initialization) {
    let info = match boot.essential().interrupt_controller {
        Some(info) => info,
        None => super::boot::fail("interrupt-controller discovery", "missing controller"),
    };
    let capabilities = match interrupt::initialize(info) {
        Ok(capabilities) => capabilities,
        Err(error) => super::boot::fail("interrupt-controller initialization", error),
    };
    crate::println!(
        "HypeR: GICv3 initialized with {} interrupt IDs; local IRQs remain masked",
        capabilities.interrupt_count
    );
    boot.set_interrupts(capabilities);
}

/// Installs the permanent exception vectors and validates guest trap handling.
pub(crate) fn initialize_exceptions() {
    // SAFETY: The final RX kernel mapping, stack, console, and interrupt
    // controller are active. IRQ delivery remains masked until timer setup.
    unsafe { crate::arch::install_runtime_vectors() };
    if let Err(error) = crate::arch::validate_runtime_vectors() {
        super::boot::fail("runtime exception-vector validation", error);
    }
    if let Err(error) = crate::arch::validate_vsysreg() {
        super::boot::fail("guest system-register emulation validation", error);
    }
    crate::println!("HypeR: guest synchronous trap and vSysReg emulation validated");
}

/// Activates the interrupt-controller virtualization backend.
pub(crate) fn initialize_virtualization(boot: &super::boot::Initialization) {
    let interrupts = boot.interrupts();
    let capabilities =
        match vgic::initialize(interrupts.root_domain, interrupts.maintenance_interrupt) {
            Ok(capabilities) => capabilities,
            Err(error) => super::boot::fail("vGIC initialization", error),
        };
    crate::println!(
        "HypeR: vGICv3 active with {} LRs, {} priority bits, {} preemption bits, {} INTID bits, maintenance VIRQ {}",
        capabilities.list_registers,
        capabilities.priority_bits,
        capabilities.preemption_bits,
        capabilities.interrupt_id_bits,
        capabilities.maintenance_interrupt.get()
    );
}

/// Starts the periodic kernel timer and publishes its guest-visible mapping.
pub(crate) fn initialize_timer(boot: &mut super::boot::Initialization) {
    let info = match boot.essential().timer {
        Some(info) => info,
        None => super::boot::fail("timer discovery", "missing architectural timer"),
    };
    let capabilities = match timer::initialize(info, boot.interrupts().root_domain) {
        Ok(capabilities) => capabilities,
        Err(error) => super::boot::fail("periodic timer initialization", error),
    };
    crate::println!(
        "HypeR: Arm Generic Timer: EL2 INTID {}, guest virtual INTID {} (host VIRQ {}), {} Hz tick from a {} Hz counter",
        capabilities.hardware_interrupt.get(),
        capabilities.guest_virtual_interrupt.get(),
        capabilities.guest_virtual_host_interrupt.get(),
        capabilities.ticks_per_second,
        capabilities.counter_frequency_hz
    );
    crate::println!(
        "HypeR: timer mapped to dynamic VIRQ {}",
        capabilities.virtual_interrupt.get()
    );
    boot.set_timer(capabilities);
}

/// Updates IRQ/timer policy after all secondary CPUs are online.
pub(crate) fn publish_online_cpu_count(boot: &super::boot::Initialization) {
    if let Err(error) = timer::set_online_cpu_count(boot.cpus().online_cpus) {
        super::boot::fail("timer CPU-count publication", error);
    }
}
