//! Platform-device discovery and driver lifecycle management.

use alloc::{boxed::Box, string::String, vec::Vec};

use crate::platform::{
    PhysicalRange,
    fdt::{NodeId, NodeResources, NodeVisitor},
};

#[derive(Debug)]
/// A firmware property retained for a discovered platform device.
pub struct DeviceProperty {
    name: String,
    value: Vec<u8>,
}

impl DeviceProperty {
    /// Returns the firmware property name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the unmodified firmware property value.
    pub fn value(&self) -> &[u8] {
        &self.value
    }
}

#[derive(Debug)]
/// Resources discovered for one unclaimed, enabled platform device.
pub struct PlatformDevice {
    id: NodeId,
    name: String,
    compatibles: Vec<String>,
    registers: Vec<PhysicalRange>,
    interrupt_cells: Vec<u32>,
    properties: Vec<DeviceProperty>,
}

impl PlatformDevice {
    /// Returns the device tree node identifier.
    pub const fn id(&self) -> NodeId {
        self.id
    }

    /// Returns the device tree node name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the compatible strings advertised by the device.
    pub fn compatibles(&self) -> &[String] {
        &self.compatibles
    }

    /// Reports whether the device advertises `compatible`.
    pub fn is_compatible(&self, compatible: &str) -> bool {
        self.compatibles
            .iter()
            .any(|candidate| candidate == compatible)
    }

    /// Returns the physical register ranges described by the device.
    pub fn registers(&self) -> &[PhysicalRange] {
        &self.registers
    }

    /// Returns cells that must be translated by the parent IRQ domain.
    pub fn interrupt_cells(&self) -> &[u32] {
        &self.interrupt_cells
    }

    /// Returns unrecognized, unnormalized firmware properties.
    pub fn properties(&self) -> &[DeviceProperty] {
        &self.properties
    }

    /// Returns an unnormalized firmware property.
    ///
    /// Core binding data such as `compatible` and `interrupts` is exposed by
    /// dedicated accessors and is not duplicated in this collection.
    pub fn property(&self, name: &str) -> Option<&[u8]> {
        self.properties
            .iter()
            .find(|property| property.name == name)
            .map(|property| property.value.as_slice())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Errors reported while scanning platform devices from firmware data.
pub enum ScanError {
    /// Memory could not be reserved for copied firmware data.
    Allocation,
    /// A property did not have the format required by this scanner.
    MalformedProperty,
    /// Firmware node nesting was invalid.
    MalformedTree,
    /// A string property was not valid UTF-8.
    InvalidUtf8,
}

struct DeviceBuilder {
    id: NodeId,
    name: String,
    compatibles: Vec<String>,
    interrupt_cells: Vec<u32>,
    properties: Vec<DeviceProperty>,
}

/// Heap-backed visitor used only after the global allocator is installed.
pub struct DeviceScanner<'a> {
    claims: &'a [Option<NodeId>],
    stack: Vec<DeviceBuilder>,
    devices: Vec<PlatformDevice>,
    error: Option<ScanError>,
}

impl<'a> DeviceScanner<'a> {
    /// Creates a scanner that excludes the supplied claimed node identifiers.
    pub const fn new(claims: &'a [Option<NodeId>]) -> Self {
        Self {
            claims,
            stack: Vec::new(),
            devices: Vec::new(),
            error: None,
        }
    }

    /// Completes the scan and returns the discovered devices.
    ///
    /// # Errors
    ///
    /// Returns the first scan error or [`ScanError::MalformedTree`] when nodes
    /// remain open.
    pub fn finish(self) -> Result<Vec<PlatformDevice>, ScanError> {
        match self.error {
            Some(error) => Err(error),
            None if !self.stack.is_empty() => Err(ScanError::MalformedTree),
            None => Ok(self.devices),
        }
    }

    fn is_claimed(&self, id: NodeId) -> bool {
        self.claims.iter().flatten().any(|claim| *claim == id)
    }

