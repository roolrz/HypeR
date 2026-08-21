//! Allocation-free FDT validation, walking, and generic resource decoding.
//!
//! This facade exposes one borrowed parser model to boot discovery and the
//! post-allocator platform scanner. It owns format validation and generic
//! resource decoding, but deliberately leaves device matching and binding
//! policy to architecture discovery and physical drivers.

mod blob;
mod property;
mod resources;
mod walker;

use super::PhysicalRange;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    BadAddress,
    BadMagic,
    BadStructure,
    NoMemory,
    TooDeep,
    TooLarge,
    TooManyRegions,
    TooManyCpus,
    DuplicateCpu,
    Truncated,
    UntranslatedAddress,
    UnsupportedCells,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NodeId(u32);

impl NodeId {
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Resources decoded without assigning device-specific meaning to them.
pub struct NodeResources<'a> {
    pub id: NodeId,
    pub enabled: bool,
    pub registers: &'a [PhysicalRange],
    pub interrupt_cells: &'a [u32],
}

pub use property::{CellList, Property, PropertyError, StringList};
pub use walker::{
    NodeVisitor, VisitorPair, VisitorPairError, WalkError, discover, discover_from_bytes,
    discover_from_bytes_with, discover_with,
};
