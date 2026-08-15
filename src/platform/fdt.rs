use core::str;

use super::{
    CpuInfo, CpuList, CpuListError, MAX_MEMORY_REGIONS, MAX_MMIO_REGIONS, MAX_NO_MAP_REGIONS,
    MAX_RESERVED_REGIONS, PhysicalRange, PlatformInfo, RegionList,
};

const FDT_MAGIC: u32 = 0xd00d_feed;
const FDT_BEGIN_NODE: u32 = 1;
const FDT_END_NODE: u32 = 2;
const FDT_PROP: u32 = 3;
const FDT_NOP: u32 = 4;
const FDT_END: u32 = 9;
const HEADER_SIZE: usize = 40;
const MAX_DTB_SIZE: usize = 2 * 1024 * 1024;
const MAX_DEPTH: usize = 32;
const MAX_NODE_REGIONS: usize = 8;
const MAX_BUS_RANGES: usize = 8;
const MAX_RANGE_CELLS: usize = 64;
const MAX_INTERRUPT_CELLS: usize = 16;

#[derive(Clone, Copy)]
struct RegisterList {
    entries: [PhysicalRange; MAX_NODE_REGIONS],
    length: usize,
}

impl RegisterList {
    const fn new() -> Self {
        Self {
            entries: [PhysicalRange::EMPTY; MAX_NODE_REGIONS],
            length: 0,
        }
    }

    fn as_slice(&self) -> &[PhysicalRange] {
        &self.entries[..self.length]
    }

    fn push(&mut self, range: PhysicalRange) -> Result<(), Error> {
        if self.length == MAX_NODE_REGIONS {
            return Err(Error::TooManyRegions);
        }
        self.entries[self.length] = range;
        self.length += 1;
        Ok(())
    }
}

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

/// Receives raw properties and translated resources during an FDT walk.
pub trait NodeVisitor {
    fn begin_node(&mut self, _id: NodeId, _name: &str) {}
    fn property(&mut self, _id: NodeId, _name: &str, _value: &[u8]) {}
    fn end_node(&mut self, _node: NodeResources<'_>) {}
}

/// Forwards one FDT walk to two independent allocation-free consumers.
pub struct VisitorPair<'a, First, Second> {
    first: &'a mut First,
    second: &'a mut Second,
}

impl<'a, First, Second> VisitorPair<'a, First, Second> {
    pub const fn new(first: &'a mut First, second: &'a mut Second) -> Self {
        Self { first, second }
    }
}

impl<First: NodeVisitor, Second: NodeVisitor> NodeVisitor for VisitorPair<'_, First, Second> {
    fn begin_node(&mut self, id: NodeId, name: &str) {
        self.first.begin_node(id, name);
        self.second.begin_node(id, name);
    }

    fn property(&mut self, id: NodeId, name: &str, value: &[u8]) {
        self.first.property(id, name, value);
        self.second.property(id, name, value);
    }

    fn end_node(&mut self, node: NodeResources<'_>) {
        self.first.end_node(NodeResources {
            id: node.id,
            enabled: node.enabled,
            registers: node.registers,
            interrupt_cells: node.interrupt_cells,
        });
        self.second.end_node(node);
    }
}

struct IgnoreNodes;

impl NodeVisitor for IgnoreNodes {}

#[derive(Clone, Copy)]
struct BusRange {
    child_start: u64,
    parent_start: u64,
    size: u64,
}

impl BusRange {
    const EMPTY: Self = Self {
        child_start: 0,
        parent_start: 0,
        size: 0,
    };
}

#[derive(Clone, Copy)]
struct Header {
    total_size: usize,
    structure_offset: usize,
    strings_offset: usize,
    reservation_offset: usize,
    strings_size: usize,
    structure_size: usize,
}

struct DiscoveryState {
    cpus: CpuList,
    memory: RegionList<MAX_MEMORY_REGIONS>,
    reserved: RegionList<MAX_RESERVED_REGIONS>,
    no_map: RegionList<MAX_NO_MAP_REGIONS>,
    mmio: RegionList<MAX_MMIO_REGIONS>,
}

