use hyper::drivers::console;
use hyper::hal::barrier::{Barrier, BarrierAccess, BarrierDomain};
use hyper::hal::cache::CacheMaintenance;
use hyper::hal::memory::AddressTranslation;
use hyper::platform::{ConsoleInfo, chosen, fdt};

use alloc::boxed::Box;
use alloc::vec::Vec;

use self::state::BootState;
use super::{cpu, device, irq, log, mm, task, time};

pub(crate) mod image;
mod state;

/// Architecture-independent boot orchestration on the bootstrap mapping.
pub fn boot(dtb_address: usize) -> ! {
    let image_physical_start = image::layout().physical_start;
    let mut essential_discovery = crate::arch::EssentialDeviceDiscovery::new();
    let mut chosen_discovery = chosen::Discovery::new();
    let mut visitors = fdt::VisitorPair::new(&mut essential_discovery, &mut chosen_discovery);
    // SAFETY: The architectural entry receives the DTB address directly from
    // the Linux AArch64 boot ABI and has not repurposed x0.
    let platform = match unsafe { fdt::discover_with(dtb_address, &mut visitors) } {
        Ok(platform) => platform,
        Err(error) => boot_failure("platform discovery", error),
    };
    let essential = match essential_discovery.finish() {
        Ok(essential) => essential,
        Err(error) => boot_failure("essential-device discovery", error),
    };
    let chosen = match chosen_discovery.finish() {
        Ok(chosen) => Some(chosen),
        Err(error) => {
            crate::pr_warn!("HypeR: ignoring invalid DTB /chosen properties: {error:?}");
            None
        }
    };
    let early_console =
        match console::early_console(chosen.as_ref().and_then(chosen::Properties::command_line)) {
            Ok(Some(info)) if early_console_is_accessible(&platform, info) => Some(info),
            Ok(Some(info)) => {
                crate::pr_warn!(
                    "HypeR: ignoring inaccessible early console at {:#x}",
                    info.base
                );
                None
            }
            Ok(None) => None,
            Err(error) => {
                crate::pr_warn!("HypeR: ignoring invalid earlycon argument: {error:?}");
                None
            }
        };

    if let Some(console_info) = early_console {
        install_console(console_info, console_info.base);
        crate::println!("HypeR: early console initialized");
    }
    crate::println!("HypeR: DTB at {:#x}", dtb_address);

    let command_line = chosen.as_ref().and_then(chosen::Properties::command_line);
    let kaslr_seed = if hyper::config::RANDOMIZE_BASE
        && !command_line.is_some_and(|arguments| arguments.contains("nokaslr"))
    {
        chosen.as_ref().and_then(chosen::Properties::kaslr_seed)
    } else {
        None
    };
    let kaslr = match crate::arch::select_kaslr_layout(kaslr_seed, image::layout().total_size) {
        Ok(layout) => layout,
        Err(error) => boot_failure("KASLR layout selection", error),
    };

    // SAFETY: The architecture established and owns the bootstrap mapping used
    // by its early physical-page allocation policy.
    let memory =
        match unsafe { mm::memory::prepare(&platform, dtb_address as u64, kaslr.kernel_base) } {
            Ok(memory) => memory,
            Err(error) => boot_failure("final address-space preparation", error),
        };
    let activation = memory.activation_context();

    crate::println!(
        "HypeR: discovered {} RAM and {} MMIO regions",
        platform.memory.len(),
        platform.mmio.len()
    );
    crate::println!(
        "HypeR: root page table at {:#x}; switching to kernel VA {:#x}",
        memory.root_address(),
        memory.kernel_base()
    );

    if let Err(error) = state::install(BootState {
        platform,
        essential,
        early_console,
        memory,
        dtb_address: dtb_address as u64,
        image_physical_start,
    }) {
        boot_failure("boot-state installation", error);
    }

    // SAFETY: The prepared hierarchy is retained in global boot state and the
    // activation context was issued by the active architecture implementation.
    unsafe { crate::arch::activate_memory(activation) }
}

