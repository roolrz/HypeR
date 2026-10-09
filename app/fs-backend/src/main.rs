// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Filesystem control plane. Detect the logical volume and start one confined
//! format worker; file requests travel directly between kernel and worker.
use core::mem::MaybeUninit;
use hyper_os::{
    block::NativeBlock,
    capability_channel::{CapabilityChannel, CapabilityReceiveSlot},
    fs::{Directory, DirectoryRights, FileRights},
    handle::{ByteChannelObject, NativeBlockObject, Rights, RightsOffer},
    startup,
    task::{ProcessBuilder, ResourceLimits, create_resource_domain, create_task_group},
};
use hyper_service::filesystem;
const LIMITS: ResourceLimits = ResourceLimits {
    kernel_memory_bytes: 64 * 1024 * 1024,
    processes: 1,
    threads: 4,
    handles: 64,
    kernel_objects: 256,
    committed_pages: 32 * 1024,
    pinned_pages: 1024,
    guest_pages: 0,
    ipc_messages: 32,
    ipc_bytes: 1024 * 1024,
    ipc_handles: 16,
    subscriptions: 32,
    timers: 32,
    virtual_machines: 0,
    virtual_cpus: 0,
    device_leases: 0,
    dma_mappings: 0,
    user_address_spaces: 1,
    user_mappings: 256,
};
fn run() -> Result<(), String> {
    let mut startup = hyper_rt::process::startup().map_err(show)?;
    let attach = CapabilityChannel::from_handle(startup.take(filesystem::ATTACH).map_err(show)?);
    let mut record = [MaybeUninit::uninit(); filesystem::ATTACH_BYTES];
    let mut slots = [
        CapabilityReceiveSlot::new::<NativeBlockObject>(hyper_os::block::RIGHTS),
        CapabilityReceiveSlot::new::<ByteChannelObject>(
            Rights::WAIT.union(Rights::WRITE).union(Rights::TRANSFER),
        ),
    ];
    let message = attach
        .receive(hyper_os::DEADLINE_INFINITE, &mut record, &mut slots)
        .map_err(show)?;
    let volume = filesystem::decode_attach(message.bytes()).ok_or("invalid volume attachment")?;
    let sectors = volume.sectors;
    let block = NativeBlock::from_handle(
        slots[0]
            .take::<NativeBlockObject>()
            .map_err(show)?
            .ok_or("missing volume")?,
    );
    let ready = slots[1]
        .take::<ByteChannelObject>()
        .map_err(show)?
        .ok_or("missing readiness endpoint")?;
    let mut sector = [0; 512];
    block
        .read_sectors(0, &mut sector)
        .map_err(context("read volume boot sector"))?;
    let format =
        hyper_filesystem::driver::identify(&sector).ok_or("unsupported filesystem format")?;
    let root = Directory::from_handle(startup.take(startup::ROOT_DIRECTORY).map_err(show)?);
    let image = root
        .open(format.worker(), FileRights::EXECUTE)
        .map_err(context("open filesystem worker"))?;
    let factory = startup.borrow(startup::TASK_FACTORY).map_err(show)?;
    let domain = create_resource_domain(
        startup.borrow(startup::RESOURCE_DOMAIN).map_err(show)?,
        LIMITS,
    )
    .map_err(context("create worker resource domain"))?;
    let group = create_task_group(factory, domain.as_handle_ref())
        .map_err(context("create worker task group"))?;
    let builder = ProcessBuilder::create(
        factory,
        group.as_handle_ref(),
        domain.as_handle_ref(),
        image.as_handle_ref(),
    )
    .map_err(context("create worker process"))?;
    builder
        .set_name(format.worker_name())
        .map_err(context("name worker"))?;
    builder
        .add_argument(format.worker())
        .map_err(context("set worker program"))?;
    builder
        .add_argument(&sectors.to_string())
        .map_err(context("set volume geometry"))?;
    std::fs::create_dir_all("/data").map_err(context("create mount directory"))?;
    let mount_directory = root
        .open_directory(
            "data",
            DirectoryRights::READ
                .union(DirectoryRights::WRITE)
                .union(DirectoryRights::EXECUTE)
                .union(DirectoryRights::TRANSFER),
        )
        .map_err(context("open mount directory"))?;
    builder.add_argument(".").map_err(show)?;
    builder
        .add_argument(if volume.read_only { "ro" } else { "rw" })
        .map_err(show)?;
    builder
        .add_handle_move(
            mount_directory.into_handle(),
            startup::ROOT_DIRECTORY.as_raw(),
            RightsOffer::Exact(Rights::READ.union(Rights::WRITE).union(Rights::EXECUTE)),
        )
        .map_err(|e| format!("delegate mount directory: {:?}", e.error()))?;
    builder
        .add_handle_duplicate(
            startup
                .borrow(hyper_service::process::CHILD_LIBRARY_DIRECTORY)
                .map_err(context("borrow child library capability"))?,
            startup::DYNAMIC_LIBRARY_DIRECTORY.as_raw(),
            RightsOffer::Exact(Rights::READ.union(Rights::EXECUTE)),
        )
        .map_err(context("delegate worker libraries"))?;
    builder
        .add_handle_duplicate(
            domain.as_handle_ref(),
            startup::RESOURCE_DOMAIN.as_raw(),
            RightsOffer::Exact(Rights::RESOURCE_DOMAIN_SPONSOR),
        )
        .map_err(context("delegate worker memory sponsor"))?;
    for (output, contract) in [
        (
            hyper_rt::process::stdout().map_err(show)?,
            hyper_service::stdio::STANDARD_OUTPUT_CONTRACT,
        ),
        (
            hyper_rt::process::stderr().map_err(show)?,
            hyper_service::stdio::STANDARD_ERROR_CONTRACT,
        ),
    ] {
        builder
            .add_handle_duplicate(
                output.as_handle_ref(),
                contract.purpose(),
                RightsOffer::Exact(contract.required_rights()),
            )
            .map_err(context("delegate worker output"))?;
    }
    let (_owner, worker) = hyper_os::channel::create_pair().map_err(show)?;
    builder
        .add_handle_move(
            block.into_handle(),
            filesystem::BLOCK.as_raw(),
            RightsOffer::Exact(hyper_os::block::RIGHTS),
        )
        .map_err(|e| format!("delegate volume: {:?}", e.error()))?;
    builder
        .add_handle_move(
            ready,
            filesystem::READY.as_raw(),
            RightsOffer::Exact(Rights::WAIT.union(Rights::WRITE)),
        )
        .map_err(|e| format!("delegate readiness: {:?}", e.error()))?;
    builder
        .add_handle_move(
            worker,
            filesystem::OWNER.as_raw(),
            RightsOffer::Exact(Rights::WAIT.union(Rights::READ)),
        )
        .map_err(|e| format!("delegate owner endpoint: {:?}", e.error()))?;
    builder.seal().map_err(context("seal filesystem worker"))?;
    let child = builder
        .start()
        .map_err(|_| "filesystem worker could not start")?;
    println!("HypeR fs-backend: {:?} worker started", format);
    child
        .as_process_supervisor()
        .wait_terminated(hyper_os::DEADLINE_INFINITE)
        .map_err(show)?;
    Err("filesystem worker terminated; mount retired".into())
}
fn context<E: core::fmt::Debug>(operation: &'static str) -> impl FnOnce(E) -> String {
    move |error| format!("{operation}: {error:?}")
}
fn show(error: impl core::fmt::Debug) -> String {
    format!("{error:?}")
}
fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("HypeR fs-backend: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