#[derive(Clone, Copy)]
struct NodeState {
    id: NodeId,
    child_address_cells: u32,
    child_size_cells: u32,
    register_address_cells: u32,
    register_size_cells: u32,
    is_memory: bool,
    is_cpu: bool,
    cpu_hardware_id: Option<u64>,
    is_reserved_region: bool,
    no_map: bool,
    children_are_reserved: bool,
    disabled: bool,
    registers: RegisterList,
    raw_interrupts: [u32; MAX_INTERRUPT_CELLS],
    raw_interrupt_cells: usize,
    ranges_present: bool,
    ranges_supported: bool,
    ranges: [BusRange; MAX_BUS_RANGES],
    range_count: usize,
    raw_ranges: [u32; MAX_RANGE_CELLS],
    raw_range_cells: usize,
}

impl NodeState {
    const EMPTY: Self = Self {
        id: NodeId(0),
        child_address_cells: 2,
        child_size_cells: 1,
        register_address_cells: 2,
        register_size_cells: 1,
        is_memory: false,
        is_cpu: false,
        cpu_hardware_id: None,
        is_reserved_region: false,
        no_map: false,
        children_are_reserved: false,
        disabled: false,
        registers: RegisterList::new(),
        raw_interrupts: [0; MAX_INTERRUPT_CELLS],
        raw_interrupt_cells: 0,
        ranges_present: false,
        ranges_supported: true,
        ranges: [BusRange::EMPTY; MAX_BUS_RANGES],
        range_count: 0,
        raw_ranges: [0; MAX_RANGE_CELLS],
        raw_range_cells: 0,
    };
}

/// Validates and discovers platform data from a firmware-owned DTB.
///
/// # Safety
///
/// `address` must point to readable memory containing the complete DTB supplied
/// by the boot environment. The memory must remain stable during this call.
pub unsafe fn discover(address: usize) -> Result<PlatformInfo, Error> {
    unsafe { discover_with(address, &mut IgnoreNodes) }
}

/// Walks the DTB while collecting architecture-independent platform memory.
///
/// # Safety
///
/// The safety requirements are identical to [`discover`].
pub unsafe fn discover_with(
    address: usize,
    visitor: &mut impl NodeVisitor,
) -> Result<PlatformInfo, Error> {
    if address == 0 || address & 0x7 != 0 || address.checked_add(HEADER_SIZE).is_none() {
        return Err(Error::BadAddress);
    }

    // Read only the fixed header before trusting the DTB-reported total size.
    let header_bytes = unsafe { core::slice::from_raw_parts(address as *const u8, HEADER_SIZE) };
    let header = parse_header(header_bytes)?;
    address
        .checked_add(header.total_size)
        .ok_or(Error::BadAddress)?;
    let blob = unsafe { core::slice::from_raw_parts(address as *const u8, header.total_size) };
    discover_from_bytes_with(blob, visitor)
}

/// Discovers platform data in a complete, memory-backed DTB.
pub fn discover_from_bytes(blob: &[u8]) -> Result<PlatformInfo, Error> {
    discover_from_bytes_with(blob, &mut IgnoreNodes)
}

pub fn discover_from_bytes_with(
    blob: &[u8],
    visitor: &mut impl NodeVisitor,
) -> Result<PlatformInfo, Error> {
    let header = parse_header(blob)?;
    if blob.len() < header.total_size {
        return Err(Error::Truncated);
    }
    discover_in_blob(&blob[..header.total_size], header, visitor)
}

fn parse_header(bytes: &[u8]) -> Result<Header, Error> {
    if bytes.len() < HEADER_SIZE {
        return Err(Error::Truncated);
    }
    if read_u32(bytes, 0)? != FDT_MAGIC {
        return Err(Error::BadMagic);
    }

    let header = Header {
        total_size: read_u32(bytes, 4)? as usize,
        structure_offset: read_u32(bytes, 8)? as usize,
        strings_offset: read_u32(bytes, 12)? as usize,
        reservation_offset: read_u32(bytes, 16)? as usize,
        strings_size: read_u32(bytes, 32)? as usize,
        structure_size: read_u32(bytes, 36)? as usize,
    };
    if header.total_size < HEADER_SIZE || header.reservation_offset >= header.total_size {
        return Err(Error::Truncated);
    }
    if header.total_size > MAX_DTB_SIZE {
        return Err(Error::TooLarge);
    }
    checked_region(
        header.total_size,
        header.structure_offset,
        header.structure_size,
    )?;
    checked_region(
        header.total_size,
        header.strings_offset,
        header.strings_size,
    )?;
    Ok(header)
}

