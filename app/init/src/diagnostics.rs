// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Allocation-free rendering of supervised Process termination details.

use hyper_os::task::{ProcessInfo, ProcessTermination};

/// Writes the reason-specific portion of one typed Process lifecycle report.
///
/// The callback receives short borrowed fragments so the init supervisor can
/// report through its recovery Console without formatting allocation.
pub fn write_process_termination(info: ProcessInfo, mut write: impl FnMut(&[u8])) {
    match info.terminal {
        Some(ProcessTermination::Requested) => write(b"reason=requested"),
        Some(ProcessTermination::ThreadExited { status }) => {
            write(b"reason=thread-exited status=");
            write_signed(status, &mut write);
        }
        Some(ProcessTermination::ProcessExited { status }) => {
            write(b"reason=process-exited status=");
            write_signed(status, &mut write);
        }
        Some(ProcessTermination::LastThreadExited { status }) => {
            write(b"reason=last-thread-exited status=");
            write_signed(status, &mut write);
        }
        Some(ProcessTermination::Fault { class, code }) => {
            write(b"reason=fault class=");
            write_unsigned(u64::from(class), &mut write);
            write(b" code=");
            write_unsigned(code, &mut write);
        }
        Some(ProcessTermination::TaskGroupStop { generation }) => {
            write(b"reason=task-group-stop generation=");
            write_unsigned(generation, &mut write);
        }
        None => write(b"reason=unavailable"),
    }
}

fn write_signed(value: i64, write: &mut impl FnMut(&[u8])) {
    if value.is_negative() {
        write(b"-");
    }
    write_unsigned(value.unsigned_abs(), write);
}

fn write_unsigned(mut value: u64, write: &mut impl FnMut(&[u8])) {
    let mut buffer = [0_u8; 20];
    let mut cursor = buffer.len();
    loop {
        cursor -= 1;
        buffer[cursor] = b'0' + (value % 10) as u8;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    write(&buffer[cursor..]);
}

#[cfg(test)]
mod tests {
    extern crate std;

    use hyper_os::task::{ProcessInfo, ProcessPhase, ProcessTermination};
    use std::string::String;
    use std::vec::Vec;

    use super::write_process_termination;

    fn render(terminal: Option<ProcessTermination>) -> String {
        let mut bytes = Vec::new();
        write_process_termination(
            ProcessInfo {
                phase: ProcessPhase::Stopped,
                terminal,
            },
            |fragment| bytes.extend_from_slice(fragment),
        );
        match String::from_utf8(bytes) {
            Ok(text) => text,
            Err(_) => String::new(),
        }
    }

    #[test]
    fn renders_exit_statuses_without_losing_sign_or_range() {
        assert_eq!(
            render(Some(ProcessTermination::ThreadExited { status: 0 })),
            "reason=thread-exited status=0"
        );
        assert_eq!(
            render(Some(ProcessTermination::ProcessExited { status: i64::MIN })),
            "reason=process-exited status=-9223372036854775808"
        );
        assert_eq!(
            render(Some(ProcessTermination::LastThreadExited {
                status: i64::MAX,
            })),
            "reason=last-thread-exited status=9223372036854775807"
        );
    }

    #[test]
    fn renders_non_exit_reasons_and_missing_terminal_detail() {
        assert_eq!(
            render(Some(ProcessTermination::Requested)),
            "reason=requested"
        );
        assert_eq!(
            render(Some(ProcessTermination::Fault {
                class: u32::MAX,
                code: u64::MAX,
            })),
            "reason=fault class=4294967295 code=18446744073709551615"
        );
        assert_eq!(
            render(Some(ProcessTermination::TaskGroupStop {
                generation: u64::MAX,
            })),
            "reason=task-group-stop generation=18446744073709551615"
        );
        assert_eq!(render(None), "reason=unavailable");
    }
}
