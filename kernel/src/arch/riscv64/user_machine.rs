// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Sv39 Native roots and CPU-local translation transitions.
//! Policy retains table storage, logical identifiers, and residency. Kernel
//! upper tables are shared, supervisor-only, and retained for the machine life.

use super::registers;
use core::arch::asm;
use core::marker::PhantomData;
use core::ptr::{read_volatile, write_volatile};
use hyper::mm::PhysicalAddress;
use hyper::sync::atomic::{AtomicU8, AtomicU64, Ordering};

const USER_LIMIT: u64 = 1 << 38;
const ASID_SHIFT: u64 = 44;
const ASID_MASK: u64 = 0xffff << ASID_SHIFT;
const PPN_MASK: u64 = (1 << 44) - 1;
const PRIVILEGED_ACCESS: u64 = (1 << 18) | (1 << 19);
static ASID_BITS: AtomicU8 = AtomicU8::new(16);
static KERNEL_ROOT: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Error {
    Allocation,
    InvalidAddress,
    InvalidRange,
    Conflict,
    NotInitialized,
    InvalidLocalState,
}
pub(crate) type ContractError = Error;

/// Probes one hart before Native execution is published. SMP admission completes
/// before the first process is built, making the minimum immutable thereafter.
/// Essential-device discovery rejects every enabled CPU without the complete
/// RV64IMAFDC + Zicsr/Zifencei/H/Sstc/Zicbom contract.
pub(super) fn discover_local() -> bool {
    let previous: u64;
    let probed: u64;
    // SAFETY: Boot admission keeps interrupts masked and no Native roots exist.
    // Only the ASID changes; the live kernel PPN and Sv39 mode remain intact.
    unsafe {
        asm!(
        "csrr {previous}, satp",
        "or {probe}, {previous}, {mask}",
        "csrw satp, {probe}",
        "csrr {probe}, satp",
        "csrw satp, {previous}",
        "sfence.vma",
        previous = out(reg) previous, probe = out(reg) probed,
        mask = in(reg) ASID_MASK, options(nostack))
    };
    if previous >> 60 != 8 {
        return false;
    }
    let bits = ((probed & ASID_MASK) >> ASID_SHIFT).count_ones() as u8;
    ASID_BITS.fetch_min(bits, Ordering::AcqRel);
    let root = previous & PPN_MASK;
    match KERNEL_ROOT.compare_exchange(0, root, Ordering::Release, Ordering::Acquire) {
        Ok(_) => true,
        Err(existing) => existing == root,
    }
}

pub(crate) fn identifier_bits() -> Result<u8, Error> {
    ensure_initialized()?;
    // Untagged implementations still need software allocation generations.
    let bits = ASID_BITS.load(Ordering::Acquire);
    Ok(if bits == 0 { 8 } else { bits.min(8) })
}
pub(crate) fn user_address_limit() -> Result<u64, Error> {
    ensure_initialized()?;
    Ok(USER_LIMIT)
}
pub(crate) fn assert_kernel_access() -> Result<(), Error> {
    ensure_initialized()?;
    // SAFETY: Clearing SUM/MXR denies supervisor access to U pages and prevents
    // executable-only mappings from becoming readable. Copies use linear aliases.
    unsafe { asm!("csrc sstatus, {bits}", bits = in(reg) PRIVILEGED_ACCESS, options(nostack)) };
    Ok(())
}
fn ensure_initialized() -> Result<(), Error> {
    if KERNEL_ROOT.load(Ordering::Acquire) == 0 {
        Err(Error::NotInitialized)
    } else {
        Ok(())
    }
}
fn hardware_identifier(identifier: u16) -> u64 {
    if ASID_BITS.load(Ordering::Acquire) == 0 {
        0
    } else {
        u64::from(identifier)
    }
}