fn discover_in_blob(
    blob: &[u8],
    header: Header,
    visitor: &mut impl NodeVisitor,
) -> Result<PlatformInfo, Error> {
    let structure = region(blob, header.structure_offset, header.structure_size)?;
    let strings = region(blob, header.strings_offset, header.strings_size)?;
    let mut discovered = DiscoveryState {
        cpus: CpuList::new(),
        memory: RegionList::new(),
        reserved: parse_reservations(blob, header.reservation_offset)?,
        no_map: RegionList::new(),
        mmio: RegionList::new(),
    };
    let mut cursor = 0;
    let mut depth = 0usize;
    let mut next_node_id = 0u32;
    let mut nodes = [NodeState::EMPTY; MAX_DEPTH];

    loop {
        let token = take_u32(structure, &mut cursor)?;
        match token {
            FDT_BEGIN_NODE => {
                if depth == MAX_DEPTH {
                    return Err(Error::TooDeep);
                }
                let name = take_c_string(structure, &mut cursor)?;
                let node_id = NodeId(next_node_id);
                next_node_id = next_node_id.checked_add(1).ok_or(Error::TooLarge)?;
                visitor.begin_node(node_id, name);
                align_cursor(&mut cursor, structure.len())?;
                if depth != 0 {
                    decode_ranges(&mut nodes[depth - 1])?;
                }
                let (address_cells, size_cells, reserved_child, parent_disabled) = if depth == 0 {
                    (2, 1, false, false)
                } else {
                    let parent = nodes[depth - 1];
                    (
                        parent.child_address_cells,
                        parent.child_size_cells,
                        parent.children_are_reserved,
                        parent.disabled,
                    )
                };
                nodes[depth] = NodeState {
                    id: node_id,
                    register_address_cells: address_cells,
                    register_size_cells: size_cells,
                    is_memory: name == "memory" || name.starts_with("memory@"),
                    is_cpu: name == "cpu" || name.starts_with("cpu@"),
                    is_reserved_region: reserved_child,
                    children_are_reserved: name == "reserved-memory",
                    disabled: parent_disabled,
                    ..NodeState::EMPTY
                };
                depth += 1;
            }
            FDT_END_NODE => {
                if depth == 0 {
                    return Err(Error::BadStructure);
                }
                depth -= 1;
                decode_ranges(&mut nodes[depth])?;
                finish_node(nodes[depth], &mut discovered, &nodes[..depth], visitor)?;
            }
            FDT_PROP => {
                if depth == 0 {
                    return Err(Error::BadStructure);
                }
                let length = take_u32(structure, &mut cursor)? as usize;
                let name_offset = take_u32(structure, &mut cursor)? as usize;
                let value = take_bytes(structure, &mut cursor, length)?;
                align_cursor(&mut cursor, structure.len())?;
                let name = string_at(strings, name_offset)?;
                visitor.property(nodes[depth - 1].id, name, value);
                apply_property(&mut nodes[depth - 1], name, value)?;
            }
            FDT_NOP => {}
            FDT_END => {
                if depth != 0 {
                    return Err(Error::BadStructure);
                }
                break;
            }
            _ => return Err(Error::BadStructure),
        }
    }

    if discovered.memory.is_empty() {
        return Err(Error::NoMemory);
    }
    Ok(PlatformInfo {
        cpus: discovered.cpus,
        memory: discovered.memory,
        reserved: discovered.reserved,
        no_map: discovered.no_map,
        mmio: discovered.mmio,
        dtb_size: header.total_size as u64,
    })
}

