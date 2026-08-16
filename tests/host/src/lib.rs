//! Host-side unit and subsystem tests for architecture-independent code.

#[cfg(test)]
fn require_ok<T, E: core::fmt::Debug>(result: Result<T, E>) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("required success, received {error:?}"),
    }
}

#[cfg(test)]
fn require_some<T>(value: Option<T>) -> T {
    match value {
        Some(value) => value,
        None => panic!("required a value"),
    }
}

#[cfg(test)]
mod virtual_pl011 {
    use hyper::drivers::serial::pl011_registers as reg;
    use hyper::vm::device::pl011::VirtualPl011;

    #[test]
    fn exposes_primecell_identity_and_transmits_bytes() {
        let mut uart = VirtualPl011::new();
        let identity = super::require_ok(uart.read(reg::PERIPH_ID0 as u64, 4));
        assert_eq!(identity.value, Some(reg::PERIPH_ID0_VALUE as u64));

        let output = super::require_ok(uart.write(reg::DR as u64, 4, u64::from(b'X')));
        assert_eq!(output.transmitted, Some(b'X'));
    }

    #[test]
    fn models_receive_fifo_and_level_interrupts() {
        let mut uart = VirtualPl011::new();
        let mask = reg::INT_RX | reg::INT_RT | reg::INT_ERROR_MASK;
        let _ = super::require_ok(uart.write(reg::IMSC as u64, 2, u64::from(mask)));
        assert!(uart.receive(b'A'));

        let status = super::require_ok(uart.read(reg::MIS as u64, 4));
        assert_ne!(
            super::require_some(status.value) & u64::from(reg::INT_RT),
            0
        );
        let data = super::require_ok(uart.read(reg::DR as u64, 4));
        assert_eq!(data.value, Some(u64::from(b'A')));
        assert!(!data.interrupt_asserted);
    }

    #[test]
    fn reports_receive_overrun() {
        let mut uart = VirtualPl011::new();
        let _ = super::require_ok(uart.write(reg::IMSC as u64, 4, u64::from(reg::INT_OE)));
        for value in 0..33 {
            let _ = uart.receive(value);
        }
        let status = super::require_ok(uart.read(reg::RSR_ECR as u64, 4));
        assert_ne!(
            super::require_some(status.value) & u64::from(reg::RSR_OE),
            0
        );
        assert!(status.interrupt_asserted);
    }
}

#[cfg(test)]
mod ns16550 {
    use hyper::drivers::serial::{
        MmioAccess, Ns16550, Ns16550DataBits, Ns16550FifoTrigger, Ns16550LineConfig, Ns16550Parity,
        Ns16550StopBits,
    };
    use hyper::hal::console::Console;

    #[test]
    fn configures_and_uses_a_byte_wide_register_bank() {
        let mut registers = [0u8; 8];
        registers[5] = (1 << 5) | (1 << 6);
        // SAFETY: The test array remains alive and exclusively owned for the
        // complete lifetime of the UART handle.
        let uart = unsafe { Ns16550::from_mmio_base(registers.as_mut_ptr() as usize) };
        super::require_ok(uart.configure(
            24_000_000,
            115_200,
            Ns16550LineConfig::EIGHT_N_ONE,
            Ns16550FifoTrigger::FourteenBytes,
        ));

        assert_eq!(registers[0], 13);
        assert_eq!(registers[3], 3);
        assert_eq!(registers[2], 0xc7);
        assert_eq!(registers[4], 0x0b);
        registers[5] = 1 | (1 << 1) | (1 << 5);
        registers[0] = b'R';
        let received = super::require_some(uart.try_read());
        assert_eq!(received.byte, b'R');
        assert!(received.overrun_error);
        assert!(received.has_error());
        registers[5] = 1 << 5;
        assert!(uart.try_write(b'T'));
        assert_eq!(registers[0], b'T');
    }

    #[test]
    fn honors_word_access_and_register_shift() {
        let mut registers = [0u32; 8];
        registers[5] = 1 << 5;
        // SAFETY: The aligned word array models one word-spaced MMIO bank.
        let uart = super::require_ok(unsafe {
            Ns16550::from_mmio(registers.as_mut_ptr() as usize, MmioAccess::WORD)
        });
        uart.write_byte(b'W');
        assert_eq!(registers[0], u32::from(b'W'));
        uart.write_scratch(0x5a);
        assert_eq!(uart.read_scratch(), 0x5a);

        let line = Ns16550LineConfig {
            data_bits: Ns16550DataBits::Seven,
            stop_bits: Ns16550StopBits::Two,
            parity: Ns16550Parity::Even,
        };
        super::require_ok(uart.configure(1_843_200, 9_600, line, Ns16550FifoTrigger::OneByte));
        assert_eq!(registers[0], 12);
        assert_eq!(registers[3], 0x1e);
    }
}

#[cfg(test)]
mod generic_timer {
    use hyper::drivers::timer::arm_generic::VirtualTimerState;
    use hyper::hal::timer::{
        ConversionError, deadline_reached, nanoseconds_to_ticks, ticks_to_nanoseconds,
    };

    #[test]
    fn converts_time_without_early_deadlines() {
        assert_eq!(super::require_ok(nanoseconds_to_ticks(1, 24_000_000)), 1);
        assert_eq!(
            super::require_ok(nanoseconds_to_ticks(1_000_000_000, 24_000_000)),
            24_000_000
        );
        assert_eq!(
            super::require_ok(ticks_to_nanoseconds(24_000_000, 24_000_000)),
            1_000_000_000
        );
    }

    #[test]
    fn rejects_invalid_or_unrepresentable_conversions() {
        assert_eq!(
            nanoseconds_to_ticks(1, 0),
            Err(ConversionError::InvalidFrequency)
        );
        assert_eq!(
            nanoseconds_to_ticks(u64::MAX, u64::MAX),
            Err(ConversionError::Overflow)
        );
        assert_eq!(
            ticks_to_nanoseconds(u64::MAX, 1),
            Err(ConversionError::Overflow)
        );
    }

    #[test]
    fn compares_deadlines_across_counter_wraparound() {
        assert!(!deadline_reached(u64::MAX - 2, 1));
        assert!(deadline_reached(1, 1));
        assert!(deadline_reached(2, u64::MAX));
    }

    #[test]
    fn models_guest_offset_masking_and_level_output() {
        let mut timer = VirtualTimerState::empty();
        timer.set_offset(1_000);
        timer.set_compare_value(250);
        timer.set_enabled(true);

        assert!(!timer.interrupt_asserted_at(1_249));
        assert!(timer.interrupt_asserted_at(1_250));
        timer.set_masked(true);
        assert!(!timer.interrupt_asserted_at(2_000));

        timer.restore_hardware_state(2_000, 10, 0b111);
        assert!(timer.enabled());
        assert!(timer.masked());
        assert_eq!(timer.writable_control(), 0b11);
    }
}

#[cfg(test)]
mod software_timers {
    use hyper::time::{DeadlineQueue, TimerEvent, TimerMode, TimerQueueError};

    fn callback(_: TimerEvent, _: usize) {}

    #[test]
    fn orders_cancels_and_reschedules_deadlines() {
        let mut queue = DeadlineQueue::<4>::new();
        let late = super::require_ok(queue.schedule(30, TimerMode::OneShot, callback, 30));
        let early = super::require_ok(queue.schedule(10, TimerMode::OneShot, callback, 10));
        let cancelled = super::require_ok(queue.schedule(20, TimerMode::OneShot, callback, 20));
        super::require_ok(queue.reschedule(late, 5));
        super::require_ok(queue.cancel(cancelled));

        assert!(queue.pop_expired(4).is_none());
        let (event, _, context) = super::require_some(queue.pop_expired(5));
        assert_eq!(event.handle, late);
        assert_eq!(context, 30);
        assert_eq!(queue.cancel(late), Err(TimerQueueError::InvalidHandle));
        let (event, _, context) = super::require_some(queue.pop_expired(10));
        assert_eq!(event.handle, early);
        assert_eq!(context, 10);
        assert_eq!(queue.next_deadline(), None);

        let stats = queue.stats();
        assert_eq!(stats.peak_timers, 3);
        assert_eq!(stats.cancellations, 1);
        assert_eq!(stats.reschedules, 1);
        assert_eq!(stats.callbacks, 2);
    }

    #[test]
    fn periodic_timer_preserves_phase_and_reports_overruns() {
        let mut queue = DeadlineQueue::<2>::new();
        let periodic = super::require_ok(queue.schedule(
            100,
            TimerMode::Periodic { interval: 10 },
            callback,
            7,
        ));

        let (event, _, context) = super::require_some(queue.pop_expired(135));
        assert_eq!(event.handle, periodic);
        assert_eq!(event.deadline, 100);
        assert_eq!(event.overruns, 3);
        assert_eq!(context, 7);
        assert_eq!(queue.next_deadline(), Some(140));
        super::require_ok(queue.cancel(periodic));
        assert_eq!(queue.stats().overruns, 3);
    }

    #[test]
    fn preserves_fifo_order_for_equal_deadlines() {
        let mut queue = DeadlineQueue::<3>::new();
        for context in 1..=3 {
            super::require_ok(queue.schedule(10, TimerMode::OneShot, callback, context));
        }
        for expected in 1..=3 {
            let (_, _, context) = super::require_some(queue.pop_expired(10));
            assert_eq!(context, expected);
        }
    }

