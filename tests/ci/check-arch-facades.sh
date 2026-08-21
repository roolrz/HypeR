#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Keep migrated architecture contracts behind their topical facade.
set -eu

root=${HYPER_ARCH_FACADE_ROOT:-$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)}
cd "$root"

set -- src
if [ -d tests/kernel ]; then
    set -- "$@" tests/kernel
fi

root_alias=$(LC_ALL=C rg -n -U --glob '*.rs' \
    'use\s+(?:crate\s*::\s*arch|(?:super\s*::\s*)+arch)\s*(?:;|\s+as\s+|::\s*\{[^}]*\bself\s*(?:,|\s+as\s+|\}))|use\s+crate\s*::\s*\{[^}]*\barch\s*(?:,|\s+as\s+|\})|use\s+crate\s*::\s*\{[^;]*\barch\s*::\s*\{[^}]*\bself\s*(?:,|\s+as\s+|\})' \
    "$@" || true)
if [ -n "$root_alias" ]; then
    echo "the root architecture module must not be aliased around topical facade checks:" >&2
    printf '%s\n' "$root_alias" >&2
    exit 1
fi

backend_bypass=$(LC_ALL=C rg -n -U --glob '*.rs' \
    '(?:crate\s*::\s*arch|(?:super\s*::\s*)+arch)\s*::\s*(?:aarch64|riscv64|x86_64)\b|use\s+(?:crate\s*::\s*arch|(?:super\s*::\s*)+arch)\s*::\s*\{[^}]*(?:aarch64|riscv64|x86_64)\b|use\s+crate\s*::\s*\{[^;]*\barch\s*::\s*\{[^}]*(?:aarch64|riscv64|x86_64)\b' \
    "$@" | sed '\#^src/arch/#d' || true)
if [ -n "$backend_bypass" ]; then
    echo "architecture backends must only be accessed through topical facades:" >&2
    printf '%s\n' "$backend_bypass" >&2
    exit 1
fi

flat_time=$(LC_ALL=C rg -n -U --glob '*.rs' \
    'crate\s*::\s*arch\s*::\s*(?:ArchitectureCounter|ArchitectureTimer|TimerError|TimerDescriptionError|KernelTimerDescription|decode_kernel_timer|prepare_timekeeping)\b|use\s+crate\s*::\s*arch\s*::[^;]*\{[^}]*(?:ArchitectureCounter|ArchitectureTimer|TimerError|TimerDescriptionError|KernelTimerDescription|decode_kernel_timer|prepare_timekeeping)' \
    "$@" || true)

if [ -n "$flat_time" ]; then
    echo "host timer mechanisms must be accessed through crate::arch::time:" >&2
    printf '%s\n' "$flat_time" >&2
    exit 1
fi

flat_cpu=$(LC_ALL=C rg -n -U --glob '*.rs' \
    'crate\s*::\s*arch\s*::\s*(?:ArchitectureCpuPower|CpuPowerError|SecondaryBootParameters|current_cpu_index|current_hardware_id|secondary_entry_physical|register_secondary_hardware_id|mark_current_cpu_online|secondary_cpu_is_compatible|initialize_cpu_power|send_event|wait_for_event|halt)\b|use\s+crate\s*::\s*arch\s*::[^;]*\{[^}]*(?:ArchitectureCpuPower|CpuPowerError|SecondaryBootParameters|current_cpu_index|current_hardware_id|secondary_entry_physical|register_secondary_hardware_id|mark_current_cpu_online|secondary_cpu_is_compatible|initialize_cpu_power|send_event|wait_for_event|halt)' \
    "$@" || true)

if [ -n "$flat_cpu" ]; then
    echo "CPU lifecycle mechanisms must be accessed through crate::arch::cpu:" >&2
    printf '%s\n' "$flat_cpu" >&2
    exit 1
fi

flat_context=$(LC_ALL=C rg -n -U --glob '*.rs' \
    '(?:crate\s*::\s*arch|(?:super\s*::\s*)+arch)\s*::\s*(?:ThreadContext|UserContext|reset_stack_and_enter|switch_thread_context)\b|use\s+(?:crate\s*::\s*arch|(?:super\s*::\s*)+arch)\s*::\s*\{[^}]*(?:ThreadContext|UserContext|reset_stack_and_enter|switch_thread_context)\b|use\s+crate\s*::\s*\{[^;]*\barch\s*::\s*\{[^}]*(?:ThreadContext|UserContext|reset_stack_and_enter|switch_thread_context)\b' \
    "$@" || true)