fn finish_node(
    node: NodeState,
    discovered: &mut DiscoveryState,
    ancestors: &[NodeState],
    visitor: &mut impl NodeVisitor,
) -> Result<(), Error> {
    let translate_register = |range| translate_range(range, ancestors);

    let mut translated_registers = [PhysicalRange::EMPTY; MAX_NODE_REGIONS];
    let mut translated_count = 0usize;
    for &range in node.registers.as_slice() {
        match translate_register(range) {
            Ok(range) => {
                translated_registers[translated_count] = range;
                translated_count += 1;
            }
            Err(Error::UntranslatedAddress) if !node.is_memory && !node.is_reserved_region => {}
            Err(error) => return Err(error),
        }
    }

    visitor.end_node(NodeResources {
        id: node.id,
        enabled: !node.disabled,
        registers: &translated_registers[..translated_count],
        interrupt_cells: &node.raw_interrupts[..node.raw_interrupt_cells],
    });

    if node.disabled {
        return Ok(());
    }

    if node.is_cpu {
        let hardware_id = node.cpu_hardware_id.ok_or(Error::BadStructure)?;
        discovered
            .cpus
            .push(CpuInfo { hardware_id })
            .map_err(|error| match error {
                CpuListError::Capacity => Error::TooManyCpus,
                CpuListError::Duplicate => Error::DuplicateCpu,
            })?;
    }

    let (target, require_translation) = if node.is_memory {
        (Some(&mut discovered.memory as &mut dyn RegionSink), true)
    } else if node.is_reserved_region {
        (Some(&mut discovered.reserved as &mut dyn RegionSink), true)
    } else if !node.is_cpu {
        (Some(&mut discovered.mmio as &mut dyn RegionSink), false)
    } else {
        (None, false)
    };

    if let Some(target) = target {
        for &range in node.registers.as_slice() {
            let range = match translate_register(range) {
                Ok(range) => range,
                Err(Error::UntranslatedAddress) if !require_translation => continue,
                Err(error) => return Err(error),
            };
            target.insert(range)?;
            if node.is_reserved_region && node.no_map {
                discovered
                    .no_map
                    .insert(range)
                    .map_err(|_| Error::TooManyRegions)?;
            }
        }
    }
    Ok(())
}

trait RegionSink {
    fn insert(&mut self, range: PhysicalRange) -> Result<(), Error>;
}

impl<const CAPACITY: usize> RegionSink for RegionList<CAPACITY> {
    fn insert(&mut self, range: PhysicalRange) -> Result<(), Error> {
        RegionList::insert(self, range).map_err(|_| Error::TooManyRegions)
    }
}

fn apply_property(node: &mut NodeState, name: &str, value: &[u8]) -> Result<(), Error> {
    match name {
        "#address-cells" => node.child_address_cells = read_u32(value, 0)?,
        "#size-cells" => node.child_size_cells = read_u32(value, 0)?,
        "device_type" => {
            node.is_memory |= c_string_equals(value, "memory");
            node.is_cpu |= c_string_equals(value, "cpu");
        }
        // The DT specification defines only "ok" and "okay" as available.
        // Treat reserved, failed, disabled, and unknown states conservatively.
        "status" => {
            node.disabled |= !(c_string_equals(value, "ok") || c_string_equals(value, "okay"));
        }
        "reg" if node.is_cpu => parse_cpu_identifier(node, value)?,
        "reg" => parse_registers(node, value)?,
        "ranges" => parse_ranges(node, value)?,
        "no-map" => node.no_map = true,
        "interrupts" => parse_interrupts(node, value)?,
        _ => {}
    }
    Ok(())
}

fn parse_cpu_identifier(node: &mut NodeState, value: &[u8]) -> Result<(), Error> {
    let address_cells = supported_cells(node.register_address_cells, false)?;
    if node.register_size_cells != 0 || value.len() != address_cells * 4 {
        return Err(Error::BadStructure);
    }
    node.cpu_hardware_id = Some(read_cells(value, address_cells)?);
    Ok(())
}

fn parse_interrupts(node: &mut NodeState, value: &[u8]) -> Result<(), Error> {
    if !value.len().is_multiple_of(4) {
        return Err(Error::Truncated);
    }
    let count = value.len() / 4;
    if count > MAX_INTERRUPT_CELLS {
        node.raw_interrupt_cells = 0;
        return Ok(());
    }
    for (index, cell) in value.chunks_exact(4).enumerate() {
        node.raw_interrupts[index] = read_u32(cell, 0)?;
    }
    node.raw_interrupt_cells = count;
    Ok(())
}

fn parse_ranges(node: &mut NodeState, value: &[u8]) -> Result<(), Error> {
    node.ranges_present = true;
    if !value.len().is_multiple_of(4) {
        return Err(Error::Truncated);
    }
    let cell_count = value.len() / 4;
    if cell_count > MAX_RANGE_CELLS {
        return Err(Error::TooManyRegions);
    }
    for (index, cell) in value.chunks_exact(4).enumerate() {
        node.raw_ranges[index] = read_u32(cell, 0)?;
    }
    node.raw_range_cells = cell_count;
    Ok(())
}