    #[test]
    fn rejects_invalid_periodic_intervals() {
        let mut queue = DeadlineQueue::<2>::new();
        assert_eq!(
            queue.schedule(10, TimerMode::Periodic { interval: 0 }, callback, 0,),
            Err(TimerQueueError::InvalidInterval)
        );
        assert_eq!(
            queue.schedule(
                10,
                TimerMode::Periodic {
                    interval: 1u64 << 63,
                },
                callback,
                0,
            ),
            Err(TimerQueueError::InvalidInterval)
        );
        assert_eq!(queue.stats().schedule_failures, 2);
        assert_eq!(queue.stats().active_timers, 0);
    }

    #[test]
    fn rejects_stale_handles_and_capacity_overflow() {
        let mut queue = DeadlineQueue::<1>::new();
        let old = super::require_ok(queue.schedule(1, TimerMode::OneShot, callback, 0));
        assert_eq!(
            queue.schedule(2, TimerMode::OneShot, callback, 0),
            Err(TimerQueueError::Full)
        );
        super::require_ok(queue.cancel(old));
        let replacement = super::require_ok(queue.schedule(3, TimerMode::OneShot, callback, 0));
        assert_ne!(replacement, old);
        assert_eq!(queue.cancel(old), Err(TimerQueueError::InvalidHandle));

        let mut other_queue = DeadlineQueue::<1>::with_id(1);
        assert_eq!(
            other_queue.cancel(replacement),
            Err(TimerQueueError::InvalidHandle)
        );
    }

    #[test]
    fn binds_a_static_queue_identity_only_before_first_use() {
        let mut queue = DeadlineQueue::<1>::new();
        super::require_ok(queue.initialize_id(7));
        let handle = super::require_ok(queue.schedule(1, TimerMode::OneShot, callback, 0));
        assert_eq!(
            queue.initialize_id(8),
            Err(TimerQueueError::QueueAlreadyUsed)
        );

        let mut other_queue = DeadlineQueue::<1>::new();
        assert_eq!(
            other_queue.cancel(handle),
            Err(TimerQueueError::InvalidHandle)
        );
    }

    #[test]
    fn compares_deadlines_across_counter_wraparound() {
        let mut queue = DeadlineQueue::<2>::new();
        let before_wrap =
            super::require_ok(queue.schedule(u64::MAX - 2, TimerMode::OneShot, callback, 1));
        let after_wrap = super::require_ok(queue.schedule(3, TimerMode::OneShot, callback, 2));

        assert_eq!(
            super::require_some(queue.pop_expired(u64::MAX - 1))
                .0
                .handle,
            before_wrap
        );
        assert_eq!(
            super::require_some(queue.pop_expired(3)).0.handle,
            after_wrap
        );
    }
}

#[cfg(test)]
mod cpio {
    use hyper::archive::cpio::{Archive, EntryKind, Error};
    use hyper::vm::bundle;

    fn append_hex(output: &mut Vec<u8>, value: u32) {
        output.extend_from_slice(format!("{value:08x}").as_bytes());
    }

    fn append_entry(output: &mut Vec<u8>, name: &str, mode: u32, data: &[u8]) {
        append_entry_with_checksum(output, name, mode, data, false);
    }

    fn append_entry_with_checksum(
        output: &mut Vec<u8>,
        name: &str,
        mode: u32,
        data: &[u8],
        checksum: bool,
    ) {
        output.extend_from_slice(if checksum { b"070702" } else { b"070701" });
        append_hex(output, 1);
        append_hex(output, mode);
        for value in [0, 0, 1, 0] {
            append_hex(output, value);
        }
        append_hex(output, data.len() as u32);
        for value in [0, 0, 0, 0] {
            append_hex(output, value);
        }
        append_hex(output, (name.len() + 1) as u32);
        append_hex(
            output,
            if checksum {
                data.iter()
                    .fold(0u32, |sum, byte| sum.wrapping_add(u32::from(*byte)))
            } else {
                0
            },
        );
        output.extend_from_slice(name.as_bytes());
        output.push(0);
        while output.len() & 3 != 0 {
            output.push(0);
        }
        output.extend_from_slice(data);
        while output.len() & 3 != 0 {
            output.push(0);
        }
    }

    fn archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut output = Vec::new();
        for (name, data) in entries {
            append_entry(&mut output, name, 0o100_644, data);
        }
        append_entry(&mut output, "TRAILER!!!", 0, &[]);
        output
    }

    #[test]
    fn parses_newc_files_without_allocating_in_the_parser() {
        let bytes = archive(&[("hypervisor/boot.conf", b"default=demo"), ("empty", b"")]);
        let archive = super::require_ok(Archive::new(&bytes));
        let entry = super::require_some(super::require_ok(
            archive.find_unique("hypervisor/boot.conf"),
        ));
        assert_eq!(entry.kind(), EntryKind::File);
        assert_eq!(entry.data(), b"default=demo");
        assert_eq!(super::require_ok(archive.find_unique("missing")), None);
    }

    #[test]
    fn rejects_duplicate_and_truncated_entries() {
        let duplicate = archive(&[("manifest", b"one"), ("manifest", b"two")]);
        let parsed = super::require_ok(Archive::new(&duplicate));
        assert_eq!(parsed.find_unique("manifest"), Err(Error::DuplicateEntry));

        let mut truncated = archive(&[("manifest", b"payload")]);
        truncated.truncate(truncated.len() - 120);
        assert!(Archive::new(&truncated).is_err());
    }

    #[test]
    fn validates_crc_archives() {
        let mut bytes = Vec::new();
        append_entry_with_checksum(&mut bytes, "payload", 0o100_644, b"checked", true);
        append_entry(&mut bytes, "TRAILER!!!", 0, &[]);
        let parsed = super::require_ok(Archive::new(&bytes));
        let payload = super::require_some(super::require_ok(parsed.find_unique("payload")));
        assert_eq!(payload.data(), b"checked");

        let data_offset = payload.data().as_ptr() as usize - bytes.as_ptr() as usize;
        bytes[data_offset] ^= 1;
        assert_eq!(
            Archive::new(&bytes).map(|_| ()),
            Err(Error::InvalidChecksum)
        );

        let mut newc_with_checksum = archive(&[("payload", b"unchecked")]);
        newc_with_checksum[102..110].copy_from_slice(b"00000001");
        assert_eq!(
            Archive::new(&newc_with_checksum).map(|_| ()),
            Err(Error::InvalidChecksum)
        );
    }

    #[test]
    fn loads_a_versioned_nested_vm_bundle() {
        let mut kernel = [0u8; 64];
        kernel[56..60].copy_from_slice(b"ARMd");
        let manifest = b"format=hyper-vm-v1\ntype=linux\narchitecture=aarch64\nmemory=134217728\nvcpus=1\ncommand_line=console=ttyAMA0 rdinit=/init\nkernel=kernel/Image\ninitramfs=initramfs/root.cpio\n";
        let inner = archive(&[
            ("manifest", manifest),
            ("kernel/Image", &kernel),
            ("initramfs/root.cpio", b"guest initramfs"),
        ]);
        let outer = archive(&[
            (
                "hypervisor/boot.conf",
                b"format=hyper-boot-v1\ndefault=demo\n",
            ),
            ("hypervisor/vms/demo.cpio", &inner),
        ]);
        let guest = super::require_ok(bundle::select_default(&outer));
        assert_eq!(guest.name(), "demo");
        assert_eq!(guest.guest_type(), "linux");
        assert_eq!(guest.architecture(), "aarch64");
        assert_eq!(guest.memory_size(), 128 * 1024 * 1024);
        assert_eq!(guest.vcpu_count(), 1);
        assert_eq!(guest.kernel(), &kernel);
        assert_eq!(guest.initramfs(), Some(b"guest initramfs".as_slice()));
    }

    #[test]
    fn rejects_unknown_manifest_properties() {
        let mut kernel = [0u8; 64];
        kernel[56..60].copy_from_slice(b"ARMd");
        let manifest = b"format=hyper-vm-v1\ntype=linux\narchitecture=aarch64\nmemory=134217728\nvcpus=1\ncommand_line=console=ttyAMA0\nkernel=kernel/Image\ninitramfs=\ntypo=value\n";
        let inner = archive(&[("manifest", manifest), ("kernel/Image", &kernel)]);
        let outer = archive(&[
            (
                "hypervisor/boot.conf",
                b"format=hyper-boot-v1\ndefault=demo\n",
            ),
            ("hypervisor/vms/demo.cpio", &inner),
        ]);
        assert_eq!(
            bundle::select_default(&outer).map(|_| ()),
            Err(bundle::Error::UnknownProperty)
        );
    }

    #[test]
    fn keeps_package_parsing_independent_from_runner_capabilities() {
        let manifest = b"format=hyper-vm-v1\ntype=freebsd\narchitecture=x86_64\nmemory=134217728\nvcpus=64\ncommand_line=\nkernel=kernel/guest.bin\ninitramfs=\n";
        let inner = archive(&[("manifest", manifest), ("kernel/guest.bin", b"opaque")]);
        let outer = archive(&[
            (
                "hypervisor/boot.conf",
                b"format=hyper-boot-v1\ndefault=portable\n",
            ),
            ("hypervisor/vms/portable.cpio", &inner),
        ]);
        let guest = super::require_ok(bundle::select_default(&outer));
        assert_eq!(guest.guest_type(), "freebsd");
        assert_eq!(guest.architecture(), "x86_64");
        assert_eq!(guest.vcpu_count(), 64);
        assert_eq!(guest.command_line(), "");
        assert_eq!(guest.kernel(), b"opaque");
    }

    #[test]
    fn reports_the_archive_layer_that_is_malformed() {
        assert!(matches!(
            bundle::select_default(b"invalid"),
            Err(bundle::Error::BootArchive(_))
        ));

        let outer = archive(&[
            (
                "hypervisor/boot.conf",
                b"format=hyper-boot-v1\ndefault=broken\n",
            ),
            ("hypervisor/vms/broken.cpio", b"invalid"),
        ]);
        assert!(matches!(
            bundle::select_default(&outer),
            Err(bundle::Error::BundleArchive(_))
        ));
    }
}