#[derive(Clone, Copy)]
pub(crate) struct MappingPage {
    pub(crate) address: u64,
    pub(crate) physical: PhysicalAddress,
    pub(crate) readable: bool,
    pub(crate) writable: bool,
    pub(crate) executable: bool,
}
#[derive(Clone, Copy)]
pub(crate) struct PreparedAddressSpace {
    root: u64,
    identifier: u16,
    generation: u64,
}
#[derive(Clone, Copy)]
pub(crate) enum LocalOperation {
    Replace,
    Invalidate,
}
#[derive(Clone, Copy)]
pub(crate) struct LocalRequest {
    root: PreparedAddressSpace,
    operation: LocalOperation,
}
#[derive(Clone, Copy)]
pub(crate) struct LocalIdentity {
    root: PreparedAddressSpace,
}
pub(crate) struct LocalActivation {
    previous_root: u64,
    _not_send: PhantomData<*mut ()>,
}
struct LocalOwner {
    root: AtomicU64,
    identifier: AtomicU64,
    generation: AtomicU64,
}
impl LocalOwner {
    const fn new() -> Self {
        Self {
            root: AtomicU64::new(0),
            identifier: AtomicU64::new(0),
            generation: AtomicU64::new(0),
        }
    }
    fn publish(&self, root: &PreparedAddressSpace) {
        self.identifier
            .store(u64::from(root.identifier), Ordering::Relaxed);
        self.generation.store(root.generation, Ordering::Relaxed);
        self.root.store(root.root, Ordering::Release);
    }
}
static LOCAL: [LocalOwner; hyper::config::MAX_CPUS as usize] =
    [const { LocalOwner::new() }; hyper::config::MAX_CPUS as usize];
fn local() -> &'static LocalOwner {
    match LOCAL.get(super::current_cpu_index()) {
        Some(local) => local,
        None => super::halt(),
    }
}
impl PreparedAddressSpace {
    pub(crate) fn root_register(&self) -> u64 {
        registers::SATP_MODE_SV39 | (hardware_identifier(self.identifier) << ASID_SHIFT) | self.root
    }
    pub(crate) const fn local_request(&self, operation: LocalOperation) -> LocalRequest {
        LocalRequest {
            root: *self,
            operation,
        }
    }
    pub(crate) const fn local_identity(&self) -> LocalIdentity {
        LocalIdentity { root: *self }
    }
}