fn decode_ranges(node: &mut NodeState) -> Result<(), Error> {
    node.ranges_supported = true;
    node.range_count = 0;
    if !node.ranges_present || node.raw_range_cells == 0 {
        return Ok(());
    }

    let Ok(child_cells) = supported_cells(node.child_address_cells, false) else {
        node.ranges_supported = false;
        return Ok(());
    };
    let Ok(parent_cells) = supported_cells(node.register_address_cells, false) else {
        node.ranges_supported = false;
        return Ok(());
    };
    let Ok(size_cells) = supported_cells(node.child_size_cells, true) else {
        node.ranges_supported = false;
        return Ok(());
    };
    let tuple_cells = child_cells
        .checked_add(parent_cells)
        .and_then(|cells| cells.checked_add(size_cells))
        .ok_or(Error::BadStructure)?;
    if tuple_cells == 0 || !node.raw_range_cells.is_multiple_of(tuple_cells) {
        return Err(Error::Truncated);
    }

    for tuple in node.raw_ranges[..node.raw_range_cells].chunks_exact(tuple_cells) {
        if node.range_count == MAX_BUS_RANGES {
            return Err(Error::TooManyRegions);
        }
        let parent_offset = child_cells;
        let size_offset = child_cells + parent_cells;
        let child_start = read_cell_words(tuple, child_cells)?;
        let parent_start = read_cell_words(&tuple[parent_offset..], parent_cells)?;
        let size = read_cell_words(&tuple[size_offset..], size_cells)?;
        if size == 0 {
            continue;
        }
        child_start.checked_add(size).ok_or(Error::BadStructure)?;
        parent_start.checked_add(size).ok_or(Error::BadStructure)?;
        node.ranges[node.range_count] = BusRange {
            child_start,
            parent_start,
            size,
        };
        node.range_count += 1;
    }
    Ok(())
}

fn read_cell_words(cells: &[u32], count: usize) -> Result<u64, Error> {
    let mut result = 0u64;
    for &cell in cells.get(..count).ok_or(Error::Truncated)? {
        result = result.checked_shl(32).ok_or(Error::UnsupportedCells)? | u64::from(cell);
    }
    Ok(result)
}

fn translate_range(
    mut range: PhysicalRange,
    ancestors: &[NodeState],
) -> Result<PhysicalRange, Error> {
    // The root has no parent address space. Every lower ancestor represents a
    // bus through which the child address must be translated.
    for bus in ancestors.iter().skip(1).rev() {
        if !bus.ranges_present || !bus.ranges_supported {
            return Err(Error::UntranslatedAddress);
        }
        if bus.range_count == 0 {
            continue;
        }
        let mapping = bus.ranges[..bus.range_count]
            .iter()
            .find(|mapping| {
                mapping.child_start <= range.start()
                    && range.end() <= mapping.child_start + mapping.size
            })
            .ok_or(Error::UntranslatedAddress)?;
        let translated = mapping
            .parent_start
            .checked_add(range.start() - mapping.child_start)
            .ok_or(Error::BadStructure)?;
        range = PhysicalRange::new(translated, range.size()).ok_or(Error::BadStructure)?;
    }
    Ok(range)
}

fn supported_cells(value: u32, allow_zero: bool) -> Result<usize, Error> {
    let cells = usize::try_from(value).map_err(|_| Error::UnsupportedCells)?;
    if (allow_zero && cells == 0) || (1..=2).contains(&cells) {
        Ok(cells)
    } else {
        Err(Error::UnsupportedCells)
    }
}

fn parse_registers(node: &mut NodeState, value: &[u8]) -> Result<(), Error> {
    let address_cells =
        usize::try_from(node.register_address_cells).map_err(|_| Error::UnsupportedCells)?;
    let size_cells =
        usize::try_from(node.register_size_cells).map_err(|_| Error::UnsupportedCells)?;
    if !(1..=2).contains(&address_cells) || size_cells > 2 {
        return if node.is_memory || node.is_reserved_region {
            Err(Error::UnsupportedCells)
        } else {
            Ok(())
        };
    }
    let tuple_cells = address_cells
        .checked_add(size_cells)
        .ok_or(Error::BadStructure)?;
    let tuple_bytes = tuple_cells.checked_mul(4).ok_or(Error::BadStructure)?;
    if tuple_bytes == 0 || !value.len().is_multiple_of(tuple_bytes) {
        return Err(Error::Truncated);
    }

    for tuple in value.chunks_exact(tuple_bytes) {
        let start = read_cells(tuple, address_cells)?;
        let size = read_cells(&tuple[address_cells * 4..], size_cells)?;
        if let Some(range) = PhysicalRange::new(start, size) {
            node.registers.push(range)?;
        }
    }
    Ok(())
}