#[cfg(test)]
mod fdt {
    use std::boxed::Box;

    use hyper::{
        drivers::console,
        drivers::platform::{
            DeviceScanner, DriverInstance, DriverManager, DriverServices, PlatformDevice,
            PlatformDriver, ProbeError,
        },
        platform::{chosen, fdt},
    };

    const FDT_MAGIC: u32 = 0xd00d_feed;
    const FDT_BEGIN_NODE: u32 = 1;
    const FDT_END_NODE: u32 = 2;
    const FDT_PROP: u32 = 3;
    const FDT_END: u32 = 9;
    const HEADER_SIZE: usize = 40;

    fn push_u32(output: &mut Vec<u8>, value: u32) {
        output.extend_from_slice(&value.to_be_bytes());
    }

    fn pad(output: &mut Vec<u8>) {
        while output.len() & 3 != 0 {
            output.push(0);
        }
    }

    fn begin_node(structure: &mut Vec<u8>, name: &[u8]) {
        push_u32(structure, FDT_BEGIN_NODE);
        structure.extend_from_slice(name);
        structure.push(0);
        pad(structure);
    }

    fn property(structure: &mut Vec<u8>, name_offset: u32, value: &[u8]) {
        push_u32(structure, FDT_PROP);
        push_u32(structure, value.len() as u32);
        push_u32(structure, name_offset);
        structure.extend_from_slice(value);
        pad(structure);
    }

    fn cells(values: &[u32]) -> Vec<u8> {
        values
            .iter()
            .flat_map(|value| value.to_be_bytes())
            .collect()
    }

    fn qemu_like_dtb() -> Vec<u8> {
        const ADDRESS_CELLS: u32 = 0;
        const SIZE_CELLS: u32 = 15;
        const REG: u32 = 27;
        const COMPATIBLE: u32 = 31;
        const DEVICE_TYPE: u32 = 42;
        const RANGES: u32 = 54;
        const NO_MAP: u32 = 61;
        const INTERRUPTS: u32 = 68;
        const METHOD: u32 = 79;
        const STATUS: u32 = 86;
        const BOOTARGS: u32 = 93;
        const KASLR_SEED: u32 = 102;
        const INITRD_START: u32 = 113;
        const INITRD_END: u32 = 132;

        let strings = b"#address-cells\0#size-cells\0reg\0compatible\0device_type\0ranges\0no-map\0interrupts\0method\0status\0bootargs\0kaslr-seed\0linux,initrd-start\0linux,initrd-end\0";
        let mut structure = Vec::new();
        begin_node(&mut structure, b"");
        property(&mut structure, ADDRESS_CELLS, &2u32.to_be_bytes());
        property(&mut structure, SIZE_CELLS, &2u32.to_be_bytes());
        begin_node(&mut structure, b"chosen");
        property(
            &mut structure,
            BOOTARGS,
            b"earlycon=pl011,mmio32,0x09000000 loglevel=7\0",
        );
        property(
            &mut structure,
            KASLR_SEED,
            &0x0123_4567_89ab_cdef_u64.to_be_bytes(),
        );
        property(
            &mut structure,
            INITRD_START,
            &0x0000_0000_4800_0000_u64.to_be_bytes(),
        );
        property(
            &mut structure,
            INITRD_END,
            &0x0000_0000_4900_0000_u64.to_be_bytes(),
        );
        push_u32(&mut structure, FDT_END_NODE);
        begin_node(&mut structure, b"memory@40000000");
        property(&mut structure, DEVICE_TYPE, b"memory\0");
        property(
            &mut structure,
            REG,
            &[0, 0, 0, 0, 0x40, 0, 0, 0, 0, 0, 0, 0, 0x20, 0, 0, 0],
        );
        push_u32(&mut structure, FDT_END_NODE);
        begin_node(&mut structure, b"cpus");
        property(&mut structure, ADDRESS_CELLS, &2u32.to_be_bytes());
        property(&mut structure, SIZE_CELLS, &0u32.to_be_bytes());
        for hardware_id in 0..5u32 {
            begin_node(&mut structure, b"cpu");
            property(&mut structure, DEVICE_TYPE, b"cpu\0");
            property(&mut structure, REG, &cells(&[0, hardware_id]));
            if hardware_id == 4 {
                property(&mut structure, STATUS, b"fail\0");
            }
            push_u32(&mut structure, FDT_END_NODE);
        }
        push_u32(&mut structure, FDT_END_NODE);
        begin_node(&mut structure, b"reserved-memory");
        property(&mut structure, RANGES, &[]);
        property(&mut structure, ADDRESS_CELLS, &2u32.to_be_bytes());
        property(&mut structure, SIZE_CELLS, &2u32.to_be_bytes());
        begin_node(&mut structure, b"firmware@41000000");
        property(
            &mut structure,
            REG,
            &[0, 0, 0, 0, 0x41, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x20, 0],
        );
        property(&mut structure, NO_MAP, &[]);
        push_u32(&mut structure, FDT_END_NODE);
        push_u32(&mut structure, FDT_END_NODE);
        begin_node(&mut structure, b"intc@8000000");
        property(
            &mut structure,
            REG,
            &cells(&[
                0,
                0x0800_0000,
                0,
                0x0001_0000,
                0,
                0x080a_0000,
                0,
                0x00f6_0000,
            ]),
        );
        property(&mut structure, COMPATIBLE, b"arm,gic-v3\0");
        push_u32(&mut structure, FDT_END_NODE);
        begin_node(&mut structure, b"timer");
        property(
            &mut structure,
            COMPATIBLE,
            b"arm,armv8-timer\0arm,armv7-timer\0",
        );
        property(
            &mut structure,
            INTERRUPTS,
            &cells(&[1, 13, 4, 1, 14, 4, 1, 11, 4, 1, 10, 4]),
        );
        push_u32(&mut structure, FDT_END_NODE);
        begin_node(&mut structure, b"psci");
        property(
            &mut structure,
            COMPATIBLE,
            b"arm,psci-1.0\0arm,psci-0.2\0arm,psci\0",
        );
        property(&mut structure, METHOD, b"smc\0");
        push_u32(&mut structure, FDT_END_NODE);
        begin_node(&mut structure, b"soc");
        property(
            &mut structure,
            RANGES,
            &[0, 0, 0, 0, 0, 0, 0, 0, 0x09, 0, 0, 0, 0, 0x10, 0, 0],
        );
        property(&mut structure, ADDRESS_CELLS, &1u32.to_be_bytes());
        property(&mut structure, SIZE_CELLS, &1u32.to_be_bytes());
        begin_node(&mut structure, b"pl011@0");
        property(&mut structure, REG, &[0, 0, 0, 0, 0, 0, 0x10, 0]);
        property(&mut structure, COMPATIBLE, b"arm,pl011\0arm,primecell\0");
        push_u32(&mut structure, FDT_END_NODE);
        push_u32(&mut structure, FDT_END_NODE);
        push_u32(&mut structure, FDT_END_NODE);
        push_u32(&mut structure, FDT_END);

        let reservation_offset = HEADER_SIZE;
        let structure_offset = reservation_offset + 16;
        let strings_offset = structure_offset + structure.len();
        let total_size = strings_offset + strings.len();
        let mut blob = Vec::new();
        for value in [
            FDT_MAGIC,
            total_size as u32,
            structure_offset as u32,
            strings_offset as u32,
            reservation_offset as u32,
            17,
            16,
            0,
            strings.len() as u32,
            structure.len() as u32,
        ] {
            push_u32(&mut blob, value);
        }
        blob.extend_from_slice(&[0; 16]);
        blob.extend_from_slice(&structure);
        blob.extend_from_slice(strings);
        blob
    }

    fn replace_first(blob: &mut [u8], old: &[u8], new: &[u8]) {
        assert_eq!(old.len(), new.len());
        let index = super::require_some(blob.windows(old.len()).position(|window| window == old));
        blob[index..index + new.len()].copy_from_slice(new);
    }

