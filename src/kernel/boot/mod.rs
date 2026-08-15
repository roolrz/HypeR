use hyper::drivers::console;
use hyper::hal::memory::AddressTranslation;
use hyper::platform::{ConsoleInfo, chosen, fdt};

use self::state::BootState;
use super::{irq, log, mm};

pub(crate) mod image;
mod state;

/// State shared only between the top-level kernel initialization steps.
///
/// Keeping this type small makes dependencies between subsystem entry points
/// explicit without exposing the full early-boot state to every subsystem.
pub(crate) struct Initialization {
    essential: crate::arch::EssentialPlatformInfo,
    early_console: Option<ConsoleInfo>,
    linear_dtb: usize,
    interrupts: Option<irq::interrupt::Capabilities>,
    timer: Option<irq::timer::Capabilities>,
    cpus: Option<super::cpu::Capabilities>,
}

impl Initialization {
    pub(crate) fn essential(&self) -> &crate::arch::EssentialPlatformInfo {
        &self.essential
    }

    pub(crate) const fn linear_dtb(&self) -> usize {
        self.linear_dtb
    }

    pub(crate) const fn early_console(&self) -> Option<ConsoleInfo> {
        self.early_console
    }

    pub(crate) fn interrupts(&self) -> irq::interrupt::Capabilities {
        self.interrupts
            .unwrap_or_else(|| fail("interrupt capability access", "controller not initialized"))
    }

    pub(crate) fn set_interrupts(&mut self, capabilities: irq::interrupt::Capabilities) {
        self.interrupts = Some(capabilities);
    }

    pub(crate) fn timer(&self) -> irq::timer::Capabilities {
        self.timer
            .unwrap_or_else(|| fail("timer capability access", "timer not initialized"))
    }

    pub(crate) fn set_timer(&mut self, capabilities: irq::timer::Capabilities) {
        self.timer = Some(capabilities);
    }

    pub(crate) fn cpus(&self) -> super::cpu::Capabilities {
        self.cpus
            .unwrap_or_else(|| fail("CPU capability access", "SMP not initialized"))
    }

    pub(crate) fn set_cpus(&mut self, capabilities: super::cpu::Capabilities) {
        self.cpus = Some(capabilities);
    }
}

/// Prepares kernel state while executing on the bootstrap mapping.
pub(crate) fn prepare_boot_environment(dtb_address: usize) -> ! {
    let image_layout = image::layout();
    let (platform, essential, chosen) = discover_boot_inputs(dtb_address);
    report_chosen_errors(&chosen);
    let early_console = initialize_early_console(&platform, &chosen);
    crate::println!("HypeR: DTB at {:#x}", dtb_address);

    let kernel_base = select_kernel_base(&chosen, image_layout.total_size);
    let initial_ramdisk = select_initial_ramdisk(&platform, &chosen, dtb_address, image_layout);
    let memory = prepare_final_memory(&platform, dtb_address, initial_ramdisk, kernel_base);
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
        image_physical_start: image_layout.physical_start,
        initial_ramdisk,
    }) {
        fail("boot-state installation", error);
    }

    // SAFETY: The prepared hierarchy is retained in global boot state and the
    // activation context was issued by the active architecture implementation.
    unsafe { crate::arch::activate_memory(activation) }
}

fn discover_boot_inputs(
    dtb_address: usize,
) -> (
    hyper::platform::PlatformInfo,
    crate::arch::EssentialPlatformInfo,
    chosen::Properties,
) {
    let mut essential_discovery = crate::arch::EssentialDeviceDiscovery::new();
    let mut chosen_discovery = chosen::Discovery::new();
    let mut visitors = fdt::VisitorPair::new(&mut essential_discovery, &mut chosen_discovery);
    // SAFETY: The architectural entry receives the DTB address directly from
    // the Linux AArch64 boot ABI and has not repurposed x0.
    let platform = match unsafe { fdt::discover_with(dtb_address, &mut visitors) } {
        Ok(platform) => platform,
        Err(error) => fail("platform discovery", error),
    };
    let essential = match essential_discovery.finish() {
        Ok(essential) => essential,
        Err(error) => fail("essential-device discovery", error),
    };
    let chosen = match chosen_discovery.finish() {
        Ok(chosen) => chosen,
        Err(error) => fail("DTB /chosen discovery", error),
    };

    (platform, essential, chosen)
}

fn report_chosen_errors(chosen: &chosen::Properties) {
    if let Some(error) = chosen.command_line_error() {
        crate::pr_warn!("HypeR: ignoring invalid DTB bootargs: {error:?}");
    }
    if let Some(error) = chosen.kaslr_seed_error() {
        crate::pr_warn!("HypeR: ignoring invalid DTB kaslr-seed: {error:?}");
    }
}

