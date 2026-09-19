// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Test-only abrupt owner loss at real VM power transition boundaries.

pub(super) fn at(state: &str) {
    if option_env!("HYPER_TEST_POWER_CRASH") == Some(state) {
        // Pending CPU_ON has reserved its target but has not been accepted.
        // CPU_OFF is published only after hardware detach; successful runtime
        // completion commits Off before this hook. Its scheduler Thread may
        // still be parking, and retirement must safely handle that race too.
        eprintln!("HYPER_POWER_CRASH state={state}");
        std::process::exit(94);
    }
}