    #[test]
    fn discovers_generic_devices_and_translates_resources() {
        let blob = qemu_like_dtb();
        let mut scanner = DeviceScanner::new(&[]);
        let mut chosen = chosen::Discovery::new();
        let platform = {
            let mut visitors = fdt::VisitorPair::new(&mut scanner, &mut chosen);
            super::require_ok(fdt::discover_from_bytes_with(&blob, &mut visitors))
        };
        let devices = super::require_ok(scanner.finish());
        let chosen = super::require_ok(chosen.finish());
        let command_line = super::require_some(chosen.command_line());
        assert_eq!(command_line.value("loglevel"), Some("7"));
        assert_eq!(chosen.kaslr_seed(), Some(0x0123_4567_89ab_cdef));
        assert_eq!(chosen.command_line_error(), None);
        assert_eq!(chosen.kaslr_seed_error(), None);
        let initrd = super::require_some(chosen.initial_ramdisk());
        assert_eq!(initrd.start(), 0x4800_0000);
        assert_eq!(initrd.end(), 0x4900_0000);
        assert_eq!(
            super::require_ok(console::early_console(Some(&command_line))),
            Some(hyper::platform::ConsoleInfo {
                kind: hyper::platform::ConsoleKind::Pl011,
                base: 0x0900_0000,
                access: hyper::platform::ConsoleRegisterAccess::Native,
            })
        );

        let console = super::require_some(
            devices
                .iter()
                .find(|device| device.is_compatible("arm,pl011")),
        );
        assert_eq!(console.registers()[0].start(), 0x0900_0000);
        let gic = super::require_some(
            devices
                .iter()
                .find(|device| device.is_compatible("arm,gic-v3")),
        );
        assert_eq!(gic.registers()[0].start(), 0x0800_0000);
        assert_eq!(gic.registers()[1].start(), 0x080a_0000);
        let timer = super::require_some(
            devices
                .iter()
                .find(|device| device.is_compatible("arm,armv8-timer")),
        );
        assert_eq!(&timer.interrupt_cells()[9..12], &[1, 10, 4]);
        let psci = super::require_some(
            devices
                .iter()
                .find(|device| device.is_compatible("arm,psci-1.0")),
        );
        assert_eq!(psci.property("method"), Some(b"smc\0".as_slice()));
        assert_eq!(
            platform.memory.as_slice(),
            &[super::require_some(hyper::platform::PhysicalRange::new(
                0x4000_0000,
                0x2000_0000,
            ))]
        );
        assert_eq!(
            platform
                .cpus
                .as_slice()
                .iter()
                .map(|cpu| cpu.hardware_id)
                .collect::<Vec<_>>(),
            vec![0, 1, 2, 3]
        );
        assert!(
            platform
                .mmio
                .as_slice()
                .iter()
                .any(|range| range.start() <= 0x0900_0000 && 0x0900_1000 <= range.end())
        );
        assert_eq!(
            platform.no_map.as_slice(),
            &[super::require_some(hyper::platform::PhysicalRange::new(
                0x4100_0000,
                0x2000,
            ))]
        );
    }

    #[test]
    fn preserves_the_initrd_when_optional_bootargs_are_invalid() {
        let mut blob = qemu_like_dtb();
        let old = b"earlycon=pl011,mmio32,0x09000000 loglevel=7\0";
        let mut invalid = *old;
        invalid[0] = 0xff;
        replace_first(&mut blob, old, &invalid);

        let mut chosen = chosen::Discovery::new();
        let _ = super::require_ok(fdt::discover_from_bytes_with(&blob, &mut chosen));
        let chosen = super::require_ok(chosen.finish());
        assert!(chosen.command_line().is_none());
        assert_eq!(
            chosen.command_line_error(),
            Some(chosen::Error::InvalidEncoding)
        );
        let initrd = super::require_some(chosen.initial_ramdisk());
        assert_eq!(initrd.start(), 0x4800_0000);
        assert_eq!(initrd.end(), 0x4900_0000);
    }

    #[test]
    fn leaves_the_early_console_disabled_without_an_earlycon_argument() {
        let mut blob = qemu_like_dtb();
        replace_first(&mut blob, b"earlycon", b"consolex");
        let mut discovery = chosen::Discovery::new();
        let _platform = super::require_ok(fdt::discover_from_bytes_with(&blob, &mut discovery));
        let chosen = super::require_ok(discovery.finish());
        let command_line = super::require_some(chosen.command_line());

        assert_eq!(
            super::require_ok(console::early_console(Some(&command_line))),
            None
        );
    }

    #[test]
    fn rejects_an_invalid_early_console_address_without_rejecting_the_dtb() {
        let mut blob = qemu_like_dtb();
        replace_first(&mut blob, b"0x09000000", b"not-an-hex");
        let mut discovery = chosen::Discovery::new();
        let _platform = super::require_ok(fdt::discover_from_bytes_with(&blob, &mut discovery));
        let chosen = super::require_ok(discovery.finish());
        let command_line = super::require_some(chosen.command_line());

        assert_eq!(
            console::early_console(Some(&command_line)),
            Err(console::EarlyConsoleError::InvalidAddress)
        );
    }

    #[test]
    fn admits_only_available_device_tree_status_values() {
        let mut blob = qemu_like_dtb();
        replace_first(&mut blob, b"fail\0", b"okay\0");

        let platform = super::require_ok(fdt::discover_from_bytes(&blob));
        assert_eq!(platform.cpus.len(), 5);

        replace_first(&mut blob, b"okay\0", b"nope\0");
        let platform = super::require_ok(fdt::discover_from_bytes(&blob));
        assert_eq!(platform.cpus.len(), 4);
    }

    #[test]
    fn rejects_a_bad_magic_number() {
        let mut blob = qemu_like_dtb();
        blob[0] = 0;

        assert!(matches!(
            fdt::discover_from_bytes(&blob),
            Err(fdt::Error::BadMagic)
        ));
    }

    #[test]
    fn discovers_the_hvc_psci_conduit() {
        let mut blob = qemu_like_dtb();
        replace_first(&mut blob, b"smc\0", b"hvc\0");

        let mut scanner = DeviceScanner::new(&[]);
        let _platform = super::require_ok(fdt::discover_from_bytes_with(&blob, &mut scanner));
        let devices = super::require_ok(scanner.finish());
        let psci = super::require_some(
            devices
                .iter()
                .find(|device| device.is_compatible("arm,psci-1.0")),
        );
        assert_eq!(psci.property("method"), Some(b"hvc\0".as_slice()));
    }

    #[test]
    fn leaves_legacy_psci_policy_to_the_architecture() {
        let mut blob = qemu_like_dtb();
        replace_first(&mut blob, b"arm,psci-1.0", b"old,psci-1.0");
        replace_first(&mut blob, b"arm,psci-0.2", b"old,psci-0.2");

        let mut scanner = DeviceScanner::new(&[]);
        let _platform = super::require_ok(fdt::discover_from_bytes_with(&blob, &mut scanner));
        let devices = super::require_ok(scanner.finish());
        assert!(
            devices
                .iter()
                .any(|device| device.is_compatible("arm,psci"))
        );
    }

    #[test]
    fn rejects_a_dtb_larger_than_the_linux_boot_limit() {
        let mut blob = qemu_like_dtb();
        blob[4..8].copy_from_slice(&(2_097_153u32).to_be_bytes());

        assert!(matches!(
            fdt::discover_from_bytes(&blob),
            Err(fdt::Error::TooLarge)
        ));
    }

    struct TestServices;

    impl DriverServices for TestServices {
        fn map_mmio(&self, physical_address: u64) -> Option<usize> {
            usize::try_from(physical_address).ok()
        }
    }

    struct TestInstance;

    impl DriverInstance for TestInstance {}

    struct TestDriver;

    impl PlatformDriver for TestDriver {
        fn name(&self) -> &'static str {
            "test-pl011"
        }

        fn compatible_table(&self) -> &'static [&'static str] {
            &["arm,pl011"]
        }

        fn probe(
            &self,
            device: &PlatformDevice,
            services: &dyn DriverServices,
        ) -> Result<Box<dyn DriverInstance>, ProbeError> {
            let register = device.registers().first().ok_or(ProbeError::Resource)?;
            services
                .map_mmio(register.start())
                .ok_or(ProbeError::Resource)?;
            Ok(Box::new(TestInstance))
        }
    }

    static TEST_DRIVER: TestDriver = TestDriver;

    #[test]
    fn matches_and_binds_a_registered_platform_driver() {
        let blob = qemu_like_dtb();
        let mut scanner = DeviceScanner::new(&[]);
        let _platform = super::require_ok(fdt::discover_from_bytes_with(&blob, &mut scanner));
        let devices = super::require_ok(scanner.finish());
        let console = super::require_some(
            devices
                .iter()
                .find(|device| device.is_compatible("arm,pl011")),
        );
        let mut manager = DriverManager::new();
        super::require_ok(manager.register(&TEST_DRIVER));
        let report = manager.probe_devices(&devices, &TestServices);

        assert_eq!(report.bound, 1);
        assert_eq!(manager.binding_driver(console.id()), Some("test-pl011"));
        assert!(super::require_ok(manager.remove(console.id())));
    }
}

#[cfg(test)]
mod physical_ranges {
    use hyper::platform::{PhysicalRange, RegionList};

    #[test]
    fn rejects_empty_and_overflowing_ranges() {
        assert!(PhysicalRange::new(0, 0).is_none());
        assert!(PhysicalRange::new(u64::MAX, 2).is_none());
        assert_eq!(super::require_some(PhysicalRange::new(4, 8)).end(), 12);
    }

