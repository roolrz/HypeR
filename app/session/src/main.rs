// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Virtual console owner: the transport survives each disposable shell client.

mod child;

use hyper_os::channel::{self, ByteRelay};
use hyper_os::handle::{ByteChannelObject, CapabilityChannelObject, OwnedHandle, ProcessObject};
use hyper_os::startup::Startup;
use hyper_os::wait::{ObjectSignals, WaitItem, wait_many};
use hyper_service::session as contract;
use std::process::ExitCode;
use std::time::{Duration, Instant};

/// One capability-bound console. No global shell or transport state: another
/// serial transport can be supervised by another instance of this owner.
struct VirtualConsole {
    shell_image: String,
    connection: Option<OwnedHandle<CapabilityChannelObject>>,
    input: OwnedHandle<ByteChannelObject>,
    output: OwnedHandle<ByteChannelObject>,
}

impl VirtualConsole {
    fn serve(&self, startup: &Startup<'_>, root: &hyper_os::fs::Directory) -> Result<(), ()> {
        loop {
            let started = Instant::now();
            let prepared = (|| -> hyper_os::Result<_> {
                let (input_writer, input_reader) = channel::create_pair()?;
                let (output_writer, output_reader) = channel::create_pair()?;
                let (error_writer, error_reader) = channel::create_pair()?;
                let process = child::launch(
                    startup,
                    root,
                    &self.shell_image,
                    self.connection.as_ref(),
                    input_reader,
                    output_writer,
                    error_writer,
                )?;
                Ok((process, input_writer, output_reader, error_reader))
            })();
            let (process, input_writer, output_reader, error_reader) = match prepared {
                Ok(client) => client,
                Err(error) => {
                    eprintln!(
                        "HypeR virtual console: shell launch failed ({error}); retrying in one second"
                    );
                    std::thread::sleep(Duration::from_secs(1));
                    continue;
                }
            };
            let result = self.relay(&process, &input_writer, &output_reader, &error_reader);
            // Errors must not leave an unsupervised shell owning the old input.
            if result.is_err() {
                process
                    .as_process_supervisor()
                    .request_stop()
                    .map_err(|_| ())?;
                process
                    .as_process_supervisor()
                    .wait_terminated(hyper_os::DEADLINE_INFINITE)
                    .map_err(|_| ())?;
                return Err(());
            }
            process
                .as_process_supervisor()
                .wait_terminated(hyper_os::DEADLINE_INFINITE)
                .map_err(|_| ())?;
            println!("HypeR virtual console: shell exited; restarting");
            // Rapid crashes/EOF must not turn supervision into a spawn loop.
            if let Some(delay) = Duration::from_millis(250).checked_sub(started.elapsed()) {
                std::thread::sleep(delay);
            }
        }
    }

    fn relay(
        &self,
        process: &OwnedHandle<ProcessObject>,
        input: &OwnedHandle<ByteChannelObject>,
        output: &OwnedHandle<ByteChannelObject>,
        error: &OwnedHandle<ByteChannelObject>,
    ) -> Result<(), ()> {
        let mut buffers = [
            vec![0; channel::MAX_MESSAGE_BYTES],
            vec![0; channel::MAX_MESSAGE_BYTES],
            vec![0; channel::MAX_MESSAGE_BYTES],
        ];
        let [out, err, keys] = &mut buffers;
        let mut routes = [
            ByteRelay::new(output.as_byte_channel(), self.output.as_byte_channel(), out),
            ByteRelay::new(error.as_byte_channel(), self.output.as_byte_channel(), err),
            ByteRelay::new(self.input.as_byte_channel(), input.as_byte_channel(), keys),
        ];
        let mut first = 0;
        let mut input_closed = false;
        loop {
            let exited = process
                .as_process_supervisor()
                .info()
                .map_err(|_| ())?
                .terminal
                .is_some();
            let mut progressed = false;
            for offset in 0..3 {
                let index = (first + offset) % 3;
                if index == 2 && (exited || input_closed) {
                    continue;
                }
                match routes[index].poll() {
                    Ok(progress) => progressed |= progress,
                    Err(hyper_os::Error::Status(hyper_os::Status::PEER_CLOSED)) if index == 2 => {
                        input_closed = true
                    }
                    Err(_) => return Err(()),
                }
            }
            first = (first + 1) % 3;
            // Drain already queued output; descendants retaining a writer must
            // not indefinitely prevent a new foreground shell.
            if exited && !progressed && !routes[0].has_pending() && !routes[1].has_pending() {
                return Ok(());
            }
            if routes[2].is_finished() {
                return Err(());
            }
            if progressed {
                continue;
            }
            let mut waits = Vec::with_capacity(4);
            if !exited {
                waits.push(WaitItem::new(
                    process.as_handle_ref(),
                    ObjectSignals::<ProcessObject>::TERMINATED,
                ));
            }
            for (index, route) in routes.iter().enumerate() {
                if index == 2 && (exited || input_closed) {
                    continue;
                }
                if let Some(wait) = route.wait_item() {
                    waits.push(wait);
                }
            }
            wait_many(&waits, hyper_os::DEADLINE_INFINITE).map_err(|_| ())?;
        }
    }
}

fn run(mut startup: Startup<'_>) -> Result<(), ()> {
    hyper_os::require_core_abi().map_err(|_| ())?;
    let mut arguments = std::env::args().skip(1);
    let shell_image = arguments.next().unwrap_or_else(|| "/bin/sh".into());
    if arguments.next().is_some() || !shell_image.starts_with('/') {
        eprintln!("usage: session [absolute-shell-image]");
        return Err(());
    }
    let console = VirtualConsole {
        shell_image,
        connection: startup
            .take_optional(hyper_service::vm::MANAGER_CONNECTION)
            .map_err(|_| ())?,
        input: startup.take(contract::CONSOLE_INPUT).map_err(|_| ())?,
        output: startup.take(contract::CONSOLE_OUTPUT).map_err(|_| ())?,
    };
    let root = startup.take_root_directory().map_err(|_| ())?;
    println!("HypeR virtual console: ready");
    console.serve(&startup, &root)
}

fn main() -> ExitCode {
    match hyper_rt::process::startup().map_err(|_| ()).and_then(run) {
        Ok(()) => ExitCode::SUCCESS,
        Err(()) => {
            eprintln!("HypeR virtual console: transport or supervision failed");
            ExitCode::FAILURE
        }
    }
}