    fn record_property(&mut self, name: &str, value: &[u8]) -> Result<(), ScanError> {
        let builder = self.stack.last_mut().ok_or(ScanError::MalformedTree)?;
        if name == "compatible" {
            for compatible in value
                .split(|byte| *byte == 0)
                .filter(|candidate| !candidate.is_empty())
            {
                let compatible =
                    core::str::from_utf8(compatible).map_err(|_| ScanError::InvalidUtf8)?;
                builder
                    .compatibles
                    .try_reserve(1)
                    .map_err(|_| ScanError::Allocation)?;
                builder.compatibles.push(copy_string(compatible)?);
            }
        }
        if name == "interrupts" {
            if !value.len().is_multiple_of(4) {
                return Err(ScanError::MalformedProperty);
            }
            builder
                .interrupt_cells
                .try_reserve_exact(value.len() / 4)
                .map_err(|_| ScanError::Allocation)?;
            for cell in value.chunks_exact(4) {
                let raw: [u8; 4] = cell.try_into().map_err(|_| ScanError::MalformedProperty)?;
                builder.interrupt_cells.push(u32::from_be_bytes(raw));
            }
        }
        if matches!(name, "compatible" | "interrupts") {
            return Ok(());
        }
        builder
            .properties
            .try_reserve(1)
            .map_err(|_| ScanError::Allocation)?;
        let mut property_value = Vec::new();
        property_value
            .try_reserve_exact(value.len())
            .map_err(|_| ScanError::Allocation)?;
        property_value.extend_from_slice(value);
        builder.properties.push(DeviceProperty {
            name: copy_string(name)?,
            value: property_value,
        });
        Ok(())
    }

    fn complete_node(&mut self, node: NodeResources<'_>) -> Result<(), ScanError> {
        let builder = self.stack.pop().ok_or(ScanError::MalformedTree)?;
        if !node.enabled
            || builder.compatibles.is_empty()
            || self.is_claimed(node.id)
            || self.stack.is_empty()
        {
            return Ok(());
        }
        self.devices
            .try_reserve(1)
            .map_err(|_| ScanError::Allocation)?;
        let mut registers = Vec::new();
        registers
            .try_reserve_exact(node.registers.len())
            .map_err(|_| ScanError::Allocation)?;
        registers.extend_from_slice(node.registers);
        self.devices.push(PlatformDevice {
            id: builder.id,
            name: builder.name,
            compatibles: builder.compatibles,
            registers,
            interrupt_cells: builder.interrupt_cells,
            properties: builder.properties,
        });
        Ok(())
    }
}

impl NodeVisitor for DeviceScanner<'_> {
    fn begin_node(&mut self, id: NodeId, name: &str) {
        if self.error.is_some() {
            return;
        }
        if self.stack.try_reserve(1).is_err() {
            self.error = Some(ScanError::Allocation);
            return;
        }
        self.stack.push(DeviceBuilder {
            id,
            name: match copy_string(name) {
                Ok(name) => name,
                Err(error) => {
                    self.error = Some(error);
                    return;
                }
            },
            compatibles: Vec::new(),
            interrupt_cells: Vec::new(),
            properties: Vec::new(),
        });
    }

    fn property(&mut self, _id: NodeId, name: &str, value: &[u8]) {
        if self.error.is_none() {
            self.error = self.record_property(name, value).err();
        }
    }

    fn end_node(&mut self, node: NodeResources<'_>) {
        if self.error.is_none() {
            self.error = self.complete_node(node).err();
        } else {
            let _ = self.stack.pop();
        }
    }
}