if [ -n "$flat_context" ]; then
    echo "thread-context mechanisms must be accessed through crate::arch::context:" >&2
    printf '%s\n' "$flat_context" >&2
    exit 1
fi

flat_exception=$(LC_ALL=C rg -n -U --glob '*.rs' \
    '(?:crate\s*::\s*arch|(?:super\s*::\s*)+arch)\s*::\s*(?:CrashContext|RuntimeVectorError|bootstrap_stack_bounds|broadcast_crash_stop|capture_crash_context|crash_stop_interrupt|install_exception_stacks|install_runtime_vectors|is_crash_stop_interrupt|run_on_emergency_stack|validate_runtime_vectors)\b|use\s+(?:crate\s*::\s*arch|(?:super\s*::\s*)+arch)\s*::\s*\{[^}]*(?:CrashContext|RuntimeVectorError|bootstrap_stack_bounds|broadcast_crash_stop|capture_crash_context|crash_stop_interrupt|install_exception_stacks|install_runtime_vectors|is_crash_stop_interrupt|run_on_emergency_stack|validate_runtime_vectors)\b|use\s+crate\s*::\s*\{[^;]*\barch\s*::\s*\{[^}]*(?:CrashContext|RuntimeVectorError|bootstrap_stack_bounds|broadcast_crash_stop|capture_crash_context|crash_stop_interrupt|install_exception_stacks|install_runtime_vectors|is_crash_stop_interrupt|run_on_emergency_stack|validate_runtime_vectors)\b' \
    "$@" || true)

if [ -n "$flat_exception" ]; then
    echo "exception and fail-stop mechanisms must be accessed through crate::arch::exception:" >&2
    printf '%s\n' "$flat_exception" >&2
    exit 1
fi

flat_irq=$(LC_ALL=C rg -n -U --glob '*.rs' \
    'crate\s*::\s*arch\s*::\s*(?:ArchitectureInterruptController|InterruptControllerError|LocalInterruptMask|decode_platform_interrupt|disable_local_interrupts|enable_local_irq|interrupt_is_per_cpu|local_irq_enabled)\b|use\s+crate\s*::\s*arch\s*::[^;]*\{[^}]*(?:ArchitectureInterruptController|InterruptControllerError|LocalInterruptMask|decode_platform_interrupt|disable_local_interrupts|enable_local_irq|interrupt_is_per_cpu|local_irq_enabled)' \
    "$@" || true)

if [ -n "$flat_irq" ]; then
    echo "host interrupt mechanisms must be accessed through crate::arch::irq:" >&2
    printf '%s\n' "$flat_irq" >&2
    exit 1
fi

flat_memory=$(LC_ALL=C rg -n -U --glob '*.rs' \
    '(?:crate\s*::\s*arch|(?:super\s*::\s*)+arch)\s*::\s*(?:ActivationContext|ArchitectureAddressTranslation|ArchitectureBarrier|ArchitectureCache|AtomicCapabilities|MemoryError|PreparedAddressSpace|StackMapping|activate_memory|atomic_capabilities|enable_local_memory_protection|local_memory_protection_enabled|prepare_address_space|prepare_cache|inspect_stage1_mapping)\b|use\s+(?:crate\s*::\s*arch|(?:super\s*::\s*)+arch)\s*::[^;]*\{[^}]*(?:ActivationContext|ArchitectureAddressTranslation|ArchitectureBarrier|ArchitectureCache|AtomicCapabilities|MemoryError|PreparedAddressSpace|StackMapping|activate_memory|atomic_capabilities|enable_local_memory_protection|local_memory_protection_enabled|prepare_address_space|prepare_cache|inspect_stage1_mapping)\b|use\s+crate\s*::\s*\{[^;]*\barch\s*::\s*\{[^}]*(?:ActivationContext|ArchitectureAddressTranslation|ArchitectureBarrier|ArchitectureCache|AtomicCapabilities|MemoryError|PreparedAddressSpace|StackMapping|activate_memory|atomic_capabilities|enable_local_memory_protection|local_memory_protection_enabled|prepare_address_space|prepare_cache|inspect_stage1_mapping)\b' \
    "$@" || true)