/// Builds a private user hierarchy beneath shared, supervisor-only kernel slots.
///
/// # Safety
/// Every allocator result is a new zeroed page, exclusively transferred to this
/// builder and retained through acknowledged retirement. Mapping owners keep
/// each physical user page live. Kernel root slots are frozen after boot.
pub(crate) unsafe fn prepare_host(
    identifier: u16,
    generation: u64,
    mut enumerate: impl FnMut(&mut dyn FnMut(MappingPage)),
    allocator: &mut impl FnMut(usize) -> Option<PhysicalAddress>,
) -> Result<PreparedAddressSpace, Error> {
    let bits = identifier_bits()?;
    if identifier == 0 || u32::from(identifier) >= 1u32 << bits || generation == 0 {
        return Err(Error::InvalidRange);
    }
    let root = allocator(0).ok_or(Error::Allocation)?;
    validate_table(root)?;
    let kernel = PhysicalAddress::new(KERNEL_ROOT.load(Ordering::Acquire) << 12);
    for slot in 256..512 {
        let entry = read_entry(kernel, slot)?;
        if entry & registers::PTE_USER != 0 {
            return Err(Error::InvalidAddress);
        }
        write_entry(root, slot, entry)?;
    }
    let mut result = Ok(());
    enumerate(&mut |page| {
        if result.is_ok() {
            result = map_page(root, page, allocator);
        }
    });
    result?;
    // SAFETY: Publish descriptor stores before another hart can acquire a root.
    unsafe { asm!("fence rw, rw", options(nostack)) };
    Ok(PreparedAddressSpace {
        root: root.get() >> 12,
        identifier,
        generation,
    })
}
fn map_page(
    root: PhysicalAddress,
    page: MappingPage,
    allocator: &mut impl FnMut(usize) -> Option<PhysicalAddress>,
) -> Result<(), Error> {
    if page.address >= USER_LIMIT
        || page.address & 4095 != 0
        || page.physical.get() & 4095 != 0
        || page.physical.get() >= registers::PHYSICAL_ADDRESS_LIMIT
        || (page.writable && !page.readable)
    {
        return Err(Error::InvalidAddress);
    }
    if !page.readable && !page.executable {
        return Ok(());
    }
    let mut table = root;
    for shift in [30, 21] {
        let slot = ((page.address >> shift) & 511) as usize;
        let entry = read_entry(table, slot)?;
        table = if entry == 0 {
            let child = allocator(0).ok_or(Error::Allocation)?;
            validate_table(child)?;
            write_entry(table, slot, (child.get() >> 2) | registers::PTE_VALID)?;
            child
        } else if entry & 15 == registers::PTE_VALID {
            PhysicalAddress::new((entry >> 10) << 12)
        } else {
            return Err(Error::Conflict);
        };
    }
    let slot = ((page.address >> 12) & 511) as usize;
    if read_entry(table, slot)? != 0 {
        return Err(Error::Conflict);
    }
    let mut flags =
        registers::PTE_VALID | registers::PTE_USER | registers::PTE_ACCESSED | registers::PTE_DIRTY;
    if page.readable {
        flags |= registers::PTE_READ;
    }
    if page.writable {
        flags |= registers::PTE_WRITE;
    }
    if page.executable {
        flags |= registers::PTE_EXECUTE;
    }
    write_entry(table, slot, (page.physical.get() >> 2) | flags)
}
fn validate_table(table: PhysicalAddress) -> Result<(), Error> {
    if table.get() == 0 || table.get() & 4095 != 0 || table.get() >= 1 << 36 {
        Err(Error::InvalidAddress)
    } else {
        Ok(())
    }
}
fn table_pointer(table: PhysicalAddress) -> Result<*mut u64, Error> {
    validate_table(table)?;
    Ok(core::ptr::with_exposed_provenance_mut(
        (registers::SV39_LINEAR_BASE + table.get()) as usize,
    ))
}
fn read_entry(table: PhysicalAddress, slot: usize) -> Result<u64, Error> {
    let pointer = table_pointer(table)?;
    // SAFETY: Private callers produce bounded slots and retain the hierarchy.
    Ok(unsafe { read_volatile(pointer.add(slot)) })
}
fn write_entry(table: PhysicalAddress, slot: usize, value: u64) -> Result<(), Error> {
    let pointer = table_pointer(table)?;
    // SAFETY: Only the unpublished builder writes these uniquely owned pages.
    unsafe { write_volatile(pointer.add(slot), value) };
    Ok(())
}
fn read_satp() -> u64 {
    let value: u64;
    // SAFETY: SATP is a readable HS CSR.
    unsafe { asm!("csrr {value}, satp", value = out(reg) value, options(nomem, nostack)) };
    value
}
fn invalidate(identifier: u16) {
    let tag = hardware_identifier(identifier);
    // SAFETY: Caller retains the ID and excludes U execution. rs2 is a register
    // even for tag zero; no user mapping is global. SFENCE also terminates stale
    // speculative page-table walks before the retirement acknowledgement.
    unsafe { asm!("sfence.vma zero, {tag}", tag = in(reg) tag, options(nostack)) };
}
fn install(root: &PreparedAddressSpace) {
    // SAFETY: Shared supervisor mappings cover the executing code and stack
    // before and after SATP; the caller retains all hierarchy pages.
    unsafe { asm!("csrw satp, {root}", root = in(reg) root.root_register(), options(nostack)) };
    invalidate(root.identifier);
    // SAFETY: The caller retains every newly published executable page. Each
    // consuming hart must synchronize its own instruction stream after root
    // replacement as well as initial activation.
    unsafe { asm!("fence.i", options(nostack)) };
    local().publish(root);
}
/// Installs one retained root on a pinned, interrupt-masked CPU.
///
/// # Safety
/// Caller closes admission against replacement and retains the root and ID for
/// the complete CPU-local token lifetime. Native activations cannot nest.
pub(crate) unsafe fn activate_local(root: &PreparedAddressSpace) -> LocalActivation {
    if local().root.load(Ordering::Acquire) != 0 {
        super::halt();
    }
    let previous_root = read_satp();
    install(root);
    LocalActivation {
        previous_root,
        _not_send: PhantomData,
    }
}
/// Restores the predecessor after Native execution and active-root use stop.
///
/// # Safety
/// Token must be consumed on its original pinned, masked CPU under admission.
pub(crate) unsafe fn deactivate_local(activation: LocalActivation) {
    let owner = local();
    if owner.root.load(Ordering::Acquire) == 0 {
        super::halt();
    }
    let identifier = owner.identifier.load(Ordering::Relaxed) as u16;
    // Switch first so old user roots cannot start new walks after the fence.
    // SAFETY: The token retains the preceding kernel root and shared mappings.
    unsafe { asm!("csrw satp, {root}", root = in(reg) activation.previous_root, options(nostack)) };
    invalidate(identifier);
    owner.root.store(0, Ordering::Release);
}
pub(crate) fn local_identity_is_active(identity: LocalIdentity) -> bool {
    let owner = local();
    owner.root.load(Ordering::Acquire) == identity.root.root
        && owner.identifier.load(Ordering::Relaxed) == u64::from(identity.root.identifier)
        && owner.generation.load(Ordering::Relaxed) == identity.root.generation
        && read_satp() == identity.root.root_register()
}
/// Applies a retained root replacement or final invalidation before RPC ack.
///
/// # Safety
/// Caller retains the request root/ID and excludes concurrent U execution;
/// Replace targets an active owner whose old root remains retained until ack.
pub(crate) unsafe fn service_local_request(request: LocalRequest) -> Result<(), Error> {
    match request.operation {
        LocalOperation::Replace => {
            if local().root.load(Ordering::Acquire) == 0 {
                return Err(Error::InvalidLocalState);
            }
            let old = local().identifier.load(Ordering::Relaxed) as u16;
            install(&request.root);
            invalidate(old);
        }
        LocalOperation::Invalidate => invalidate(request.root.identifier),
    }
    Ok(())
}

