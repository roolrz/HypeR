// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native /init acceptance fixture. No fault injection enters product services.

use hyper_os::{
    fs::{Directory, DirectoryRights, FileRights},
    handle::{ByteChannelObject, OwnedHandle, ProcessObject, Rights, RightsOffer, TypedObject},
    startup::{self, Startup, StartupPurpose},
    task::ProcessBuilder,
    wait::{self, ObjectSignals, WaitItem},
};
use hyper_service::filesystem;
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    time::Duration,
};

mod volume;
type Result<T> = std::result::Result<T, String>;
const NOTICE: StartupPurpose<ByteChannelObject> = StartupPurpose::new(0x1000);

fn show(error: impl std::fmt::Debug) -> String {
    format!("{error:?}")
}
fn deadline() -> Result<u64> {
    hyper_os::time::deadline_after(Duration::from_secs(10))
        .map(|value| value.as_raw())
        .map_err(show)
}
fn receive(channel: &OwnedHandle<ByteChannelObject>, expected: &[u8]) -> Result<()> {
    wait::wait_many(
        &[WaitItem::new(
            channel.as_handle_ref(),
            ObjectSignals::<ByteChannelObject>::READABLE
                .union(ObjectSignals::<ByteChannelObject>::PEER_CLOSED),
        )],
        deadline()?,
    )
    .map_err(show)?;
    let mut bytes = [0; 64];
    let length = channel
        .as_byte_channel()
        .try_receive(&mut bytes)
        .map_err(show)?;
    if &bytes[..length] != expected {
        return Err(format!("unexpected worker message: {:?}", &bytes[..length]));
    }
    Ok(())
}
fn inherit<T: TypedObject>(
    builder: &ProcessBuilder,
    startup: &Startup<'_>,
    purpose: StartupPurpose<T>,
) -> Result<()> {
    builder
        .add_handle_duplicate(
            startup.borrow(purpose).map_err(show)?,
            purpose.as_raw(),
            RightsOffer::SameRights,
        )
        .map_err(show)
}

struct Worker {
    process: OwnedHandle<ProcessObject>,
    owner: Option<OwnedHandle<ByteChannelObject>>,
    notice: OwnedHandle<ByteChannelObject>,
}
impl Worker {
    fn launch(startup: &Startup<'_>, root: &Directory, path: &str) -> Result<Self> {
        root.create_directory(path, 0o755).map_err(show)?;
        let image = root.open("/init", FileRights::EXECUTE).map_err(show)?;
        let builder = ProcessBuilder::create(
            startup.borrow(startup::TASK_FACTORY).map_err(show)?,
            startup.borrow(startup::TASK_GROUP).map_err(show)?,
            startup.borrow(startup::RESOURCE_DOMAIN).map_err(show)?,
            image.as_handle_ref(),
        )
        .map_err(show)?;
        builder.set_name("fs-failure-worker").map_err(show)?;
        builder.add_argument("/init").map_err(show)?;
        builder.add_argument("--worker").map_err(show)?;
        // Fixture mount authority is restricted to this fresh empty directory.
        let mount = root
            .open_directory(
                path,
                DirectoryRights::READ
                    .union(DirectoryRights::WRITE)
                    .union(DirectoryRights::EXECUTE)
                    .union(DirectoryRights::TRANSFER),
            )
            .map_err(show)?;
        builder
            .add_handle_move(
                mount.into_handle(),
                startup::ROOT_DIRECTORY.as_raw(),
                RightsOffer::Exact(Rights::READ.union(Rights::WRITE).union(Rights::EXECUTE)),
            )
            .map_err(|error| show(error.error()))?;
        inherit(&builder, startup, startup::DYNAMIC_LIBRARY_DIRECTORY)?;
        builder
            .add_handle_duplicate(
                hyper_rt::process::console().map_err(show)?.as_handle_ref(),
                startup::CONSOLE.as_raw(),
                RightsOffer::SameRights,
            )
            .map_err(show)?;
        let (owner, child_owner) = hyper_os::channel::create_pair().map_err(show)?;
        let (ready, child_ready) = hyper_os::channel::create_pair().map_err(show)?;
        let (notice, child_notice) = hyper_os::channel::create_pair().map_err(show)?;
        for (handle, purpose, rights) in [
            (
                child_owner,
                filesystem::OWNER,
                Rights::WAIT.union(Rights::READ),
            ),
            (
                child_ready,
                filesystem::READY,
                Rights::WAIT.union(Rights::WRITE),
            ),
            (
                child_notice,
                NOTICE,
                Rights::WAIT.union(Rights::WRITE).union(Rights::READ),
            ),
        ] {
            builder
                .add_handle_move(handle, purpose.as_raw(), RightsOffer::Exact(rights))
                .map_err(|error| show(error.error()))?;
        }
        builder.seal().map_err(show)?;
        let process = builder.start().map_err(|error| show(error.error()))?;
        let worker = Self {
            process,
            owner: Some(owner),
            notice,
        };
        receive(&ready, filesystem::READY_MESSAGE)?;
        Ok(worker)
    }
    fn stop(&self) -> Result<()> {
        let supervisor = self.process.as_process_supervisor();
        supervisor.request_stop().map_err(show)?;
        supervisor.wait_terminated(deadline()?).map_err(show)
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        // Cleanup also cancels a worker when an assertion returns early.
        let _ = self.process.as_process_supervisor().request_stop();
    }
}