fn parse_reservations(
    blob: &[u8],
    mut cursor: usize,
) -> Result<RegionList<MAX_RESERVED_REGIONS>, Error> {
    let mut reservations = RegionList::new();
    loop {
        let address = read_u64(blob, cursor)?;
        let size = read_u64(blob, cursor.checked_add(8).ok_or(Error::Truncated)?)?;
        cursor = cursor.checked_add(16).ok_or(Error::Truncated)?;
        if address == 0 && size == 0 {
            return Ok(reservations);
        }
        if let Some(range) = PhysicalRange::new(address, size) {
            reservations
                .insert(range)
                .map_err(|_| Error::TooManyRegions)?;
        }
    }
}

fn c_string_equals(value: &[u8], expected: &str) -> bool {
    value.split(|byte| *byte == 0).next() == Some(expected.as_bytes())
}

fn read_cells(bytes: &[u8], cells: usize) -> Result<u64, Error> {
    let mut result = 0u64;
    for index in 0..cells {
        result = (result << 32) | u64::from(read_u32(bytes, index * 4)?);
    }
    Ok(result)
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, Error> {
    let raw: [u8; 4] = bytes
        .get(offset..offset.checked_add(4).ok_or(Error::Truncated)?)
        .ok_or(Error::Truncated)?
        .try_into()
        .map_err(|_| Error::Truncated)?;
    Ok(u32::from_be_bytes(raw))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, Error> {
    let raw: [u8; 8] = bytes
        .get(offset..offset.checked_add(8).ok_or(Error::Truncated)?)
        .ok_or(Error::Truncated)?
        .try_into()
        .map_err(|_| Error::Truncated)?;
    Ok(u64::from_be_bytes(raw))
}

fn take_u32(bytes: &[u8], cursor: &mut usize) -> Result<u32, Error> {
    let value = read_u32(bytes, *cursor)?;
    *cursor = cursor.checked_add(4).ok_or(Error::Truncated)?;
    Ok(value)
}

fn take_bytes<'a>(bytes: &'a [u8], cursor: &mut usize, length: usize) -> Result<&'a [u8], Error> {
    let end = cursor.checked_add(length).ok_or(Error::Truncated)?;
    let value = bytes.get(*cursor..end).ok_or(Error::Truncated)?;
    *cursor = end;
    Ok(value)
}

fn take_c_string<'a>(bytes: &'a [u8], cursor: &mut usize) -> Result<&'a str, Error> {
    let tail = bytes.get(*cursor..).ok_or(Error::Truncated)?;
    let length = tail
        .iter()
        .position(|byte| *byte == 0)
        .ok_or(Error::Truncated)?;
    let value = str::from_utf8(&tail[..length]).map_err(|_| Error::BadStructure)?;
    *cursor = cursor.checked_add(length + 1).ok_or(Error::Truncated)?;
    Ok(value)
}

fn string_at(strings: &[u8], offset: usize) -> Result<&str, Error> {
    let tail = strings.get(offset..).ok_or(Error::Truncated)?;
    let length = tail
        .iter()
        .position(|byte| *byte == 0)
        .ok_or(Error::Truncated)?;
    str::from_utf8(&tail[..length]).map_err(|_| Error::BadStructure)
}

fn align_cursor(cursor: &mut usize, limit: usize) -> Result<(), Error> {
    *cursor = cursor.checked_add(3).ok_or(Error::Truncated)? & !3;
    if *cursor > limit {
        return Err(Error::Truncated);
    }
    Ok(())
}

fn checked_region(total: usize, offset: usize, size: usize) -> Result<(), Error> {
    if offset
        .checked_add(size)
        .filter(|end| *end <= total)
        .is_none()
    {
        return Err(Error::Truncated);
    }
    Ok(())
}

fn region(bytes: &[u8], offset: usize, size: usize) -> Result<&[u8], Error> {
    let end = offset.checked_add(size).ok_or(Error::Truncated)?;
    bytes.get(offset..end).ok_or(Error::Truncated)
}
