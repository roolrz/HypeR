use core::arch::asm;
use core::ptr::addr_of;
use hyper::sync::atomic::{AtomicBool, AtomicU64, Ordering};

const MAX_CPUS: usize = hyper::config::MAX_CPUS as usize;
static HART_IDS: [AtomicU64; MAX_CPUS] = [const { AtomicU64::new(u64::MAX) }; MAX_CPUS];
static HART_ONLINE: [AtomicBool; MAX_CPUS] = [const { AtomicBool::new(false) }; MAX_CPUS];

unsafe extern "C" {
    static riscv64_secondary_entry: u8;
}

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
            rust_entry: crate::start_secondary_cpu as *const () as usize as u64,
        }
    }
}

pub fn secondary_entry_physical(image_start: u64, kernel_base: u64) -> Option<u64> {
    let offset = (addr_of!(riscv64_secondary_entry) as u64).checked_sub(kernel_base)?;
    image_start.checked_add(offset)
}

pub fn current_cpu_index() -> usize {
    let value: usize;
    unsafe { asm!("mv {value}, tp", value = out(reg) value, options(nomem, nostack)) };
    value
}

pub fn current_hardware_id() -> u64 {
    HART_IDS
        .get(current_cpu_index())
        .map(|id| id.load(Ordering::Acquire))
        .unwrap_or(u64::MAX)
}

pub fn initialize_boot_hart(hart_id: u64) {
    unsafe { asm!("mv tp, zero", options(nomem, nostack)) };
    HART_IDS[0].store(hart_id, Ordering::Release);
    HART_ONLINE[0].store(true, Ordering::Release);
}

pub fn mark_current_hart_online() {
    if let Some(online) = HART_ONLINE.get(current_cpu_index()) {
        online.store(true, Ordering::Release);
    }
}

pub fn for_each_online_remote_hart(
    mut operation: impl FnMut(u64) -> Result<(), super::sbi::Error>,
) -> Result<(), super::sbi::Error> {
    let current = current_cpu_index();
    for (cpu, online) in HART_ONLINE.iter().enumerate() {
        if cpu == current || !online.load(Ordering::Acquire) {
            continue;
        }
        let hart_id = HART_IDS[cpu].load(Ordering::Acquire);
        if hart_id != u64::MAX {
            operation(hart_id)?;
        }
    }
    Ok(())
}

pub fn register_hart(cpu_index: usize, hart_id: u64) -> bool {
    HART_IDS.get(cpu_index).is_some_and(|slot| {
        slot.store(hart_id, Ordering::Release);
        true
    })
}

pub fn send_event() {
    unsafe { asm!("fence iorw, iorw", options(nostack)) };
    let _ = for_each_online_remote_hart(super::sbi::send_ipi);
}