if [ -n "$flat_memory" ]; then
    echo "host stage-1 memory mechanisms must be accessed through crate::arch::memory:" >&2
    printf '%s\n' "$flat_memory" >&2
    exit 1
fi

flat_platform=$(LC_ALL=C rg -n -U --glob '*.rs' \
    '(?:crate\s*::\s*arch|(?:super\s*::\s*)+arch)\s*::\s*(?:EssentialDeviceDiscovery|EssentialPlatformInfo|PlatformDiscoveryError|KaslrError|select_kaslr_layout|port_io|report_runtime_architecture|describe_runtime)\b|use\s+(?:crate\s*::\s*arch|(?:super\s*::\s*)+arch)\s*::[^;]*\{[^}]*(?:EssentialDeviceDiscovery|EssentialPlatformInfo|PlatformDiscoveryError|KaslrError|select_kaslr_layout|port_io|report_runtime_architecture|describe_runtime)\b|use\s+crate\s*::\s*\{[^;]*\barch\s*::\s*\{[^}]*(?:EssentialDeviceDiscovery|EssentialPlatformInfo|PlatformDiscoveryError|KaslrError|select_kaslr_layout|port_io|report_runtime_architecture|describe_runtime)\b' \
    "$@" || true)

if [ -n "$flat_platform" ]; then
    echo "host platform mechanisms must be accessed through crate::arch::platform:" >&2
    printf '%s\n' "$flat_platform" >&2
    exit 1
fi

flat_guest=$(LC_ALL=C rg -n -U --glob '*.rs' \
    '(?:crate\s*::\s*arch|(?:super\s*::\s*)+arch)\s*::\s*(?:LinuxAbi|PayloadLoadError|PayloadMemory|PayloadRange|LINUX_GUEST_KERNEL_IPA|LINUX_GUEST_RAM_IPA|LINUX_GUEST_TIMER_INTERRUPT|linux_abi|linux_guest_architecture|linux_kernel_occupied_size|load_linux_payload|prepare_linux_vcpu_context|report_linux_guest_layout|describe_linux_guest_layout|describe_linux_host|describe_linux_layout|validate_linux_host|validate_linux_kernel)\b|use\s+(?:crate\s*::\s*arch|(?:super\s*::\s*)+arch)\s*::\s*\{[^}]*(?:LinuxAbi|PayloadLoadError|PayloadMemory|PayloadRange|LINUX_GUEST_KERNEL_IPA|LINUX_GUEST_RAM_IPA|LINUX_GUEST_TIMER_INTERRUPT|linux_abi|linux_guest_architecture|linux_kernel_occupied_size|load_linux_payload|prepare_linux_vcpu_context|report_linux_guest_layout|describe_linux_guest_layout|describe_linux_host|describe_linux_layout|validate_linux_host|validate_linux_kernel)\b|use\s+crate\s*::\s*\{[^;]*\barch\s*::\s*\{[^}]*(?:LinuxAbi|PayloadLoadError|PayloadMemory|PayloadRange|LINUX_GUEST_KERNEL_IPA|LINUX_GUEST_RAM_IPA|LINUX_GUEST_TIMER_INTERRUPT|linux_abi|linux_guest_architecture|linux_kernel_occupied_size|load_linux_payload|prepare_linux_vcpu_context|report_linux_guest_layout|describe_linux_guest_layout|describe_linux_host|describe_linux_layout|validate_linux_host|validate_linux_kernel)\b' \
    "$@" || true)

if [ -n "$flat_guest" ]; then
    echo "Linux guest ABI mechanisms must be accessed through crate::arch::guest:" >&2
    printf '%s\n' "$flat_guest" >&2
    exit 1
fi