fn cached_file(path: &str) -> Result<File> {
    let mut file = File::open(path).map_err(show)?;
    let mut bytes = [0; 4096];
    file.read_exact(&mut bytes).map_err(show)?;
    if bytes.iter().any(|byte| *byte != 0x5a) {
        return Err("backend content differs".into());
    }
    file.seek(SeekFrom::Start(0)).map_err(show)?;
    Ok(file)
}
fn disconnected(file: &mut File, path: &str) -> Result<()> {
    let mut bytes = [0xa7; 64];
    if file.read(&mut bytes).is_ok() || bytes != [0xa7; 64] {
        return Err("dead server returned cached bytes or modified destination".into());
    }
    if file.metadata().is_ok() || File::open(path).is_ok() {
        return Err("dead server retained live metadata/namespace".into());
    }
    Ok(())
}
fn suite(startup: &mut Startup<'_>) -> Result<()> {
    let root = startup.take_root_directory().map_err(show)?;
    for (name, owner_loss) in [("/fs-worker-stop", false), ("/fs-owner-loss", true)] {
        let mut worker = Worker::launch(startup, &root, name)?;
        let path = format!("{name}/cached");
        drop(cached_file(&path)?);
        receive(&worker.notice, b"read")?;
        // Reopening retains identity. No backend read is allowed for this hit.
        let mut file = cached_file(&path)?;
        let mut unexpected = [0; 16];
        if !matches!(
            worker.notice.as_byte_channel().try_receive(&mut unexpected),
            Err(hyper_os::Error::Status(hyper_os::Status::WOULD_BLOCK))
        ) {
            return Err("close/reopen lost page-cache identity".into());
        }
        if owner_loss {
            drop(worker.owner.take());
            worker
                .process
                .as_process_supervisor()
                .wait_terminated(deadline()?)
                .map_err(show)?;
        } else {
            worker.stop()?;
        }
        disconnected(&mut file, &path)?;
        println!("FS-FAILURE: {name} PASS");
    }
    let worker = Worker::launch(startup, &root, "/fs-pending-stop")?;
    std::thread::scope(|scope| -> Result<()> {
        let pending = scope.spawn(|| -> Result<()> {
            let mut file = File::open("/fs-pending-stop/blocked").map_err(show)?;
            let mut bytes = [0xa7; 32];
            if file.read(&mut bytes).is_ok() || bytes != [0xa7; 32] {
                return Err("cancelled in-flight read returned data".into());
            }
            Ok(())
        });
        receive(&worker.notice, b"blocked")?;
        worker.stop()?;
        pending.join().map_err(|_| "read thread panicked")??;
        Ok(())
    })?;
    println!("FS-FAILURE: pending-stop PASS");
    caller_stop(startup, &root)?;
    write_overflow(startup, &root)?;
    Ok(())
}

/// A worker-controlled file size cannot wrap append or positioned-write offsets,
/// and a rejected request must leave the shared mount usable.
fn write_overflow(startup: &Startup<'_>, root: &Directory) -> Result<()> {
    let worker = Worker::launch(startup, root, "/fs-write-overflow")?;
    let file = root
        .open("/fs-write-overflow/max-size", FileRights::WRITE)
        .map_err(show)?;
    for (name, result) in [
        ("positioned", file.write_at(u64::MAX, b"x")),
        ("append", file.append(b"x").map(|(count, _)| count)),
    ] {
        if !matches!(
            result,
            Err(hyper_os::Error::Status(hyper_os::Status::INVALID_ARGUMENT))
        ) {
            return Err(format!(
                "overflowing {name} write was not rejected: {result:?}"
            ));
        }
    }
    let mut unexpected = [0; 16];
    if !matches!(
        worker.notice.as_byte_channel().try_receive(&mut unexpected),
        Err(hyper_os::Error::Status(hyper_os::Status::WOULD_BLOCK))
    ) {
        return Err("overflowing write reached the filesystem worker".into());
    }
    drop(cached_file("/fs-write-overflow/cached")?);
    receive(&worker.notice, b"read")?;
    worker.stop()?;
    println!("FS-FAILURE: write-overflow PASS");
    Ok(())
}