    #[test]
    fn coalesces_adjacent_ranges_and_preserves_capacity_on_failure() {
        let mut regions = RegionList::<2>::new();
        super::require_ok(regions.insert(super::require_some(PhysicalRange::new(0x3000, 0x1000))));
        super::require_ok(regions.insert(super::require_some(PhysicalRange::new(0x1000, 0x2000))));
        assert_eq!(
            regions.as_slice(),
            &[super::require_some(PhysicalRange::new(0x1000, 0x3000))]
        );

        super::require_ok(regions.insert(super::require_some(PhysicalRange::new(0x8000, 0x1000))));
        assert!(
            regions
                .insert(super::require_some(PhysicalRange::new(0xa000, 0x1000)))
                .is_err()
        );
        assert_eq!(regions.len(), 2);
    }
}

#[cfg(test)]
mod kaslr {
    use hyper::mm::kaslr::{self, Error};

    #[test]
    fn selects_a_reproducible_aligned_offset_inside_the_window() {
        let first = super::require_ok(kaslr::select_offset(
            0x0123_4567_89ab_cdef,
            0x90000,
            512 * 1024 * 1024 * 1024,
            2 * 1024 * 1024,
        ));
        let second = super::require_ok(kaslr::select_offset(
            0x0123_4567_89ab_cdef,
            0x90000,
            512 * 1024 * 1024 * 1024,
            2 * 1024 * 1024,
        ));

        assert_eq!(first, second);
        assert_eq!(first % (2 * 1024 * 1024), 0);
        assert!(first + 0x20_0000 <= 512 * 1024 * 1024 * 1024);
    }

    #[test]
    fn rejects_invalid_kaslr_geometry() {
        assert_eq!(
            kaslr::select_offset(1, 0, 0x4000_0000, 0x20_0000),
            Err(Error::InvalidImage)
        );
        assert_eq!(
            kaslr::select_offset(1, 0x1000, 0x4000_0000, 0x30_0000),
            Err(Error::InvalidAlignment)
        );
        assert_eq!(
            kaslr::select_offset(1, 0x8000_0000, 0x4000_0000, 0x20_0000),
            Err(Error::ImageTooLarge)
        );
    }
}

#[cfg(test)]
mod kallsyms {
    use hyper::debug::kallsyms::{
        COMPACT_HEADER_SIZE, COMPACT_MAGIC, COMPACT_RECORD_SIZE, COMPACT_VERSION,
        CompactSymbolTable, Error, SymbolTable,
    };

    fn symbol(name: u32, info: u8, section: u16, value: u64, size: u64) -> [u8; 24] {
        let mut bytes = [0u8; 24];
        bytes[0..4].copy_from_slice(&name.to_le_bytes());
        bytes[4] = info;
        bytes[6..8].copy_from_slice(&section.to_le_bytes());
        bytes[8..16].copy_from_slice(&value.to_le_bytes());
        bytes[16..24].copy_from_slice(&size.to_le_bytes());
        bytes
    }

    #[test]
    fn resolves_the_nearest_preceding_runtime_function() {
        let mut symbols = Vec::new();
        symbols.extend_from_slice(&symbol(0, 0, 0, 0, 0));
        symbols.extend_from_slice(&symbol(1, 2, 1, 0x100, 0x40));
        symbols.extend_from_slice(&symbol(7, 2, 1, 0x180, 0x20));
        let table = super::require_ok(SymbolTable::new(
            &symbols,
            b"\0first\0second\0",
            0xff00_2000_0000,
            0x1000,
        ));

        let resolved = super::require_some(super::require_ok(table.lookup(0xff00_2000_0118)));
        assert_eq!(resolved.name, "first");
        assert_eq!(resolved.address, 0xff00_2000_0100);
        assert_eq!(resolved.size, 0x40);
        assert_eq!(resolved.offset, 0x18);

        let resolved = super::require_some(super::require_ok(table.lookup(0xff00_2000_0188)));
        assert_eq!(resolved.name, "second");
        assert_eq!(resolved.offset, 8);
        assert!(super::require_ok(table.lookup_containing(0xff00_2000_0148)).is_none());
        let contained =
            super::require_some(super::require_ok(table.lookup_containing(0xff00_2000_0188)));
        assert_eq!(contained.name, "second");
        assert!(super::require_ok(table.lookup(0x100)).is_none());
    }

    #[test]
    fn displays_runtime_symbols_with_rust_names_demangled() {
        let symbol = hyper::debug::kallsyms::Symbol {
            name: "_RNvCskwGfYPst2Cb_3foo16example_function",
            address: 0x1000,
            size: 0x40,
            offset: 0x18,
        };
        assert_eq!(symbol.to_string(), "foo::example_function+0x18/0x40");

        let foreign = hyper::debug::kallsyms::Symbol {
            name: "start_kernel",
            address: 0x2000,
            size: 0x20,
            offset: 4,
        };
        assert_eq!(foreign.to_string(), "start_kernel+0x4/0x20");
    }

    #[test]
    fn resolves_a_containing_symbol_from_the_compact_runtime_table() {
        let strings = b"first\0second\0";
        let strings_offset = COMPACT_HEADER_SIZE + 2 * COMPACT_RECORD_SIZE;
        let mut image = vec![0; strings_offset + strings.len()];
        image[..8].copy_from_slice(&COMPACT_MAGIC);
        put_u32(&mut image, 8, COMPACT_VERSION);
        put_u32(&mut image, 12, 2);
        put_u32(&mut image, 16, strings_offset as u32);
        put_u32(&mut image, 20, strings.len() as u32);
        put_record(&mut image, 0, 0x100, 0x40, 0);
        put_record(&mut image, 1, 0x180, 0x20, 6);
        image[strings_offset..].copy_from_slice(strings);

        let base = 0xff00_4000_0000;
        let table = super::require_ok(CompactSymbolTable::new(&image, base, 0x1000));
        let symbol = super::require_some(super::require_ok(table.lookup_containing(base + 0x188)));
        assert_eq!(symbol.name, "second");
        assert_eq!(symbol.offset, 8);
        assert!(super::require_ok(table.lookup_containing(base + 0x160)).is_none());
    }

    fn put_record(image: &mut [u8], index: usize, address: u64, size: u32, name: u32) {
        let offset = COMPACT_HEADER_SIZE + index * COMPACT_RECORD_SIZE;
        image[offset..offset + 8].copy_from_slice(&address.to_le_bytes());
        put_u32(image, offset + 8, size);
        put_u32(image, offset + 12, name);
    }

