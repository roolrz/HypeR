// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use hyper_os::handle::{ByteChannelObject, OwnedHandle};
use hyper_os::wait::{ObjectSignals, WaitItem, wait_many};
use std::io::Write;

pub(crate) fn console_session(
    input: &OwnedHandle<ByteChannelObject>,
    output: &mut impl Write,
    console_channel: &OwnedHandle<ByteChannelObject>,
) -> Result<(), Box<dyn std::error::Error>> {
    use hyper_vmm::console::{Input, InputAction};
    let mut bytes = [0u8; hyper_os::channel::MAX_MESSAGE_BYTES];
    let mut keyboard = Input::default();
    loop {
        if keyboard.is_pending() {
            match console_channel
                .as_byte_channel()
                .try_send(keyboard.pending())
            {
                Ok(()) => keyboard.sent(),
                Err(hyper_os::Error::Status(hyper_os::Status::WOULD_BLOCK)) => {}
                Err(error) => return console_error(output, b"write", error),
            }
        }
        let mut guest_signals = ObjectSignals::<ByteChannelObject>::READABLE
            .union(ObjectSignals::<ByteChannelObject>::PEER_CLOSED);
        if keyboard.is_pending() {
            guest_signals = guest_signals.union(ObjectSignals::<ByteChannelObject>::WRITABLE);
        }
        let waits = [
            WaitItem::new(
                input.as_handle_ref(),
                ObjectSignals::<ByteChannelObject>::READABLE
                    .union(ObjectSignals::<ByteChannelObject>::PEER_CLOSED),
            ),
            WaitItem::new(console_channel.as_handle_ref(), guest_signals),
        ];
        // stdout is line-buffered. Publish prompts and partial guest output
        // before waiting for more input from either peer.
        output.flush()?;
        let observation = match wait_many(&waits, hyper_os::DEADLINE_INFINITE) {
            Ok(observation) => observation,
            Err(error) => return console_error(output, b"wait", error),
        };
        if observation.index == 1 {
            if ObjectSignals::<ByteChannelObject>::READABLE.is_present_in(observation.observed) {
                let count = match console_channel.as_byte_channel().try_receive(&mut bytes) {
                    Ok(count) => count,
                    Err(hyper_os::Error::Status(hyper_os::Status::WOULD_BLOCK)) => continue,
                    Err(error) => return console_error(output, b"read", error),
                };
                output.write_all(&bytes[..count])?;
                continue;
            }
            if !ObjectSignals::<ByteChannelObject>::PEER_CLOSED.is_present_in(observation.observed)
            {
                continue; // Writable: retry the buffered input on the next pass.
            }
            output.write_all(b"\n[vmm] virtual machine disconnected\n")?;
            return Ok(());
        }
        if observation.index != 0
            || !ObjectSignals::<ByteChannelObject>::READABLE.is_present_in(observation.observed)
        {
            return Ok(());
        }
        let count = match input.as_byte_channel().try_receive(&mut bytes) {
            Ok(count) => count,
            Err(hyper_os::Error::Status(hyper_os::Status::WOULD_BLOCK)) => continue,
            Err(error) => return console_error(output, b"input", error),
        };
        for byte in bytes[..count].iter().copied() {
            match keyboard.push(byte) {
                InputAction::Continue => {}
                InputAction::Menu => output.write_all(b"\n[vmm] d/q: detach, any other key: resume\n")?,
                InputAction::Resume => output.write_all(b"\n[vmm] resumed\n")?,
                InputAction::Overflow => output.write_all(b"\n[vmm] guest input buffer full; excess input discarded (Ctrl-] still works)\n")?,
                InputAction::Detach => {
                    output.write_all(b"\n[vmm] detached\n")?;
                    return Ok(());
                }
            }
        }
    }
}

fn console_error(
    output: &mut impl Write,
    operation: &[u8],
    error: hyper_os::Error,
) -> Result<(), Box<dyn std::error::Error>> {
    let _ = output.write_all(b"\n[vmm] console ");
    let _ = output.write_all(operation);
    let _ = output.write_all(b" failed: ");
    let reason: &[u8] = match error {
        hyper_os::Error::Status(hyper_os::Status::ACCESS_DENIED) => b"access denied",
        hyper_os::Error::Status(hyper_os::Status::BAD_HANDLE) => b"bad handle",
        hyper_os::Error::Status(hyper_os::Status::BAD_STATE) => b"bad state",
        hyper_os::Error::Status(hyper_os::Status::BUSY) => b"busy",
        hyper_os::Error::Status(hyper_os::Status::FAULT) => b"fault",
        hyper_os::Error::Status(hyper_os::Status::INVALID_ARGUMENT) => b"invalid argument",
        hyper_os::Error::Status(hyper_os::Status::PEER_CLOSED) => b"disconnected",
        hyper_os::Error::Status(_) => b"kernel error",
        _ => b"invalid response",
    };
    let _ = output.write_all(reason);
    let _ = output.write_all(b"\n");
    Err(error.into())
}
