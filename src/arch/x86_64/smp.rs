use core::arch::asm;
use hyper::sync::atomic::{AtomicBool, AtomicU64, Ordering};

const MAX_CPUS: usize = hyper::config::MAX_CPUS as usize;
static APIC_IDS: [AtomicU64; MAX_CPUS] = [const { AtomicU64::new(u64::MAX) }; MAX_CPUS];
static ONLINE: [AtomicBool; MAX_CPUS] = [const { AtomicBool::new(false) }; MAX_CPUS];

#[repr(C, align(64))]
pub struct SecondaryBootParameters {
    pub root: u64,
    pub physical_stack_top: u64,
    pub virtual_stack_top: u64,
    pub cpu_index: u64,
    pub rust_entry: u64,
}

impl SecondaryBootParameters {
    pub fn new(
        root: u64,
        physical_stack_top: u64,
        virtual_stack_top: u64,
        cpu_index: usize,
    ) -> Self {
        Self {
            root,
            physical_stack_top,
            virtual_stack_top,
            cpu_index: cpu_index as u64,
            rust_entry: x86_64_secondary_rust_entry as *const () as usize as u64,
        }
    }
}

extern "C" fn x86_64_secondary_rust_entry(cpu_index: usize) -> ! {
    super::exception::install_local_vectors();
    crate::start_secondary_cpu(cpu_index)
}

pub const fn secondary_entry_physical(_image_start: u64, _kernel_base: u64) -> Option<u64> {
    Some(0x8000)
}

pub fn current_cpu_index() -> usize {
    let hardware_id = current_hardware_id();
    APIC_IDS
        .iter()
        .position(|id| id.load(Ordering::Acquire) == hardware_id)
        .unwrap_or(usize::MAX)
}

pub fn current_hardware_id() -> u64 {
    let leaf = core::arch::x86_64::__cpuid_count(0xb, 0);
    u64::from(leaf.edx)
}

pub fn initialize_boot_cpu() {
    APIC_IDS[0].store(current_hardware_id(), Ordering::Release);
    ONLINE[0].store(true, Ordering::Release);
}

pub fn register_cpu(cpu_index: usize, hardware_id: u64) -> bool {
    APIC_IDS.get(cpu_index).is_some_and(|slot| {
        slot.store(hardware_id, Ordering::Release);
        true
    })
}

pub fn mark_current_cpu_online() {
    if let Some(slot) = ONLINE.get(current_cpu_index()) {
        slot.store(true, Ordering::Release);
    }
}

pub fn send_event() {
    // SAFETY: MFENCE has no pointer operands and publishes state before polling CPUs.
    unsafe { asm!("mfence", options(nostack)) };
}