    fn put_u32(image: &mut [u8], offset: usize, value: u32) {
        image[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    #[test]
    fn rejects_malformed_symbol_metadata() {
        assert!(matches!(
            SymbolTable::new(&[0; 23], b"\0", 0, 0x1000),
            Err(Error::InvalidSymbolTable)
        ));
        assert!(matches!(
            SymbolTable::new(&[0; 24], b"bad", 0, 0x1000),
            Err(Error::InvalidStringTable)
        ));
    }
}

#[cfg(test)]
mod kernel_log {
    use hyper::log::{Level, ReadResult, RecordFlags, RingBuffer};

    #[test]
    fn preserves_record_metadata_across_wraparound() {
        let mut ring = RingBuffer::<64>::new();
        let first = super::require_ok(ring.append(Level::Info, b"first", RecordFlags::NONE));
        let second = super::require_ok(ring.append(Level::Warning, b"second", RecordFlags::NONE));
        assert_eq!(first, 0);
        assert_eq!(second, 1);

        let mut output = [0; 16];
        let record = match super::require_ok(ring.read(second, &mut output)) {
            ReadResult::Record(record) => record,
            result => panic!("required a record, received {result:?}"),
        };
        assert_eq!(record.level, Level::Warning);
        assert_eq!(&output[..record.copied], b"second");

        for index in 0..8u8 {
            super::require_ok(ring.append(Level::Debug, &[index; 12], RecordFlags::NONE));
        }
        assert!(ring.dropped() != 0);
        assert!(matches!(
            super::require_ok(ring.read(first, &mut output)),
            ReadResult::Overrun { .. }
        ));
    }

    #[test]
    fn truncates_a_record_that_exceeds_the_ring_capacity() {
        let mut ring = RingBuffer::<32>::new();
        let sequence = super::require_ok(ring.append(
            Level::Error,
            b"a message that cannot fit in this tiny ring",
            RecordFlags::NONE,
        ));
        let mut output = [0; 32];
        let record = match super::require_ok(ring.read(sequence, &mut output)) {
            ReadResult::Record(record) => record,
            result => panic!("required a record, received {result:?}"),
        };
        assert!(record.flags.contains(RecordFlags::TRUNCATED));
        assert_eq!(record.length, 16);
    }

    #[test]
    fn reports_empty_buffers_and_partial_reads() {
        let mut ring = RingBuffer::<64>::new();
        let mut output = [0; 3];
        assert_eq!(
            super::require_ok(ring.read(0, &mut output)),
            ReadResult::Empty { next_sequence: 0 }
        );

        let sequence = super::require_ok(ring.append(Level::Notice, b"abcdef", RecordFlags::NONE));
        let record = match super::require_ok(ring.read(sequence, &mut output)) {
            ReadResult::Record(record) => record,
            result => panic!("required a record, received {result:?}"),
        };
        assert_eq!(record.length, 6);
        assert_eq!(record.copied, 3);
        assert_eq!(&output, b"abc");
    }

    #[test]
    fn rejects_a_ring_smaller_than_its_record_header() {
        let mut ring = RingBuffer::<8>::new();
        assert_eq!(
            ring.append(Level::Info, b"message", RecordFlags::NONE),
            Err(hyper::log::AppendError::BufferTooSmall)
        );
    }
}

#[cfg(test)]
mod psci {
    use core::sync::atomic::{AtomicU32, Ordering};
    use std::sync::{Mutex, MutexGuard};

    use hyper::drivers::power::psci::{CallWidth, Conduit, Error, Psci};
    use hyper::hal::cpu_power::{CpuHardwareId, CpuPower, ResumeAddress};
    use hyper::platform::PsciCompatibleVersion;

    const PSCI_VERSION: u32 = 0x8400_0000;
    const PSCI_FEATURES: u32 = 0x8400_000a;
    const PSCI_CPU_ON_32: u32 = 0x8400_0003;
    const PSCI_CPU_ON_64: u32 = 0xc400_0003;

    static LAST_FUNCTION: AtomicU32 = AtomicU32::new(0);
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    #[derive(Clone, Copy)]
    struct Fake32;

    #[derive(Clone, Copy)]
    struct Fake64;

    fn invoke(function_id: u32) -> u64 {
        LAST_FUNCTION.store(function_id, Ordering::Release);
        match function_id {
            PSCI_VERSION => 0x0001_0001,
            PSCI_FEATURES => 0,
            _ => 0,
        }
    }

    fn test_lock() -> MutexGuard<'static, ()> {
        match TEST_LOCK.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    impl Conduit for Fake32 {
        const CALL_WIDTH: CallWidth = CallWidth::Bits32;

        fn invoke(
            self,
            function_id: u32,
            _argument0: u64,
            _argument1: u64,
            _argument2: u64,
        ) -> u64 {
            invoke(function_id)
        }
    }

    impl Conduit for Fake64 {
        const CALL_WIDTH: CallWidth = CallWidth::Bits64;

        fn invoke(
            self,
            function_id: u32,
            _argument0: u64,
            _argument1: u64,
            _argument2: u64,
        ) -> u64 {
            invoke(function_id)
        }
    }

    #[test]
    fn selects_smccc32_function_ids_for_a_32_bit_conduit() {
        let _guard = test_lock();
        let controller = super::require_ok(Psci::initialize(Fake32, PsciCompatibleVersion::V1_0));
        super::require_ok(controller.cpu_on(CpuHardwareId::new(1), ResumeAddress::new(0x8000), 7));
        assert_eq!(LAST_FUNCTION.load(Ordering::Acquire), PSCI_CPU_ON_32);
        assert_eq!(
            controller.cpu_on(
                CpuHardwareId::new(1),
                ResumeAddress::new(u64::from(u32::MAX) + 1),
                0,
            ),
            Err(Error::InvalidAddress)
        );
    }

    #[test]
    fn selects_smccc64_function_ids_for_a_64_bit_conduit() {
        let _guard = test_lock();
        let controller = super::require_ok(Psci::initialize(Fake64, PsciCompatibleVersion::V1_0));
        super::require_ok(controller.cpu_on(
            CpuHardwareId::new(1),
            ResumeAddress::new(0x1_0000_8000),
            7,
        ));
        assert_eq!(LAST_FUNCTION.load(Ordering::Acquire), PSCI_CPU_ON_64);
    }
}

#[cfg(test)]
mod synchronization {
    use core::sync::atomic::{AtomicUsize, Ordering};

    use hyper::hal::interrupt::InterruptMask;
    use hyper::sync::InterruptSpinLock;
    use hyper::sync::atomic::{AtomicFlag, AtomicU64, Ordering as AtomicOrdering, fence};

    static MASK_DEPTH: AtomicUsize = AtomicUsize::new(0);

    struct TestInterruptMask;

    impl InterruptMask for TestInterruptMask {
        type State = usize;

        fn save_and_disable() -> Self::State {
            MASK_DEPTH.fetch_add(1, Ordering::SeqCst)
        }

        fn restore(state: Self::State) {
            MASK_DEPTH.store(state, Ordering::SeqCst);
        }
    }

    #[test]
    fn interrupt_lock_restores_the_previous_mask_state() {
        let lock = InterruptSpinLock::<_, TestInterruptMask>::new(41usize);
        lock.with(|value| {
            assert_eq!(MASK_DEPTH.load(Ordering::SeqCst), 1);
            *value += 1;
            assert!(lock.try_with(|_| ()).is_none());
            assert_eq!(MASK_DEPTH.load(Ordering::SeqCst), 1);
        });
        assert_eq!(MASK_DEPTH.load(Ordering::SeqCst), 0);
        assert_eq!(lock.try_with(|value| *value), Some(42));
        assert_eq!(MASK_DEPTH.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn atomic_flag_and_counter_use_explicit_ordering() {
        let flag = AtomicFlag::default();
        assert!(flag.try_acquire());
        assert!(!flag.try_acquire());
        assert!(flag.is_acquired(AtomicOrdering::Relaxed));
        flag.release();
        assert!(!flag.is_acquired(AtomicOrdering::Acquire));

        let counter = AtomicU64::new(40);
        assert_eq!(counter.fetch_add(2, AtomicOrdering::AcqRel), 40);
        fence(AtomicOrdering::SeqCst);
        assert_eq!(counter.load(AtomicOrdering::Acquire), 42);
    }
}

#[cfg(test)]
mod gicv3 {
    use core::ptr::{read_volatile, write_volatile};
    use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

    use hyper::drivers::interrupt::gicv3::{CpuInterface, GicV3};
    use hyper::hal::barrier::{Barrier, BarrierAccess, BarrierDomain};
    use hyper::hal::interrupt::{InterruptController, InterruptId, InterruptTrigger};
    use hyper::platform::{GicV3Info, MAX_GIC_REDISTRIBUTOR_REGIONS, PhysicalRange, RegionList};

    static CPU_INITIALIZED: AtomicBool = AtomicBool::new(false);
    static ACKNOWLEDGED: AtomicU32 = AtomicU32::new(40);
    static COMPLETED: AtomicU32 = AtomicU32::new(u32::MAX);
    static CURRENT_AFFINITY: AtomicU32 = AtomicU32::new(0);

    struct TestCpuInterface;

    impl CpuInterface for TestCpuInterface {
        unsafe fn initialize() -> bool {
            CPU_INITIALIZED.store(true, Ordering::Release);
            true
        }

        fn acknowledge() -> u32 {
            ACKNOWLEDGED.load(Ordering::Acquire)
        }

        fn end(interrupt: u32) {
            COMPLETED.store(interrupt, Ordering::Release);
        }

        fn affinity() -> u32 {
            CURRENT_AFFINITY.load(Ordering::Acquire)
        }
    }

    struct TestBarrier;

    impl Barrier for TestBarrier {
        fn data_memory(_domain: BarrierDomain, _access: BarrierAccess) {}
        fn data_synchronization(_domain: BarrierDomain, _access: BarrierAccess) {}
        fn instruction_synchronization() {}
    }

    #[test]
    fn initializes_and_configures_the_boot_cpu_interface() {
        const DISTRIBUTOR_PHYSICAL: u64 = 0x1000_0000;
        const REDISTRIBUTOR_PHYSICAL: u64 = 0x2000_0000;
        const AFFINITY: u32 = 0x0102_0304;
        CURRENT_AFFINITY.store(AFFINITY, Ordering::Release);

        CPU_INITIALIZED.store(false, Ordering::Relaxed);
        COMPLETED.store(u32::MAX, Ordering::Relaxed);
        let mut distributor = vec![0u64; 0x1_0000 / 8];
        let mut redistributor = vec![0u64; 0x2_0000 / 8];
        let distributor_base = distributor.as_mut_ptr() as usize;
        let redistributor_base = redistributor.as_mut_ptr() as usize;
        unsafe {
            write_volatile(distributor_base.wrapping_add(0x4) as *mut u32, 1);
            write_volatile(
                redistributor_base.wrapping_add(0x8) as *mut u64,
                (u64::from(AFFINITY) << 32) | (1 << 4),
            );
        }

        let mut redistributors = RegionList::<MAX_GIC_REDISTRIBUTOR_REGIONS>::new();
        super::require_ok(
            redistributors.insert(super::require_some(PhysicalRange::new(
                REDISTRIBUTOR_PHYSICAL,
                0x2_0000,
            ))),
        );
        let info = GicV3Info {
            distributor: super::require_some(PhysicalRange::new(DISTRIBUTOR_PHYSICAL, 0x1_0000)),
            redistributors,
            redistributor_stride: None,
            maintenance_interrupt: None,
        };
        let mut controller = super::require_ok(unsafe {
            GicV3::<TestCpuInterface, TestBarrier>::bind(info, |address| match address {
                DISTRIBUTOR_PHYSICAL => Some(distributor_base),
                REDISTRIBUTOR_PHYSICAL => Some(redistributor_base),
                _ => None,
            })
        });
        super::require_ok(unsafe { controller.initialize(AFFINITY) });

        assert!(CPU_INITIALIZED.load(Ordering::Acquire));
        assert_eq!(controller.interrupt_count(), 64);
        assert_eq!(
            unsafe { read_volatile(distributor_base as *const u32) },
            0x13
        );

        let interrupt = InterruptId::new(40);
        super::require_ok(controller.configure(interrupt, 0x55, InterruptTrigger::Edge));
        assert_eq!(
            unsafe { read_volatile(distributor_base.wrapping_add(0x428) as *const u8) },
            0x55
        );
        assert_ne!(
            unsafe { read_volatile(distributor_base.wrapping_add(0x0c08) as *const u32) }
                & (1 << 17),
            0
        );
        assert_eq!(
            unsafe { read_volatile(distributor_base.wrapping_add(0x6140) as *const u64) },
            u64::from(AFFINITY)
        );

        super::require_ok(controller.enable(interrupt));
        assert_eq!(
            unsafe { read_volatile(distributor_base.wrapping_add(0x104) as *const u32) },
            1 << 8
        );
        super::require_ok(controller.disable(interrupt));
        assert_eq!(
            unsafe { read_volatile(distributor_base.wrapping_add(0x184) as *const u32) },
            1 << 8
        );
        assert_eq!(controller.acknowledge(), Some(interrupt));
        controller.end(interrupt);
        assert_eq!(COMPLETED.load(Ordering::Acquire), 40);
    }
}

#[cfg(test)]
mod vgic {
    use hyper::vm::interrupt::gicv3::{decode_list_register, encode_list_register};
    use hyper::vm::interrupt::{
        Error, InterruptGroup, InterruptTrigger, ListEntry, ListState, VirtualCpuId,
        VirtualInterruptController, VirtualInterruptId,
    };

    fn interrupt(value: u32) -> VirtualInterruptId {
        super::require_some(VirtualInterruptId::new(value))
    }

    #[test]
    fn schedules_pending_interrupts_by_priority_and_cpu() {
        let mut vgic = super::require_ok(VirtualInterruptController::new(2));
        let cpu0 = VirtualCpuId::new(0);
        let cpu1 = VirtualCpuId::new(1);
        for (id, cpu, priority) in [(27, cpu0, 0x80), (27, cpu1, 0x70), (40, cpu0, 0x20)] {
            super::require_ok(vgic.configure(
                interrupt(id),
                cpu,
                priority,
                InterruptGroup::Group1,
                InterruptTrigger::Level,
            ));
            super::require_ok(vgic.set_enabled(interrupt(id), cpu, true));
            super::require_ok(vgic.inject(interrupt(id), cpu));
        }

        let mut slots = [None; 2];
        assert_eq!(super::require_ok(vgic.refill(cpu0, &mut slots)), 2);
        assert_eq!(slots[0].map(|entry| entry.interrupt), Some(interrupt(40)));
        assert_eq!(slots[1].map(|entry| entry.interrupt), Some(interrupt(27)));

        let mut other_slots = [None; 1];
        assert_eq!(super::require_ok(vgic.refill(cpu1, &mut other_slots)), 1);
        assert_eq!(
            other_slots[0].map(|entry| entry.interrupt),
            Some(interrupt(27))
        );
    }

    #[test]
    fn requests_eoi_maintenance_for_a_virtual_timer_ppi() {
        let mut vgic = super::require_ok(VirtualInterruptController::new(1));
        let cpu = VirtualCpuId::new(0);
        let timer = interrupt(27);
        super::require_ok(vgic.configure(
            timer,
            cpu,
            0x80,
            InterruptGroup::Group1,
            InterruptTrigger::Level,
        ));
        super::require_ok(vgic.set_maintenance_on_eoi(timer, cpu, true));
        super::require_ok(vgic.set_enabled(timer, cpu, true));
        super::require_ok(vgic.inject(timer, cpu));
        let mut slots = [None; 1];
        assert_eq!(super::require_ok(vgic.refill(cpu, &mut slots)), 1);
        assert_eq!(
            slots[0].map(|entry| entry.request_eoi_maintenance),
            Some(true)
        );
    }

    #[test]
    fn tracks_active_reinjection_and_guest_completion() {
        let mut vgic = super::require_ok(VirtualInterruptController::new(1));
        let cpu = VirtualCpuId::new(0);
        let id = interrupt(48);
        super::require_ok(vgic.configure(
            id,
            cpu,
            0x40,
            InterruptGroup::Group1,
            InterruptTrigger::Level,
        ));
        super::require_ok(vgic.set_enabled(id, cpu, true));
        super::require_ok(vgic.inject(id, cpu));

        let mut slots = [None; 1];
        assert_eq!(super::require_ok(vgic.refill(cpu, &mut slots)), 1);
        slots[0] = Some(ListEntry {
            interrupt: id,
            priority: 0x40,
            group: InterruptGroup::Group1,
            state: ListState::Active,
            request_eoi_maintenance: true,
        });
        super::require_ok(vgic.synchronize(cpu, &slots));
        super::require_ok(vgic.inject(id, cpu));
        assert_eq!(super::require_ok(vgic.refill(cpu, &mut slots)), 0);
        assert_eq!(
            slots[0].map(|entry| entry.state),
            Some(ListState::PendingActive)
        );

        super::require_ok(vgic.synchronize(cpu, &[None]));
        let snapshot = super::require_ok(vgic.snapshot(id, cpu));
        assert!(!snapshot.pending);
        assert!(!snapshot.active);
        assert!(!snapshot.listed);
    }

    #[test]
    fn withdraws_disabled_pending_entries_without_losing_pending_state() {
        let mut vgic = super::require_ok(VirtualInterruptController::new(1));
        let cpu = VirtualCpuId::new(0);
        let id = interrupt(72);
        super::require_ok(vgic.configure(
            id,
            cpu,
            0x80,
            InterruptGroup::Group1,
            InterruptTrigger::Level,
        ));
        super::require_ok(vgic.set_enabled(id, cpu, true));
        super::require_ok(vgic.inject(id, cpu));
        let mut slots = [None; 1];
        assert_eq!(super::require_ok(vgic.refill(cpu, &mut slots)), 1);

        super::require_ok(vgic.set_enabled(id, cpu, false));
        assert_eq!(super::require_ok(vgic.refill(cpu, &mut slots)), 0);
        assert_eq!(slots, [None]);
        let snapshot = super::require_ok(vgic.snapshot(id, cpu));
        assert!(snapshot.pending);
        assert!(!snapshot.listed);

        super::require_ok(vgic.set_priority(id, cpu, 0x20));
        super::require_ok(vgic.set_enabled(id, cpu, true));
        assert_eq!(super::require_ok(vgic.refill(cpu, &mut slots)), 1);
        assert_eq!(slots[0].map(|entry| entry.priority), Some(0x20));
    }

    #[test]
    fn rejects_duplicate_spis_and_malformed_snapshots() {
        let mut vgic = super::require_ok(VirtualInterruptController::new(2));
        let cpu0 = VirtualCpuId::new(0);
        let cpu1 = VirtualCpuId::new(1);
        let id = interrupt(64);
        super::require_ok(vgic.configure(
            id,
            cpu0,
            0x80,
            InterruptGroup::Group1,
            InterruptTrigger::Edge,
        ));
        assert_eq!(
            vgic.configure(
                id,
                cpu1,
                0x80,
                InterruptGroup::Group1,
                InterruptTrigger::Edge,
            ),
            Err(Error::AlreadyConfigured)
        );
        let duplicate = Some(ListEntry {
            interrupt: id,
            priority: 0x80,
            group: InterruptGroup::Group1,
            state: ListState::Pending,
            request_eoi_maintenance: true,
        });
        assert_eq!(
            vgic.synchronize(cpu0, &[duplicate, duplicate]),
            Err(Error::SnapshotContainsDuplicate)
        );
    }

    #[test]
    fn encodes_the_gicv3_list_register_layout() {
        let entry = ListEntry {
            interrupt: interrupt(55),
            priority: 0xa0,
            group: InterruptGroup::Group1,
            state: ListState::PendingActive,
            request_eoi_maintenance: true,
        };
        let encoded = encode_list_register(Some(entry));
        assert_eq!(
            encoded,
            55 | (1 << 41) | (0xa0 << 48) | (1 << 60) | (3 << 62)
        );
        assert_eq!(
            super::require_ok(decode_list_register(encoded)),
            Some(entry)
        );
        assert_eq!(super::require_ok(decode_list_register(0)), None);
    }
}

#[cfg(test)]
mod boot_allocator {
    use hyper::mm::{BootAllocator, BootAllocatorError, PAGE_SIZE};
    use hyper::platform::{MAX_MEMORY_REGIONS, MAX_RESERVED_REGIONS, PhysicalRange, RegionList};

    #[test]
    fn skips_reservations_and_records_allocations() {
        let mut memory = RegionList::<MAX_MEMORY_REGIONS>::new();
        super::require_ok(memory.insert(super::require_some(PhysicalRange::new(
            PAGE_SIZE,
            PAGE_SIZE * 8,
        ))));
        let mut reserved = RegionList::<MAX_RESERVED_REGIONS>::new();
        super::require_ok(reserved.insert(super::require_some(PhysicalRange::new(
            PAGE_SIZE * 2,
            PAGE_SIZE * 2,
        ))));
        let mut allocator =
            super::require_ok(BootAllocator::new(&memory, &reserved, PAGE_SIZE * 16));

        assert_eq!(
            super::require_ok(allocator.allocate_pages(1, 1)).get(),
            PAGE_SIZE
        );
        assert_eq!(
            super::require_ok(allocator.allocate_pages(1, 1)).get(),
            PAGE_SIZE * 4
        );
        assert_eq!(allocator.reservations().len(), 1);
        assert_eq!(allocator.reservations()[0].start(), PAGE_SIZE);
        assert_eq!(allocator.reservations()[0].size(), PAGE_SIZE * 4);
    }

    #[test]
    fn honors_multi_page_alignment() {
        let mut memory = RegionList::<MAX_MEMORY_REGIONS>::new();
        super::require_ok(memory.insert(super::require_some(PhysicalRange::new(
            PAGE_SIZE,
            PAGE_SIZE * 16,
        ))));
        let reserved = RegionList::<MAX_RESERVED_REGIONS>::new();
        let mut allocator =
            super::require_ok(BootAllocator::new(&memory, &reserved, PAGE_SIZE * 32));

        assert_eq!(
            super::require_ok(allocator.allocate_pages(2, 4)).get(),
            PAGE_SIZE * 4
        );
    }

    #[test]
    fn rejects_invalid_requests_and_honors_the_accessible_limit() {
        let mut memory = RegionList::<MAX_MEMORY_REGIONS>::new();
        super::require_ok(memory.insert(super::require_some(PhysicalRange::new(
            PAGE_SIZE,
            PAGE_SIZE * 8,
        ))));
        let reserved = RegionList::<MAX_RESERVED_REGIONS>::new();
        let mut allocator =
            super::require_ok(BootAllocator::new(&memory, &reserved, PAGE_SIZE * 3));

        assert_eq!(
            allocator.allocate_pages(0, 1),
            Err(BootAllocatorError::InvalidRequest)
        );
        assert_eq!(
            allocator.allocate_pages(1, 3),
            Err(BootAllocatorError::InvalidAlignment)
        );
        assert_eq!(
            super::require_ok(allocator.allocate_pages(2, 1)).get(),
            PAGE_SIZE
        );
        assert_eq!(
            allocator.allocate_pages(1, 1),
            Err(BootAllocatorError::OutOfMemory)
        );
    }

    #[test]
    fn reports_only_reserved_pages_that_overlap_ram() {
        let mut memory = RegionList::<MAX_MEMORY_REGIONS>::new();
        super::require_ok(memory.insert(super::require_some(PhysicalRange::new(
            PAGE_SIZE,
            PAGE_SIZE * 8,
        ))));
        let mut reserved = RegionList::<MAX_RESERVED_REGIONS>::new();
        super::require_ok(
            reserved.insert(super::require_some(PhysicalRange::new(0, PAGE_SIZE * 3))),
        );
        super::require_ok(reserved.insert(super::require_some(PhysicalRange::new(
            PAGE_SIZE * 32,
            PAGE_SIZE,
        ))));
        let allocator = super::require_ok(BootAllocator::new(&memory, &reserved, PAGE_SIZE * 64));

        let stats = allocator.stats();
        assert_eq!(stats.ram_pages, 8);
        assert_eq!(stats.reserved_pages, 2);
        assert_eq!(stats.available_pages, 6);
    }
}

#[cfg(test)]
mod runtime_allocators {
    use std::alloc::{Layout, alloc_zeroed, dealloc};

    use hyper::hal::interrupt::InterruptMask;
    use hyper::mm::heap::{KernelGlobalAllocator, PageOwner, SlabAllocator};
    use hyper::mm::{BootAllocator, BuddyAllocator, PAGE_SIZE};
    use hyper::platform::{MAX_MEMORY_REGIONS, MAX_RESERVED_REGIONS, PhysicalRange, RegionList};

    struct AlignedMemory {
        pointer: *mut u8,
        layout: Layout,
    }

    impl AlignedMemory {
        fn new(pages: usize) -> Self {
            let layout = super::require_ok(Layout::from_size_align(
                pages * PAGE_SIZE as usize,
                PAGE_SIZE as usize,
            ));
            // SAFETY: The test owns the allocation until `Drop`.
            let pointer = unsafe { alloc_zeroed(layout) };
            assert!(!pointer.is_null());
            Self { pointer, layout }
        }
    }

    impl Drop for AlignedMemory {
        fn drop(&mut self) {
            // SAFETY: `pointer` was allocated with this exact layout.
            unsafe { dealloc(self.pointer, self.layout) };
        }
    }

    fn handoff(pages: usize) -> (AlignedMemory, hyper::mm::MemoryHandoff) {
        let memory_buffer = AlignedMemory::new(pages);
        let mut memory = RegionList::<MAX_MEMORY_REGIONS>::new();
        super::require_ok(memory.insert(super::require_some(PhysicalRange::new(
            0,
            pages as u64 * PAGE_SIZE,
        ))));
        let reserved = RegionList::<MAX_RESERVED_REGIONS>::new();
        let boot = super::require_ok(BootAllocator::new(
            &memory,
            &reserved,
            pages as u64 * PAGE_SIZE,
        ));
        (memory_buffer, boot.handoff())
    }

    #[test]
    fn buddy_splits_and_coalesces_blocks() {
        let (memory, handoff) = handoff(64);
        // SAFETY: The aligned test buffer is the direct map for physical zero.
        let mut buddy = super::require_ok(unsafe {
            BuddyAllocator::from_handoff(&handoff, memory.pointer as u64)
        });
        let initial = buddy.free_pages();
        let initial_stats = buddy.stats();
        assert_eq!(initial_stats.managed_pages, 64);
        assert_eq!(initial_stats.free_blocks[6], 1);
        let first = super::require_ok(buddy.allocate(0));
        let second = super::require_ok(buddy.allocate(2));
        assert_eq!(buddy.free_pages(), initial - 5);
        let allocated = buddy.stats();
        assert_eq!(allocated.allocated_pages, 5);
        assert_eq!(allocated.peak_allocated_pages, 5);
        assert_eq!(allocated.allocation_requests, 2);
        assert_eq!(
            free_pages_from_blocks(&allocated.free_blocks),
            allocated.free_pages
        );

        // SAFETY: Both blocks are live allocations with matching orders.
        unsafe {
            super::require_ok(buddy.deallocate(first, 0));
            super::require_ok(buddy.deallocate(second, 2));
        }
        assert_eq!(buddy.free_pages(), initial);
        assert_eq!(buddy.stats().deallocations, 2);
        assert!(buddy.allocate(6).is_ok());
    }

    #[test]
    fn slab_reuses_small_objects_and_returns_empty_pages() {
        let (memory, handoff) = handoff(128);
        // SAFETY: The aligned test buffer is a stable writable direct map.
        let mut slab = super::require_ok(unsafe {
            SlabAllocator::from_handoff(&handoff, memory.pointer as u64)
        });
        let initial = slab.stats().free_pages;
        let small = super::require_ok(Layout::from_size_align(24, 16));
        let large = super::require_ok(Layout::from_size_align(9000, 4096));

        // SAFETY: Allocations are paired with matching deallocations below.
        unsafe {
            let mut small_objects = Vec::new();
            for _ in 0..200 {
                let pointer = slab.allocate(small);
                assert!(!pointer.is_null());
                small_objects.push(pointer);
            }
            let big = slab.allocate(large);
            assert!(!big.is_null());
            assert_eq!((small_objects[0] as usize) & 15, 0);
            assert_eq!((big as usize) & 4095, 0);
            assert_ne!(small_objects[0], small_objects[1]);
            for pointer in small_objects.into_iter().rev() {
                slab.deallocate(pointer, small);
            }
            slab.deallocate(big, large);
        }

        let stats = slab.stats();
        assert_eq!(stats.live_allocations, 0);
        assert_eq!(stats.slab_pages, 0);
        assert_eq!(stats.large_heap_pages, 0);
        assert_eq!(stats.requested_bytes, 0);
        assert_eq!(stats.peak_live_allocations, 201);
        assert!(stats.peak_requested_bytes >= 200 * 24 + 9000);
        assert_eq!(stats.free_pages, initial);
    }

    struct TestInterruptMask;

    impl InterruptMask for TestInterruptMask {
        type State = ();

        fn save_and_disable() -> Self::State {}

        fn restore(_: Self::State) {}
    }

    #[test]
    fn accounts_direct_pages_by_owner() {
        let (memory, handoff) = handoff(64);
        let allocator = KernelGlobalAllocator::<TestInterruptMask>::new();
        // SAFETY: The aligned test buffer is the direct map and outlives the allocator.
        super::require_ok(unsafe { allocator.initialize(&handoff, memory.pointer as u64) });

        let guest = super::require_ok(allocator.allocate_pages_for(3, PageOwner::Guest));
        let table = super::require_ok(allocator.allocate_pages_for(0, PageOwner::PageTable));
        let stats = super::require_some(allocator.stats());
        assert_eq!(stats.guest_pages.pages, 8);
        assert_eq!(stats.page_table_pages.pages, 1);
        assert_eq!(stats.buddy.allocated_pages, 9);

        // SAFETY: These are the exact live blocks and owners returned above.
        unsafe {
            super::require_ok(allocator.deallocate_pages_for(table, 0, PageOwner::PageTable));
            super::require_ok(allocator.deallocate_pages_for(guest, 3, PageOwner::Guest));
        }
        let stats = super::require_some(allocator.stats());
        assert_eq!(stats.guest_pages.pages, 0);
        assert_eq!(stats.guest_pages.peak_pages, 8);
        assert_eq!(stats.page_table_pages.pages, 0);
        assert_eq!(stats.buddy.allocated_pages, 0);
    }

    fn free_pages_from_blocks(blocks: &[usize]) -> usize {
        blocks
            .iter()
            .enumerate()
            .map(|(order, blocks)| blocks << order)
            .sum()
    }
}