/// Continues initialization after execution and the stack move to kernel VAs.
pub fn finish_boot() -> ! {
    let (essential, early_console, dtb_address) =
        with_boot_state(|state| (state.essential, state.early_console, state.dtb_address));
    let cpu_power_info = essential.cpu_power;
    let interrupt_controller_info = essential.interrupt_controller;
    let timer_info = essential.timer;
    let linear_dtb = match mm::memory::linear_address(dtb_address) {
        Some(address) => address,
        None => boot_failure("DTB linear-address translation", dtb_address),
    };
    if let Some(console_info) = early_console {
        let virtual_console = match mm::memory::mmio_address(console_info.base) {
            Some(address) => address,
            None => boot_failure("console MMIO-address translation", console_info.base),
        };
        install_console(console_info, virtual_console as u64);
    }

    if let Err(error) = with_boot_state(|state| {
        state
            .memory
            .initialize_global_allocator(&mm::allocator::GLOBAL_ALLOCATOR)
    }) {
        boot_failure("global allocator initialization", error);
    }
    if let Err(error) = verify_rust_allocator_interface() {
        boot_failure("Rust allocator interface validation", error);
    }
    let kallsyms_lookup_address = crate::kernel::debug::kallsyms::lookup as *const () as usize;
    let kallsyms_symbol = match crate::kernel::debug::kallsyms::lookup(kallsyms_lookup_address) {
        Ok(Some(symbol)) => symbol,
        Ok(None) => boot_failure("kallsyms self lookup", "symbol not found"),
        Err(error) => boot_failure("kallsyms self lookup", error),
    };
    if kallsyms_symbol.name != "hyper_kallsyms_lookup" || kallsyms_symbol.offset != 0 {
        boot_failure("kallsyms self lookup", kallsyms_symbol);
    }
    crate::println!(
        "HypeR: kallsyms resolved {} at {:#x}",
        kallsyms_symbol.name,
        kallsyms_symbol.address
    );
    let scheduler_capabilities = match task::scheduler::initialize() {
        Ok(capabilities) => capabilities,
        Err(error) => boot_failure("scheduler initialization", error),
    };
    crate::println!(
        "HypeR: scheduler active on bootstrap thread {}",
        scheduler_capabilities.bootstrap_thread.get()
    );
    let cpu_power_info = match cpu_power_info {
        Some(info) => info,
        None => boot_failure("CPU power discovery", "missing firmware interface"),
    };
    let cpu_power_capabilities = match device::cpu_power::initialize(cpu_power_info) {
        Ok(capabilities) => capabilities,
        Err(error) => boot_failure("CPU power initialization", error),
    };

    let interrupt_controller_info = match interrupt_controller_info {
        Some(info) => info,
        None => boot_failure("interrupt-controller discovery", "missing controller"),
    };
    let interrupt_capabilities = match irq::interrupt::initialize(interrupt_controller_info) {
        Ok(capabilities) => capabilities,
        Err(error) => boot_failure("interrupt-controller initialization", error),
    };

    // SAFETY: The final RX kernel mapping, stack, console, and interrupt
    // controller are active. IRQ delivery remains masked until timer setup.
    unsafe { crate::arch::install_runtime_vectors() };
    if let Err(error) = crate::arch::validate_runtime_vectors() {
        boot_failure("runtime exception-vector validation", error);
    }
    let vgic_capabilities = match irq::vgic::initialize(
        interrupt_capabilities.root_domain,
        interrupt_capabilities.maintenance_interrupt,
    ) {
        Ok(capabilities) => capabilities,
        Err(error) => boot_failure("vGIC initialization", error),
    };
    crate::println!(
        "HypeR: vGICv3 active with {} LRs, {} priority bits, {} preemption bits, {} INTID bits, maintenance VIRQ {}",
        vgic_capabilities.list_registers,
        vgic_capabilities.priority_bits,
        vgic_capabilities.preemption_bits,
        vgic_capabilities.interrupt_id_bits,
        vgic_capabilities.maintenance_interrupt.get()
    );
    let time_capabilities = match time::initialize() {
        Ok(capabilities) => capabilities,
        Err(error) => boot_failure("monotonic timekeeping initialization", error),
    };
    let timer_info = match timer_info {
        Some(info) => info,
        None => boot_failure("timer discovery", "missing architectural timer"),
    };
    let timer_capabilities =
        match irq::timer::initialize(timer_info, interrupt_capabilities.root_domain) {
            Ok(capabilities) => capabilities,
            Err(error) => boot_failure("periodic timer initialization", error),
        };
    if let Err(error) = super::vm::validate_arch_timer(timer_capabilities.guest_virtual_interrupt) {
        boot_failure("virtual architected timer validation", error);
    }
    crate::println!("HypeR: virtual architected timer injection validated");

    let platform_bus_report = match device::platform_bus::initialize(linear_dtb, essential.claims())
    {
        Ok(report) => report,
        Err(error) => boot_failure("platform-bus initialization", error),
    };

    let smp_capabilities = match with_boot_state(|state| {
        cpu::initialize(
            &state.platform,
            state.memory.root_address(),
            state.image_physical_start,
            state.memory.kernel_base(),
        )
    }) {
        Ok(capabilities) => capabilities,
        Err(error) => boot_failure("SMP initialization", error),
    };
    if let Err(error) = irq::timer::set_online_cpu_count(smp_capabilities.online_cpus) {
        boot_failure("timer CPU-count publication", error);
    }
    if let Err(error) =
        with_boot_state(|state| state.memory.retire_identity_mappings(&state.platform))
    {
        boot_failure("identity-map retirement", error);
    }

    let layout = mm::memory::virtual_memory_layout();
    crate::arch::ArchitectureBarrier::data_memory(BarrierDomain::FullSystem, BarrierAccess::All);
    let data_cache_line = crate::arch::ArchitectureCache::data_line_size();
    let instruction_cache_line = crate::arch::ArchitectureCache::instruction_line_size();
    let atomic_capabilities: crate::arch::AtomicCapabilities = crate::arch::atomic_capabilities();
    with_boot_state(|state| {
        crate::println!("HypeR: final address space active");
        crate::println!("HypeR: transition identity mappings retired");
        crate::println!(
            "HypeR: SMP online: {}/{} discovered CPUs",
            smp_capabilities.online_cpus,
            smp_capabilities.discovered_cpus
        );
        crate::println!("HypeR: linear map base {:#x}", layout.linear_base);
        crate::println!("HypeR: MMIO map base {:#x}", layout.mmio_base);
        crate::println!(
            "HypeR: randomized kernel base {:#x}, KASLR offset {:#x}",
            state.memory.kernel_base(),
            state.memory.kernel_base() - layout.kernel_base
        );
        crate::println!("HypeR: DTB physical address {:#x}", state.dtb_address);
        crate::println!(
            "HypeR: cache line sizes: data {} bytes, instruction {} bytes",
            data_cache_line,
            instruction_cache_line
        );
        crate::println!(
            "HypeR: atomic RMW backend: {}",
            if atomic_capabilities.lse {
                "LSE"
            } else {
                "LL/SC"
            }
        );
        crate::println!(
            "HypeR: GICv3 initialized with {} interrupt IDs; local IRQs remain masked",
            interrupt_capabilities.interrupt_count
        );
        crate::println!(
            "HypeR: Arm Generic Timer: EL2 INTID {}, guest virtual INTID {} (host VIRQ {}), {} Hz tick from a {} Hz counter",
            timer_capabilities.hardware_interrupt.get(),
            timer_capabilities.guest_virtual_interrupt.get(),
            timer_capabilities.guest_virtual_host_interrupt.get(),
            timer_capabilities.ticks_per_second,
            timer_capabilities.counter_frequency_hz
        );
        crate::println!(
            "HypeR: monotonic clocksource active at {} Hz",
            time_capabilities.counter_frequency_hz
        );
        crate::println!(
            "HypeR: CPU power interface version {}.{}: on={}, off={}, suspend={}, reset={}",
            cpu_power_capabilities.version.major,
            cpu_power_capabilities.version.minor,
            cpu_power_capabilities.cpu_on,
            cpu_power_capabilities.cpu_off,
            cpu_power_capabilities.cpu_suspend,
            cpu_power_capabilities.system_reset
        );
        crate::println!(
            "HypeR: timer mapped to dynamic VIRQ {}",
            timer_capabilities.virtual_interrupt.get()
        );
        crate::println!(
            "HypeR: platform bus: {} bound, {} unmatched, {} deferred, {} failed",
            platform_bus_report.bound,
            platform_bus_report.unmatched,
            platform_bus_report.deferred,
            platform_bus_report.failed
        );
        crate::println!(
            "HypeR: {} boot reservations, {} RAM regions, root {:#x}",
            state.memory.reservation_count(),
            state.platform.memory.len(),
            state.memory.root_address()
        );
    });
    if let Some(stats) = mm::allocator::GLOBAL_ALLOCATOR.stats() {
        crate::println!(
            "HypeR: global buddy/slab allocator active: {} free pages, {} live allocations",
            stats.free_pages,
            stats.live_allocations
        );
    }
    let log_statistics = log::statistics();
    crate::println!(
        "HypeR: kernel log ring: {} bytes, {} records dropped",
        log_statistics.capacity,
        log_statistics.dropped
    );
    crate::println!("HypeR: kernel initialization complete; bootstrap thread becoming idle");
    crate::arch::enable_local_irq();
    task::scheduler::thread_become_idle()
}