/// Copies externally mutable, resident memory without Rust reference aliasing.
///
/// # Safety
/// Ranges must remain resident, valid for length, and nonoverlapping. Kernel
/// aliases, rather than user VAs, must be passed while SUM remains clear.
pub(crate) unsafe fn copy_from_exposed(source: *const u8, destination: *mut u8, length: usize) {
    // SAFETY: Caller retains both ranges. The assembly memory clobber and byte
    // accesses admit concurrent userspace modification without a Rust borrow.
    unsafe {
        asm!(
        "beqz {length}, 3f", "2:", "lbu {byte}, 0({source})", "sb {byte}, 0({destination})",
        "addi {source}, {source}, 1", "addi {destination}, {destination}, 1",
        "addi {length}, {length}, -1", "bnez {length}, 2b", "3:",
        source = inout(reg) source => _, destination = inout(reg) destination => _,
        length = inout(reg) length => _, byte = out(reg) _, options(nostack))
    };
}
/// Copies private bytes to externally mutable memory.
///
/// # Safety
/// The same residency, linear-alias, and nonoverlap contract as the read copy.
pub(crate) unsafe fn copy_to_exposed(source: *const u8, destination: *mut u8, length: usize) {
    // SAFETY: The byte-loop mechanism has the same requirements in either direction.
    unsafe { copy_from_exposed(source, destination, length) };
}
