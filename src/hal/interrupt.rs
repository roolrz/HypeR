/// Architecture policy used by locks that must be safe against local IRQs.
///
/// Implementations must restore the exact interrupt mask captured by
/// `save_and_disable`, including when interrupts were already disabled.
pub trait InterruptMask {
    type State: Copy;

    fn save_and_disable() -> Self::State;
    fn restore(state: Self::State);
}

/// Architecture-independent hardware interrupt number.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct InterruptId(u32);

impl InterruptId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Electrical behavior selected for a configurable interrupt input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InterruptTrigger {
    Level,
    Edge,
}

/// Common interrupt-controller operations used by kernel policy.
pub trait InterruptController {
    type Error;

    fn enable(&mut self, interrupt: InterruptId) -> Result<(), Self::Error>;
    fn disable(&mut self, interrupt: InterruptId) -> Result<(), Self::Error>;
    fn acknowledge(&self) -> Option<InterruptId>;
    fn end(&self, interrupt: InterruptId);
}