flat_vm=$(LC_ALL=C rg -n -U --glob '*.rs' \
    '(?:crate\s*::\s*arch|(?:super\s*::\s*)+arch)\s*::\s*(?:Stage2AddressSpace|Stage2Error|VcpuContext|VcpuInterruptError|VirtualInterruptError|VmInterruptController|VmInterruptError|GuestValidationError|VirtualDeviceInitializationError|InterruptVirtualizationError|initialize_virtual_devices|initialize_interrupt_virtualization|enable_interrupts_for_guest_entry|handle_guest_virtual_timer_interrupt|poll_guest_timer|take_guest_timer_wakeup|receive_guest_console_input|validate_vsysreg|GuestSyncAction|GuestSyncFrame|handle_guest_sync|handle_guest_device_access|deliver_guest_software_interrupt|GuestDataAccess|VgicError|disable_vgic)\b|use\s+(?:crate\s*::\s*arch|(?:super\s*::\s*)+arch)\s*::\s*\{[^}]*(?:Stage2AddressSpace|Stage2Error|VcpuContext|VcpuInterruptError|VirtualInterruptError|VmInterruptController|VmInterruptError|GuestValidationError|VirtualDeviceInitializationError|InterruptVirtualizationError|initialize_virtual_devices|initialize_interrupt_virtualization|enable_interrupts_for_guest_entry|handle_guest_virtual_timer_interrupt|poll_guest_timer|take_guest_timer_wakeup|receive_guest_console_input|validate_vsysreg|GuestSyncAction|GuestSyncFrame|handle_guest_sync|handle_guest_device_access|deliver_guest_software_interrupt|GuestDataAccess|VgicError|disable_vgic)\b|use\s+crate\s*::\s*\{[^;]*\barch\s*::\s*\{[^}]*(?:Stage2AddressSpace|Stage2Error|VcpuContext|VcpuInterruptError|VirtualInterruptError|VmInterruptController|VmInterruptError|GuestValidationError|VirtualDeviceInitializationError|InterruptVirtualizationError|initialize_virtual_devices|initialize_interrupt_virtualization|enable_interrupts_for_guest_entry|handle_guest_virtual_timer_interrupt|poll_guest_timer|take_guest_timer_wakeup|receive_guest_console_input|validate_vsysreg|GuestSyncAction|GuestSyncFrame|handle_guest_sync|handle_guest_device_access|deliver_guest_software_interrupt|GuestDataAccess|VgicError|disable_vgic)\b' \
    "$@" || true)

if [ -n "$flat_vm" ]; then
    echo "hardware virtualization mechanisms must be accessed through crate::arch::vm:" >&2
    printf '%s\n' "$flat_vm" >&2
    exit 1
fi

