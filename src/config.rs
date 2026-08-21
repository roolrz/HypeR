//! Build-time kernel configuration generated from `.config`.

include!(concat!(env!("OUT_DIR"), "/kernel_config.rs"));