fn copy_string(value: &str) -> Result<String, ScanError> {
    let mut result = String::new();
    result
        .try_reserve_exact(value.len())
        .map_err(|_| ScanError::Allocation)?;
    result.push_str(value);
    Ok(result)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Errors returned when registering, probing, or managing a driver.
pub enum ProbeError {
    /// The driver requires another device to bind first.
    Deferred,
    /// Required memory or hardware resources were unavailable.
    Resource,
    /// No supported binding is available for the device.
    Unsupported,
    /// The driver reported an implementation-defined failure code.
    Driver(i32),
}

/// Services provided by the kernel to platform drivers.
pub trait DriverServices {
    /// Maps a physical MMIO address and returns its virtual address.
    fn map_mmio(&self, physical_address: u64) -> Option<usize>;
}

/// A bound platform-driver instance.
pub trait DriverInstance: Send {
    /// Suspends the device before a system-wide low-power transition.
    fn suspend(&mut self) -> Result<(), ProbeError> {
        Ok(())
    }

    /// Resumes the device after a successful suspension.
    fn resume(&mut self) -> Result<(), ProbeError> {
        Ok(())
    }

    /// Removes the driver instance and releases its device resources.
    fn remove(&mut self) -> Result<(), ProbeError> {
        Ok(())
    }
}

/// Matches and binds a platform device.
pub trait PlatformDriver: Sync {
    /// Returns the stable name used to identify this driver.
    fn name(&self) -> &'static str;

    /// Returns the device tree compatible strings supported by this driver.
    fn compatible_table(&self) -> &'static [&'static str];

    /// Binds this driver to a discovered device.
    fn probe(
        &self,
        device: &PlatformDevice,
        services: &dyn DriverServices,
    ) -> Result<Box<dyn DriverInstance>, ProbeError>;
}

struct Binding {
    device_id: NodeId,
    driver_name: &'static str,
    instance: Box<dyn DriverInstance>,
    suspended: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
/// The outcome of a platform-device probing pass.
pub struct ProbeReport {
    /// Number of devices successfully bound to a driver.
    pub bound: usize,
    /// Number of devices for which no driver matched.
    pub unmatched: usize,
    /// Number of devices still waiting for a dependency.
    pub deferred: usize,
    /// Number of devices whose driver probe failed.
    pub failed: usize,
}

/// Registry of platform drivers and their bound instances.
pub struct DriverManager {
    drivers: Vec<&'static dyn PlatformDriver>,
    bindings: Vec<Binding>,
}

impl DriverManager {
    /// Creates an empty driver registry.
    pub const fn new() -> Self {
        Self {
            drivers: Vec::new(),
            bindings: Vec::new(),
        }
    }

    /// Registers a driver for future device probing.
    ///
    /// # Errors
    ///
    /// Returns [`ProbeError::Driver`] if another driver has the same name or
    /// [`ProbeError::Resource`] if the registry cannot grow.
    pub fn register(&mut self, driver: &'static dyn PlatformDriver) -> Result<(), ProbeError> {
        if self
            .drivers
            .iter()
            .any(|registered| registered.name() == driver.name())
        {
            return Err(ProbeError::Driver(-1));
        }
        self.drivers
            .try_reserve(1)
            .map_err(|_| ProbeError::Resource)?;
        self.drivers.push(driver);
        Ok(())
    }

    /// Probes every unbound device and retries drivers whose dependencies defer.
    pub fn probe_devices(
        &mut self,
        devices: &[PlatformDevice],
        services: &dyn DriverServices,
    ) -> ProbeReport {
        let mut report = ProbeReport::default();
        let mut deferred = Vec::new();
        for (index, device) in devices.iter().enumerate() {
            if self
                .bindings
                .iter()
                .any(|binding| binding.device_id == device.id())
            {
                continue;
            }
            let Some(driver) = self.matching_driver(device) else {
                report.unmatched += 1;
                continue;
            };
            match self.bind(driver, device, services) {
                Ok(()) => report.bound += 1,
                Err(ProbeError::Deferred) => {
                    if deferred.try_reserve(1).is_ok() {
                        deferred.push(index);
                    } else {
                        report.failed += 1;
                    }
                }
                Err(_) => report.failed += 1,
            }
        }
        self.retry_deferred(devices, services, deferred, &mut report);
        report
    }