facade_source=src/arch/mod.rs
if [ -f "$facade_source" ]; then
    backend_type_aliases=$(LC_ALL=C rg -n -U \
        'pub(?:\(crate\))?\s+type\s+[A-Za-z_][A-Za-z0-9_]*(?:\s*<[^;=]*>)?\s*=\s*imp\s*::' \
        "$facade_source" || true)
    if [ -n "$backend_type_aliases" ]; then
        echo "the root architecture facade must not hide backend types behind aliases:" >&2
        printf '%s\n' "$backend_type_aliases" >&2
        exit 1
    fi

    flat_exports=$(LC_ALL=C rg -n -U \
        'pub(?:\(crate\))?\s+use\s+imp\s*::(?:\s*(?:ArchitectureCounter|ArchitectureTimer|TimerError|decode_kernel_timer|prepare_timekeeping)\b|[^;]*\{[^}]*(?:ArchitectureCounter|ArchitectureTimer|TimerError|decode_kernel_timer|prepare_timekeeping))' \
        "$facade_source" || true)
    if [ -n "$flat_exports" ]; then
        echo "the root architecture facade must not re-export migrated host timer mechanisms:" >&2
        printf '%s\n' "$flat_exports" >&2
        exit 1
    fi

    flat_cpu_exports=$(LC_ALL=C rg -n -U \
        'pub(?:\(crate\))?\s+use\s+imp\s*::(?:\s*(?:ArchitectureCpuPower|CpuPowerError|SecondaryBootParameters|current_cpu_index|current_hardware_id|secondary_entry_physical|register_secondary_hardware_id|mark_current_cpu_online|secondary_cpu_is_compatible|initialize_cpu_power|send_event|wait_for_event|halt)\b|[^;]*\{[^}]*(?:ArchitectureCpuPower|CpuPowerError|SecondaryBootParameters|current_cpu_index|current_hardware_id|secondary_entry_physical|register_secondary_hardware_id|mark_current_cpu_online|secondary_cpu_is_compatible|initialize_cpu_power|send_event|wait_for_event|halt))' \
        "$facade_source" || true)
    if [ -n "$flat_cpu_exports" ]; then
        echo "the root architecture facade must not re-export migrated CPU mechanisms:" >&2
        printf '%s\n' "$flat_cpu_exports" >&2
        exit 1
    fi

    flat_context_exports=$(LC_ALL=C rg -n -U \
        '(?:pub(?:\(crate\))?\s+)?use\s+imp\s*::(?:\s*(?:ThreadContext|UserContext|reset_stack_and_enter|switch_thread_context)\b|[^;]*\{[^}]*(?:ThreadContext|UserContext|reset_stack_and_enter|switch_thread_context)\b)' \
        "$facade_source" || true)
    if [ -n "$flat_context_exports" ]; then
        echo "the root architecture facade must not re-export thread-context mechanisms:" >&2
        printf '%s\n' "$flat_context_exports" >&2
        exit 1
    fi

    flat_exception_exports=$(LC_ALL=C rg -n -U \
        '(?:pub(?:\(crate\))?\s+)?use\s+imp\s*::(?:\s*(?:CrashContext|RuntimeVectorError|bootstrap_stack_bounds|broadcast_crash_stop|capture_crash_context|crash_stop_interrupt|install_exception_stacks|install_runtime_vectors|is_crash_stop_interrupt|run_on_emergency_stack|validate_runtime_vectors)\b|[^;]*\{[^}]*(?:CrashContext|RuntimeVectorError|bootstrap_stack_bounds|broadcast_crash_stop|capture_crash_context|crash_stop_interrupt|install_exception_stacks|install_runtime_vectors|is_crash_stop_interrupt|run_on_emergency_stack|validate_runtime_vectors)\b)' \
        "$facade_source" || true)
    if [ -n "$flat_exception_exports" ]; then
        echo "the root architecture facade must not re-export exception mechanisms:" >&2
        printf '%s\n' "$flat_exception_exports" >&2
        exit 1
    fi

    flat_irq_exports=$(LC_ALL=C rg -n -U \
        'pub(?:\(crate\))?\s+use\s+imp\s*::(?:\s*(?:ArchitectureInterruptController|InterruptControllerError|LocalInterruptMask|decode_platform_interrupt|disable_local_interrupts|enable_local_irq|interrupt_is_per_cpu|local_irq_enabled)\b|[^;]*\{[^}]*(?:ArchitectureInterruptController|InterruptControllerError|LocalInterruptMask|decode_platform_interrupt|disable_local_interrupts|enable_local_irq|interrupt_is_per_cpu|local_irq_enabled))' \
        "$facade_source" || true)
    if [ -n "$flat_irq_exports" ]; then
        echo "the root architecture facade must not re-export migrated IRQ mechanisms:" >&2
        printf '%s\n' "$flat_irq_exports" >&2
        exit 1
    fi

    flat_memory_exports=$(LC_ALL=C rg -n -U \
        '(?:pub(?:\(crate\))?\s+)?use\s+imp\s*::(?:\s*(?:ActivationContext|ArchitectureAddressTranslation|ArchitectureBarrier|ArchitectureCache|AtomicCapabilities|MemoryError|PreparedAddressSpace|StackMapping|activate_memory|atomic_capabilities|enable_local_memory_protection|local_memory_protection_enabled|prepare_address_space|prepare_cache|inspect_stage1_mapping)\b|[^;]*\{[^}]*(?:ActivationContext|ArchitectureAddressTranslation|ArchitectureBarrier|ArchitectureCache|AtomicCapabilities|MemoryError|PreparedAddressSpace|StackMapping|activate_memory|atomic_capabilities|enable_local_memory_protection|local_memory_protection_enabled|prepare_address_space|prepare_cache|inspect_stage1_mapping)\b)' \
        "$facade_source" || true)
    if [ -n "$flat_memory_exports" ]; then
        echo "the root architecture facade must not re-export migrated memory mechanisms:" >&2
        printf '%s\n' "$flat_memory_exports" >&2
        exit 1
    fi

    flat_platform_exports=$(LC_ALL=C rg -n -U \
        '(?:pub(?:\(crate\))?\s+)?use\s+imp\s*::(?:\s*(?:EssentialDeviceDiscovery|EssentialPlatformInfo|PlatformDiscoveryError|KaslrError|select_kaslr_layout|port_io|report_runtime_architecture|describe_runtime)\b|[^;]*\{[^}]*(?:EssentialDeviceDiscovery|EssentialPlatformInfo|PlatformDiscoveryError|KaslrError|select_kaslr_layout|port_io|report_runtime_architecture|describe_runtime)\b)' \
        "$facade_source" || true)
    if [ -n "$flat_platform_exports" ]; then
        echo "the root architecture facade must not re-export migrated platform mechanisms:" >&2
        printf '%s\n' "$flat_platform_exports" >&2
        exit 1
    fi

    flat_guest_exports=$(LC_ALL=C rg -n -U \
        '(?:pub(?:\(crate\))?\s+)?use\s+imp\s*::(?:\s*(?:LinuxAbi|PayloadLoadError|PayloadMemory|PayloadRange|LINUX_GUEST_KERNEL_IPA|LINUX_GUEST_RAM_IPA|LINUX_GUEST_TIMER_INTERRUPT|linux_abi|linux_guest_architecture|linux_kernel_occupied_size|load_linux_payload|prepare_linux_vcpu_context|report_linux_guest_layout|describe_linux_guest_layout|describe_linux_host|describe_linux_layout|validate_linux_host|validate_linux_kernel)\b|[^;]*\{[^}]*(?:LinuxAbi|PayloadLoadError|PayloadMemory|PayloadRange|LINUX_GUEST_KERNEL_IPA|LINUX_GUEST_RAM_IPA|LINUX_GUEST_TIMER_INTERRUPT|linux_abi|linux_guest_architecture|linux_kernel_occupied_size|load_linux_payload|prepare_linux_vcpu_context|report_linux_guest_layout|describe_linux_guest_layout|describe_linux_host|describe_linux_layout|validate_linux_host|validate_linux_kernel)\b)' \
        "$facade_source" || true)
    if [ -n "$flat_guest_exports" ]; then
        echo "the root architecture facade must not re-export migrated Linux guest ABI mechanisms:" >&2
        printf '%s\n' "$flat_guest_exports" >&2
        exit 1
    fi

    flat_vm_exports=$(LC_ALL=C rg -n -U \
        '(?:pub(?:\(crate\))?\s+)?use\s+imp\s*::(?:\s*(?:Stage2AddressSpace|Stage2Error|VcpuContext|VcpuInterruptError|VirtualInterruptError|VmInterruptController|VmInterruptError|GuestValidationError|VirtualDeviceInitializationError|InterruptVirtualizationError|initialize_virtual_devices|initialize_interrupt_virtualization|enable_interrupts_for_guest_entry|handle_guest_virtual_timer_interrupt|poll_guest_timer|take_guest_timer_wakeup|receive_guest_console_input|validate_vsysreg|GuestSyncAction|GuestSyncFrame|handle_guest_sync|handle_guest_device_access|deliver_guest_software_interrupt|GuestDataAccess|VgicError|disable_vgic)\b|[^;]*\{[^}]*(?:Stage2AddressSpace|Stage2Error|VcpuContext|VcpuInterruptError|VirtualInterruptError|VmInterruptController|VmInterruptError|GuestValidationError|VirtualDeviceInitializationError|InterruptVirtualizationError|initialize_virtual_devices|initialize_interrupt_virtualization|enable_interrupts_for_guest_entry|handle_guest_virtual_timer_interrupt|poll_guest_timer|take_guest_timer_wakeup|receive_guest_console_input|validate_vsysreg|GuestSyncAction|GuestSyncFrame|handle_guest_sync|handle_guest_device_access|deliver_guest_software_interrupt|GuestDataAccess|VgicError|disable_vgic)\b)' \
        "$facade_source" || true)
    if [ -n "$flat_vm_exports" ]; then
        echo "the root architecture facade must not re-export migrated VM mechanisms:" >&2
        printf '%s\n' "$flat_vm_exports" >&2
        exit 1
    fi
fi
