// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use hyper_os::vm::{Architecture, PlatformProfile, VirtualMachineInfo, VirtualMachinePhase};
use hyper_service::io::{decode_observation, encode_observation};

#[test]
fn observations_validate_wire_format_and_preserve_metrics() {
    for phase in [
        VirtualMachinePhase::Installed,
        VirtualMachinePhase::Running,
        VirtualMachinePhase::Stopping,
        VirtualMachinePhase::Stopped,
    ] {
        let info = VirtualMachineInfo {
            phase,
            vcpu_count: 4,
            guest_physical_base: 0x4000_0000,
            memory_size: 512 * 1024 * 1024,
            architecture: Architecture::Aarch64,
            platform_profile: PlatformProfile::Aarch64Reference,
        };
        let bytes = encode_observation(info, 64 * 1024 * 1024);
        assert_eq!(
            decode_observation(&bytes),
            Some((phase, 4, 64 * 1024 * 1024))
        );
        assert_eq!(decode_observation(&bytes[..23]), None);
        let mut invalid = bytes;
        invalid[8] = 4;
        assert_eq!(decode_observation(&invalid), None);
        invalid = bytes;
        invalid[9] = 1;
        assert_eq!(decode_observation(&invalid), None);
    }
}