    fn retry_deferred(
        &mut self,
        devices: &[PlatformDevice],
        services: &dyn DriverServices,
        mut deferred: Vec<usize>,
        report: &mut ProbeReport,
    ) {
        while !deferred.is_empty() {
            let mut next = Vec::new();
            if next.try_reserve_exact(deferred.len()).is_err() {
                report.failed += deferred.len();
                return;
            }
            let mut progress = false;
            for index in deferred {
                let Some(device) = devices.get(index) else {
                    report.failed += 1;
                    continue;
                };
                let Some(driver) = self.matching_driver(device) else {
                    report.unmatched += 1;
                    continue;
                };
                match self.bind(driver, device, services) {
                    Ok(()) => {
                        report.bound += 1;
                        progress = true;
                    }
                    Err(ProbeError::Deferred) => next.push(index),
                    Err(_) => report.failed += 1,
                }
            }
            if !progress {
                report.deferred += next.len();
                return;
            }
            deferred = next;
        }
    }

    fn matching_driver(&self, device: &PlatformDevice) -> Option<&'static dyn PlatformDriver> {
        self.drivers.iter().copied().find(|driver| {
            driver
                .compatible_table()
                .iter()
                .any(|compatible| device.is_compatible(compatible))
        })
    }

    fn bind(
        &mut self,
        driver: &'static dyn PlatformDriver,
        device: &PlatformDevice,
        services: &dyn DriverServices,
    ) -> Result<(), ProbeError> {
        let instance = driver.probe(device, services)?;
        self.bindings
            .try_reserve(1)
            .map_err(|_| ProbeError::Resource)?;
        self.bindings.push(Binding {
            device_id: device.id(),
            driver_name: driver.name(),
            instance,
            suspended: false,
        });
        Ok(())
    }

    /// Suspends bound instances in reverse binding order.
    ///
    /// # Errors
    ///
    /// Returns the error from the first instance that fails to suspend.
    pub fn suspend_all(&mut self) -> Result<(), ProbeError> {
        for index in (0..self.bindings.len()).rev() {
            if self.bindings[index].suspended {
                continue;
            }
            if let Err(error) = self.bindings[index].instance.suspend() {
                // Restore the pre-suspend state on a best-effort basis.
                // Per-binding state makes a later resume retry safe even if
                // rollback itself encounters a device failure.
                let _ = self.resume_all();
                return Err(error);
            }
            self.bindings[index].suspended = true;
        }
        Ok(())
    }

    /// Resumes suspended instances in binding order.
    ///
    /// # Errors
    ///
    /// Returns the error from the first instance that fails to resume.
    pub fn resume_all(&mut self) -> Result<(), ProbeError> {
        for binding in &mut self.bindings {
            if binding.suspended {
                binding.instance.resume()?;
                binding.suspended = false;
            }
        }
        Ok(())
    }

    /// Removes the instance bound to `device_id`.
    ///
    /// Returns `false` if no driver is bound to the device.
    ///
    /// # Errors
    ///
    /// Returns an error if the driver cannot remove its instance.
    pub fn remove(&mut self, device_id: NodeId) -> Result<bool, ProbeError> {
        let Some(index) = self
            .bindings
            .iter()
            .position(|binding| binding.device_id == device_id)
        else {
            return Ok(false);
        };
        self.bindings[index].instance.remove()?;
        self.bindings.remove(index);
        Ok(true)
    }

    /// Returns the driver name bound to `device_id`, if any.
    pub fn binding_driver(&self, device_id: NodeId) -> Option<&'static str> {
        self.bindings
            .iter()
            .find(|binding| binding.device_id == device_id)
            .map(|binding| binding.driver_name)
    }
}

impl Default for DriverManager {
    fn default() -> Self {
        Self::new()
    }
}
