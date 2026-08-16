//! Platform-Level Interrupt Controller for supervisor-mode RISC-V kernels.

use core::marker::PhantomData;
use core::ptr::{read_volatile, write_volatile};

use crate::hal::barrier::{Barrier, BarrierAccess, BarrierDomain};
use crate::hal::interrupt::{
    InterruptController, InterruptId, InterruptTrigger, KernelInterruptController,
};
use crate::platform::{MAX_PLIC_CONTEXTS, PlicInfo};

const TIMER_INTERRUPT: u32 = 0;
const PRIORITY_BASE: usize = 0;
const ENABLE_BASE: usize = 0x2000;
const ENABLE_STRIDE: usize = 0x80;
const CONTEXT_BASE: usize = 0x20_0000;
const CONTEXT_STRIDE: usize = 0x1000;
const CONTEXT_THRESHOLD: usize = 0;
const CONTEXT_CLAIM: usize = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    AddressOverflow,
    InvalidContext,
    InvalidInterrupt,
    InvalidRegisterRange,
    UnsupportedTrigger,
}

pub struct Plic<B: Barrier> {
    base: usize,
    source_count: u32,
    contexts: [u32; MAX_PLIC_CONTEXTS],
    context_count: usize,
    hardware_id: fn() -> u64,
    barrier: PhantomData<B>,
}

impl<B: Barrier> Plic<B> {
    /// Binds a PLIC to one permanent MMIO mapping.
    ///
    /// # Safety
    ///
    /// `map` must return a mapping for the complete DT-described register
    /// range, and the PLIC must have a single driver owner.
    pub unsafe fn bind(
        info: PlicInfo,
        mut map: impl FnMut(u64) -> Option<usize>,
        hardware_id: fn() -> u64,
    ) -> Result<Self, Error> {
        validate_register_range(info)?;
        let base = map(info.registers.start()).ok_or(Error::AddressOverflow)?;
        let mut controller = Self {
            base,
            source_count: info.source_count,
            contexts: info.supervisor_contexts,
            context_count: info.context_count,
            hardware_id,
            barrier: PhantomData,
        };
        unsafe { controller.initialize_local()? };
        Ok(controller)
    }

    fn context(&self) -> Result<usize, Error> {
        let hardware_id =
            usize::try_from((self.hardware_id)()).map_err(|_| Error::InvalidContext)?;
        if hardware_id >= self.context_count {
            return Err(Error::InvalidContext);
        }
        let context = *self
            .contexts
            .get(hardware_id)
            .ok_or(Error::InvalidContext)?;
        self.base
            .checked_add(CONTEXT_BASE + context as usize * CONTEXT_STRIDE)
            .ok_or(Error::AddressOverflow)
    }

    fn validate(&self, interrupt: InterruptId) -> Result<u32, Error> {
        let source = interrupt.get();
        if source > self.source_count {
            Err(Error::InvalidInterrupt)
        } else {
            Ok(source)
        }
    }

    fn update_enable(&self, source: u32, enabled: bool) -> Result<(), Error> {
        if source == TIMER_INTERRUPT {
            return Ok(());
        }
        let hardware_id =
            usize::try_from((self.hardware_id)()).map_err(|_| Error::InvalidContext)?;
        if hardware_id >= self.context_count {
            return Err(Error::InvalidContext);
        }
        let context = *self
            .contexts
            .get(hardware_id)
            .ok_or(Error::InvalidContext)? as usize;
        let word = source as usize / 32;
        let register = self
            .base
            .checked_add(ENABLE_BASE + context * ENABLE_STRIDE + word * 4)
            .ok_or(Error::AddressOverflow)? as *mut u32;
        unsafe {
            let mut value = read_mmio::<B>(register);
            if enabled {
                value |= 1 << (source % 32);
            } else {
                value &= !(1 << (source % 32));
            }
            write_mmio::<B>(register, value);
        }
        Ok(())
    }
}