fn initialize_early_console(
    platform: &hyper::platform::PlatformInfo,
    chosen: &chosen::Properties,
) -> Option<ConsoleInfo> {
    let early_console = match console::early_console(chosen.command_line()) {
        Ok(Some(info)) if early_console_is_accessible(platform, info) => Some(info),
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
    early_console
}

fn select_kernel_base(chosen: &chosen::Properties, image_size: u64) -> u64 {
    let command_line = chosen.command_line();
    let kaslr_seed = if hyper::config::RANDOMIZE_BASE
        && !command_line.is_some_and(|arguments| arguments.contains("nokaslr"))
    {
        chosen.kaslr_seed()
    } else {
        None
    };
    match crate::arch::select_kaslr_layout(kaslr_seed, image_size) {
        Ok(layout) => layout,
        Err(error) => fail("KASLR layout selection", error),
    }
    .kernel_base
}

fn select_initial_ramdisk(
    platform: &hyper::platform::PlatformInfo,
    chosen: &chosen::Properties,
    dtb_address: usize,
    image_layout: hyper::hal::memory::KernelImageLayout,
) -> hyper::platform::PhysicalRange {
    match chosen.initial_ramdisk() {
        Some(range)
            if initial_ramdisk_is_accessible(platform, range, dtb_address as u64, image_layout) =>
        {
            range
        }
        Some(range) => fail("initial ramdisk validation", range),
        None => fail("initial ramdisk discovery", "missing /chosen initrd range"),
    }
}

fn prepare_final_memory(
    platform: &hyper::platform::PlatformInfo,
    dtb_address: usize,
    initial_ramdisk: hyper::platform::PhysicalRange,
    kernel_base: u64,
) -> mm::PreparedMemory {
    // SAFETY: The architecture established and owns the bootstrap mapping used
    // by its early physical-page allocation policy.
    match unsafe { mm::memory::prepare(platform, dtb_address as u64, initial_ramdisk, kernel_base) }
    {
        Ok(memory) => memory,
        Err(error) => fail("final address-space preparation", error),
    }
}

/// Enters architecture-independent initialization on the permanent mappings.
pub(crate) fn enter_runtime() -> Initialization {
    let (essential, early_console, dtb_address) =
        with_boot_state(|state| (state.essential, state.early_console, state.dtb_address));
    let linear_dtb = match mm::memory::linear_address(dtb_address) {
        Some(address) => address,
        None => fail("DTB linear-address translation", dtb_address),
    };
    if let Some(console_info) = early_console {
        let virtual_console = match mm::memory::mmio_address(console_info.base) {
            Some(address) => address,
            None => fail("console MMIO-address translation", console_info.base),
        };
        install_console(console_info, virtual_console as u64);
    }

    Initialization {
        essential,
        early_console,
        linear_dtb,
        interrupts: None,
        timer: None,
        cpus: None,
    }
}

fn initial_ramdisk_is_accessible(
    platform: &hyper::platform::PlatformInfo,
    range: hyper::platform::PhysicalRange,
    dtb_address: u64,
    image: hyper::hal::memory::KernelImageLayout,
) -> bool {
    let in_ram = platform
        .memory
        .as_slice()
        .iter()
        .any(|memory| memory.start() <= range.start() && range.end() <= memory.end());
    let excluded = platform
        .no_map
        .as_slice()
        .iter()
        .any(|reserved| reserved.overlaps(range));
    let overlaps_dtb = hyper::platform::PhysicalRange::new(dtb_address, platform.dtb_size)
        .is_none_or(|dtb| dtb.overlaps(range));
    let overlaps_image =
        hyper::platform::PhysicalRange::new(image.physical_start, image.total_size)
            .is_none_or(|kernel| kernel.overlaps(range));
    in_ram && !excluded && !overlaps_dtb && !overlaps_image
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
        fail("console address translation", address);
    };
    // SAFETY: Platform discovery validated the device range and the
    // architecture supplied an address with Device memory attributes.
    log::console::install(unsafe { console::bind(console_info, base) });
}

pub(crate) fn fail(operation: &str, error: impl core::fmt::Debug) -> ! {
    crate::pr_crit!("HypeR: {operation} failed: {error:?}");
    crate::arch::halt()
}

pub(crate) fn with_boot_state<R>(operation: impl FnOnce(&BootState) -> R) -> R {
    match state::with(operation) {
        Ok(result) => result,
        Err(error) => fail("boot-state access", error),
    }
}