fn early_console_is_accessible(
    platform: &hyper::platform::PlatformInfo,
    info: ConsoleInfo,
) -> bool {
    const MINIMUM_REGISTER_WINDOW: u64 = 0x1000;
    let Some(end) = info.base.checked_add(MINIMUM_REGISTER_WINDOW) else {
        return false;
    };
    if end > crate::arch::ArchitectureAddressTranslation::bootstrap_accessible_limit() {
        return false;
    }
    platform
        .mmio
        .as_slice()
        .iter()
        .any(|range| range.start() <= info.base && end <= range.end())
}

fn install_console(console_info: ConsoleInfo, address: u64) {
    let Ok(base) = usize::try_from(address) else {
        boot_failure("console address translation", address);
    };
    // SAFETY: Platform discovery validated the device range and the
    // architecture supplied an address with Device memory attributes.
    log::console::install(unsafe { console::bind(console_info, base) });
}

fn boot_failure(operation: &str, error: impl core::fmt::Debug) -> ! {
    crate::pr_crit!("HypeR: {operation} failed: {error:?}");
    crate::arch::halt()
}

fn with_boot_state<R>(operation: impl FnOnce(&BootState) -> R) -> R {
    match state::with(operation) {
        Ok(result) => result,
        Err(error) => boot_failure("boot-state access", error),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AllocatorSmokeError {
    BoxValue,
    VectorContents,
    VectorLength,
}

fn verify_rust_allocator_interface() -> Result<(), AllocatorSmokeError> {
    let boxed = Box::new(0x0048_5950_4552_u64);
    let mut vector = Vec::with_capacity(1024);
    for value in 0..1024u64 {
        vector.push(value);
    }
    if *boxed != 0x0048_5950_4552 {
        return Err(AllocatorSmokeError::BoxValue);
    }
    if vector.len() != 1024 {
        return Err(AllocatorSmokeError::VectorLength);
    }
    if vector.get(1023) != Some(&1023) {
        return Err(AllocatorSmokeError::VectorContents);
    }
    drop(vector);
    drop(boxed);
    Ok(())
}