fn validate_register_range(info: PlicInfo) -> Result<(), Error> {
    if info.source_count == 0 || info.context_count == 0 || info.context_count > MAX_PLIC_CONTEXTS {
        return Err(Error::InvalidRegisterRange);
    }
    let largest_context = info.supervisor_contexts[..info.context_count]
        .iter()
        .copied()
        .max()
        .ok_or(Error::InvalidRegisterRange)? as u64;
    let priority_end = u64::from(info.source_count)
        .checked_mul(4)
        .and_then(|offset| offset.checked_add(4))
        .ok_or(Error::AddressOverflow)?;
    let enable_end = u64::try_from(ENABLE_BASE)
        .ok()
        .and_then(|base| {
            largest_context
                .checked_mul(ENABLE_STRIDE as u64)
                .and_then(|v| base.checked_add(v))
        })
        .and_then(|offset| offset.checked_add(u64::from(info.source_count / 32) * 4))
        .and_then(|offset| offset.checked_add(4))
        .ok_or(Error::AddressOverflow)?;
    let context_end = u64::try_from(CONTEXT_BASE)
        .ok()
        .and_then(|base| {
            largest_context
                .checked_mul(CONTEXT_STRIDE as u64)
                .and_then(|v| base.checked_add(v))
        })
        .and_then(|offset| offset.checked_add((CONTEXT_CLAIM + 4) as u64))
        .ok_or(Error::AddressOverflow)?;
    if priority_end.max(enable_end).max(context_end) > info.registers.size() {
        return Err(Error::InvalidRegisterRange);
    }
    Ok(())
}

impl<B: Barrier> InterruptController for Plic<B> {
    type Error = Error;

    fn enable(&mut self, interrupt: InterruptId) -> Result<(), Self::Error> {
        self.update_enable(self.validate(interrupt)?, true)
    }

    fn disable(&mut self, interrupt: InterruptId) -> Result<(), Self::Error> {
        self.update_enable(self.validate(interrupt)?, false)
    }

    fn acknowledge(&self) -> Option<InterruptId> {
        let context = self.context().ok()?;
        let source = unsafe { read_mmio::<B>((context + CONTEXT_CLAIM) as *const u32) };
        (source != 0 && source <= self.source_count).then_some(InterruptId::new(source))
    }

    fn end(&self, interrupt: InterruptId) {
        if interrupt.get() == TIMER_INTERRUPT {
            return;
        }
        if let Ok(context) = self.context() {
            unsafe { write_mmio::<B>((context + CONTEXT_CLAIM) as *mut u32, interrupt.get()) };
        }
    }
}

impl<B: Barrier> KernelInterruptController for Plic<B> {
    fn interrupt_count(&self) -> u32 {
        self.source_count.saturating_add(1)
    }

    fn configure(
        &mut self,
        interrupt: InterruptId,
        _priority: u8,
        trigger: InterruptTrigger,
    ) -> Result<(), Self::Error> {
        let source = self.validate(interrupt)?;
        if source == TIMER_INTERRUPT {
            return Ok(());
        }
        if trigger != InterruptTrigger::Level {
            return Err(Error::UnsupportedTrigger);
        }
        unsafe {
            write_mmio::<B>(
                (self.base + PRIORITY_BASE + source as usize * 4) as *mut u32,
                1,
            )
        };
        Ok(())
    }

    fn is_per_cpu(&self, interrupt: InterruptId) -> bool {
        interrupt.get() == TIMER_INTERRUPT
    }

    unsafe fn initialize_local(&mut self) -> Result<(), Self::Error> {
        let context = self.context()?;
        unsafe { write_mmio::<B>((context + CONTEXT_THRESHOLD) as *mut u32, 0) };
        Ok(())
    }
}

unsafe fn read_mmio<B: Barrier>(register: *const u32) -> u32 {
    B::data_memory(BarrierDomain::FullSystem, BarrierAccess::All);
    let value = unsafe { read_volatile(register) };
    B::data_memory(BarrierDomain::FullSystem, BarrierAccess::All);
    value
}

unsafe fn write_mmio<B: Barrier>(register: *mut u32, value: u32) {
    B::data_memory(BarrierDomain::FullSystem, BarrierAccess::All);
    unsafe { write_volatile(register, value) };
    B::data_memory(BarrierDomain::FullSystem, BarrierAccess::All);
}