/// Once a request is published, stopping its caller must drain that transaction
/// without poisoning a healthy filesystem used by unrelated processes.
fn caller_stop(startup: &Startup<'_>, root: &Directory) -> Result<()> {
    let worker = Worker::launch(startup, root, "/fs-caller-stop")?;
    let image = root.open("/init", FileRights::EXECUTE).map_err(show)?;
    let builder = ProcessBuilder::create(
        startup.borrow(startup::TASK_FACTORY).map_err(show)?,
        startup.borrow(startup::TASK_GROUP).map_err(show)?,
        startup.borrow(startup::RESOURCE_DOMAIN).map_err(show)?,
        image.as_handle_ref(),
    )
    .map_err(show)?;
    builder.set_name("fs-failure-reader").map_err(show)?;
    builder.add_argument("/init").map_err(show)?;
    builder.add_argument("--reader").map_err(show)?;
    let directory = root
        .open_directory(
            "/fs-caller-stop",
            DirectoryRights::READ
                .union(DirectoryRights::INSPECT)
                .union(DirectoryRights::DUPLICATE)
                .union(DirectoryRights::EXECUTE)
                .union(DirectoryRights::TRANSFER),
        )
        .map_err(show)?;
    builder
        .add_handle_move(
            directory.into_handle(),
            startup::ROOT_DIRECTORY.as_raw(),
            RightsOffer::Exact(
                Rights::READ
                    .union(Rights::EXECUTE)
                    .union(Rights::INSPECT)
                    .union(Rights::DUPLICATE),
            ),
        )
        .map_err(|error| show(error.error()))?;
    inherit(&builder, startup, startup::DYNAMIC_LIBRARY_DIRECTORY)?;
    builder
        .add_handle_duplicate(
            hyper_rt::process::console().map_err(show)?.as_handle_ref(),
            startup::CONSOLE.as_raw(),
            RightsOffer::SameRights,
        )
        .map_err(show)?;
    builder.seal().map_err(show)?;
    let caller = builder.start().map_err(|error| show(error.error()))?;
    receive(&worker.notice, b"delayed")?;
    caller
        .as_process_supervisor()
        .request_stop()
        .map_err(show)?;
    // Let the woken kernel continuation observe cancellation before replying.
    // It should remain alive solely to retire the published transaction.
    std::thread::sleep(Duration::from_millis(100));
    worker
        .notice
        .as_byte_channel()
        .send(b"release")
        .map_err(show)?;
    caller
        .as_process_supervisor()
        .wait_terminated(deadline()?)
        .map_err(show)?;
    drop(
        cached_file("/fs-caller-stop/cached")
            .map_err(|error| format!("healthy mount failed after caller stop: {error}"))?,
    );
    receive(&worker.notice, b"read")?;
    worker.stop()?;
    println!("FS-FAILURE: caller-stop PASS");
    Ok(())
}

fn run() -> Result<bool> {
    let mut startup = hyper_rt::process::startup().map_err(show)?;
    if std::env::args().nth(1).as_deref() == Some("--worker") {
        let notice = startup.take(NOTICE).map_err(show)?;
        hyper_fs_service::serve(&mut startup, ".", volume::FixtureVolume(notice))?;
        Ok(false)
    } else if std::env::args().nth(1).as_deref() == Some("--reader") {
        let mut file = File::open("/delayed").map_err(show)?;
        let mut bytes = [0; 32];
        file.read_exact(&mut bytes).map_err(show)?;
        Err("cancelled caller returned to userspace".into())
    } else {
        suite(&mut startup)?;
        Ok(true)
    }
}
fn main() -> std::process::ExitCode {
    match run() {
        Ok(true) => {
            println!("FS-FAILURE: PASS");
            std::process::ExitCode::SUCCESS
        }
        Ok(false) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("FS-FAILURE: FAIL {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
