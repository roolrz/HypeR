mod pl011;
mod virtual_pl011;

pub use pl011::registers as pl011_registers;
pub use pl011::{
    DataBits, Error, FifoLevel, InterruptStatus, LineConfig, PLATFORM_DRIVER, Parity, Pl011,
    ReceivedByte, StopBits,
};
pub use virtual_pl011::{VirtualPl011, VirtualPl011Access, VirtualPl011Error};
