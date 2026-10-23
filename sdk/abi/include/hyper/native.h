/* SPDX-FileCopyrightText: 2026 roolrz
 * SPDX-License-Identifier: Apache-2.0
 *
 * Generated from schema/native.rs. Do not edit.
 */

/**
 * @file
 * @brief Fixed-width HypeR Native capability invocation ABI.
 * The receiver handle selects authority; operation numbers are object-local.
 * Constants remain integers on the wire, including values grouped as enum domains.
 * See @ref hyper_native_contract for ownership and lifecycle rules.
 * @defgroup hyper_native HypeR Native ABI
 * @{
 */
/**
 * @page hyper_native_contract Native ABI semantic contract
 *
 * - Native invocations carry a process-local target handle in the selector register, a separate object-local operation, and six business argument registers. AArch64 uses x8/x6/x0..x5; RISC-V uses a7/a6/a0..a5. No global syscall number or implicit privileged target exists. Common handle operations use 0..5; object-specific operations start at 256 and may repeat across object kinds. Unknown target handles fail before operation dispatch; unknown operations return NOT_SUPPORTED. The receiver is not duplicated in the business arguments.
 *
 * - ExecutionContext and ObjectFactory are ordinary process-local, nontransferable capabilities bound to their creating Process. They never grant operations on another Process, Thread, address space or resource domain. ExecutionContext methods act on the invoking Thread or its Process; ObjectFactory charges every allocation to that Process's resource domain. SystemInfo grants public read-only ABI, configuration and clock observations. Closing or attenuating a capability does not implicitly recreate its authority.
 *
 * - device_dma_fault_info requires INSPECT and ASSIGN_DEVICE on the physical-device handle. It returns the first retained DMA containment report without consuming it; an all-zero record means no report. DMA_FAULT is a persistent level signal observed with WAIT. Claim and claim_matching grant WAIT. Fault evidence follows the device object across handle transfer; querying does not acknowledge or resume DMA. The kernel never selects a userspace process or stops a VM for this notification. The formal I/O runtime queries after adoption and journals each device's first report; bootstrap does not consume it. timestamp_ns is monotonic boot time. source 1 identifies SMMUv3. scope 1 means stream quarantine, with code holding the SMMUv3 event ID; scope 2 means controller containment, with code holding the captured GERROR bits (zero is possible for a software-detected failure). stream_id always identifies this handle's device. ADDRESS_VALID, READ and WRITE describe the address only for translation/access faults; controller reports disclose no other device's address. reserved is zero. A first stream report remains unchanged if the whole controller later fails. Quarantine and backing retention remain independent of observer presence or progress.
 *
 * - Scheduling reads require INSPECT on ExecutionContext, Thread or VirtualCpu. ExecutionContext targets only the invoking Thread; VirtualCpu targets its scheduler Thread. Configuration is a complete 64-byte record with canonical zero inactive/reserved fields. Fair weight is 1..1024 and quantum 1..1000000000 nanoseconds; Fair backend 2 requests the configured default, while reads return concrete backend 0 or 1. Budget Q, P and burst are positive, Q and burst do not exceed P, and P must equal the kernel arbitration period. Disabled policies or malformed parameters return INVALID_ARGUMENT before mutation.
 *
 * - Scheduling changes compare old and requested parameters atomically under scheduler ownership. SCHED_LOWER permits lowering band, Fair weight or FIFO priority; SCHED_PRIORITY permits raising those fields and changing quantum; SCHED_CLASS permits policy, arbitration role or Fair-backend changes; SCHED_BUDGET permits any budget change. A compound change may require multiple rights. WRITE, thread ownership, executable names and configuration text confer no elevation. Immutable target ceilings remain enforced after handle attenuation or transfer. Rejection preserves the old parameters.
 *
 * - ProcessBuilder scheduling configuration also declares the child's execution-context scheduling subset. It cannot delegate rights absent from the builder. Ordinary process creation uses the native App boundary and preserves ordinary demotion; in-process thread_create inherits its caller's current parameters and cannot restore a higher initial weight. All threads receive SCHED_LOWER for self demotion; child Thread handles receive only scheduling rights present in both the execution-context receiver and process grant. VM creation authorities propagate explicitly held scheduling rights through leases, pending machines, installed machines and vCPUs. Thread handles may transfer only through explicit capability rendezvous, with the receiver retaining an attenuated subset of the sender's rights. Creating, blocking, migrating or switching policy does not authorize resetting an existing Thread's budget accounting. Budget entitlement is per Thread, not an aggregate Process or VM cgroup quota; separately authorized construction creates independently accounted Threads.
 *
 * - thread_set_current_name changes only the calling Native Thread's diagnostic label. The caller must hold its ExecutionContext with WRITE; no other Thread or Process can be renamed. Input is 1..thread_name_max_bytes (64) bytes of UTF-8 without control characters, excluding any NUL terminator. Empty, oversized, invalid UTF-8, control-containing names and nonzero unused arguments return INVALID_ARGUMENT without changing the old name. Inspectors receive complete old or new snapshots; names confer no identity or authority. Process names remain unchanged.
 *
 * - memory_inspector_read_statistics requires MemoryInspector INSPECT; it is observational and never drains caches or initiates reclaim. Topic selects a typed extensible record: heap (index 0), buddy (orders 0..18), owner (0 kernel, 1 page tables, 2 guest, 3 user, 4 file cache), file_cache (index 0), or heap_class (0..8 kernel, 9..17 file-cache pool; eight slab sizes 16..2048 then large allocations). Unknown topic/index and a nonzero unused argument are INVALID_ARGUMENT. Heap-class records require CONFIG_HEAP_PROFILING, otherwise NOT_SUPPORTED. Other records are always available after allocator initialization.
 *
 * - Memory statistics are independently sampled, including fields sourced from atomic counters; they are not a transaction across topics, CPUs, or logical and physical accounting. Page counts use memory_observation.page_size. Heap-class reserved slots include live objects, magazine objects and in-flight transfers; large buckets report zero capacity/reserved/cached and object_size=0. File-cache metadata physical pages and reservations overlap other accounting and must not be added twice. Cumulative heap-class allocation/deallocation counters saturate and reset only on reboot. No address or allocation authority is exposed.
 *
 * - object_inspector_read_details requires INSPECT and INSPECT_DETAILS on an ObjectInspector. With process_koid zero, target is a KOID and only system scope is allowed. Otherwise target is a generation-qualified handle in the named, scope-visible process. Inspection takes a diagnostic reference under the lookup lock, releases that lock, and copies object-local metadata only; it does not grant operation authority. Cursor zero starts a query; next_cursor zero finishes it. Empty records may advance a bounded scan without yielding a mapping. Concurrent mutation makes pages weakly consistent. Unsupported object kinds return NOT_SUPPORTED; inaccessible or expired identities return NOT_FOUND. Inspector derivation continues to produce only the original basic rights, never implicitly acquiring INSPECT_DETAILS.
 *
 * - Object-details payload consists of eight little-endian u64 words. All unused words are zero. Record 0 is empty. Thread (1): scheduler TID (zero is a valid bootstrap TID), role (task_thread role values), user lifecycle phase (0 unavailable, 1 prepared, 2 dormant, 3 runnable, 4 stop-requested, 5 detached), TID present (0/1; absent TID word is zero). VMAR (2): base, length, live (0/1). Mapping (3): base, length, current R/W/X bits, maximum R/W/X bits. Channel (4): peer KOID (zero until pair publication), local open (0/1), peer open (0/1), queued messages or receivers, queued bytes, byte-queue counters present (0/1). Device (5): profile, virtio device ID, PCI vendor/device word, host IRQ domain, first host interrupt, interrupt count, lifecycle (1 claimed, 2 attached, 3 active, 4 retired, 5 quarantined), resource count. Device resource (6): resource kind, host physical base, length, guest aperture offset, attributes. Addresses never contain kernel virtual pointers. Device details read metadata without MMIO/config-space probes; host interrupt identities are not guest IRQs.
 *
 * - Device assignment authority separates INSPECT (firmware discovery), ASSIGN_DEVICE (claims), and MAP_DMA (resident DMA extent disclosure). Claim and claim_matching return TRANSFER|DUPLICATE|INSPECT|ASSIGN_DEVICE; userspace bundles additionally grant READ|WRITE|WAIT|ACK_INTERRUPT. Assigning a physical device requires ASSIGN_DEVICE on that handle and WRITE on the pending VM. device_mmio checks READ for operation zero and WRITE for operation one after validating the operation selector; its unconditional schema rights are empty. MMIO reads may have hardware side effects and still require an active assignment with retained backing. IRQ pending requires WAIT; IRQ completion requires ACK_INTERRUPT. An inspection-only handle cannot claim, assign, access registers, acknowledge interrupts, or disclose VMO physical addresses.
 *
 * - Device firmware inspection reads one immutable boot snapshot. Field PROPERTY_NAMES (5) returns every property name as a NUL-separated list, without granting ownership or exposing mapping capabilities; field PROPERTY (4) reads a named property's original bytes. Other fields report node identity, compatible strings, translated registers and interrupt metadata. Empty output queries the required byte length. The caller supplies a name only for PROPERTY; all other fields require an empty name. Inspection cannot substitute for an atomic device claim.
 *
 * - Physical device profiles identify the admitted transport contract: virtio-mmio SCSI (1), userspace-managed registers (2), virtio-mmio network (3), and an exclusively assigned PCI function (4). Firmware matching combines profile with identity and rejects ambiguous matches. PCI identity kind 3 uses exactly nine lowercase ASCII bytes vvvv:dddd (vendor and device identifiers). Physical MMIO is mediated against exact resource extents; a device handle never grants a host MMIO page mapping. DEVICE_PROFILE_INFO reports the complete guest aperture, resource count, interrupt count, PCI device/vendor word and admitted DMA bus offset. Non-PCI profiles use a 64 KiB aperture and zero PCI identity and DMA offset. Virtio profiles have one interrupt; userspace profiles report zero or one physical interrupt. Non-PCI assignment still reserves one virtual interrupt when no physical interrupt is present. A pending VM admits at most DEVICE_ASSIGNMENT_MAX_DEVICES controllers with disjoint apertures and interrupt ranges; all share its DMA backing lifetime. Partial installation rolls back every controller, and retirement releases pages only after every controller is quiescent.
 *
 * - The PCI-function profile presents one endpoint at guest BDF 00:00.0 through an 8 MiB aperture. Resource kind PCI_ECAM describes a virtual 1 MiB ECAM window. Interrupt delivery uses either PCI_MSI, a 4 KiB GICv2m frame, or PCI_INTX with interrupt_count one and zero offset, length, flags and bus_address; PCI_BAR0 through PCI_BAR0+5 describe implemented memory BARs. Resource offsets are relative to the assigned guest aperture; bus_address is the initial virtual PCI address of a BAR. MEMORY_64 and PREFETCHABLE flags apply only to BAR resources; other resources have zero flags and bus_address. PCI configuration and MSI-X are mediated: physical host-bridge, interrupt-controller and DMA-window registers are never guest resources. The interrupt argument selects the first of interrupt_count consecutive GIC SPIs (at most 64); the AArch64 reference GIC has 256 IDs. Linux owns the entire endpoint and its child drivers. DMA bus addresses equal the admitted host physical extent plus dma_bus_offset. The offset alone is not an IOMMU boundary. On the QEMU ECAM/SMMUv3 path the kernel binds requester IDs to the VM domain and maps only retained initial RAM and live grants before bus mastering; MMIO and BCM2712 assignments remain trusted. Firmware-initialized devices remain fail-closed if link or DMA translation validation fails. Clearing bus mastering alone is not proof of DMA retirement; a PCI assignment without a proven reset/quiescence protocol retains its DMA backing and physical claim on teardown.
 *
 * - system_config queries a public scalar system property by u64 key. PAGE_SIZE (1) returns the Native mapping granule in bytes, immutable for the lifetime of a process. Unknown keys return NOT_SUPPORTED. Unused argument registers must be zero. Success returns the value in value0 and zero in value1. This query requires no inspector capability and does not expose privileged observations.
 *
 * - vmar_allocate(parent, address, size, options) uses address as the low end. Options zero treats nonzero address as a hint, choosing the nearest fitting free interval with lower-base tie breaking; address zero selects the lowest free interval. VMAR_ALLOCATE_EXACT requires the exact address, including zero. Unknown flags are invalid. Size is nonzero, addresses and size page aligned, and intervals must not overflow. Success returns child in value0 and actual base in value1. Selection and reservation commit atomically within parent authority. No fitting free interval returns NO_MEMORY.
 *
 * - system_config key SYSTEM_CONFIG_APPLICATION_ADDRESS_LIMIT returns the HAL application-exclusive address limit in value0 and zero in value1. It describes profile geometry, not VMAR authority. The kernel grants ROOT_VMAR from one page to that limit.
 *
 * - Every Native process enters a caller-selected loader. Process-builder create derives the executable File into the child using explicit READ|EXECUTE|PROTECT|DUPLICATE|TRANSFER authority, and borrows the loader with READ|EXECUTE. The target is not frozen by builder creation. The kernel takes only a temporary stable loader image while preparing its restricted bootstrap mappings. Seal validates and maps only the loader's restricted Native ET_DYN bootstrap image and an empty guarded stack; the first argument register holds a ByteChannel handle. Only initial boot selects a loader by pathname: /lib/userspace-loader-hyper-&lt;architecture&gt; launches /init from the ramdisk root. The queued message contains loader_startup, handle_count startup_handle records, then data_size opaque bytes. size is 96; flags permits only loader_startup_reply. execution_context, object_factory and system_info are process-installed capabilities for current execution, local object allocation and public system queries. Executable is an ordinary File with READ|EXECUTE|PROTECT. The user loader acquires content protection before querying size or parsing ELF. root_vmar and stack_vmar identify child-owned mapping authority; stack_base and stack_size describe mapped usable stack bytes with an unmapped guard at each end. loader_base and loader_size describe the loader mappings. Library directories are ordinary explicitly supplied startup capabilities; the kernel neither selects nor opens them. No argv, environment, auxv, application ELF parsing or relocation belongs to this kernel entry contract. The selected runtime owns bootstrap-handle closure and mapping retirement. Closing a VMAR handle alone does not unmap or destroy its reservation.
 *
 * - Virtual CPU set_affinity borrows a VirtualCpu with WRITE and accepts the same nonempty little-endian u64 CPU-mask word array as process-builder affinity. Bits above the kernel CPU limit are rejected; the mask must contain a schedulable CPU. The affinity update retains the current assignment when allowed; otherwise the scheduler selects an allowed registered CPU and safely migrates the saved context. Automatic Fair balancing may later move a Ready vCPU within this allowed set; a singleton mask prevents automatic movement. OK means the update or required handoff was accepted; migration may complete before or after return. An overlapping incompatible handoff or CPU-local wait returns busy. VM stop or guest power operations may supersede the request. Stopping, detached, and reaped endpoints reject new affinity updates. VirtualCpu info extends its 24-byte prefix with host_cpu and migration_target at offsets 24 and 28, one scheduler placement observation rather than running state or a completion token. Value 0xffffffff means unavailable assignment or no pending migration; older kernels return only the original prefix.
 *
 * - Guest mapping creation borrows backend VirtualMachine WRITE and GuestMemory MAP, populates the full stable grant, and returns an owned mapping handle plus a backend-VM-local non-reused token. The backend IPA envelope must contain all sparse aliases: alias = 64 GiB + host physical address, with host addresses below 256 GiB. Unadmitted envelope holes have no RAM or page bitmap entries. The kernel constructs immutable frontend-ordered, coalesced physical extents; neither Native policy nor Linux may replace their addresses. Backend notification write64 0x28 admits one token exactly once to that unique notification route; read64 0x30/0x38/0x40/0x48 return frontend base, total bytes, status, extent count. Write64 0x50 selects an extent index; read64 0x58/0x60/0x68/0x70 return alias, frontend-relative offset, byte length, and query status. Rejection clears the old reply. Extents cover the entire frontend range without holes or overlapping physical pages. After draining vhost and releasing all page pins, only that backend route may write64 the token at 0x20 to certify quiescence. Mapping release accepts never-admitted or quiescent tokens, withdraws lookup, clears leaves, waits for every CPU translation acknowledgement and, for an IOMMU-bound backend, completed DMA invalidation, then releases backing. Closing a mapping handle alone quarantines its pages until backend VM retirement. Notification control operation 3 permanently disconnects both routes, only when no admitted mapping remains; a disconnected route never affects an address or IRQ reused by a subsequent connection.
 *
 * - Native block creation dedicates one fully resident 1048576-byte guest-memory grant permanently to one kernel virtio-scsi initiator; repeated creation from that grant fails. The backend VM must retain the matching mapping until execution and physical DMA are quiescent. Six standard split queues (control, event, and four request queues) have size 8, descriptor offsets n*4096, available offsets n*4096+256, and used offsets n*4096+512, relative to guest_base. Userspace negotiates VERSION_1 with the backend before activation. Activation enables notifications and issues READ CAPACITY(16), accepting only 512-byte sectors; capacity is never supplied by userspace. The block capability remains the data-plane owner across filesystem worker handoff; a clean unmount returns the same activated object rather than creating or activating a replacement. Reads use bounded batches across the four request queues; writes and flushes remain ordered. Requests use shared queues and blocking kernel notifications, not per-request userspace RPC. Unretired request failures permanently disconnect the initiator and never recycle its buffer. PEER_CLOSED reports disconnection. The physical pages remain retained by backend mappings until VM/DMA retirement; notification closure alone never proves DMA quiescence.
 *
 * - filesystem_mount borrows a READ|WRITE|WAIT ByteChannel, a READ|WRITE|MAP shared VMO and a WRITE|EXECUTE Directory. The kernel retains the transport and buffer, validates the worker root, and publishes a fresh mount and filesystem identity at the capability-relative path. The VMO must satisfy the published filesystem protocol capacity and pinning contract. read_only is exactly zero or one; read-only mounts reject mutating protocol operations in the kernel independently of worker policy. Media discovery, format selection and exclusive NativeBlock ownership belong to the userspace filesystem manager and format worker, not this syscall.
 *
 * - filesystem_mount_ramfs and filesystem_switch_root require MANAGE_NAMESPACE|WRITE|EXECUTE on the live namespace-root Directory anchor. Ordinary directory opens never confer MANAGE_NAMESPACE; only explicit capability duplication, delegation or root-anchor scope creation preserves it. Ramfs mount does not create or configure block filesystems. switch_root copies its request, at most 16 mount moves and every bounded UTF-8 path before namespace mutation; zero moves requires a null moves pointer. new_root and move sources resolve in the old namespace, while put_old and move targets resolve inside new_root. The root and all requested moves publish atomically. Existing non-anchor Directory and File capabilities retain their opened locations; existing root anchors follow the published root. A rejected transaction preserves all mount edges and the current root. Paths are at most 4096 bytes, nonempty and contain no NUL bytes. Layout, boot stages and the choice of put_old are userspace policy.
 *
 * - filesystem_unmount requires Directory WRITE|EXECUTE and an exact mounted-root path. The namespace root cannot be detached. Live File, Directory, cwd, path-resolution or nested-mount uses return busy. Admission closes before synchronous backend flush and namespace publication; no mount-table spinlock is held across backend IPC. A failed flush or concurrent topology change leaves the mount attached. Successful detach permanently retires its transport health and content identity, so retained snapshots or speculative cache work cannot revive the old mount. Userspace then stops and joins the worker before reusing the returned NativeBlock; the syscall itself does not terminate processes or release backend DMA mappings.
 *
 * - VM power control requires WRITE authority. virtual_machine_get_power_request returns a non-consuming snapshot of one pending request, or would_block when none exists. Request IDs identify one completion and stale IDs are rejected. Operations are CPU_ON (1), CPU_OFF (2), SYSTEM_OFF (3), and SYSTEM_RESET (4); accept is strictly 0 or 1. Reserved output is zero. POWER_REQUEST signals pending work, and VCPU_TERMINATED prompts inspection of per-vCPU terminal state. virtual_machine_open_vcpu returns a control handle for an already configured vCPU; it does not change the immutable topology. The runtime owns lifecycle policy; no guest request is forwarded to host firmware. Guest suspend operations remain unsupported.
 *
 * - PROTECT grants authority to acquire content protection, not READ, WRITE or EXECUTE. Directory/file derivation can only attenuate this right. file_protect returns a process-local nonduplicable, nontransferable FileProtection with MAP|INSPECT and the size observed under protection. Protection rejects writes and resize through every alias but does not freeze names or other metadata. Closing the token releases its reference; protected file mappings retain separate references until unmap or owner-process teardown. After all owner threads stop and its handles close, teardown detaches protected sources without allocation. External VMAR owners can retain the dead address space and its charged pages, but not protection or permission to refill from the file. vmar_map_file requires File READ, additionally EXECUTE for executable mappings, FileProtection MAP for that same canonical file, and MAP on a VMAR of the calling process. It uses private_mapping range/zero-fill semantics. COPY_ON_WRITE permits clean pages to retire after translation invalidation and fault back from the protected file. Both file mapping modes populate clean pageable contents; EAGER does not grant residency. Full clean pages share canonical cache storage, while partial pages and private writes remain isolated. LOCK_MEMORY on a process-owned VMAR authorizes vmar_lock_memory/ vmar_unlock_memory. Range calls require page-aligned nonempty ranges; VMAR_MEMORY_CURRENT selects only currently mapped pages, including descendants, with address and size zero. Lock success populates the range and charges PinnedPages to the requester; repeated lock is idempotent. Unmap and process teardown release locks; future mappings are not locked. Shared pages remain cached while a mapping retains them. ProcessBuilder memory-lock delegation requires TaskFactory LOCK_MEMORY and explicit enablement before seal. Both modes retain protection until unmap or owner-process teardown; private modified pages cannot be discarded without another backing store. File snapshot VMOs are not exported to userspace. Explicit mapping and user-buffer copy syscalls report allocation, quota or I/O failures. Hardware refaults have no syscall result; when bounded reclaim/refill retries fail, the kernel terminates the Process as an unhandled page fault. Reclaim is best effort and page-table withdrawal may require temporary transaction resources before releasing pages.
 *
 * - Immutable VMO snapshots grant READ and MAP but never WRITE. vmo_create_snapshot coherently copies its source and reports Busy while writable mappings or hardware writers exist. Private mappings require source READ|MAP and VMAR MAP: source_offset is page aligned, data_offset is less than one page, source_length bytes starting at source_offset+data_offset initialize destination data_offset; all other destination bytes are zero. Writable private views never permit EXECUTE. Executable private views additionally require an executable source VMO and source EXECUTE rights; sanitized pages are sealed and instruction-published before mapping, with no writable alias. Copy-on-write mode shares immutable full pages until first write; eager mode allocates all private pages before publication. Private writes, including kernel copyout and atomic waits, cannot change the source or another private view. Ordinary writable/shared VMOs and exclusive hardware leases keep their stable backing contract. Snapshot bytes and old page versions remain owned through acknowledged translation retirement.
 *
 * - VM platform inspection borrows an INSPECT creation lease without consuming it. Metadata describes the selected local platform: counter_frequency_hz is the actual guest counter frequency, and riscv_isa is a guaranteed subset across all admitted CPUs, not a complete host ISA listing. Other architectures return a zero RISC-V mask. The guarantee remains valid across local CPU migration; cross-machine migration is not implied.
 *
 * - WaitSets are process-local, non-transferable objects with capacity 1..1024. BIND_WAIT authorizes add/rearm/remove, WAIT authorizes consumption. Add requires source WAIT and reserves one event slot; WaitSet and CapabilityChannel sources are unsupported. Registration IDs are globally non-reused. Bind/rearm observe signal levels and sequence under the source lock; one-shot publication does not allocate. Rearm is busy until successful event consumption. Wait returns one exact 24-byte record (registration ID, signal bits, sequence), using an absolute monotonic deadline. Copyout failure restores the event unless removal or closure cancelled it. Source handle close does not cancel object-lifetime subscriptions; final set handle close detaches registrations and wakes consumers. Future CapabilityChannel subscriptions require ownership-epoch invalidation.
 *
 * - ByteChannel duplicate authority permits shared endpoint ownership; peer_closed is published only when the last active endpoint handle closes. Internal operation pins, including WaitSet subscriptions, do not retain active endpoint authority.
 *
 * - process_get_current_id returns the calling Process KOID for observation only. It creates no handle or operational authority and is not a PID-to-handle lookup.
 *
 * - Handles are opaque nonzero process-local u64 values with bits 63..56 fixed to 0x0F. Zero is the absent/invalid handle sentinel; all other high-byte values are reserved and rejected as handles. Invocations return a separate signed status and unsigned result words. The low 56 bits are masked table-local data: callers must not infer slot/generation fields, allocation order, or numeric relationships. Slot reuse advances a wrapping generation without permanently retiring capacity; numeric values may repeat. Using a closed, replaced, or moved handle is a caller error and may operate on a later entry with that value. Inspector and retained crash records report the exact value held by the observed Process; queries use that Process namespace. Transfers publish destination-local values. Numeric values alone never grant authority outside the owning Process.
 *
 * - KOIDs encode a nonzero 32-bit slot in bits 0..31 and a 32-bit generation in bits 32..63. A slot remains unique until final payload destruction, after which reuse advances its generation, wrapping to one at the maximum. Live objects have distinct IDs, but complete values may repeat after destruction. Diagnostic queries describe the current object and do not establish identity across destruction and reuse. A language runtime with a 32-bit process ID surface may expose the KOID slot, accepting reuse after object destruction. Neither form grants authority or permits identifier-to-handle conversion; KOIDs do not encode creation order. Kernel operations retain direct counted object references rather than recovering authority from historical IDs.
 *
 * - Ramfs storage and namespace policy remain kernel-owned; disk filesystem parsing runs in userspace. Open File and Directory objects pin their nodes after unlink. Directory WRITE permits namespace mutation; File WRITE permits content mutation. directory_create_file exclusively creates a regular file and publishes its requested handle atomically with the new name. Mode accepts permission bits 0777. directory_remove options 0 removes a non-directory entry without following the final symlink; 1 removes an empty directory. file_write_at options 0 uses offset, 1 atomically appends (offset must be zero); returns actual bytes and end offset. Transfers may complete short. file_resize zero-fills extension. Active file protections reject content changes through every File alias.
 *
 * - Thread creation prepares a dormant thread in the calling Process and returns duplicate, wait, inspect, start and request_stop authority. Entry, 16-byte-aligned stack, TLS and an opaque first argument define its initial context. Affinity pointer/count zero together inherit the calling Thread current allowed CPU set; otherwise they name one to four little-endian u64 bitmap words. An explicit mask must be nonempty, fit supported CPU indices and include at least one schedulable CPU. The child remains dormant until start, with affinity already applied. The caller retains stack/TLS ownership through the thread terminated signal. Start publishes runnable execution; request_stop abandons execution without language destructors. Closing the last handle to a dormant thread cancels it; running threads continue independently of handle ownership. Existing thread_exit terminates only the caller, while process_exit stops every thread.
 *
 * - Atomic wait/wake currently operate on aligned writable u32 words in the calling Process. Wait checks the value and publishes its generation atomically with respect to wake; mismatch returns ok, and spurious wakes are allowed. Wait uses an absolute monotonic nanosecond deadline, with UINT64_MAX meaning infinite; timeout returns timed_out and cancellation returns cancelled. Wake returns the number of registrations actually notified, bounded by count. Wake does not replace the caller's release operation or the waiter's acquire predicate check. Keys include the non-reused mapping identity and virtual address; aliases and other Processes have separate wait domains. Keep the mapping live while waiting: unmap/protect does not move an existing wait to a replacement mapping. Pinned backing survives concurrent unmap until admitted accesses finish; process stop cancels outstanding waits.
 *
 * - Thread sleep accepts an absolute monotonic nanosecond deadline and returns ok when it expires, including an already elapsed deadline. UINT64_MAX sleeps until cancellation. It parks the current thread through scheduler timeout arbitration rather than polling or yielding.
 *
 * - The monotonic clock operation requires SystemInfo READ and returns absolute nanoseconds from the kernel's monotonic clock domain. The read-only capability confers no mutable clock authority.
 *
 * - Extensible input records carry their caller size in the syscall's explicit byte-count argument. The size must be at least the record's published minimum prefix and no greater than extensible_record_max_bytes; missing bytes through the kernel's current record size default to zero, while bytes beyond that size must be zero. Extensible information outputs accept any capacity from the record's minimum prefix through extensible_record_max_bytes, write only the intersection of caller capacity and kernel-supported size, and return the kernel-supported size in value0 on ok. Bytes beyond that intersection remain untouched.
 *
 * - Directory lookup is capability-relative. A leading slash restarts at that Directory's traversal root, and parent components cannot escape it. Every open requires read traversal authority, and every requested File right must already be present on the source Directory before the result is further bounded by the File object's node-specific ceiling.
 *
 * - directory_begin_read requires only Directory READ and creates an independent DirectoryStream. Its grant is the source Directory grant intersected with READ|DUPLICATE|TRANSFER|INSPECT, so attenuated propagation or inspection authority is never restored. Each stream retains its opened namespace location and small trusted backend position, never directory cache pages. Duplicated or transferred stream handles share progress. directory_read now requires DirectoryStream READ and the exact published page capacity. Cookie zero reads the first page; subsequent cookies must have been returned by that stream. Nonzero cookies are globally monotonic and never reused. Only the most recent input cookie can be replayed for copyout retry; older, forged or foreign-stream cookies are invalid. A zero next_cookie marks EOF; short nonempty pages may still have a continuation. Every name is one valid path component encoded as name_length UTF-8 bytes followed by zero-filled capacity. Remote mutations of the scanned directory, or renames of that directory or an ancestor, invalidate its ongoing scans and require a new stream; mutations of unrelated directories and file-content changes preserve traversal. Ramfs enumeration remains weakly consistent across mutations. No scan is an immutable directory snapshot. Callers must not interpret or synthesize cookies. Closing the final stream handle releases the position and namespace pin.
 *
 * - Native task and object inspectors are immutable capability-scoped views. Process, thread, and object KOIDs plus scan cursors are observation-only values and can never be exchanged for operational authority. Out-of-scope targeted lookup returns not_found.
 *
 * - Task inspector records carry a bounded UTF-8 name as name_length bytes followed by zero-filled capacity. Process names are the immutable labels committed by ProcessBuilder publication; Thread names are immutable scheduler identity labels retained through the retiring registry phase.
 *
 * - Inspector derivation is monotonic: a derived Process, TaskGroup, or ResourceDomain view cannot widen its parent's task scope, object scope, visibility, or rights. Derivation requires DUPLICATE | TRANSFER | INSPECT | DERIVE and returns exactly that base set; INSPECT_DETAILS is never added or preserved by the existing derivation calls. Callers attenuate the returned handle before delegation. Native task operations remain handle-based; numeric PID and TID namespaces belong exclusively to compatibility personalities.
 *
 * - process_derive_observer requires WAIT | TRANSFER | REQUEST_STOP on a Process and returns a new WAIT | TRANSFER handle for the same KOID. It never copies supervisor stop or inspect authority and the derived observer cannot itself derive further observers. General handle duplication still requires DUPLICATE.
 *
 * - task_group_request_stop closes new membership admission and requests asynchronous member stop. TASK_GROUP_TERMINATED is sticky and becomes observable only after pending construction reservations have completed or rolled back and every member Process has completed retirement. It covers this flat TaskGroup only, not processes attached to other groups.
 *
 * - console_discard_input requires READ and atomically discards the currently accepted physical Console input prefix. It returns the byte count; input arriving later remains queued. An outstanding transactional read claim returns would_block without discarding any bytes. This operation has no terminal or character interpretation.
 *
 * - Every live TaskGroup handle participates in shared group-lifetime ownership regardless of its attenuated rights. Closing the last TaskGroup handle asynchronously requests stop for every member. Rights control operations available through a handle; they do not change this ownership effect.
 *
 * - Inspector scans require the exact published page capacity for their record type. Cursor zero starts a scan and a returned next_cursor of zero ends it. Pages and complete scans are weakly consistent with concurrent task, object, and handle-table mutation; generation-qualified handle values prevent slot reuse from aliasing an earlier observation.
 *
 * - Memory and CPU inspectors publish immutable point-in-time copies. Their handles grant observation only; they never expose writable accounting storage or allocator and scheduler synchronization to userspace. CPU categories and thread runtime_ns are elapsed monotonic nanoseconds, including a snapshot-time charge for executing Threads. reserved_clock is zero. A multi-CPU snapshot is weakly consistent across CPUs.
 *
 * - File and Directory information reports attribute snapshots plus filesystem, mount, and node identities for diagnostics and correlation. These identities do not grant authority, cannot be resolved back into handles, and do not define a pathname; directory entries, hard links, renames, mount namespaces, and unlinks make pathnames namespace-dependent observations rather than object identity.
 *
 * - Object wait-many borrows every input handle for the complete wait, canonicalizes duplicate object identities, and selects the lowest input index whose requested mask intersects the winning object's committed level snapshot. Source-handle close after resolution does not cancel the wait.
 *
 * - Process terminal detail fields are reason-specific: exit reasons encode the signed status as two's-complement in detail0; fault encodes class in detail0 and code in detail1; task-group stop encodes generation in detail0; unused details are zero.
 *
 * - process_builder_retain requires WRITE | RETAIN_PROCESS on an unsealed ProcessBuilder. Only a TaskFactory carrying RETAIN_PROCESS can produce that builder authority. Registration is external and precedes process start; a running process cannot register itself. The returned ProcessRetirement is a nonduplicable rendezvous-only controller. It may be transferred without creating a buffered owner. Closing its last active handle, explicit release, builder abort, or its monotonic timeout revokes all CrashView derivatives. Ordinary Process observers never extend retention. No kernel callback or application crash handler is installed.
 *
 * - Process stop closes admission and withdraws live handles while outstanding transactions cancel or finish safely. After execution quiesces, retained evidence contains register copies, numeric handle records and bounded resident private pages. It contains no live handle authority, device lease, file protection, filesystem refill source or target address space. Shared writable and clean file-backed pages are excluded; unavailable bytes return NOT_FOUND rather than faulting the target. Independently retained device and DMA owners must still finish their normal physical retirement.
 *
 * - ProcessRetirement READY publishes a bounded scene; RELEASED is sticky and may occur before READY. The timeout starts on the first stop request, including quiescence time. Current limits are 64 armed registrations, four retained scenes and 64 MiB of retained page payload per registering supervisor, with at most 60 seconds per registration. Actual limits may tighten and quota/allocation failure is observable. At most 32 thread records, 256 numeric handle records and 1024 mapping records are retained; truncation is flagged. Bounded metadata is separately resource-domain charged. Controllers should be explicitly released after observation and before joining final process or group retirement.
 *
 * - CrashView grants only read/inspect and may be duplicated or delegated. Release revokes new reads even when views remain open; already admitted bounded reads may finish. Read copies at most 64 KiB and may return a contiguous short prefix before an unavailable page. Indexed thread, mapping and handle queries begin at zero and return next_index zero at the end; absent indices return NOT_FOUND. Thread machine is ELF e_machine; general, vector and fp_control byte arrays encode little-endian u64 words in architecture register order. Mapping object_koid zero means no canonical object identity is exported. Numeric addresses and KOIDs confer no authority.
 *
 * - Object transfer classes constrain generic capability transports. General objects may be retained by buffered or rendezvous transports. Rendezvous-only objects may move or duplicate only by a direct source-to-destination commit which never creates an in-transit owner. Forbidden objects cannot cross a userspace handle table boundary.
 *
 * - AtomicOnOk capability transactions commit handle-table ownership, the rendezvous message, and live output-handle installation together only when both participants return ok. Every non-ok status preserves all input owners and the message. Non-fault failures leave output memory unchanged; fault may partially modify output byte or slot memory, but no handle value written by a failed call is live or installed. Bindings must ignore every output-memory byte after any non-ok status.
 *
 * - A capability-channel receiver advertises peer_receiving only after its byte range, typed capability slots, and destination handle-table capacity are validated, reserved, and fully published on the endpoint's FIFO receiver queue. The signal is level-triggered but may race another sender.
 *
 * - A capability disposition's rights are the sender's offered ceiling; capability_disposition_same_rights offers all source rights. A receive slot's rights are the exact installed grant, so receiver rights must be a subset of the sender offer and the offer a subset of source rights. Sender and receiver expected_kind values are nonzero and must both equal the actual object kind.
 *
 * - A capability receive slot enters with handle and flags zero. On ok, only handle changes. Implementations reserve destination entries, prewrite their future handle values, then perform an infallible handle-table commit. Buffer-too-small changes no slot or byte memory and reports both required counts.
 *
 * - Capability disposition move consumes its source only on ok and requires transfer. Capability disposition duplicate retains its source and requires transfer plus duplicate. Unknown operations, repeated source handles, kind mismatches, and rights violations reject the complete transaction.
 *
 * - Capability-channel try_send returns peer_closed when no peer remains; otherwise it returns would_block only when no receiver is queued. It FIFO-matches the oldest fully published receiver. A size, kind, rights, operation, or copy mismatch rejects that transaction for both participants without scanning later receivers.
 *
 * - Capability-channel timeout, cancellation, and peer close can win only before a receiver is matched. Once matched, sender completion or rejection owns the transaction through commit, including races with the deadline, cancellation, or close; both participants observe the same committed outcome.
 *
 * - Process-builder create borrows its authority handles, retains the target File identity for an attenuated child grant, and captures a kernel-only stable copy of the selected bootstrap loader. Closing or replacing either source handle does not change the captured identities. The target File contents may change until the userspace loader acquires PROTECT; builder creation does not freeze them. Failure before builder publication releases these owners without consuming either input File. Builders are mutable only before seal. Every successful mutator applies completely; every failure leaves the builder unchanged. Seal is irreversible, prepares only the bootstrap process image and releases the temporary loader copy. Start requires a sealed builder, consumes it atomically, and returns a supervisor Process plus a READ/WAIT startup ByteChannel. It commits child publication, not successful application loading. Invocation failure preserves the builder; asynchronous bootstrap failure does not restore it. The SDK waits for its loader result protocol and stops failed children before returning a committed launch error.
 *
 * - A process-builder name is nonempty UTF-8 without embedded NUL bytes. set_data replaces bounded opaque bytes, up to process_startup_data_max_bytes, without kernel interpretation. Arguments, environment, their limits and the loader result protocol are SDK policy. Boot init receives an empty payload.
 *
 * - Process-builder set_name, set_data and set_affinity replace their prior values. Affinity is a nonempty little-endian array of u64 CPU-mask words. Bits above process_affinity_max_cpus and bits which cannot designate an allowed CPU are rejected.
 *
 * - Process-builder add_handle requires a nonzero purpose unique within the builder, an expected nonzero exact object kind, and either exact granted rights or capability_disposition_same_rights. Move consumes the source only when the mutator returns ok; duplicate retains it and additionally requires duplicate. Failure preserves both builder and source.
 *
 * - A VirtualMachineCreationAuthority may derive one resource-domain-bound VirtualMachineCreationLease. The lease is single-use and is consumed only when VirtualMachine creation publishes a PendingVirtualMachine handle successfully.
 *
 * - Virtual-machine info appends resident_memory_bytes after the original 32-byte prefix. UINT64_MAX means unavailable during retirement. It counts currently allocated primary VMO backing, including RAM, explicitly admitted shared pools, and uploaded pages not yet mapped by stage-2; overlapping ranges of the same VMO count once. Dynamic backend alias windows, translation tables, and Native runtime memory are excluded. Shared pages are attributed independently in each guest and are not additive across guests. Inspection samples bounded page chunks and is not a globally atomic memory snapshot.
 *
 * - A PendingVirtualMachine is mutable until seal. It must own at least one explicitly backed, non-overlapping writable region inside the configured IPA envelope and one bootstrap record for boot vCPU 0. Bootstrap entry and stack must lie in admitted backing; sparse envelope gaps remain unbacked and never become anonymous RAM. The configured vcpu_count fixes immutable topology; architecture power-on protocols supply secondary-vCPU runtime entry state, and virtual_machine_open_vcpu exposes their control handles. A guest serial route is optional and exists only when a caller transfers a VirtualSerial handle with assign-device authority before seal. Successful binding consumes the supplied handle and commits a VM-owned reference until VM retirement; a rejected binding leaves the handle unchanged. Guest output is published to a read-only shared VMO consumed by the owning runtime, and VM retirement disconnects the input route without invalidating existing output mappings. Seal is irreversible; install consumes the pending handle only on ok and publishes the installed VirtualMachine and dormant boot VirtualCpu handles together. VirtualCpu start is a separate operation after handle publication. The started VirtualCpu phase means that start committed successfully; it is not an observation that the scheduler currently considers the vCPU runnable or executing. AArch64 reference guests accept 1..8 vCPUs; RISC-V reference guests currently accept one.
 *
 * - VirtualSerial handles may move through capability rendezvous or staged startup; buffered transfer is forbidden. Device assignment consumes a same-process handle. A runtime handoff transfers the serial consumer together with its output VMO and absolute consumed cursor, without closing the last active handle or re-registering the output buffer. register_output borrows a caller-allocated writable VMO of exactly 69632 bytes and registers it once before assignment. An exclusive write lease rejects existing writable mappings, direct accesses, snapshots, and further writers until port retirement; read-only mappings may coexist. Offset 0 is an atomic u64 produced count, offset 8 a saturating dropped-byte count, and offset 4096 begins 65536 atomic byte slots. Registration initializes counters; callers must not access contents during registration. The producer release-publishes bytes; the runtime acquire-loads production and reads a batch. acknowledge_output requires READ and submits the absolute consumed position after reading. Regressing or future positions return invalid_argument without mutation. Publication, acknowledgement, and closure serialize on the port: READABLE means unacknowledged output, WRITABLE means a connected input route has queue space, and PEER_CLOSED means no future output or input. WAIT authorizes object waits and WaitSet subscriptions. Acknowledgement clears READABLE only when caught up; new output reasserts it, without lost wakeups or periodic polling. Full output never overwrites unconsumed slots. The AArch64 SBSA UART retains pending bytes in its TX FIFO and exposes TXFF until runtime acknowledgement frees transport space; other producers discard new bytes and count loss when full. Counters never wrap. Last active handle closure synchronizes with writers and closes publication; registered pages remain pinned through final VM/object retirement. The SDK maps output read-only and acknowledges batches by syscall without copying payload through the syscall. Write remains nonblocking input injection: busy means the queue is full, bad_state means disconnected. Runtime policy owns retention and client transport.
 *
 * - The creating process retains its guest VMO handle, but attaching it to a PendingVirtualMachine acquires exclusive hardware-write ownership and rejects any active Native writable mapping or direct VMO operation. Direct VMO access, snapshots, and writable Native mappings remain closed until VM retirement removes and invalidates every stage-2 mapping and releases the independent backing reference; read-only Native mappings may coexist.
 *
 * - guest_memory_transfer copies 1..4096 bytes within a GuestMemory grant; operation 0 requires READ and copies into the caller, operation 1 requires WRITE and copies from the caller. The grant retains hardware-owned backing independently of any VMO handle or VM lifetime. The original VMO still forbids direct writes, snapshots and writable Native mappings. Payload copies are not snapshots or atomic word operations; a failed write may have partial effects. Native payload operations serialize without blocking on one another (busy if occupied), and dedication to a NativeBlock initiator permanently rejects these operations even through retained grants.
 *
 * - guest_memory_scalar uses the same READ/WRITE authority and dedication exclusion: operation 0 acquire-loads an aligned 1/2/4/8-byte word (value must be zero), operation 1 release-stores a width-fitting word and executes a full memory fence before return. Offset and width must fit the grant. The store fence orders queue publication before a later suppression-flags read on weakly ordered CPUs. No pointer or mapping is exported; unsupported widths and overflow are rejected.
 *
 * - guest_interrupt_create borrows VirtualMachine WRITE and exclusively reserves an AArch64 device interrupt line, excluding built-in and physically assigned interrupts and all existing notification sources. It returns a GuestInterrupt with WRITE, TRANSFER and INSPECT. Creation is allowed while installed or running, serialized with VM stop. guest_interrupt_set_level accepts only 0 or 1 and fails once the VM begins retirement or the source closes. Closing the final active handle deasserts the line; its reservation persists until VM retirement. The source implements no UART, PCI or virtio device policy.
 *
 * - guest_notification_access requires frontend VirtualMachine WRITE and addresses an existing connection by original_base. Operation 0 reads pending interrupt status without acknowledgement, 1 reads and acknowledges that status atomically, and 2 submits a queue-index notification (value &lt; 32). For operations 0 and 1 value must be zero; unknown operations are rejected. It shares the direct MMIO source state and serialization. PCI bus-master disable suppresses notifications; PCI memory-decode disable does not suppress this explicit transport fallback. Closure rejects accesses. No backend identity, control operation, DMA lease or epoch can be changed through this call.
 *
 * - guest_notification_configure requires frontend VirtualMachine WRITE. original_base identifies an existing connection by its creation address; base is a page-aligned 4 KiB window wholly contained in an installed userspace MMIO reservation and, while decoding is enabled, may not overlap another enabled I/O route. Flags 0 select the original virtio-MMIO register layout; bit 0 selects modern virtio-PCI, bit 1 enables memory decoding, bit 2 masks INTx, and bit 3 enables queue notifications (bus master). Unknown flags or PCI flags without bit 0 are rejected. PCI uses a 16-bit queue-index write at BAR+0x50 and a read-to-clear 8-bit ISR at BAR+0x60. Relocation, decode, and IRQ-mask changes serialize with source state and VM retirement. The operation cannot change the admitted backend, IRQs, DMA ownership, or epoch; disabling decode is not proof of backend quiescence.
 *
 * - cpu_inspector_trace_start requires CpuInspector SET_ATTRIBUTES and an initialized trace_config. It starts one global generation, rejects an active/stopping capture, and replaces a stopped capture. Categories are bit flags 1 scheduler, 2 IPC, 4 Native syscall, 8 block I/O, 16 memory faults, 32 IRQ, 64 VM; zero and unknown bits fail. Duration is 1..300000 ms; capacity is 64..4096 retained events per CPU. cpu_mask is four little-endian u64 words; zero selects all online CPUs, and offline bits fail. The kernel closes capture at the deadline through an ordinary callout, independent of the collector lifetime; execution can be later if that worker is delayed. Feature-disabled kernels return NOT_SUPPORTED.
 *
 * - cpu_inspector_trace_stop requires SET_ATTRIBUTES and the exact generation. It closes producer admission before joining short in-flight writers; BUSY requires retry. Repeated successful stop is idempotent until another capture starts. Generation is an observation/control token, not a capability or private capture owner. Any appropriately authorized inspector may control the global session.
 *
 * - cpu_inspector_trace_read requires INSPECT_DETAILS. CPU UINT64_MAX selects trace_info (160 bytes, generation and cursor zero). CPU UINT64_MAX-1 selects trace_thread (72 bytes, exact generation, cursor is scheduler ThreadId); names are live best-effort observations, not authority. Ordinary online CPU indices select trace_batch (2656 bytes, eight trace_record entries) with exact generation and a per-CPU cursor. Returned value0 is the exact written size. next advances across overwritten and returned records; count zero while running is not EOF. incomplete is sticky per generation/CPU and means unquantified producer loss. Stop freezes records and loss metadata until next start. Strings are UTF-8 with NUL padding, truncated to fixed bounds; all reserved/unused bytes are zero. No kernel pointer, raw handle encoding or userspace payload is exported. Event vocabulary is diagnostic and identified by the included build revision, not a stable behavioral ABI.
 */
#ifndef HYPER_NATIVE_H
#define HYPER_NATIVE_H

#include <stddef.h>
#include <stdint.h>

#if defined(__cplusplus)
#define HYPER_ABI_STATIC_ASSERT static_assert
#define HYPER_ABI_ALIGNOF alignof
#else
#define HYPER_ABI_STATIC_ASSERT _Static_assert
#define HYPER_ABI_ALIGNOF _Alignof
#endif

/**
 * Pre-release schema revision; zero does not promise a stable published ABI.
 */
#define HYPER_NATIVE_ABI_REVISION UINT64_C(0)
/**
 * Business argument registers, excluding the receiver and operation selectors.
 */
#define HYPER_NATIVE_INVOCATION_ARGUMENT_REGISTERS UINT32_C(6)
/**
 * Auxiliary result registers, excluding the signed status register.
 */
#define HYPER_NATIVE_INVOCATION_RESULT_REGISTERS UINT32_C(2)

/** Process-local, generation-qualified handle; not a KOID or pointer. */
typedef uint64_t hyper_native_handle_t;
/** Signed invocation status: zero for OK, negative for failure. */
typedef int64_t hyper_native_status_t;

/**
 * Advertised ABI feature bit: `core`.
 */
#define HYPER_NATIVE_FEATURE_CORE (UINT64_C(1) << 0)

/**
 * Invocation status `ok`. Auxiliary results on failure require an explicit method contract.
 */
#define HYPER_NATIVE_STATUS_OK INT64_C(0)
/**
 * Invocation status `invalid_argument`. Auxiliary results on failure require an explicit method contract.
 */
#define HYPER_NATIVE_STATUS_INVALID_ARGUMENT (-INT64_C(1))
/**
 * Invocation status `bad_handle`. Auxiliary results on failure require an explicit method contract.
 */
#define HYPER_NATIVE_STATUS_BAD_HANDLE (-INT64_C(2))
/**
 * Invocation status `access_denied`. Auxiliary results on failure require an explicit method contract.
 */
#define HYPER_NATIVE_STATUS_ACCESS_DENIED (-INT64_C(3))
/**
 * Invocation status `not_supported`. Auxiliary results on failure require an explicit method contract.
 */
#define HYPER_NATIVE_STATUS_NOT_SUPPORTED (-INT64_C(4))
/**
 * Invocation status `no_memory`. Auxiliary results on failure require an explicit method contract.
 */
#define HYPER_NATIVE_STATUS_NO_MEMORY (-INT64_C(5))
/**
 * Invocation status `bad_state`. Auxiliary results on failure require an explicit method contract.
 */
#define HYPER_NATIVE_STATUS_BAD_STATE (-INT64_C(6))
/**
 * Invocation status `fault`. Auxiliary results on failure require an explicit method contract.
 */
#define HYPER_NATIVE_STATUS_FAULT (-INT64_C(7))
/**
 * Invocation status `resource_limit`. Auxiliary results on failure require an explicit method contract.
 */
#define HYPER_NATIVE_STATUS_RESOURCE_LIMIT (-INT64_C(8))
/**
 * Invocation status `busy`. Auxiliary results on failure require an explicit method contract.
 */
#define HYPER_NATIVE_STATUS_BUSY (-INT64_C(9))
/**
 * Invocation status `internal`. Auxiliary results on failure require an explicit method contract.
 */
#define HYPER_NATIVE_STATUS_INTERNAL (-INT64_C(10))
/**
 * Invocation status `timed_out`. Auxiliary results on failure require an explicit method contract.
 */
#define HYPER_NATIVE_STATUS_TIMED_OUT (-INT64_C(11))
/**
 * Invocation status `cancelled`. Auxiliary results on failure require an explicit method contract.
 */
#define HYPER_NATIVE_STATUS_CANCELLED (-INT64_C(12))
/**
 * Invocation status `would_block`. Auxiliary results on failure require an explicit method contract.
 */
#define HYPER_NATIVE_STATUS_WOULD_BLOCK (-INT64_C(13))
/**
 * Invocation status `buffer_too_small`. Auxiliary results on failure require an explicit method contract.
 */
#define HYPER_NATIVE_STATUS_BUFFER_TOO_SMALL (-INT64_C(14))
/**
 * Invocation status `peer_closed`. Auxiliary results on failure require an explicit method contract.
 */
#define HYPER_NATIVE_STATUS_PEER_CLOSED (-INT64_C(15))
/**
 * Invocation status `not_found`. Auxiliary results on failure require an explicit method contract.
 */
#define HYPER_NATIVE_STATUS_NOT_FOUND (-INT64_C(16))
/**
 * Invocation status `already_exists`. Auxiliary results on failure require an explicit method contract.
 */
#define HYPER_NATIVE_STATUS_ALREADY_EXISTS (-INT64_C(17))
/**
 * Invocation status `not_empty`. Auxiliary results on failure require an explicit method contract.
 */
#define HYPER_NATIVE_STATUS_NOT_EMPTY (-INT64_C(18))
/**
 * Invocation status `not_directory`. Auxiliary results on failure require an explicit method contract.
 */
#define HYPER_NATIVE_STATUS_NOT_DIRECTORY (-INT64_C(19))
/**
 * Invocation status `is_directory`. Auxiliary results on failure require an explicit method contract.
 */
#define HYPER_NATIVE_STATUS_IS_DIRECTORY (-INT64_C(20))
/**
 * Invocation status `symlink_loop`. Auxiliary results on failure require an explicit method contract.
 */
#define HYPER_NATIVE_STATUS_SYMLINK_LOOP (-INT64_C(21))
/**
 * Invocation status `cross_device`. Auxiliary results on failure require an explicit method contract.
 */
#define HYPER_NATIVE_STATUS_CROSS_DEVICE (-INT64_C(22))
/**
 * Invocation status `io_error`. Auxiliary results on failure require an explicit method contract.
 */
#define HYPER_NATIVE_STATUS_IO_ERROR (-INT64_C(23))
/**
 * Invocation status `no_space`. Auxiliary results on failure require an explicit method contract.
 */
#define HYPER_NATIVE_STATUS_NO_SPACE (-INT64_C(24))
/**
 * Invocation status `read_only`. Auxiliary results on failure require an explicit method contract.
 */
#define HYPER_NATIVE_STATUS_READ_ONLY (-INT64_C(25))
/**
 * Invocation status `revoked`. Auxiliary results on failure require an explicit method contract.
 */
#define HYPER_NATIVE_STATUS_REVOKED (-INT64_C(26))

/**
 * Object-kind tag `none` used with an object-local operation number.
 */
#define HYPER_NATIVE_OBJECT_NONE UINT32_C(0)
/**
 * Object-kind tag `event` used with an object-local operation number.
 */
#define HYPER_NATIVE_OBJECT_EVENT UINT32_C(1)
/**
 * Object-kind tag `byte_channel` used with an object-local operation number.
 */
#define HYPER_NATIVE_OBJECT_BYTE_CHANNEL UINT32_C(2)
/**
 * Object-kind tag `thread` used with an object-local operation number.
 */
#define HYPER_NATIVE_OBJECT_THREAD UINT32_C(3)
/**
 * Object-kind tag `process` used with an object-local operation number.
 */
#define HYPER_NATIVE_OBJECT_PROCESS UINT32_C(4)
/**
 * Object-kind tag `task_group` used with an object-local operation number.
 */
#define HYPER_NATIVE_OBJECT_TASK_GROUP UINT32_C(5)
/**
 * Object-kind tag `resource_domain` used with an object-local operation number.
 */
#define HYPER_NATIVE_OBJECT_RESOURCE_DOMAIN UINT32_C(6)
/**
 * Object-kind tag `task_factory` used with an object-local operation number.
 */
#define HYPER_NATIVE_OBJECT_TASK_FACTORY UINT32_C(7)
/**
 * Object-kind tag `executable_authority` used with an object-local operation number.
 */
#define HYPER_NATIVE_OBJECT_EXECUTABLE_AUTHORITY UINT32_C(8)
/**
 * Object-kind tag `vmo` used with an object-local operation number.
 */
#define HYPER_NATIVE_OBJECT_VMO UINT32_C(9)
/**
 * Object-kind tag `vmar` used with an object-local operation number.
 */
#define HYPER_NATIVE_OBJECT_VMAR UINT32_C(10)
/**
 * Object-kind tag `console` used with an object-local operation number.
 */
#define HYPER_NATIVE_OBJECT_CONSOLE UINT32_C(11)
/**
 * Object-kind tag `directory` used with an object-local operation number.
 */
#define HYPER_NATIVE_OBJECT_DIRECTORY UINT32_C(12)
/**
 * Object-kind tag `file` used with an object-local operation number.
 */
#define HYPER_NATIVE_OBJECT_FILE UINT32_C(13)
/**
 * Object-kind tag `capability_channel` used with an object-local operation number.
 */
#define HYPER_NATIVE_OBJECT_CAPABILITY_CHANNEL UINT32_C(14)
/**
 * Object-kind tag `process_builder` used with an object-local operation number.
 */
#define HYPER_NATIVE_OBJECT_PROCESS_BUILDER UINT32_C(15)
/**
 * Object-kind tag `task_inspector` used with an object-local operation number.
 */
#define HYPER_NATIVE_OBJECT_TASK_INSPECTOR UINT32_C(16)
/**
 * Object-kind tag `object_inspector` used with an object-local operation number.
 */
#define HYPER_NATIVE_OBJECT_OBJECT_INSPECTOR UINT32_C(17)
/**
 * Object-kind tag `memory_inspector` used with an object-local operation number.
 */
#define HYPER_NATIVE_OBJECT_MEMORY_INSPECTOR UINT32_C(18)
/**
 * Object-kind tag `cpu_inspector` used with an object-local operation number.
 */
#define HYPER_NATIVE_OBJECT_CPU_INSPECTOR UINT32_C(19)
/**
 * Object-kind tag `virtual_machine_creation_authority` used with an object-local operation number.
 */
#define HYPER_NATIVE_OBJECT_VIRTUAL_MACHINE_CREATION_AUTHORITY UINT32_C(20)
/**
 * Object-kind tag `virtual_machine_creation_lease` used with an object-local operation number.
 */
#define HYPER_NATIVE_OBJECT_VIRTUAL_MACHINE_CREATION_LEASE UINT32_C(21)
/**
 * Object-kind tag `pending_virtual_machine` used with an object-local operation number.
 */
#define HYPER_NATIVE_OBJECT_PENDING_VIRTUAL_MACHINE UINT32_C(22)
/**
 * Object-kind tag `virtual_machine` used with an object-local operation number.
 */
#define HYPER_NATIVE_OBJECT_VIRTUAL_MACHINE UINT32_C(23)
/**
 * Object-kind tag `virtual_cpu` used with an object-local operation number.
 */
#define HYPER_NATIVE_OBJECT_VIRTUAL_CPU UINT32_C(24)
/**
 * Object-kind tag `virtual_serial` used with an object-local operation number.
 */
#define HYPER_NATIVE_OBJECT_VIRTUAL_SERIAL UINT32_C(25)
/**
 * Object-kind tag `wait_set` used with an object-local operation number.
 */
#define HYPER_NATIVE_OBJECT_WAIT_SET UINT32_C(26)
/**
 * Object-kind tag `guest_memory` used with an object-local operation number.
 */
#define HYPER_NATIVE_OBJECT_GUEST_MEMORY UINT32_C(27)
/**
 * Object-kind tag `device_assignment_authority` used with an object-local operation number.
 */
#define HYPER_NATIVE_OBJECT_DEVICE_ASSIGNMENT_AUTHORITY UINT32_C(28)
/**
 * Object-kind tag `physical_device` used with an object-local operation number.
 */
#define HYPER_NATIVE_OBJECT_PHYSICAL_DEVICE UINT32_C(29)
/**
 * Object-kind tag `guest_mailbox` used with an object-local operation number.
 */
#define HYPER_NATIVE_OBJECT_GUEST_MAILBOX UINT32_C(30)
/**
 * Object-kind tag `guest_notification` used with an object-local operation number.
 */
#define HYPER_NATIVE_OBJECT_GUEST_NOTIFICATION UINT32_C(31)
/**
 * Object-kind tag `native_block` used with an object-local operation number.
 */
#define HYPER_NATIVE_OBJECT_NATIVE_BLOCK UINT32_C(32)
/**
 * Object-kind tag `guest_mapping` used with an object-local operation number.
 */
#define HYPER_NATIVE_OBJECT_GUEST_MAPPING UINT32_C(33)
/**
 * Object-kind tag `file_protection` used with an object-local operation number.
 */
#define HYPER_NATIVE_OBJECT_FILE_PROTECTION UINT32_C(34)
/**
 * Object-kind tag `directory_stream` used with an object-local operation number.
 */
#define HYPER_NATIVE_OBJECT_DIRECTORY_STREAM UINT32_C(35)
/**
 * Object-kind tag `root_transition` used with an object-local operation number.
 */
#define HYPER_NATIVE_OBJECT_ROOT_TRANSITION UINT32_C(36)
/**
 * Object-kind tag `guest_interrupt` used with an object-local operation number.
 */
#define HYPER_NATIVE_OBJECT_GUEST_INTERRUPT UINT32_C(37)
/**
 * Object-kind tag `process_retirement` used with an object-local operation number.
 */
#define HYPER_NATIVE_OBJECT_PROCESS_RETIREMENT UINT32_C(38)
/**
 * Object-kind tag `crash_view` used with an object-local operation number.
 */
#define HYPER_NATIVE_OBJECT_CRASH_VIEW UINT32_C(39)
/**
 * Object-kind tag `backend_memory_lease` used with an object-local operation number.
 */
#define HYPER_NATIVE_OBJECT_BACKEND_MEMORY_LEASE UINT32_C(40)
/**
 * Object-kind tag `execution_context` used with an object-local operation number.
 */
#define HYPER_NATIVE_OBJECT_EXECUTION_CONTEXT UINT32_C(41)
/**
 * Object-kind tag `object_factory` used with an object-local operation number.
 */
#define HYPER_NATIVE_OBJECT_OBJECT_FACTORY UINT32_C(42)
/**
 * Object-kind tag `system_info` used with an object-local operation number.
 */
#define HYPER_NATIVE_OBJECT_SYSTEM_INFO UINT32_C(43)

/**
 * Handle transport is forbidden for this object kind.
 */
#define HYPER_NATIVE_TRANSFER_CLASS_FORBIDDEN UINT32_C(0)
/**
 * Buffered capability transfer and rendezvous are permitted with transfer authority.
 */
#define HYPER_NATIVE_TRANSFER_CLASS_GENERAL UINT32_C(1)
/**
 * Transfer requires rendezvous or staged startup; buffered transfer is forbidden.
 */
#define HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY UINT32_C(2)

/**
 * Return the permitted transport class for an object kind.
 * Unknown kinds return FORBIDDEN. This classification does not grant TRANSFER authority.
 */
static inline uint32_t hyper_native_object_transfer_class(uint32_t object_kind) {
    switch (object_kind) {
        case HYPER_NATIVE_OBJECT_NONE: return HYPER_NATIVE_TRANSFER_CLASS_FORBIDDEN;
        case HYPER_NATIVE_OBJECT_EVENT: return HYPER_NATIVE_TRANSFER_CLASS_GENERAL;
        case HYPER_NATIVE_OBJECT_BYTE_CHANNEL: return HYPER_NATIVE_TRANSFER_CLASS_GENERAL;
        case HYPER_NATIVE_OBJECT_THREAD: return HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY;
        case HYPER_NATIVE_OBJECT_PROCESS: return HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY;
        case HYPER_NATIVE_OBJECT_TASK_GROUP: return HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY;
        case HYPER_NATIVE_OBJECT_RESOURCE_DOMAIN: return HYPER_NATIVE_TRANSFER_CLASS_GENERAL;
        case HYPER_NATIVE_OBJECT_TASK_FACTORY: return HYPER_NATIVE_TRANSFER_CLASS_GENERAL;
        case HYPER_NATIVE_OBJECT_EXECUTABLE_AUTHORITY: return HYPER_NATIVE_TRANSFER_CLASS_GENERAL;
        case HYPER_NATIVE_OBJECT_VMO: return HYPER_NATIVE_TRANSFER_CLASS_GENERAL;
        case HYPER_NATIVE_OBJECT_VMAR: return HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY;
        case HYPER_NATIVE_OBJECT_CONSOLE: return HYPER_NATIVE_TRANSFER_CLASS_GENERAL;
        case HYPER_NATIVE_OBJECT_DIRECTORY: return HYPER_NATIVE_TRANSFER_CLASS_GENERAL;
        case HYPER_NATIVE_OBJECT_FILE: return HYPER_NATIVE_TRANSFER_CLASS_GENERAL;
        case HYPER_NATIVE_OBJECT_CAPABILITY_CHANNEL: return HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY;
        case HYPER_NATIVE_OBJECT_PROCESS_BUILDER: return HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY;
        case HYPER_NATIVE_OBJECT_TASK_INSPECTOR: return HYPER_NATIVE_TRANSFER_CLASS_GENERAL;
        case HYPER_NATIVE_OBJECT_OBJECT_INSPECTOR: return HYPER_NATIVE_TRANSFER_CLASS_GENERAL;
        case HYPER_NATIVE_OBJECT_MEMORY_INSPECTOR: return HYPER_NATIVE_TRANSFER_CLASS_GENERAL;
        case HYPER_NATIVE_OBJECT_CPU_INSPECTOR: return HYPER_NATIVE_TRANSFER_CLASS_GENERAL;
        case HYPER_NATIVE_OBJECT_VIRTUAL_MACHINE_CREATION_AUTHORITY: return HYPER_NATIVE_TRANSFER_CLASS_GENERAL;
        case HYPER_NATIVE_OBJECT_VIRTUAL_MACHINE_CREATION_LEASE: return HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY;
        case HYPER_NATIVE_OBJECT_PENDING_VIRTUAL_MACHINE: return HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY;
        case HYPER_NATIVE_OBJECT_VIRTUAL_MACHINE: return HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY;
        case HYPER_NATIVE_OBJECT_VIRTUAL_CPU: return HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY;
        case HYPER_NATIVE_OBJECT_VIRTUAL_SERIAL: return HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY;
        case HYPER_NATIVE_OBJECT_WAIT_SET: return HYPER_NATIVE_TRANSFER_CLASS_FORBIDDEN;
        case HYPER_NATIVE_OBJECT_GUEST_MEMORY: return HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY;
        case HYPER_NATIVE_OBJECT_DEVICE_ASSIGNMENT_AUTHORITY: return HYPER_NATIVE_TRANSFER_CLASS_GENERAL;
        case HYPER_NATIVE_OBJECT_PHYSICAL_DEVICE: return HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY;
        case HYPER_NATIVE_OBJECT_GUEST_MAILBOX: return HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY;
        case HYPER_NATIVE_OBJECT_GUEST_NOTIFICATION: return HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY;
        case HYPER_NATIVE_OBJECT_NATIVE_BLOCK: return HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY;
        case HYPER_NATIVE_OBJECT_GUEST_MAPPING: return HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY;
        case HYPER_NATIVE_OBJECT_FILE_PROTECTION: return HYPER_NATIVE_TRANSFER_CLASS_FORBIDDEN;
        case HYPER_NATIVE_OBJECT_DIRECTORY_STREAM: return HYPER_NATIVE_TRANSFER_CLASS_GENERAL;
        case HYPER_NATIVE_OBJECT_ROOT_TRANSITION: return HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY;
        case HYPER_NATIVE_OBJECT_GUEST_INTERRUPT: return HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY;
        case HYPER_NATIVE_OBJECT_PROCESS_RETIREMENT: return HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY;
        case HYPER_NATIVE_OBJECT_CRASH_VIEW: return HYPER_NATIVE_TRANSFER_CLASS_GENERAL;
        case HYPER_NATIVE_OBJECT_BACKEND_MEMORY_LEASE: return HYPER_NATIVE_TRANSFER_CLASS_FORBIDDEN;
        case HYPER_NATIVE_OBJECT_EXECUTION_CONTEXT: return HYPER_NATIVE_TRANSFER_CLASS_FORBIDDEN;
        case HYPER_NATIVE_OBJECT_OBJECT_FACTORY: return HYPER_NATIVE_TRANSFER_CLASS_FORBIDDEN;
        case HYPER_NATIVE_OBJECT_SYSTEM_INFO: return HYPER_NATIVE_TRANSFER_CLASS_GENERAL;
        default: return HYPER_NATIVE_TRANSFER_CLASS_FORBIDDEN;
    }
}

/**
 * Capability right; the receiving object's method determines the required rights: `duplicate`.
 */
#define HYPER_NATIVE_RIGHT_DUPLICATE (UINT64_C(1) << 0)
/**
 * Capability right; the receiving object's method determines the required rights: `transfer`.
 */
#define HYPER_NATIVE_RIGHT_TRANSFER (UINT64_C(1) << 1)
/**
 * Capability right; the receiving object's method determines the required rights: `wait`.
 */
#define HYPER_NATIVE_RIGHT_WAIT (UINT64_C(1) << 2)
/**
 * Capability right; the receiving object's method determines the required rights: `inspect`.
 */
#define HYPER_NATIVE_RIGHT_INSPECT (UINT64_C(1) << 3)
/**
 * Capability right; the receiving object's method determines the required rights: `read`.
 */
#define HYPER_NATIVE_RIGHT_READ (UINT64_C(1) << 4)
/**
 * Capability right; the receiving object's method determines the required rights: `write`.
 */
#define HYPER_NATIVE_RIGHT_WRITE (UINT64_C(1) << 5)
/**
 * Capability right; the receiving object's method determines the required rights: `map`.
 */
#define HYPER_NATIVE_RIGHT_MAP (UINT64_C(1) << 6)
/**
 * Capability right; the receiving object's method determines the required rights: `execute`.
 */
#define HYPER_NATIVE_RIGHT_EXECUTE (UINT64_C(1) << 7)
/**
 * Capability right; the receiving object's method determines the required rights: `resize`.
 */
#define HYPER_NATIVE_RIGHT_RESIZE (UINT64_C(1) << 8)
/**
 * Capability right; the receiving object's method determines the required rights: `pin`.
 */
#define HYPER_NATIVE_RIGHT_PIN (UINT64_C(1) << 9)
/**
 * Capability right; the receiving object's method determines the required rights: `start`.
 */
#define HYPER_NATIVE_RIGHT_START (UINT64_C(1) << 10)
/**
 * Capability right; the receiving object's method determines the required rights: `request_stop`.
 */
#define HYPER_NATIVE_RIGHT_REQUEST_STOP (UINT64_C(1) << 11)
/**
 * Capability right; the receiving object's method determines the required rights: `run_vcpu`.
 */
#define HYPER_NATIVE_RIGHT_RUN_VCPU (UINT64_C(1) << 12)
/**
 * Capability right; the receiving object's method determines the required rights: `inject_interrupt`.
 */
#define HYPER_NATIVE_RIGHT_INJECT_INTERRUPT (UINT64_C(1) << 13)
/**
 * Capability right; the receiving object's method determines the required rights: `grant_memory`.
 */
#define HYPER_NATIVE_RIGHT_GRANT_MEMORY (UINT64_C(1) << 14)
/**
 * Capability right; the receiving object's method determines the required rights: `assign_device`.
 */
#define HYPER_NATIVE_RIGHT_ASSIGN_DEVICE (UINT64_C(1) << 15)
/**
 * Capability right; the receiving object's method determines the required rights: `map_dma`.
 */
#define HYPER_NATIVE_RIGHT_MAP_DMA (UINT64_C(1) << 16)
/**
 * Capability right; the receiving object's method determines the required rights: `ack_interrupt`.
 */
#define HYPER_NATIVE_RIGHT_ACK_INTERRUPT (UINT64_C(1) << 17)
/**
 * Capability right; the receiving object's method determines the required rights: `revoke`.
 */
#define HYPER_NATIVE_RIGHT_REVOKE (UINT64_C(1) << 18)
/**
 * Capability right; the receiving object's method determines the required rights: `signal`.
 */
#define HYPER_NATIVE_RIGHT_SIGNAL (UINT64_C(1) << 19)
/**
 * Capability right; the receiving object's method determines the required rights: `create_process`.
 */
#define HYPER_NATIVE_RIGHT_CREATE_PROCESS (UINT64_C(1) << 20)
/**
 * Capability right; the receiving object's method determines the required rights: `create_thread`.
 */
#define HYPER_NATIVE_RIGHT_CREATE_THREAD (UINT64_C(1) << 21)
/**
 * Capability right; the receiving object's method determines the required rights: `create_task_group`.
 */
#define HYPER_NATIVE_RIGHT_CREATE_TASK_GROUP (UINT64_C(1) << 22)
/**
 * Capability right; the receiving object's method determines the required rights: `create_resource_domain`.
 */
#define HYPER_NATIVE_RIGHT_CREATE_RESOURCE_DOMAIN (UINT64_C(1) << 23)
/**
 * Capability right; the receiving object's method determines the required rights: `set_limits`.
 */
#define HYPER_NATIVE_RIGHT_SET_LIMITS (UINT64_C(1) << 24)
/**
 * Capability right; the receiving object's method determines the required rights: `create_executable`.
 */
#define HYPER_NATIVE_RIGHT_CREATE_EXECUTABLE (UINT64_C(1) << 25)
/**
 * Capability right; the receiving object's method determines the required rights: `task_group_attach_process`.
 */
#define HYPER_NATIVE_RIGHT_TASK_GROUP_ATTACH_PROCESS (UINT64_C(1) << 26)
/**
 * Capability right; the receiving object's method determines the required rights: `resource_domain_sponsor`.
 */
#define HYPER_NATIVE_RIGHT_RESOURCE_DOMAIN_SPONSOR (UINT64_C(1) << 27)
/**
 * Capability right; the receiving object's method determines the required rights: `derive`.
 */
#define HYPER_NATIVE_RIGHT_DERIVE (UINT64_C(1) << 28)
/**
 * Capability right; the receiving object's method determines the required rights: `create_virtual_machine`.
 */
#define HYPER_NATIVE_RIGHT_CREATE_VIRTUAL_MACHINE (UINT64_C(1) << 29)
/**
 * Capability right; the receiving object's method determines the required rights: `bind_wait`.
 */
#define HYPER_NATIVE_RIGHT_BIND_WAIT (UINT64_C(1) << 30)
/**
 * Capability right; the receiving object's method determines the required rights: `set_attributes`.
 */
#define HYPER_NATIVE_RIGHT_SET_ATTRIBUTES (UINT64_C(1) << 31)
/**
 * Capability right; the receiving object's method determines the required rights: `lock_file`.
 */
#define HYPER_NATIVE_RIGHT_LOCK_FILE (UINT64_C(1) << 32)
/**
 * Capability right; the receiving object's method determines the required rights: `inspect_details`.
 */
#define HYPER_NATIVE_RIGHT_INSPECT_DETAILS (UINT64_C(1) << 33)
/**
 * Capability right; the receiving object's method determines the required rights: `manage_namespace`.
 */
#define HYPER_NATIVE_RIGHT_MANAGE_NAMESPACE (UINT64_C(1) << 34)
/**
 * Capability right; the receiving object's method determines the required rights: `protect`.
 */
#define HYPER_NATIVE_RIGHT_PROTECT (UINT64_C(1) << 35)
/**
 * Capability right; the receiving object's method determines the required rights: `lock_memory`.
 */
#define HYPER_NATIVE_RIGHT_LOCK_MEMORY (UINT64_C(1) << 36)
/**
 * Capability right; the receiving object's method determines the required rights: `retain_process`.
 */
#define HYPER_NATIVE_RIGHT_RETAIN_PROCESS (UINT64_C(1) << 37)
/**
 * Capability right; the receiving object's method determines the required rights: `retire`.
 */
#define HYPER_NATIVE_RIGHT_RETIRE (UINT64_C(1) << 38)
/**
 * Capability right; the receiving object's method determines the required rights: `create_object`.
 */
#define HYPER_NATIVE_RIGHT_CREATE_OBJECT (UINT64_C(1) << 39)
/**
 * Capability right; the receiving object's method determines the required rights: `sched_lower`.
 */
#define HYPER_NATIVE_RIGHT_SCHED_LOWER (UINT64_C(1) << 40)
/**
 * Capability right; the receiving object's method determines the required rights: `sched_priority`.
 */
#define HYPER_NATIVE_RIGHT_SCHED_PRIORITY (UINT64_C(1) << 41)
/**
 * Capability right; the receiving object's method determines the required rights: `sched_class`.
 */
#define HYPER_NATIVE_RIGHT_SCHED_CLASS (UINT64_C(1) << 42)
/**
 * Capability right; the receiving object's method determines the required rights: `sched_budget`.
 */
#define HYPER_NATIVE_RIGHT_SCHED_BUDGET (UINT64_C(1) << 43)

/**
 * All declared right bits; this mask does not grant authority to a handle.
 */
#define HYPER_NATIVE_RIGHTS_MASK UINT64_C(0xfffffffffff)

/**
 * Wait signal `ready` for `process_retirement` objects; bit positions are object-local.
 */
#define HYPER_NATIVE_SIGNAL_PROCESS_RETIREMENT_READY (UINT64_C(1) << 0)
/**
 * Wait signal `released` for `process_retirement` objects; bit positions are object-local.
 */
#define HYPER_NATIVE_SIGNAL_PROCESS_RETIREMENT_RELEASED (UINT64_C(1) << 1)
/**
 * Wait signal `readable` for `physical_device` objects; bit positions are object-local.
 */
#define HYPER_NATIVE_SIGNAL_PHYSICAL_DEVICE_READABLE (UINT64_C(1) << 0)
/**
 * Wait signal `dma_fault` for `physical_device` objects; bit positions are object-local.
 */
#define HYPER_NATIVE_SIGNAL_PHYSICAL_DEVICE_DMA_FAULT (UINT64_C(1) << 1)
/**
 * Wait signal `peer_closed` for `native_block` objects; bit positions are object-local.
 */
#define HYPER_NATIVE_SIGNAL_NATIVE_BLOCK_PEER_CLOSED (UINT64_C(1) << 0)
/**
 * Wait signal `readable` for `virtual_serial` objects; bit positions are object-local.
 */
#define HYPER_NATIVE_SIGNAL_VIRTUAL_SERIAL_READABLE (UINT64_C(1) << 0)
/**
 * Wait signal `writable` for `virtual_serial` objects; bit positions are object-local.
 */
#define HYPER_NATIVE_SIGNAL_VIRTUAL_SERIAL_WRITABLE (UINT64_C(1) << 1)
/**
 * Wait signal `peer_closed` for `virtual_serial` objects; bit positions are object-local.
 */
#define HYPER_NATIVE_SIGNAL_VIRTUAL_SERIAL_PEER_CLOSED (UINT64_C(1) << 2)
/**
 * Wait signal `readable` for `wait_set` objects; bit positions are object-local.
 */
#define HYPER_NATIVE_SIGNAL_WAIT_SET_READABLE (UINT64_C(1) << 0)
/**
 * Wait signal `signaled` for `event` objects; bit positions are object-local.
 */
#define HYPER_NATIVE_SIGNAL_EVENT_SIGNALED (UINT64_C(1) << 0)
/**
 * Wait signal `readable` for `byte_channel` objects; bit positions are object-local.
 */
#define HYPER_NATIVE_SIGNAL_BYTE_CHANNEL_READABLE (UINT64_C(1) << 0)
/**
 * Wait signal `writable` for `byte_channel` objects; bit positions are object-local.
 */
#define HYPER_NATIVE_SIGNAL_BYTE_CHANNEL_WRITABLE (UINT64_C(1) << 1)
/**
 * Wait signal `peer_closed` for `byte_channel` objects; bit positions are object-local.
 */
#define HYPER_NATIVE_SIGNAL_BYTE_CHANNEL_PEER_CLOSED (UINT64_C(1) << 2)
/**
 * Wait signal `peer_receiving` for `capability_channel` objects; bit positions are object-local.
 */
#define HYPER_NATIVE_SIGNAL_CAPABILITY_CHANNEL_PEER_RECEIVING (UINT64_C(1) << 0)
/**
 * Wait signal `peer_closed` for `capability_channel` objects; bit positions are object-local.
 */
#define HYPER_NATIVE_SIGNAL_CAPABILITY_CHANNEL_PEER_CLOSED (UINT64_C(1) << 1)
/**
 * Wait signal `terminated` for `thread` objects; bit positions are object-local.
 */
#define HYPER_NATIVE_SIGNAL_THREAD_TERMINATED (UINT64_C(1) << 0)
/**
 * Wait signal `terminated` for `task_group` objects; bit positions are object-local.
 */
#define HYPER_NATIVE_SIGNAL_TASK_GROUP_TERMINATED (UINT64_C(1) << 0)
/**
 * Wait signal `terminated` for `process` objects; bit positions are object-local.
 */
#define HYPER_NATIVE_SIGNAL_PROCESS_TERMINATED (UINT64_C(1) << 0)
/**
 * Wait signal `readable` for `console` objects; bit positions are object-local.
 */
#define HYPER_NATIVE_SIGNAL_CONSOLE_READABLE (UINT64_C(1) << 0)
/**
 * Wait signal `writable` for `console` objects; bit positions are object-local.
 */
#define HYPER_NATIVE_SIGNAL_CONSOLE_WRITABLE (UINT64_C(1) << 1)
/**
 * Wait signal `terminated` for `virtual_machine` objects; bit positions are object-local.
 */
#define HYPER_NATIVE_SIGNAL_VIRTUAL_MACHINE_TERMINATED (UINT64_C(1) << 0)
/**
 * Wait signal `power_request` for `virtual_machine` objects; bit positions are object-local.
 */
#define HYPER_NATIVE_SIGNAL_VIRTUAL_MACHINE_POWER_REQUEST (UINT64_C(1) << 1)
/**
 * Wait signal `vcpu_terminated` for `virtual_machine` objects; bit positions are object-local.
 */
#define HYPER_NATIVE_SIGNAL_VIRTUAL_MACHINE_VCPU_TERMINATED (UINT64_C(1) << 2)
/**
 * Wait signal `terminated` for `virtual_cpu` objects; bit positions are object-local.
 */
#define HYPER_NATIVE_SIGNAL_VIRTUAL_CPU_TERMINATED (UINT64_C(1) << 0)
/**
 * Wait signal `mmio_request` for `virtual_cpu` objects; bit positions are object-local.
 */
#define HYPER_NATIVE_SIGNAL_VIRTUAL_CPU_MMIO_REQUEST (UINT64_C(1) << 1)
/**
 * Wait signal `peer_closed` for `guest_notification` objects; bit positions are object-local.
 */
#define HYPER_NATIVE_SIGNAL_GUEST_NOTIFICATION_PEER_CLOSED (UINT64_C(1) << 0)
/**
 * Wait signal `readable` for `guest_mailbox` objects; bit positions are object-local.
 */
#define HYPER_NATIVE_SIGNAL_GUEST_MAILBOX_READABLE (UINT64_C(1) << 0)
/**
 * Wait signal `writable` for `guest_mailbox` objects; bit positions are object-local.
 */
#define HYPER_NATIVE_SIGNAL_GUEST_MAILBOX_WRITABLE (UINT64_C(1) << 1)
/**
 * Wait signal `peer_closed` for `guest_mailbox` objects; bit positions are object-local.
 */
#define HYPER_NATIVE_SIGNAL_GUEST_MAILBOX_PEER_CLOSED (UINT64_C(1) << 2)

/**
 * @defgroup hyper_native_handle_encoding handle_encoding (constants)
 * @ingroup hyper_native
 * Handle value classes; the remaining bits are opaque process-local data.
 * @{
 */
/**
 * `handle_invalid` in the `handle_encoding` constants domain.
 *
 * Handle value classes; the remaining bits are opaque process-local data.
 * Numeric parameter or sentinel, not an enum discriminant.
 *
 * No handle is present. This value never identifies a live handle.
 */
#define HYPER_NATIVE_HANDLE_INVALID UINT64_C(0)
/**
 * `handle_prefix_mask` in the `handle_encoding` constants domain.
 *
 * Handle value classes; the remaining bits are opaque process-local data.
 * Numeric parameter or sentinel, not an enum discriminant.
 *
 * Selects the most significant byte of a wire handle.
 */
#define HYPER_NATIVE_HANDLE_PREFIX_MASK UINT64_C(18374686479671623680)
/**
 * `handle_prefix` in the `handle_encoding` constants domain.
 *
 * Handle value classes; the remaining bits are opaque process-local data.
 * Numeric parameter or sentinel, not an enum discriminant.
 *
 * Live handles have prefix 0x0F; all other prefixes are reserved.
 */
#define HYPER_NATIVE_HANDLE_PREFIX UINT64_C(1080863910568919040)
/** @} */

/**
 * @defgroup hyper_native_core_limits core_limits (constants)
 * @ingroup hyper_native
 * Page geometry, record bounds, identity encoding and wait limits.
 * @{
 */
/**
 * `page_size` in the `core_limits` constants domain.
 *
 * Page geometry, record bounds, identity encoding and wait limits.
 * Numeric parameter or sentinel, not an enum discriminant.
 *
 * Native memory-management granule in bytes.
 */
#define HYPER_NATIVE_PAGE_SIZE UINT64_C(4096)
/**
 * `extensible_record_max_bytes` in the `core_limits` constants domain.
 *
 * Page geometry, record bounds, identity encoding and wait limits.
 * Numeric parameter or sentinel, not an enum discriminant.
 *
 * Upper bound on a caller-supplied extensible record, including its tail.
 */
#define HYPER_NATIVE_EXTENSIBLE_RECORD_MAX_BYTES UINT64_C(4096)
/**
 * `koid_slot_mask` in the `core_limits` constants domain.
 *
 * Page geometry, record bounds, identity encoding and wait limits.
 * Numeric parameter or sentinel, not an enum discriminant.
 *
 * Low slot bits of a KOID; the generation is required for identity.
 */
#define HYPER_NATIVE_KOID_SLOT_MASK UINT64_C(4294967295)
/**
 * `deadline_infinite` in the `core_limits` constants domain.
 *
 * Page geometry, record bounds, identity encoding and wait limits.
 * Numeric parameter or sentinel, not an enum discriminant.
 *
 * Wait without a deadline; finite deadlines use absolute monotonic nanoseconds.
 */
#define HYPER_NATIVE_DEADLINE_INFINITE UINT64_C(18446744073709551615)
/**
 * `object_wait_many_max_items` in the `core_limits` constants domain.
 *
 * Page geometry, record bounds, identity encoding and wait limits.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_OBJECT_WAIT_MANY_MAX_ITEMS UINT64_C(64)
/** @} */

/**
 * @defgroup hyper_native_system_config system_config (enum)
 * @ingroup hyper_native
 * Selectors for system configuration queries.
 * @{
 */
/**
 * `system_config_page_size` in the `system_config` enum domain.
 *
 * Selectors for system configuration queries.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_SYSTEM_CONFIG_PAGE_SIZE UINT64_C(1)
/**
 * `system_config_application_address_limit` in the `system_config` enum domain.
 *
 * Selectors for system configuration queries.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_SYSTEM_CONFIG_APPLICATION_ADDRESS_LIMIT UINT64_C(2)
/** @} */

/**
 * @defgroup hyper_native_startup_limits startup_limits (constants)
 * @ingroup hyper_native
 * Executable identifiers and kernel-to-loader transport bounds.
 * @{
 */
/**
 * `elf_osabi` in the `startup_limits` constants domain.
 *
 * Executable identifiers and kernel-to-loader transport bounds.
 * Numeric parameter or sentinel, not an enum discriminant.
 *
 * `HypeR`'s ELF OSABI byte; this is not a Linux or generic System V image.
 */
#define HYPER_NATIVE_ELF_OSABI UINT64_C(63)
/**
 * `elf_abi_version` in the `startup_limits` constants domain.
 *
 * Executable identifiers and kernel-to-loader transport bounds.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_ELF_ABI_VERSION UINT64_C(0)
/**
 * `process_startup_data_max_bytes` in the `startup_limits` constants domain.
 *
 * Executable identifiers and kernel-to-loader transport bounds.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_PROCESS_STARTUP_DATA_MAX_BYTES UINT64_C(16384)
/**
 * `startup_max_handles` in the `startup_limits` constants domain.
 *
 * Executable identifiers and kernel-to-loader transport bounds.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_STARTUP_MAX_HANDLES UINT64_C(256)
/**
 * `boot_command_line_max_bytes` in the `startup_limits` constants domain.
 *
 * Executable identifiers and kernel-to-loader transport bounds.
 * Numeric parameter or sentinel, not an enum discriminant.
 *
 * Maximum UTF-8 boot-argument payload, excluding its length prefix.
 */
#define HYPER_NATIVE_BOOT_COMMAND_LINE_MAX_BYTES UINT64_C(512)
/** @} */

/**
 * @defgroup hyper_native_loader_startup loader_startup (enum)
 * @ingroup hyper_native
 * Kernel-to-loader startup message tags; payload semantics belong to userspace.
 * @{
 */
/**
 * `loader_startup_reply` in the `loader_startup` enum domain.
 *
 * Kernel-to-loader startup message tags; payload semantics belong to userspace.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_LOADER_STARTUP_REPLY UINT64_C(1)
/** @} */

/**
 * @defgroup hyper_native_startup_handle_purpose startup_handle_purpose (enum)
 * @ingroup hyper_native
 * Purposes assigned to capabilities in the process startup table.
 * @{
 */
/**
 * `startup_handle_purpose_resource_domain` in the `startup_handle_purpose` enum domain.
 *
 * Purposes assigned to capabilities in the process startup table.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_RESOURCE_DOMAIN UINT64_C(1)
/**
 * `startup_handle_purpose_task_group` in the `startup_handle_purpose` enum domain.
 *
 * Purposes assigned to capabilities in the process startup table.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_TASK_GROUP UINT64_C(2)
/**
 * `startup_handle_purpose_task_factory` in the `startup_handle_purpose` enum domain.
 *
 * Purposes assigned to capabilities in the process startup table.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_TASK_FACTORY UINT64_C(3)
/**
 * `startup_handle_purpose_executable_authority` in the `startup_handle_purpose` enum domain.
 *
 * Purposes assigned to capabilities in the process startup table.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_EXECUTABLE_AUTHORITY UINT64_C(4)
/**
 * `startup_handle_purpose_root_vmar` in the `startup_handle_purpose` enum domain.
 *
 * Purposes assigned to capabilities in the process startup table.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_ROOT_VMAR UINT64_C(5)
/**
 * `startup_handle_purpose_console` in the `startup_handle_purpose` enum domain.
 *
 * Purposes assigned to capabilities in the process startup table.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_CONSOLE UINT64_C(6)
/**
 * `startup_handle_purpose_root_directory` in the `startup_handle_purpose` enum domain.
 *
 * Purposes assigned to capabilities in the process startup table.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_ROOT_DIRECTORY UINT64_C(7)
/**
 * `startup_handle_purpose_task_inspector` in the `startup_handle_purpose` enum domain.
 *
 * Purposes assigned to capabilities in the process startup table.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_TASK_INSPECTOR UINT64_C(8)
/**
 * `startup_handle_purpose_object_inspector` in the `startup_handle_purpose` enum domain.
 *
 * Purposes assigned to capabilities in the process startup table.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_OBJECT_INSPECTOR UINT64_C(9)
/**
 * `startup_handle_purpose_dynamic_library_directory` in the `startup_handle_purpose` enum domain.
 *
 * Purposes assigned to capabilities in the process startup table.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_DYNAMIC_LIBRARY_DIRECTORY UINT64_C(10)
/**
 * `startup_handle_purpose_memory_inspector` in the `startup_handle_purpose` enum domain.
 *
 * Purposes assigned to capabilities in the process startup table.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_MEMORY_INSPECTOR UINT64_C(11)
/**
 * `startup_handle_purpose_cpu_inspector` in the `startup_handle_purpose` enum domain.
 *
 * Purposes assigned to capabilities in the process startup table.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_CPU_INSPECTOR UINT64_C(12)
/**
 * `startup_handle_purpose_virtual_machine_creation_authority` in the `startup_handle_purpose` enum domain.
 *
 * Purposes assigned to capabilities in the process startup table.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_VIRTUAL_MACHINE_CREATION_AUTHORITY UINT64_C(13)
/**
 * `startup_handle_purpose_device_assignment_authority` in the `startup_handle_purpose` enum domain.
 *
 * Purposes assigned to capabilities in the process startup table.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_DEVICE_ASSIGNMENT_AUTHORITY UINT64_C(14)
/**
 * `startup_handle_purpose_initial_stack_vmar` in the `startup_handle_purpose` enum domain.
 *
 * Purposes assigned to capabilities in the process startup table.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_INITIAL_STACK_VMAR UINT64_C(15)
/**
 * `startup_handle_purpose_execution_context` in the `startup_handle_purpose` enum domain.
 *
 * Purposes assigned to capabilities in the process startup table.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_EXECUTION_CONTEXT UINT64_C(16)
/**
 * `startup_handle_purpose_object_factory` in the `startup_handle_purpose` enum domain.
 *
 * Purposes assigned to capabilities in the process startup table.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_OBJECT_FACTORY UINT64_C(17)
/**
 * `startup_handle_purpose_system_info` in the `startup_handle_purpose` enum domain.
 *
 * Purposes assigned to capabilities in the process startup table.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_SYSTEM_INFO UINT64_C(18)
/**
 * `startup_handle_purpose_boot_command_line` in the `startup_handle_purpose` enum domain.
 *
 * Purposes assigned to capabilities in the process startup table.
 * Select one value; this is not a bit mask.
 *
 * Unconsumed boot arguments: immutable VMO with READ | INSPECT rights.
 *  Contains a little-endian u64 byte length followed by that many UTF-8
 *  bytes (no NUL), bounded by `BOOT_COMMAND_LINE_MAX_BYTES`. The kernel
 *  passes unrecognized arguments to its initial process. Each userspace
 *  boot stage may pass its own remainder to the successor under this purpose.
 */
#define HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_BOOT_COMMAND_LINE UINT64_C(19)
/** @} */

/**
 * @defgroup hyper_native_process_phase process_phase (enum)
 * @ingroup hyper_native
 * Process lifecycle phases.
 * @{
 */
/**
 * `process_phase_prepared` in the `process_phase` enum domain.
 *
 * Process lifecycle phases.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_PROCESS_PHASE_PREPARED UINT64_C(0)
/**
 * `process_phase_created` in the `process_phase` enum domain.
 *
 * Process lifecycle phases.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_PROCESS_PHASE_CREATED UINT64_C(1)
/**
 * `process_phase_running` in the `process_phase` enum domain.
 *
 * Process lifecycle phases.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_PROCESS_PHASE_RUNNING UINT64_C(2)
/**
 * `process_phase_stopping` in the `process_phase` enum domain.
 *
 * Process lifecycle phases.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_PROCESS_PHASE_STOPPING UINT64_C(3)
/**
 * `process_phase_stopped` in the `process_phase` enum domain.
 *
 * Process lifecycle phases.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_PROCESS_PHASE_STOPPED UINT64_C(4)
/**
 * `process_phase_retiring` in the `process_phase` enum domain.
 *
 * Process lifecycle phases.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_PROCESS_PHASE_RETIRING UINT64_C(5)
/**
 * `process_phase_retired` in the `process_phase` enum domain.
 *
 * Process lifecycle phases.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_PROCESS_PHASE_RETIRED UINT64_C(6)
/** @} */

/**
 * @defgroup hyper_native_process_terminal process_terminal (enum)
 * @ingroup hyper_native
 * Reasons a process stopped.
 * @{
 */
/**
 * `process_terminal_none` in the `process_terminal` enum domain.
 *
 * Reasons a process stopped.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_PROCESS_TERMINAL_NONE UINT64_C(0)
/**
 * `process_terminal_requested` in the `process_terminal` enum domain.
 *
 * Reasons a process stopped.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_PROCESS_TERMINAL_REQUESTED UINT64_C(1)
/**
 * `process_terminal_thread_exited` in the `process_terminal` enum domain.
 *
 * Reasons a process stopped.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_PROCESS_TERMINAL_THREAD_EXITED UINT64_C(2)
/**
 * `process_terminal_process_exited` in the `process_terminal` enum domain.
 *
 * Reasons a process stopped.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_PROCESS_TERMINAL_PROCESS_EXITED UINT64_C(3)
/**
 * `process_terminal_last_thread_exited` in the `process_terminal` enum domain.
 *
 * Reasons a process stopped.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_PROCESS_TERMINAL_LAST_THREAD_EXITED UINT64_C(4)
/**
 * `process_terminal_fault` in the `process_terminal` enum domain.
 *
 * Reasons a process stopped.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_PROCESS_TERMINAL_FAULT UINT64_C(5)
/**
 * `process_terminal_task_group_stop` in the `process_terminal` enum domain.
 *
 * Reasons a process stopped.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_PROCESS_TERMINAL_TASK_GROUP_STOP UINT64_C(6)
/** @} */

/**
 * @defgroup hyper_native_thread_role thread_role (enum)
 * @ingroup hyper_native
 * Scheduler thread roles reported by inspection.
 * @{
 */
/**
 * `thread_role_bootstrap` in the `thread_role` enum domain.
 *
 * Scheduler thread roles reported by inspection.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_THREAD_ROLE_BOOTSTRAP UINT64_C(1)
/**
 * `thread_role_idle` in the `thread_role` enum domain.
 *
 * Scheduler thread roles reported by inspection.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_THREAD_ROLE_IDLE UINT64_C(2)
/**
 * `thread_role_kernel` in the `thread_role` enum domain.
 *
 * Scheduler thread roles reported by inspection.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_THREAD_ROLE_KERNEL UINT64_C(3)
/**
 * `thread_role_user` in the `thread_role` enum domain.
 *
 * Scheduler thread roles reported by inspection.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_THREAD_ROLE_USER UINT64_C(4)
/**
 * `thread_role_vcpu` in the `thread_role` enum domain.
 *
 * Scheduler thread roles reported by inspection.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_THREAD_ROLE_VCPU UINT64_C(5)
/** @} */

/**
 * @defgroup hyper_native_scheduling_policy scheduling_policy (enum)
 * @ingroup hyper_native
 * Scheduling algorithms; these do not identify the thread execution payload.
 * @{
 */
/**
 * `scheduling_policy_fair` in the `scheduling_policy` enum domain.
 *
 * Scheduling algorithms; these do not identify the thread execution payload.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_SCHEDULING_POLICY_FAIR UINT64_C(1)
/**
 * `scheduling_policy_budget` in the `scheduling_policy` enum domain.
 *
 * Scheduling algorithms; these do not identify the thread execution payload.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_SCHEDULING_POLICY_BUDGET UINT64_C(2)
/**
 * `scheduling_policy_fifo` in the `scheduling_policy` enum domain.
 *
 * Scheduling algorithms; these do not identify the thread execution payload.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_SCHEDULING_POLICY_FIFO UINT64_C(3)
/** @} */

/**
 * @defgroup hyper_native_scheduling_role scheduling_role (enum)
 * @ingroup hyper_native
 * Eligibility for CPU-local priority and minimum-progress allowances.
 * @{
 */
/**
 * `scheduling_role_ordinary` in the `scheduling_role` enum domain.
 *
 * Eligibility for CPU-local priority and minimum-progress allowances.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_SCHEDULING_ROLE_ORDINARY UINT64_C(0)
/**
 * `scheduling_role_priority` in the `scheduling_role` enum domain.
 *
 * Eligibility for CPU-local priority and minimum-progress allowances.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_SCHEDULING_ROLE_PRIORITY UINT64_C(1)
/**
 * `scheduling_role_protected` in the `scheduling_role` enum domain.
 *
 * Eligibility for CPU-local priority and minimum-progress allowances.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_SCHEDULING_ROLE_PROTECTED UINT64_C(2)
/** @} */

/**
 * @defgroup hyper_native_scheduling_fair_backend scheduling_fair_backend (enum)
 * @ingroup hyper_native
 * Fair queue implementations supported by the selected kernel image.
 * @{
 */
/**
 * `scheduling_fair_backend_stride` in the `scheduling_fair_backend` enum domain.
 *
 * Fair queue implementations supported by the selected kernel image.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_SCHEDULING_FAIR_BACKEND_STRIDE UINT64_C(0)
/**
 * `scheduling_fair_backend_round_robin` in the `scheduling_fair_backend` enum domain.
 *
 * Fair queue implementations supported by the selected kernel image.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_SCHEDULING_FAIR_BACKEND_ROUND_ROBIN UINT64_C(1)
/**
 * `scheduling_fair_backend_default` in the `scheduling_fair_backend` enum domain.
 *
 * Fair queue implementations supported by the selected kernel image.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_SCHEDULING_FAIR_BACKEND_DEFAULT UINT64_C(2)
/** @} */

/**
 * @defgroup hyper_native_thread_registry thread_registry (enum)
 * @ingroup hyper_native
 * Thread registry membership states.
 * @{
 */
/**
 * `thread_registry_resident` in the `thread_registry` enum domain.
 *
 * Thread registry membership states.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_THREAD_REGISTRY_RESIDENT UINT64_C(1)
/**
 * `thread_registry_retiring` in the `thread_registry` enum domain.
 *
 * Thread registry membership states.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_THREAD_REGISTRY_RETIRING UINT64_C(2)
/** @} */

/**
 * @defgroup hyper_native_task_limits task_limits (constants)
 * @ingroup hyper_native
 * Task names, affinity bounds and inspection page sizes.
 * @{
 */
/**
 * `thread_name_max_bytes` in the `task_limits` constants domain.
 *
 * Task names, affinity bounds and inspection page sizes.
 * Numeric parameter or sentinel, not an enum discriminant.
 *
 * UTF-8 bytes excluding any NUL terminator; control characters are rejected.
 */
#define HYPER_NATIVE_THREAD_NAME_MAX_BYTES UINT64_C(64)
/**
 * `process_name_max_bytes` in the `task_limits` constants domain.
 *
 * Task names, affinity bounds and inspection page sizes.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_PROCESS_NAME_MAX_BYTES UINT64_C(64)
/**
 * `process_affinity_max_words` in the `task_limits` constants domain.
 *
 * Task names, affinity bounds and inspection page sizes.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_PROCESS_AFFINITY_MAX_WORDS UINT64_C(4)
/**
 * `process_affinity_max_cpus` in the `task_limits` constants domain.
 *
 * Task names, affinity bounds and inspection page sizes.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_PROCESS_AFFINITY_MAX_CPUS UINT64_C(256)
/**
 * `task_inspector_process_page_capacity` in the `task_limits` constants domain.
 *
 * Task names, affinity bounds and inspection page sizes.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_TASK_INSPECTOR_PROCESS_PAGE_CAPACITY UINT64_C(8)
/**
 * `task_inspector_thread_page_capacity` in the `task_limits` constants domain.
 *
 * Task names, affinity bounds and inspection page sizes.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_TASK_INSPECTOR_THREAD_PAGE_CAPACITY UINT64_C(8)
/**
 * `object_inspector_object_page_capacity` in the `task_limits` constants domain.
 *
 * Task names, affinity bounds and inspection page sizes.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_OBJECT_INSPECTOR_OBJECT_PAGE_CAPACITY UINT64_C(8)
/**
 * `object_inspector_handle_page_capacity` in the `task_limits` constants domain.
 *
 * Task names, affinity bounds and inspection page sizes.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_OBJECT_INSPECTOR_HANDLE_PAGE_CAPACITY UINT64_C(8)
/** @} */

/**
 * @defgroup hyper_native_object_detail object_detail (enum)
 * @ingroup hyper_native
 * Payload tags for detailed object inspection.
 * @{
 */
/**
 * `object_detail_empty` in the `object_detail` enum domain.
 *
 * Payload tags for detailed object inspection.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_OBJECT_DETAIL_EMPTY UINT64_C(0)
/**
 * `object_detail_thread` in the `object_detail` enum domain.
 *
 * Payload tags for detailed object inspection.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_OBJECT_DETAIL_THREAD UINT64_C(1)
/**
 * `object_detail_vmar` in the `object_detail` enum domain.
 *
 * Payload tags for detailed object inspection.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_OBJECT_DETAIL_VMAR UINT64_C(2)
/**
 * `object_detail_mapping` in the `object_detail` enum domain.
 *
 * Payload tags for detailed object inspection.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_OBJECT_DETAIL_MAPPING UINT64_C(3)
/**
 * `object_detail_channel` in the `object_detail` enum domain.
 *
 * Payload tags for detailed object inspection.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_OBJECT_DETAIL_CHANNEL UINT64_C(4)
/**
 * `object_detail_device` in the `object_detail` enum domain.
 *
 * Payload tags for detailed object inspection.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_OBJECT_DETAIL_DEVICE UINT64_C(5)
/**
 * `object_detail_device_resource` in the `object_detail` enum domain.
 *
 * Payload tags for detailed object inspection.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_OBJECT_DETAIL_DEVICE_RESOURCE UINT64_C(6)
/** @} */

/**
 * @defgroup hyper_native_object_handle_state object_handle_state (enum)
 * @ingroup hyper_native
 * Userspace handle publication states.
 * @{
 */
/**
 * `object_handle_state_unpublished` in the `object_handle_state` enum domain.
 *
 * Userspace handle publication states.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_OBJECT_HANDLE_STATE_UNPUBLISHED UINT64_C(1)
/**
 * `object_handle_state_active` in the `object_handle_state` enum domain.
 *
 * Userspace handle publication states.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_OBJECT_HANDLE_STATE_ACTIVE UINT64_C(2)
/**
 * `object_handle_state_retired` in the `object_handle_state` enum domain.
 *
 * Userspace handle publication states.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_OBJECT_HANDLE_STATE_RETIRED UINT64_C(3)
/** @} */

/**
 * @defgroup hyper_native_process_retirement process_retirement (enum)
 * @ingroup hyper_native
 * Retained process lifecycle phases.
 * @{
 */
/**
 * `process_retirement_prepared` in the `process_retirement` enum domain.
 *
 * Retained process lifecycle phases.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_PROCESS_RETIREMENT_PREPARED UINT64_C(0)
/**
 * `process_retirement_running` in the `process_retirement` enum domain.
 *
 * Retained process lifecycle phases.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_PROCESS_RETIREMENT_RUNNING UINT64_C(1)
/**
 * `process_retirement_stopping` in the `process_retirement` enum domain.
 *
 * Retained process lifecycle phases.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_PROCESS_RETIREMENT_STOPPING UINT64_C(2)
/**
 * `process_retirement_ready` in the `process_retirement` enum domain.
 *
 * Retained process lifecycle phases.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_PROCESS_RETIREMENT_READY UINT64_C(3)
/**
 * `process_retirement_released` in the `process_retirement` enum domain.
 *
 * Retained process lifecycle phases.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_PROCESS_RETIREMENT_RELEASED UINT64_C(4)
/** @} */

/**
 * @defgroup hyper_native_process_retirement_release process_retirement_release (enum)
 * @ingroup hyper_native
 * Reasons retained process state was released.
 * @{
 */
/**
 * `process_retirement_release_none` in the `process_retirement_release` enum domain.
 *
 * Reasons retained process state was released.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_PROCESS_RETIREMENT_RELEASE_NONE UINT64_C(0)
/**
 * `process_retirement_release_explicit` in the `process_retirement_release` enum domain.
 *
 * Reasons retained process state was released.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_PROCESS_RETIREMENT_RELEASE_EXPLICIT UINT64_C(1)
/**
 * `process_retirement_release_owner_lost` in the `process_retirement_release` enum domain.
 *
 * Reasons retained process state was released.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_PROCESS_RETIREMENT_RELEASE_OWNER_LOST UINT64_C(2)
/**
 * `process_retirement_release_deadline` in the `process_retirement_release` enum domain.
 *
 * Reasons retained process state was released.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_PROCESS_RETIREMENT_RELEASE_DEADLINE UINT64_C(3)
/**
 * `process_retirement_release_aborted` in the `process_retirement_release` enum domain.
 *
 * Reasons retained process state was released.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_PROCESS_RETIREMENT_RELEASE_ABORTED UINT64_C(4)
/** @} */

/**
 * @defgroup hyper_native_crash_partial crash_partial (flags)
 * @ingroup hyper_native
 * Incomplete sections of retained crash state.
 * @{
 */
/**
 * `crash_partial_memory` in the `crash_partial` flags domain.
 *
 * Incomplete sections of retained crash state.
 * Independent flag bit; combine permitted members with bitwise OR.
 */
#define HYPER_NATIVE_CRASH_PARTIAL_MEMORY UINT64_C(1)
/**
 * `crash_partial_threads` in the `crash_partial` flags domain.
 *
 * Incomplete sections of retained crash state.
 * Independent flag bit; combine permitted members with bitwise OR.
 */
#define HYPER_NATIVE_CRASH_PARTIAL_THREADS UINT64_C(2)
/**
 * `crash_partial_handles` in the `crash_partial` flags domain.
 *
 * Incomplete sections of retained crash state.
 * Independent flag bit; combine permitted members with bitwise OR.
 */
#define HYPER_NATIVE_CRASH_PARTIAL_HANDLES UINT64_C(4)
/**
 * `crash_partial_mappings` in the `crash_partial` flags domain.
 *
 * Incomplete sections of retained crash state.
 * Independent flag bit; combine permitted members with bitwise OR.
 */
#define HYPER_NATIVE_CRASH_PARTIAL_MAPPINGS UINT64_C(8)
/** @} */

/**
 * @defgroup hyper_native_crash_mapping_status crash_mapping_status (flags)
 * @ingroup hyper_native
 * Retention and exclusion flags for a crash mapping.
 * @{
 */
/**
 * `crash_mapping_retained` in the `crash_mapping_status` flags domain.
 *
 * Retention and exclusion flags for a crash mapping.
 * Independent flag bit; combine permitted members with bitwise OR.
 */
#define HYPER_NATIVE_CRASH_MAPPING_RETAINED UINT64_C(1)
/**
 * `crash_mapping_partial` in the `crash_mapping_status` flags domain.
 *
 * Retention and exclusion flags for a crash mapping.
 * Independent flag bit; combine permitted members with bitwise OR.
 */
#define HYPER_NATIVE_CRASH_MAPPING_PARTIAL UINT64_C(2)
/**
 * `crash_mapping_excluded_shared` in the `crash_mapping_status` flags domain.
 *
 * Retention and exclusion flags for a crash mapping.
 * Independent flag bit; combine permitted members with bitwise OR.
 */
#define HYPER_NATIVE_CRASH_MAPPING_EXCLUDED_SHARED UINT64_C(4)
/**
 * `crash_mapping_excluded_file` in the `crash_mapping_status` flags domain.
 *
 * Retention and exclusion flags for a crash mapping.
 * Independent flag bit; combine permitted members with bitwise OR.
 */
#define HYPER_NATIVE_CRASH_MAPPING_EXCLUDED_FILE UINT64_C(8)
/** @} */

/**
 * @defgroup hyper_native_crash_mapping_kind crash_mapping_kind (enum)
 * @ingroup hyper_native
 * Backing kinds reported for a crash mapping.
 * @{
 */
/**
 * `crash_mapping_anonymous` in the `crash_mapping_kind` enum domain.
 *
 * Backing kinds reported for a crash mapping.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_CRASH_MAPPING_ANONYMOUS UINT64_C(1)
/**
 * `crash_mapping_private_file` in the `crash_mapping_kind` enum domain.
 *
 * Backing kinds reported for a crash mapping.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_CRASH_MAPPING_PRIVATE_FILE UINT64_C(2)
/**
 * `crash_mapping_shared` in the `crash_mapping_kind` enum domain.
 *
 * Backing kinds reported for a crash mapping.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_CRASH_MAPPING_SHARED UINT64_C(3)
/** @} */

/**
 * @defgroup hyper_native_crash_limits crash_limits (constants)
 * @ingroup hyper_native
 * Maximum bytes returned by one crash-memory read.
 * @{
 */
/**
 * `crash_read_max_bytes` in the `crash_limits` constants domain.
 *
 * Maximum bytes returned by one crash-memory read.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_CRASH_READ_MAX_BYTES UINT64_C(65536)
/** @} */

/**
 * @defgroup hyper_native_memory_statistics memory_statistics (enum)
 * @ingroup hyper_native
 * Selectors for detailed memory statistics.
 * @{
 */
/**
 * `memory_statistics_heap` in the `memory_statistics` enum domain.
 *
 * Selectors for detailed memory statistics.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_MEMORY_STATISTICS_HEAP UINT64_C(1)
/**
 * `memory_statistics_buddy` in the `memory_statistics` enum domain.
 *
 * Selectors for detailed memory statistics.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_MEMORY_STATISTICS_BUDDY UINT64_C(2)
/**
 * `memory_statistics_owner` in the `memory_statistics` enum domain.
 *
 * Selectors for detailed memory statistics.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_MEMORY_STATISTICS_OWNER UINT64_C(3)
/**
 * `memory_statistics_file_cache` in the `memory_statistics` enum domain.
 *
 * Selectors for detailed memory statistics.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_MEMORY_STATISTICS_FILE_CACHE UINT64_C(4)
/**
 * `memory_statistics_heap_class` in the `memory_statistics` enum domain.
 *
 * Selectors for detailed memory statistics.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_MEMORY_STATISTICS_HEAP_CLASS UINT64_C(5)
/** @} */

/**
 * @defgroup hyper_native_memory_statistics_dimensions memory_statistics_dimensions (constants)
 * @ingroup hyper_native
 * Dimensions of memory statistics records.
 * @{
 */
/**
 * `memory_statistics_buddy_order_count` in the `memory_statistics_dimensions` constants domain.
 *
 * Dimensions of memory statistics records.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_MEMORY_STATISTICS_BUDDY_ORDER_COUNT UINT64_C(19)
/**
 * `memory_statistics_owner_count` in the `memory_statistics_dimensions` constants domain.
 *
 * Dimensions of memory statistics records.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_MEMORY_STATISTICS_OWNER_COUNT UINT64_C(5)
/**
 * `memory_statistics_heap_class_count` in the `memory_statistics_dimensions` constants domain.
 *
 * Dimensions of memory statistics records.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_MEMORY_STATISTICS_HEAP_CLASS_COUNT UINT64_C(18)
/** @} */

/**
 * @defgroup hyper_native_private_mapping private_mapping (enum)
 * @ingroup hyper_native
 * Private mapping population policies.
 * @{
 */
/**
 * `private_mapping_copy_on_write` in the `private_mapping` enum domain.
 *
 * Private mapping population policies.
 * Select one value; this is not a bit mask.
 *
 * Share source pages until the mapping is written; modifications remain private.
 */
#define HYPER_NATIVE_PRIVATE_MAPPING_COPY_ON_WRITE UINT64_C(0)
/**
 * `private_mapping_eager` in the `private_mapping` enum domain.
 *
 * Private mapping population policies.
 * Select one value; this is not a bit mask.
 *
 * Populate private pages immediately instead of deferring copying to write faults.
 */
#define HYPER_NATIVE_PRIVATE_MAPPING_EAGER UINT64_C(1)
/** @} */

/**
 * @defgroup hyper_native_vmar_permission vmar_permission (flags)
 * @ingroup hyper_native
 * Permissions which may be combined for a mapping.
 * @{
 */
/**
 * `vmar_permission_read` in the `vmar_permission` flags domain.
 *
 * Permissions which may be combined for a mapping.
 * Independent flag bit; combine permitted members with bitwise OR.
 */
#define HYPER_NATIVE_VMAR_PERMISSION_READ UINT64_C(1)
/**
 * `vmar_permission_write` in the `vmar_permission` flags domain.
 *
 * Permissions which may be combined for a mapping.
 * Independent flag bit; combine permitted members with bitwise OR.
 */
#define HYPER_NATIVE_VMAR_PERMISSION_WRITE UINT64_C(2)
/**
 * `vmar_permission_execute` in the `vmar_permission` flags domain.
 *
 * Permissions which may be combined for a mapping.
 * Independent flag bit; combine permitted members with bitwise OR.
 */
#define HYPER_NATIVE_VMAR_PERMISSION_EXECUTE UINT64_C(4)
/** @} */

/**
 * @defgroup hyper_native_vmar_allocate vmar_allocate (flags)
 * @ingroup hyper_native
 * Options for address-space allocation.
 * @{
 */
/**
 * `vmar_allocate_exact` in the `vmar_allocate` flags domain.
 *
 * Options for address-space allocation.
 * Independent flag bit; combine permitted members with bitwise OR.
 */
#define HYPER_NATIVE_VMAR_ALLOCATE_EXACT UINT64_C(1)
/** @} */

/**
 * @defgroup hyper_native_vmar_memory vmar_memory (flags)
 * @ingroup hyper_native
 * Options for locking currently mapped memory.
 * @{
 */
/**
 * `vmar_memory_current` in the `vmar_memory` flags domain.
 *
 * Options for locking currently mapped memory.
 * Independent flag bit; combine permitted members with bitwise OR.
 *
 * Lock mappings that exist now; future mappings do not inherit the lock.
 *  Unmapping and process teardown still release the corresponding residency.
 */
#define HYPER_NATIVE_VMAR_MEMORY_CURRENT UINT64_C(1)
/** @} */

/**
 * @defgroup hyper_native_vmo_limits vmo_limits (constants)
 * @ingroup hyper_native
 * VMO size and transfer bounds.
 * @{
 */
/**
 * `vmo_max_size_bytes` in the `vmo_limits` constants domain.
 *
 * VMO size and transfer bounds.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_VMO_MAX_SIZE_BYTES UINT64_C(4294967296)
/**
 * `vmo_max_transfer_bytes` in the `vmo_limits` constants domain.
 *
 * VMO size and transfer bounds.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_VMO_MAX_TRANSFER_BYTES UINT64_C(65536)
/**
 * `vmo_max_contiguous_size_bytes` in the `vmo_limits` constants domain.
 *
 * VMO size and transfer bounds.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_VMO_MAX_CONTIGUOUS_SIZE_BYTES UINT64_C(134217728)
/** @} */

/**
 * @defgroup hyper_native_capability_disposition capability_disposition (enum)
 * @ingroup hyper_native
 * Ownership operations for a transferred capability.
 * @{
 */
/**
 * `capability_disposition_move` in the `capability_disposition` enum domain.
 *
 * Ownership operations for a transferred capability.
 * Select one value; this is not a bit mask.
 *
 * Transfer ownership only when the operation commits; failures preserve the source.
 */
#define HYPER_NATIVE_CAPABILITY_DISPOSITION_MOVE UINT64_C(0)
/**
 * `capability_disposition_duplicate` in the `capability_disposition` enum domain.
 *
 * Ownership operations for a transferred capability.
 * Select one value; this is not a bit mask.
 *
 * Keep the source handle; requires DUPLICATE in addition to transfer authority.
 */
#define HYPER_NATIVE_CAPABILITY_DISPOSITION_DUPLICATE UINT64_C(1)
/** @} */

/**
 * @defgroup hyper_native_channel_limits channel_limits (constants)
 * @ingroup hyper_native
 * Channel message, queue and capability-transfer bounds.
 * @{
 */
/**
 * `byte_channel_max_message_bytes` in the `channel_limits` constants domain.
 *
 * Channel message, queue and capability-transfer bounds.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_BYTE_CHANNEL_MAX_MESSAGE_BYTES UINT64_C(65536)
/**
 * `byte_channel_max_queued_messages` in the `channel_limits` constants domain.
 *
 * Channel message, queue and capability-transfer bounds.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_BYTE_CHANNEL_MAX_QUEUED_MESSAGES UINT64_C(16)
/**
 * `byte_channel_max_queued_bytes` in the `channel_limits` constants domain.
 *
 * Channel message, queue and capability-transfer bounds.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_BYTE_CHANNEL_MAX_QUEUED_BYTES UINT64_C(1048576)
/**
 * `capability_channel_max_message_bytes` in the `channel_limits` constants domain.
 *
 * Channel message, queue and capability-transfer bounds.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_CAPABILITY_CHANNEL_MAX_MESSAGE_BYTES UINT64_C(4096)
/**
 * `capability_channel_max_handles` in the `channel_limits` constants domain.
 *
 * Channel message, queue and capability-transfer bounds.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_CAPABILITY_CHANNEL_MAX_HANDLES UINT64_C(64)
/**
 * `capability_disposition_same_rights` in the `channel_limits` constants domain.
 *
 * Channel message, queue and capability-transfer bounds.
 * Numeric parameter or sentinel, not an enum discriminant.
 *
 * Request the source's current rights, not every right defined by the ABI.
 */
#define HYPER_NATIVE_CAPABILITY_DISPOSITION_SAME_RIGHTS UINT64_C(18446744073709551615)
/** @} */

/**
 * @defgroup hyper_native_console_limits console_limits (constants)
 * @ingroup hyper_native
 * Console transfer bounds and virtual serial shared-memory layout.
 * @{
 */
/**
 * `console_max_transfer_bytes` in the `console_limits` constants domain.
 *
 * Console transfer bounds and virtual serial shared-memory layout.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_CONSOLE_MAX_TRANSFER_BYTES UINT64_C(4096)
/**
 * `virtual_serial_max_transfer_bytes` in the `console_limits` constants domain.
 *
 * Console transfer bounds and virtual serial shared-memory layout.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_VIRTUAL_SERIAL_MAX_TRANSFER_BYTES UINT64_C(4096)
/**
 * `virtual_serial_output_capacity` in the `console_limits` constants domain.
 *
 * Console transfer bounds and virtual serial shared-memory layout.
 * Numeric parameter or sentinel, not an enum discriminant.
 *
 * Byte slots in the producer ring; unread bytes are never overwritten.
 */
#define HYPER_NATIVE_VIRTUAL_SERIAL_OUTPUT_CAPACITY UINT64_C(65536)
/**
 * `virtual_serial_output_header_bytes` in the `console_limits` constants domain.
 *
 * Console transfer bounds and virtual serial shared-memory layout.
 * Numeric parameter or sentinel, not an enum discriminant.
 *
 * Byte offset of ring data after its shared atomic counter header.
 */
#define HYPER_NATIVE_VIRTUAL_SERIAL_OUTPUT_HEADER_BYTES UINT64_C(4096)
/**
 * `virtual_serial_output_bytes` in the `console_limits` constants domain.
 *
 * Console transfer bounds and virtual serial shared-memory layout.
 * Numeric parameter or sentinel, not an enum discriminant.
 *
 * Exact VMO size required by `virtual_serial_register_output`.
 */
#define HYPER_NATIVE_VIRTUAL_SERIAL_OUTPUT_BYTES UINT64_C(69632)
/** @} */

/**
 * @defgroup hyper_native_directory_entry_kind directory_entry_kind (enum)
 * @ingroup hyper_native
 * Kinds reported by directory enumeration.
 * @{
 */
/**
 * `directory_entry_kind_file` in the `directory_entry_kind` enum domain.
 *
 * Kinds reported by directory enumeration.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_DIRECTORY_ENTRY_KIND_FILE UINT64_C(1)
/**
 * `directory_entry_kind_directory` in the `directory_entry_kind` enum domain.
 *
 * Kinds reported by directory enumeration.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_DIRECTORY_ENTRY_KIND_DIRECTORY UINT64_C(2)
/**
 * `directory_entry_kind_symlink` in the `directory_entry_kind` enum domain.
 *
 * Kinds reported by directory enumeration.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_DIRECTORY_ENTRY_KIND_SYMLINK UINT64_C(3)
/**
 * `directory_entry_kind_other` in the `directory_entry_kind` enum domain.
 *
 * Kinds reported by directory enumeration.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_DIRECTORY_ENTRY_KIND_OTHER UINT64_C(4)
/** @} */

/**
 * @defgroup hyper_native_filesystem_limits filesystem_limits (constants)
 * @ingroup hyper_native
 * Path, directory-page, transfer and root-switch bounds.
 * @{
 */
/**
 * `directory_entry_page_capacity` in the `filesystem_limits` constants domain.
 *
 * Path, directory-page, transfer and root-switch bounds.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_DIRECTORY_ENTRY_PAGE_CAPACITY UINT64_C(4)
/**
 * `directory_entry_name_max_bytes` in the `filesystem_limits` constants domain.
 *
 * Path, directory-page, transfer and root-switch bounds.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_DIRECTORY_ENTRY_NAME_MAX_BYTES UINT64_C(255)
/**
 * `directory_max_path_bytes` in the `filesystem_limits` constants domain.
 *
 * Path, directory-page, transfer and root-switch bounds.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_DIRECTORY_MAX_PATH_BYTES UINT64_C(4096)
/**
 * `file_max_read_bytes` in the `filesystem_limits` constants domain.
 *
 * Path, directory-page, transfer and root-switch bounds.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_FILE_MAX_READ_BYTES UINT64_C(2097152)
/**
 * `filesystem_switch_root_max_moves` in the `filesystem_limits` constants domain.
 *
 * Path, directory-page, transfer and root-switch bounds.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_FILESYSTEM_SWITCH_ROOT_MAX_MOVES UINT64_C(16)
/** @} */

/**
 * @defgroup hyper_native_device_profile device_profile (enum)
 * @ingroup hyper_native
 * Physical-device assignment profiles.
 * @{
 */
/**
 * `device_profile_virtio_mmio_scsi` in the `device_profile` enum domain.
 *
 * Physical-device assignment profiles.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_DEVICE_PROFILE_VIRTIO_MMIO_SCSI UINT64_C(1)
/**
 * `device_profile_userspace` in the `device_profile` enum domain.
 *
 * Physical-device assignment profiles.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_DEVICE_PROFILE_USERSPACE UINT64_C(2)
/**
 * `device_profile_virtio_mmio_net` in the `device_profile` enum domain.
 *
 * Physical-device assignment profiles.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_DEVICE_PROFILE_VIRTIO_MMIO_NET UINT64_C(3)
/**
 * `device_profile_pci_function` in the `device_profile` enum domain.
 *
 * Physical-device assignment profiles.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_DEVICE_PROFILE_PCI_FUNCTION UINT64_C(4)
/** @} */

/**
 * @defgroup hyper_native_device_identity device_identity (enum)
 * @ingroup hyper_native
 * Selectors for device identity matching.
 * @{
 */
/**
 * `device_identity_compatible` in the `device_identity` enum domain.
 *
 * Selectors for device identity matching.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_DEVICE_IDENTITY_COMPATIBLE UINT64_C(1)
/**
 * `device_identity_fdt_path` in the `device_identity` enum domain.
 *
 * Selectors for device identity matching.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_DEVICE_IDENTITY_FDT_PATH UINT64_C(2)
/**
 * `device_identity_pci_id` in the `device_identity` enum domain.
 *
 * Selectors for device identity matching.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_DEVICE_IDENTITY_PCI_ID UINT64_C(3)
/** @} */

/**
 * @defgroup hyper_native_device_firmware_field device_firmware_field (enum)
 * @ingroup hyper_native
 * Fields exposed by firmware inspection.
 * @{
 */
/**
 * `device_firmware_field_info` in the `device_firmware_field` enum domain.
 *
 * Fields exposed by firmware inspection.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_DEVICE_FIRMWARE_FIELD_INFO UINT64_C(0)
/**
 * `device_firmware_field_path` in the `device_firmware_field` enum domain.
 *
 * Fields exposed by firmware inspection.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_DEVICE_FIRMWARE_FIELD_PATH UINT64_C(1)
/**
 * `device_firmware_field_compatible` in the `device_firmware_field` enum domain.
 *
 * Fields exposed by firmware inspection.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_DEVICE_FIRMWARE_FIELD_COMPATIBLE UINT64_C(2)
/**
 * `device_firmware_field_registers` in the `device_firmware_field` enum domain.
 *
 * Fields exposed by firmware inspection.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_DEVICE_FIRMWARE_FIELD_REGISTERS UINT64_C(3)
/**
 * `device_firmware_field_property` in the `device_firmware_field` enum domain.
 *
 * Fields exposed by firmware inspection.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_DEVICE_FIRMWARE_FIELD_PROPERTY UINT64_C(4)
/**
 * `device_firmware_field_property_names` in the `device_firmware_field` enum domain.
 *
 * Fields exposed by firmware inspection.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_DEVICE_FIRMWARE_FIELD_PROPERTY_NAMES UINT64_C(5)
/** @} */

/**
 * @defgroup hyper_native_device_resource_ids device_resource_ids (constants)
 * @ingroup hyper_native
 * PCI resource identifiers; BAR0 is the base of the BAR index range.
 * @{
 */
/**
 * `device_resource_pci_ecam` in the `device_resource_ids` constants domain.
 *
 * PCI resource identifiers; BAR0 is the base of the BAR index range.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_DEVICE_RESOURCE_PCI_ECAM UINT64_C(256)
/**
 * `device_resource_pci_msi` in the `device_resource_ids` constants domain.
 *
 * PCI resource identifiers; BAR0 is the base of the BAR index range.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_DEVICE_RESOURCE_PCI_MSI UINT64_C(257)
/**
 * `device_resource_pci_bar0` in the `device_resource_ids` constants domain.
 *
 * PCI resource identifiers; BAR0 is the base of the BAR index range.
 * Numeric parameter or sentinel, not an enum discriminant.
 *
 * Add the zero-based BAR index to identify a PCI BAR resource.
 */
#define HYPER_NATIVE_DEVICE_RESOURCE_PCI_BAR0 UINT64_C(512)
/**
 * `device_resource_pci_intx` in the `device_resource_ids` constants domain.
 *
 * PCI resource identifiers; BAR0 is the base of the BAR index range.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_DEVICE_RESOURCE_PCI_INTX UINT64_C(258)
/** @} */

/**
 * @defgroup hyper_native_device_resource_flags device_resource_flags (flags)
 * @ingroup hyper_native
 * PCI resource attributes.
 * @{
 */
/**
 * `device_resource_memory_64` in the `device_resource_flags` flags domain.
 *
 * PCI resource attributes.
 * Independent flag bit; combine permitted members with bitwise OR.
 */
#define HYPER_NATIVE_DEVICE_RESOURCE_MEMORY_64 UINT64_C(1)
/**
 * `device_resource_prefetchable` in the `device_resource_flags` flags domain.
 *
 * PCI resource attributes.
 * Independent flag bit; combine permitted members with bitwise OR.
 */
#define HYPER_NATIVE_DEVICE_RESOURCE_PREFETCHABLE UINT64_C(2)
/** @} */

/**
 * @defgroup hyper_native_device_limits device_limits (constants)
 * @ingroup hyper_native
 * Device discovery, assignment and PCI aperture bounds.
 * @{
 */
/**
 * `device_firmware_max_bytes` in the `device_limits` constants domain.
 *
 * Device discovery, assignment and PCI aperture bounds.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_DEVICE_FIRMWARE_MAX_BYTES UINT64_C(65536)
/**
 * `device_firmware_name_max_bytes` in the `device_limits` constants domain.
 *
 * Device discovery, assignment and PCI aperture bounds.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_DEVICE_FIRMWARE_NAME_MAX_BYTES UINT64_C(128)
/**
 * `device_bundle_max_entries` in the `device_limits` constants domain.
 *
 * Device discovery, assignment and PCI aperture bounds.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_DEVICE_BUNDLE_MAX_ENTRIES UINT64_C(8)
/**
 * `device_pci_aperture_size` in the `device_limits` constants domain.
 *
 * Device discovery, assignment and PCI aperture bounds.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_DEVICE_PCI_APERTURE_SIZE UINT64_C(8388608)
/**
 * `device_assignment_max_devices` in the `device_limits` constants domain.
 *
 * Device discovery, assignment and PCI aperture bounds.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_DEVICE_ASSIGNMENT_MAX_DEVICES UINT64_C(4)
/** @} */

/**
 * @defgroup hyper_native_dma_fault_source dma_fault_source (enum)
 * @ingroup hyper_native
 * DMA isolation mechanisms reporting a fault.
 * @{
 */
/**
 * `dma_fault_source_smmuv3` in the `dma_fault_source` enum domain.
 *
 * DMA isolation mechanisms reporting a fault.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_DMA_FAULT_SOURCE_SMMUV3 UINT64_C(1)
/** @} */

/**
 * @defgroup hyper_native_dma_fault_scope dma_fault_scope (enum)
 * @ingroup hyper_native
 * Scope of a DMA fault report.
 * @{
 */
/**
 * `dma_fault_scope_stream` in the `dma_fault_scope` enum domain.
 *
 * Scope of a DMA fault report.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_DMA_FAULT_SCOPE_STREAM UINT64_C(1)
/**
 * `dma_fault_scope_controller` in the `dma_fault_scope` enum domain.
 *
 * Scope of a DMA fault report.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_DMA_FAULT_SCOPE_CONTROLLER UINT64_C(2)
/** @} */

/**
 * @defgroup hyper_native_dma_fault_flags dma_fault_flags (flags)
 * @ingroup hyper_native
 * Valid-address and access-direction flags in DMA fault reports.
 * @{
 */
/**
 * `dma_fault_address_valid` in the `dma_fault_flags` flags domain.
 *
 * Valid-address and access-direction flags in DMA fault reports.
 * Independent flag bit; combine permitted members with bitwise OR.
 *
 * The reported address is meaningful; a controller-wide report may omit it.
 */
#define HYPER_NATIVE_DMA_FAULT_ADDRESS_VALID UINT64_C(1)
/**
 * `dma_fault_read` in the `dma_fault_flags` flags domain.
 *
 * Valid-address and access-direction flags in DMA fault reports.
 * Independent flag bit; combine permitted members with bitwise OR.
 */
#define HYPER_NATIVE_DMA_FAULT_READ UINT64_C(2)
/**
 * `dma_fault_write` in the `dma_fault_flags` flags domain.
 *
 * Valid-address and access-direction flags in DMA fault reports.
 * Independent flag bit; combine permitted members with bitwise OR.
 */
#define HYPER_NATIVE_DMA_FAULT_WRITE UINT64_C(4)
/** @} */

/**
 * @defgroup hyper_native_native_block_layout native_block_layout (constants)
 * @ingroup hyper_native
 * Shared-memory geometry and transfer bounds for the Native block initiator.
 * @{
 */
/**
 * `native_block_transfer_bytes` in the `native_block_layout` constants domain.
 *
 * Shared-memory geometry and transfer bounds for the Native block initiator.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_NATIVE_BLOCK_TRANSFER_BYTES UINT64_C(524288)
/**
 * `native_block_batch_max` in the `native_block_layout` constants domain.
 *
 * Shared-memory geometry and transfer bounds for the Native block initiator.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_NATIVE_BLOCK_BATCH_MAX UINT64_C(4)
/**
 * `native_block_batch_record_bytes` in the `native_block_layout` constants domain.
 *
 * Shared-memory geometry and transfer bounds for the Native block initiator.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_NATIVE_BLOCK_BATCH_RECORD_BYTES UINT64_C(16)
/**
 * `native_block_transfer_frame_bytes` in the `native_block_layout` constants domain.
 *
 * Shared-memory geometry and transfer bounds for the Native block initiator.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_NATIVE_BLOCK_TRANSFER_FRAME_BYTES UINT64_C(524352)
/**
 * `native_block_memory_bytes` in the `native_block_layout` constants domain.
 *
 * Shared-memory geometry and transfer bounds for the Native block initiator.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_NATIVE_BLOCK_MEMORY_BYTES UINT64_C(1048576)
/**
 * `native_block_queue_count` in the `native_block_layout` constants domain.
 *
 * Shared-memory geometry and transfer bounds for the Native block initiator.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_NATIVE_BLOCK_QUEUE_COUNT UINT64_C(6)
/**
 * `native_block_queue_size` in the `native_block_layout` constants domain.
 *
 * Shared-memory geometry and transfer bounds for the Native block initiator.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_NATIVE_BLOCK_QUEUE_SIZE UINT64_C(8)
/**
 * `native_block_queue_stride` in the `native_block_layout` constants domain.
 *
 * Shared-memory geometry and transfer bounds for the Native block initiator.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_NATIVE_BLOCK_QUEUE_STRIDE UINT64_C(4096)
/**
 * `native_block_available_offset` in the `native_block_layout` constants domain.
 *
 * Shared-memory geometry and transfer bounds for the Native block initiator.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_NATIVE_BLOCK_AVAILABLE_OFFSET UINT64_C(256)
/**
 * `native_block_used_offset` in the `native_block_layout` constants domain.
 *
 * Shared-memory geometry and transfer bounds for the Native block initiator.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_NATIVE_BLOCK_USED_OFFSET UINT64_C(512)
/** @} */

/**
 * @defgroup hyper_native_guest_notification guest_notification (enum)
 * @ingroup hyper_native
 * Guest notification control operations.
 * @{
 */
/**
 * `guest_notification_disable` in the `guest_notification` enum domain.
 *
 * Guest notification control operations.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_GUEST_NOTIFICATION_DISABLE UINT64_C(0)
/**
 * `guest_notification_enable` in the `guest_notification` enum domain.
 *
 * Guest notification control operations.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_GUEST_NOTIFICATION_ENABLE UINT64_C(1)
/**
 * `guest_notification_raise_config` in the `guest_notification` enum domain.
 *
 * Guest notification control operations.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_GUEST_NOTIFICATION_RAISE_CONFIG UINT64_C(2)
/**
 * `guest_notification_disconnect` in the `guest_notification` enum domain.
 *
 * Guest notification control operations.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_GUEST_NOTIFICATION_DISCONNECT UINT64_C(3)
/** @} */

/**
 * @defgroup hyper_native_io_transport_limits io_transport_limits (constants)
 * @ingroup hyper_native
 * I/O client, mailbox and guest alias-window bounds.
 * @{
 */
/**
 * `io_max_clients` in the `io_transport_limits` constants domain.
 *
 * I/O client, mailbox and guest alias-window bounds.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_IO_MAX_CLIENTS UINT64_C(9)
/**
 * `guest_mailbox_max_message_bytes` in the `io_transport_limits` constants domain.
 *
 * I/O client, mailbox and guest alias-window bounds.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_GUEST_MAILBOX_MAX_MESSAGE_BYTES UINT64_C(256)
/**
 * `guest_dynamic_alias_offset` in the `io_transport_limits` constants domain.
 *
 * I/O client, mailbox and guest alias-window bounds.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_GUEST_DYNAMIC_ALIAS_OFFSET UINT64_C(68719476736)
/**
 * `guest_dynamic_physical_limit` in the `io_transport_limits` constants domain.
 *
 * I/O client, mailbox and guest alias-window bounds.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_GUEST_DYNAMIC_PHYSICAL_LIMIT UINT64_C(274877906944)
/** @} */

/**
 * @defgroup hyper_native_virtual_machine_architecture virtual_machine_architecture (enum)
 * @ingroup hyper_native
 * Architecture identifiers for guest construction.
 * @{
 */
/**
 * `virtual_machine_architecture_aarch64` in the `virtual_machine_architecture` enum domain.
 *
 * Architecture identifiers for guest construction.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_VIRTUAL_MACHINE_ARCHITECTURE_AARCH64 UINT64_C(1)
/**
 * `virtual_machine_architecture_riscv64` in the `virtual_machine_architecture` enum domain.
 *
 * Architecture identifiers for guest construction.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_VIRTUAL_MACHINE_ARCHITECTURE_RISCV64 UINT64_C(2)
/**
 * `virtual_machine_architecture_x86_64` in the `virtual_machine_architecture` enum domain.
 *
 * Architecture identifiers for guest construction.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_VIRTUAL_MACHINE_ARCHITECTURE_X86_64 UINT64_C(3)
/** @} */

/**
 * @defgroup hyper_native_virtual_machine_power virtual_machine_power (enum)
 * @ingroup hyper_native
 * Guest power-management requests.
 * @{
 */
/**
 * `virtual_machine_power_cpu_on` in the `virtual_machine_power` enum domain.
 *
 * Guest power-management requests.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_VIRTUAL_MACHINE_POWER_CPU_ON UINT64_C(1)
/**
 * `virtual_machine_power_cpu_off` in the `virtual_machine_power` enum domain.
 *
 * Guest power-management requests.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_VIRTUAL_MACHINE_POWER_CPU_OFF UINT64_C(2)
/**
 * `virtual_machine_power_system_off` in the `virtual_machine_power` enum domain.
 *
 * Guest power-management requests.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_VIRTUAL_MACHINE_POWER_SYSTEM_OFF UINT64_C(3)
/**
 * `virtual_machine_power_system_reset` in the `virtual_machine_power` enum domain.
 *
 * Guest power-management requests.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_VIRTUAL_MACHINE_POWER_SYSTEM_RESET UINT64_C(4)
/** @} */

/**
 * @defgroup hyper_native_virtual_machine_phase virtual_machine_phase (enum)
 * @ingroup hyper_native
 * Installed guest lifecycle phases.
 * @{
 */
/**
 * `virtual_machine_phase_installed` in the `virtual_machine_phase` enum domain.
 *
 * Installed guest lifecycle phases.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_VIRTUAL_MACHINE_PHASE_INSTALLED UINT64_C(1)
/**
 * `virtual_machine_phase_running` in the `virtual_machine_phase` enum domain.
 *
 * Installed guest lifecycle phases.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_VIRTUAL_MACHINE_PHASE_RUNNING UINT64_C(2)
/**
 * `virtual_machine_phase_stopping` in the `virtual_machine_phase` enum domain.
 *
 * Installed guest lifecycle phases.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_VIRTUAL_MACHINE_PHASE_STOPPING UINT64_C(3)
/**
 * `virtual_machine_phase_stopped` in the `virtual_machine_phase` enum domain.
 *
 * Installed guest lifecycle phases.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_VIRTUAL_MACHINE_PHASE_STOPPED UINT64_C(4)
/** @} */

/**
 * @defgroup hyper_native_virtual_cpu_phase virtual_cpu_phase (enum)
 * @ingroup hyper_native
 * Virtual CPU lifecycle phases.
 * @{
 */
/**
 * `virtual_cpu_phase_dormant` in the `virtual_cpu_phase` enum domain.
 *
 * Virtual CPU lifecycle phases.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_VIRTUAL_CPU_PHASE_DORMANT UINT64_C(1)
/**
 * `virtual_cpu_phase_started` in the `virtual_cpu_phase` enum domain.
 *
 * Virtual CPU lifecycle phases.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_VIRTUAL_CPU_PHASE_STARTED UINT64_C(2)
/**
 * `virtual_cpu_phase_stopped` in the `virtual_cpu_phase` enum domain.
 *
 * Virtual CPU lifecycle phases.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_VIRTUAL_CPU_PHASE_STOPPED UINT64_C(3)
/** @} */

/**
 * @defgroup hyper_native_virtual_cpu_terminal virtual_cpu_terminal (enum)
 * @ingroup hyper_native
 * Virtual CPU termination reasons.
 * @{
 */
/**
 * `virtual_cpu_terminal_none` in the `virtual_cpu_terminal` enum domain.
 *
 * Virtual CPU termination reasons.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_VIRTUAL_CPU_TERMINAL_NONE UINT64_C(0)
/**
 * `virtual_cpu_terminal_memory_fault` in the `virtual_cpu_terminal` enum domain.
 *
 * Virtual CPU termination reasons.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_VIRTUAL_CPU_TERMINAL_MEMORY_FAULT UINT64_C(1)
/**
 * `virtual_cpu_terminal_mmio` in the `virtual_cpu_terminal` enum domain.
 *
 * Virtual CPU termination reasons.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_VIRTUAL_CPU_TERMINAL_MMIO UINT64_C(2)
/**
 * `virtual_cpu_terminal_synchronous` in the `virtual_cpu_terminal` enum domain.
 *
 * Virtual CPU termination reasons.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_VIRTUAL_CPU_TERMINAL_SYNCHRONOUS UINT64_C(3)
/**
 * `virtual_cpu_terminal_administrative` in the `virtual_cpu_terminal` enum domain.
 *
 * Virtual CPU termination reasons.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_VIRTUAL_CPU_TERMINAL_ADMINISTRATIVE UINT64_C(4)
/** @} */

/**
 * @defgroup hyper_native_virtual_cpu_sentinels virtual_cpu_sentinels (constants)
 * @ingroup hyper_native
 * Sentinel for an unknown host CPU assignment.
 * @{
 */
/**
 * `vcpu_host_cpu_unknown` in the `virtual_cpu_sentinels` constants domain.
 *
 * Sentinel for an unknown host CPU assignment.
 * Numeric parameter or sentinel, not an enum discriminant.
 *
 * No host CPU assignment is available; do not treat this as a CPU index.
 */
#define HYPER_NATIVE_VCPU_HOST_CPU_UNKNOWN UINT64_C(4294967295)
/** @} */

/**
 * @defgroup hyper_native_virtual_platform virtual_platform (enum)
 * @ingroup hyper_native
 * Reference platform identifiers.
 * @{
 */
/**
 * `virtual_platform_aarch64_reference` in the `virtual_platform` enum domain.
 *
 * Reference platform identifiers.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE UINT64_C(1)
/**
 * `virtual_platform_riscv64_reference` in the `virtual_platform` enum domain.
 *
 * Reference platform identifiers.
 * Select one value; this is not a bit mask.
 */
#define HYPER_NATIVE_VIRTUAL_PLATFORM_RISCV64_REFERENCE UINT64_C(2)
/** @} */

/**
 * @defgroup hyper_native_virtual_platform_aarch64_layout virtual_platform_aarch64_layout (constants)
 * @ingroup hyper_native
 * Immutable `AArch64` reference platform geometry shared by kernel and userspace.
 * @{
 */
/**
 * `virtual_platform_aarch64_reference_max_vcpus` in the `virtual_platform_aarch64_layout` constants domain.
 *
 * Immutable `AArch64` reference platform geometry shared by kernel and userspace.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_MAX_VCPUS UINT64_C(8)
/**
 * `virtual_platform_aarch64_reference_guest_ram_base` in the `virtual_platform_aarch64_layout` constants domain.
 *
 * Immutable `AArch64` reference platform geometry shared by kernel and userspace.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_GUEST_RAM_BASE UINT64_C(1073741824)
/**
 * `virtual_platform_aarch64_reference_dtb_offset` in the `virtual_platform_aarch64_layout` constants domain.
 *
 * Immutable `AArch64` reference platform geometry shared by kernel and userspace.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_DTB_OFFSET UINT64_C(65536)
/**
 * `virtual_platform_aarch64_reference_gic_distributor_base` in the `virtual_platform_aarch64_layout` constants domain.
 *
 * Immutable `AArch64` reference platform geometry shared by kernel and userspace.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_GIC_DISTRIBUTOR_BASE UINT64_C(134217728)
/**
 * `virtual_platform_aarch64_reference_gic_distributor_size` in the `virtual_platform_aarch64_layout` constants domain.
 *
 * Immutable `AArch64` reference platform geometry shared by kernel and userspace.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_GIC_DISTRIBUTOR_SIZE UINT64_C(65536)
/**
 * `virtual_platform_aarch64_reference_gic_redistributor_base` in the `virtual_platform_aarch64_layout` constants domain.
 *
 * Immutable `AArch64` reference platform geometry shared by kernel and userspace.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_GIC_REDISTRIBUTOR_BASE UINT64_C(134873088)
/**
 * `virtual_platform_aarch64_reference_gic_redistributor_size` in the `virtual_platform_aarch64_layout` constants domain.
 *
 * Immutable `AArch64` reference platform geometry shared by kernel and userspace.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_GIC_REDISTRIBUTOR_SIZE UINT64_C(131072)
/**
 * `virtual_platform_aarch64_reference_gicv2_distributor_size` in the `virtual_platform_aarch64_layout` constants domain.
 *
 * Immutable `AArch64` reference platform geometry shared by kernel and userspace.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_GICV2_DISTRIBUTOR_SIZE UINT64_C(4096)
/**
 * `virtual_platform_aarch64_reference_gicv2_cpu_base` in the `virtual_platform_aarch64_layout` constants domain.
 *
 * Immutable `AArch64` reference platform geometry shared by kernel and userspace.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_GICV2_CPU_BASE UINT64_C(134283264)
/**
 * `virtual_platform_aarch64_reference_gicv2_cpu_size` in the `virtual_platform_aarch64_layout` constants domain.
 *
 * Immutable `AArch64` reference platform geometry shared by kernel and userspace.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_GICV2_CPU_SIZE UINT64_C(8192)
/**
 * `virtual_platform_aarch64_reference_uart_base` in the `virtual_platform_aarch64_layout` constants domain.
 *
 * Immutable `AArch64` reference platform geometry shared by kernel and userspace.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_UART_BASE UINT64_C(150994944)
/**
 * `virtual_platform_aarch64_reference_user_mmio_base` in the `virtual_platform_aarch64_layout` constants domain.
 *
 * Immutable `AArch64` reference platform geometry shared by kernel and userspace.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_USER_MMIO_BASE UINT64_C(167772160)
/**
 * `virtual_platform_aarch64_reference_user_mmio_size` in the `virtual_platform_aarch64_layout` constants domain.
 *
 * Immutable `AArch64` reference platform geometry shared by kernel and userspace.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_USER_MMIO_SIZE UINT64_C(16777216)
/**
 * `virtual_platform_aarch64_reference_uart_size` in the `virtual_platform_aarch64_layout` constants domain.
 *
 * Immutable `AArch64` reference platform geometry shared by kernel and userspace.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_UART_SIZE UINT64_C(4096)
/**
 * `virtual_platform_aarch64_reference_uart_baud` in the `virtual_platform_aarch64_layout` constants domain.
 *
 * Immutable `AArch64` reference platform geometry shared by kernel and userspace.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_UART_BAUD UINT64_C(115200)
/**
 * `virtual_platform_aarch64_reference_uart_interrupt` in the `virtual_platform_aarch64_layout` constants domain.
 *
 * Immutable `AArch64` reference platform geometry shared by kernel and userspace.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_UART_INTERRUPT UINT64_C(33)
/**
 * `virtual_platform_aarch64_reference_timer_interrupt` in the `virtual_platform_aarch64_layout` constants domain.
 *
 * Immutable `AArch64` reference platform geometry shared by kernel and userspace.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_TIMER_INTERRUPT UINT64_C(27)
/**
 * `virtual_platform_aarch64_reference_interrupt_count` in the `virtual_platform_aarch64_layout` constants domain.
 *
 * Immutable `AArch64` reference platform geometry shared by kernel and userspace.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_INTERRUPT_COUNT UINT64_C(256)
/**
 * `virtual_platform_aarch64_reference_device_interrupt_base` in the `virtual_platform_aarch64_layout` constants domain.
 *
 * Immutable `AArch64` reference platform geometry shared by kernel and userspace.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_DEVICE_INTERRUPT_BASE UINT64_C(40)
/** @} */

/**
 * @defgroup hyper_native_virtual_platform_riscv64_layout virtual_platform_riscv64_layout (constants)
 * @ingroup hyper_native
 * Immutable RISC-V reference platform geometry shared by kernel and userspace.
 * @{
 */
/**
 * `virtual_platform_riscv64_reference_guest_ram_base` in the `virtual_platform_riscv64_layout` constants domain.
 *
 * Immutable RISC-V reference platform geometry shared by kernel and userspace.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_VIRTUAL_PLATFORM_RISCV64_REFERENCE_GUEST_RAM_BASE UINT64_C(2147483648)
/**
 * `virtual_platform_riscv64_reference_dtb_offset` in the `virtual_platform_riscv64_layout` constants domain.
 *
 * Immutable RISC-V reference platform geometry shared by kernel and userspace.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_VIRTUAL_PLATFORM_RISCV64_REFERENCE_DTB_OFFSET UINT64_C(65536)
/**
 * `virtual_platform_riscv64_reference_uart_base` in the `virtual_platform_riscv64_layout` constants domain.
 *
 * Immutable RISC-V reference platform geometry shared by kernel and userspace.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_VIRTUAL_PLATFORM_RISCV64_REFERENCE_UART_BASE UINT64_C(268435456)
/**
 * `virtual_platform_riscv64_reference_uart_size` in the `virtual_platform_riscv64_layout` constants domain.
 *
 * Immutable RISC-V reference platform geometry shared by kernel and userspace.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_VIRTUAL_PLATFORM_RISCV64_REFERENCE_UART_SIZE UINT64_C(4096)
/**
 * `virtual_platform_riscv64_reference_uart_interrupt` in the `virtual_platform_riscv64_layout` constants domain.
 *
 * Immutable RISC-V reference platform geometry shared by kernel and userspace.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_VIRTUAL_PLATFORM_RISCV64_REFERENCE_UART_INTERRUPT UINT64_C(10)
/**
 * `virtual_platform_riscv64_reference_uart_clock_frequency` in the `virtual_platform_riscv64_layout` constants domain.
 *
 * Immutable RISC-V reference platform geometry shared by kernel and userspace.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_VIRTUAL_PLATFORM_RISCV64_REFERENCE_UART_CLOCK_FREQUENCY UINT64_C(3686400)
/**
 * `virtual_platform_riscv64_reference_plic_base` in the `virtual_platform_riscv64_layout` constants domain.
 *
 * Immutable RISC-V reference platform geometry shared by kernel and userspace.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_VIRTUAL_PLATFORM_RISCV64_REFERENCE_PLIC_BASE UINT64_C(201326592)
/**
 * `virtual_platform_riscv64_reference_plic_size` in the `virtual_platform_riscv64_layout` constants domain.
 *
 * Immutable RISC-V reference platform geometry shared by kernel and userspace.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_VIRTUAL_PLATFORM_RISCV64_REFERENCE_PLIC_SIZE UINT64_C(4194304)
/**
 * `virtual_platform_riscv64_reference_plic_num_sources` in the `virtual_platform_riscv64_layout` constants domain.
 *
 * Immutable RISC-V reference platform geometry shared by kernel and userspace.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_VIRTUAL_PLATFORM_RISCV64_REFERENCE_PLIC_NUM_SOURCES UINT64_C(31)
/**
 * `virtual_platform_riscv64_reference_plic_supervisor_context` in the `virtual_platform_riscv64_layout` constants domain.
 *
 * Immutable RISC-V reference platform geometry shared by kernel and userspace.
 * Numeric parameter or sentinel, not an enum discriminant.
 */
#define HYPER_NATIVE_VIRTUAL_PLATFORM_RISCV64_REFERENCE_PLIC_SUPERVISOR_CONTEXT UINT64_C(1)
/** @} */

/**
 * @defgroup hyper_native_riscv_isa riscv_isa (flags)
 * @ingroup hyper_native
 * RISC-V ISA extensions accepted in guest configuration.
 * @{
 */
/**
 * `riscv_isa_i` in the `riscv_isa` flags domain.
 *
 * RISC-V ISA extensions accepted in guest configuration.
 * Independent flag bit; combine permitted members with bitwise OR.
 */
#define HYPER_NATIVE_RISCV_ISA_I UINT64_C(1)
/**
 * `riscv_isa_m` in the `riscv_isa` flags domain.
 *
 * RISC-V ISA extensions accepted in guest configuration.
 * Independent flag bit; combine permitted members with bitwise OR.
 */
#define HYPER_NATIVE_RISCV_ISA_M UINT64_C(2)
/**
 * `riscv_isa_a` in the `riscv_isa` flags domain.
 *
 * RISC-V ISA extensions accepted in guest configuration.
 * Independent flag bit; combine permitted members with bitwise OR.
 */
#define HYPER_NATIVE_RISCV_ISA_A UINT64_C(4)
/**
 * `riscv_isa_f` in the `riscv_isa` flags domain.
 *
 * RISC-V ISA extensions accepted in guest configuration.
 * Independent flag bit; combine permitted members with bitwise OR.
 */
#define HYPER_NATIVE_RISCV_ISA_F UINT64_C(8)
/**
 * `riscv_isa_d` in the `riscv_isa` flags domain.
 *
 * RISC-V ISA extensions accepted in guest configuration.
 * Independent flag bit; combine permitted members with bitwise OR.
 */
#define HYPER_NATIVE_RISCV_ISA_D UINT64_C(16)
/**
 * `riscv_isa_c` in the `riscv_isa` flags domain.
 *
 * RISC-V ISA extensions accepted in guest configuration.
 * Independent flag bit; combine permitted members with bitwise OR.
 */
#define HYPER_NATIVE_RISCV_ISA_C UINT64_C(32)
/**
 * `riscv_isa_zicsr` in the `riscv_isa` flags domain.
 *
 * RISC-V ISA extensions accepted in guest configuration.
 * Independent flag bit; combine permitted members with bitwise OR.
 */
#define HYPER_NATIVE_RISCV_ISA_ZICSR UINT64_C(64)
/**
 * `riscv_isa_zifencei` in the `riscv_isa` flags domain.
 *
 * RISC-V ISA extensions accepted in guest configuration.
 * Independent flag bit; combine permitted members with bitwise OR.
 */
#define HYPER_NATIVE_RISCV_ISA_ZIFENCEI UINT64_C(128)
/**
 * `riscv_isa_sstc` in the `riscv_isa` flags domain.
 *
 * RISC-V ISA extensions accepted in guest configuration.
 * Independent flag bit; combine permitted members with bitwise OR.
 */
#define HYPER_NATIVE_RISCV_ISA_SSTC UINT64_C(256)
/** @} */

/**
 * Object-local method `abi_query`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `receiver: Borrow, kind=system_info, rights=0x10` |
 * | Operation | 256 |
 * | Feature | `Core` |
 * | Arguments | — |
 * | Results | `revision: u64`, `features: u64` |
 * | Capability effects | `receiver: Borrow, kind=system_info, rights=0x10` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Abi` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_ABI_QUERY UINT64_C(256)

/**
 * Object-local method `handle_close`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `handle: ConsumeOnCommit, any, rights=0x0` |
 * | Operation | 0 |
 * | Feature | `Core` |
 * | Arguments | — |
 * | Results | — |
 * | Capability effects | `handle: ConsumeOnCommit, any, rights=0x0` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_HANDLE_CLOSE UINT64_C(0)

/**
 * Object-local method `handle_duplicate`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `source: Borrow, any, rights=0x1` |
 * | Operation | 1 |
 * | Feature | `Core` |
 * | Arguments | `requested_rights: rights` |
 * | Results | `handle: handle` |
 * | Capability effects | `source: Borrow, any, rights=0x1`, `handle: produce, same-as(source), subset-from(requested_rights)` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_HANDLE_DUPLICATE UINT64_C(1)

/**
 * Object-local method `handle_replace`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `source: ConsumeOnCommit, any, rights=0x0` |
 * | Operation | 2 |
 * | Feature | `Core` |
 * | Arguments | `requested_rights: rights` |
 * | Results | `handle: handle` |
 * | Capability effects | `source: ConsumeOnCommit, any, rights=0x0`, `handle: produce, same-as(source), subset-from(requested_rights)` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_HANDLE_REPLACE UINT64_C(2)

/**
 * Object-local method `handle_get_info`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `handle: Borrow, any, rights=0x0` |
 * | Operation | 3 |
 * | Feature | `Core` |
 * | Arguments | `output: user_address`, `output_size: byte_count` |
 * | Results | `supported_size: byte_count` |
 * | Capability effects | `handle: Borrow, any, rights=0x0` |
 * | User memory | `output: Write, len=output_size bytes, max-bytes=4096, record=handle_info; order=0` |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_HANDLE_GET_INFO UINT64_C(3)

/**
 * Object-local method `object_get_basic_info`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `handle: Borrow, any, rights=0x8` |
 * | Operation | 4 |
 * | Feature | `Core` |
 * | Arguments | `output: user_address`, `output_size: byte_count` |
 * | Results | `supported_size: byte_count` |
 * | Capability effects | `handle: Borrow, any, rights=0x8` |
 * | User memory | `output: Write, len=output_size bytes, max-bytes=4096, record=object_basic_info; order=0` |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Object` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_OBJECT_GET_BASIC_INFO UINT64_C(4)

/**
 * Object-local method `thread_yield`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `receiver: Borrow, kind=execution_context, rights=0x20` |
 * | Operation | 256 |
 * | Feature | `Core` |
 * | Arguments | — |
 * | Results | — |
 * | Capability effects | `receiver: Borrow, kind=execution_context, rights=0x20` |
 * | User memory | — |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Task` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_THREAD_YIELD UINT64_C(256)

/**
 * Object-local method `thread_exit`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `receiver: Borrow, kind=execution_context, rights=0x800` |
 * | Operation | 257 |
 * | Feature | `Core` |
 * | Arguments | `status: i64` |
 * | Results | — |
 * | Capability effects | `receiver: Borrow, kind=execution_context, rights=0x800` |
 * | User memory | — |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=NoReturn, flags=None` |
 * | Audit | `Task` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_THREAD_EXIT UINT64_C(257)

/**
 * Object-local method `process_exit`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `receiver: Borrow, kind=execution_context, rights=0x800` |
 * | Operation | 258 |
 * | Feature | `Core` |
 * | Arguments | `status: i64` |
 * | Results | — |
 * | Capability effects | `receiver: Borrow, kind=execution_context, rights=0x800` |
 * | User memory | — |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=NoReturn, flags=None` |
 * | Audit | `Task` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_PROCESS_EXIT UINT64_C(258)

/**
 * Object-local method `event_create`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `receiver: Borrow, kind=object_factory, rights=0x8000000000` |
 * | Operation | 256 |
 * | Feature | `Core` |
 * | Arguments | `options: u32` |
 * | Results | `handle: handle` |
 * | Capability effects | `receiver: Borrow, kind=object_factory, rights=0x8000000000`, `handle: produce, kind=event, fixed=0x8000f` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
 * | Audit | `Object` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_EVENT_CREATE UINT64_C(256)

/**
 * Object-local method `event_signal`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `event: Borrow, kind=event, rights=0x80000` |
 * | Operation | 256 |
 * | Feature | `Core` |
 * | Arguments | `clear_mask: u64`, `set_mask: u64` |
 * | Results | — |
 * | Capability effects | `event: Borrow, kind=event, rights=0x80000` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
 * | Audit | `Object` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_EVENT_SIGNAL UINT64_C(256)

/**
 * Object-local method `object_wait_one`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `object: Borrow, any, rights=0x4` |
 * | Operation | 5 |
 * | Feature | `Core` |
 * | Arguments | `signals: u64`, `deadline: u64` |
 * | Results | `observed: u64` |
 * | Capability effects | `object: Borrow, any, rights=0x4` |
 * | User memory | — |
 * | Execution | `blocking=MayBlock, cancellation=Explicit, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Object` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_OBJECT_WAIT_ONE UINT64_C(5)

/**
 * Object-local method `byte_channel_create`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `receiver: Borrow, kind=object_factory, rights=0x8000000000` |
 * | Operation | 257 |
 * | Feature | `Core` |
 * | Arguments | `options: u32` |
 * | Results | `endpoint0: handle`, `endpoint1: handle` |
 * | Capability effects | `receiver: Borrow, kind=object_factory, rights=0x8000000000`, `endpoint0: produce, kind=byte_channel, fixed=0x3f`, `endpoint1: produce, kind=byte_channel, fixed=0x3f` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
 * | Audit | `Object` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_BYTE_CHANNEL_CREATE UINT64_C(257)

/**
 * Object-local method `byte_channel_write`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `endpoint: Borrow, kind=byte_channel, rights=0x20` |
 * | Operation | 256 |
 * | Feature | `Core` |
 * | Arguments | `options: u32`, `bytes: user_address`, `byte_count: byte_count` |
 * | Results | — |
 * | Capability effects | `endpoint: Borrow, kind=byte_channel, rights=0x20` |
 * | User memory | `bytes: Read, len=byte_count bytes, max-bytes=65536; order=0` |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_BYTE_CHANNEL_WRITE UINT64_C(256)

/**
 * Object-local method `byte_channel_read`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `endpoint: Borrow, kind=byte_channel, rights=0x10` |
 * | Operation | 257 |
 * | Feature | `Core` |
 * | Arguments | `options: u32`, `bytes: user_address`, `byte_capacity: byte_count` |
 * | Results | `actual_bytes: byte_count; also-on=buffer_too_small` |
 * | Capability effects | `endpoint: Borrow, kind=byte_channel, rights=0x10` |
 * | User memory | `bytes: Write, len=byte_capacity bytes, max-bytes=65536; order=0` |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_BYTE_CHANNEL_READ UINT64_C(257)

/**
 * Object-local method `console_read`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `console: Borrow, kind=console, rights=0x10` |
 * | Operation | 256 |
 * | Feature | `Core` |
 * | Arguments | `options: u32`, `bytes: user_address`, `byte_capacity: byte_count` |
 * | Results | `actual_bytes: byte_count; also-on=would_block` |
 * | Capability effects | `console: Borrow, kind=console, rights=0x10` |
 * | User memory | `bytes: Write, len=byte_capacity bytes, max-bytes=4096; order=0` |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_CONSOLE_READ UINT64_C(256)

/**
 * Object-local method `console_write`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `console: Borrow, kind=console, rights=0x20` |
 * | Operation | 257 |
 * | Feature | `Core` |
 * | Arguments | `options: u32`, `bytes: user_address`, `byte_count: byte_count` |
 * | Results | `actual_bytes: byte_count; also-on=would_block` |
 * | Capability effects | `console: Borrow, kind=console, rights=0x20` |
 * | User memory | `bytes: Read, len=byte_count bytes, max-bytes=4096; order=0` |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_CONSOLE_WRITE UINT64_C(257)

/**
 * Object-local method `directory_open_file`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `directory: Borrow, kind=directory, rights=0x10` |
 * | Operation | 256 |
 * | Feature | `Core` |
 * | Arguments | `path: user_address`, `path_size: byte_count`, `requested_rights: rights`, `options: u32` |
 * | Results | `file: handle` |
 * | Capability effects | `directory: Borrow, kind=directory, rights=0x10`, `file: produce, kind=file, exact-from(requested_rights), allowed=0x9800000bb, required-from=directory` |
 * | User memory | `path: Read, len=path_size bytes, max-bytes=4096; order=0` |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_DIRECTORY_OPEN_FILE UINT64_C(256)

/**
 * Object-local method `file_read_at`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `file: Borrow, kind=file, rights=0x10` |
 * | Operation | 256 |
 * | Feature | `Core` |
 * | Arguments | `options: u32`, `offset: u64`, `output: user_address`, `output_capacity: byte_count` |
 * | Results | `actual_bytes: byte_count`, `file_size: byte_count` |
 * | Capability effects | `file: Borrow, kind=file, rights=0x10` |
 * | User memory | `output: Write, len=output_capacity bytes, max-bytes=2097152; order=0` |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_FILE_READ_AT UINT64_C(256)

/**
 * Object-local method `capability_channel_create`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `receiver: Borrow, kind=object_factory, rights=0x8000000000` |
 * | Operation | 258 |
 * | Feature | `Core` |
 * | Arguments | `options: u32` |
 * | Results | `endpoint0: handle`, `endpoint1: handle` |
 * | Capability effects | `receiver: Borrow, kind=object_factory, rights=0x8000000000`, `endpoint0: produce, kind=capability_channel, fixed=0x3f`, `endpoint1: produce, kind=capability_channel, fixed=0x3f` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
 * | Audit | `Object` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_CAPABILITY_CHANNEL_CREATE UINT64_C(258)

/**
 * Object-local method `capability_channel_try_send`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `endpoint: Borrow, kind=capability_channel, rights=0x20` |
 * | Operation | 256 |
 * | Feature | `Core` |
 * | Arguments | `options: u32`, `bytes: user_address`, `byte_count: byte_count`, `dispositions: user_address`, `disposition_count: element_count` |
 * | Results | — |
 * | Capability effects | `endpoint: Borrow, kind=capability_channel, rights=0x20` |
 * | User memory | `bytes: Read, len=byte_count bytes, max-bytes=4096; order=0`, `dispositions: Read, len=disposition_count elements, max-elements=16, element-size=24, record=capability_disposition, transactional-handles=(handle, rights, expected_kind, operation), common-rights=0x2, operations=[move=0:ConsumeOnCommit/rights=0x2+duplicate=1:Borrow/rights=0x3], commit=AtomicOnOk; order=1` |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_CAPABILITY_CHANNEL_TRY_SEND UINT64_C(256)

/**
 * Object-local method `capability_channel_receive`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `endpoint: Borrow, kind=capability_channel, rights=0x10` |
 * | Operation | 257 |
 * | Feature | `Core` |
 * | Arguments | `deadline: u64`, `bytes: user_address`, `byte_capacity: byte_count`, `capability_slots: user_address`, `slot_count: element_count` |
 * | Results | `actual_bytes: byte_count; also-on=buffer_too_small`, `actual_capabilities: element_count; also-on=buffer_too_small` |
 * | Capability effects | `endpoint: Borrow, kind=capability_channel, rights=0x10` |
 * | User memory | `bytes: Write, len=byte_capacity bytes, max-bytes=4096; order=0`, `capability_slots: ReadWrite, len=slot_count elements, max-elements=16, element-size=24, record=capability_receive_slot, typed-receive-slots=(handle, rights, expected_kind, flags), produce-transferred-handles, commit=AtomicOnOk; order=1` |
 * | Execution | `blocking=MayBlock, cancellation=Explicit, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_CAPABILITY_CHANNEL_RECEIVE UINT64_C(257)

/**
 * Object-local method `process_builder_create`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `factory: Borrow, kind=task_factory, rights=0x100000` |
 * | Operation | 256 |
 * | Feature | `Core` |
 * | Arguments | `group: handle`, `domain: handle`, `executable: handle`, `loader: handle` |
 * | Results | `builder: handle` |
 * | Capability effects | `factory: Borrow, kind=task_factory, rights=0x100000`, `group: Borrow, kind=task_group, rights=0x4000000`, `domain: Borrow, kind=resource_domain, rights=0x8000000`, `executable: Borrow, kind=file, rights=0x800000093`, `loader: Borrow, kind=file, rights=0x90`, `builder: produce, kind=process_builder, fixed=0x10000000c2a, inherited-from(factory), mask=0xe3000000000` |
 * | User memory | — |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Task` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_PROCESS_BUILDER_CREATE UINT64_C(256)

/**
 * Object-local method `process_builder_set_name`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `builder: Borrow, kind=process_builder, rights=0x20` |
 * | Operation | 256 |
 * | Feature | `Core` |
 * | Arguments | `name: user_address`, `name_size: byte_count` |
 * | Results | — |
 * | Capability effects | `builder: Borrow, kind=process_builder, rights=0x20` |
 * | User memory | `name: Read, len=name_size bytes, max-bytes=64; order=0` |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Task` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_PROCESS_BUILDER_SET_NAME UINT64_C(256)

/**
 * Object-local method `process_builder_set_data`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `builder: Borrow, kind=process_builder, rights=0x20` |
 * | Operation | 257 |
 * | Feature | `Core` |
 * | Arguments | `data: user_address`, `data_size: byte_count` |
 * | Results | — |
 * | Capability effects | `builder: Borrow, kind=process_builder, rights=0x20` |
 * | User memory | `data: Read, len=data_size bytes, max-bytes=16384; order=0` |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Task` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_PROCESS_BUILDER_SET_DATA UINT64_C(257)

/**
 * Object-local method `process_builder_set_affinity`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `builder: Borrow, kind=process_builder, rights=0x20` |
 * | Operation | 258 |
 * | Feature | `Core` |
 * | Arguments | `affinity_words: user_address`, `word_count: element_count` |
 * | Results | — |
 * | Capability effects | `builder: Borrow, kind=process_builder, rights=0x20` |
 * | User memory | `affinity_words: Read, len=word_count elements, max-elements=4, element-size=8; order=0` |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Task` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_PROCESS_BUILDER_SET_AFFINITY UINT64_C(258)

/**
 * Object-local method `process_builder_add_handle`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `builder: Borrow, kind=process_builder, rights=0x20` |
 * | Operation | 259 |
 * | Feature | `Core` |
 * | Arguments | `source: handle`, `purpose: u32`, `expected_kind: u32`, `rights: rights`, `operation: u32` |
 * | Results | — |
 * | Capability effects | `builder: Borrow, kind=process_builder, rights=0x20`, `source: ByOperation(operation: move=0:ConsumeOnCommit/rights=0x2+duplicate=1:Borrow/rights=0x3), any, common-rights=0x2` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_PROCESS_BUILDER_ADD_HANDLE UINT64_C(259)

/**
 * Object-local method `process_builder_seal`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `builder: Borrow, kind=process_builder, rights=0x20` |
 * | Operation | 260 |
 * | Feature | `Core` |
 * | Arguments | — |
 * | Results | — |
 * | Capability effects | `builder: Borrow, kind=process_builder, rights=0x20` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Task` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_PROCESS_BUILDER_SEAL UINT64_C(260)

/**
 * Object-local method `process_builder_start`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `builder: ConsumeOnCommit, kind=process_builder, rights=0x400` |
 * | Operation | 261 |
 * | Feature | `Core` |
 * | Arguments | — |
 * | Results | `process: handle`, `startup_channel: handle` |
 * | Capability effects | `builder: ConsumeOnCommit, kind=process_builder, rights=0x400`, `process: produce, kind=process, fixed=0x80e`, `startup_channel: produce, kind=byte_channel, fixed=0x14` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Task` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_PROCESS_BUILDER_START UINT64_C(261)

/**
 * Object-local method `process_builder_abort`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `builder: ConsumeOnCommit, kind=process_builder, rights=0x800` |
 * | Operation | 262 |
 * | Feature | `Core` |
 * | Arguments | — |
 * | Results | — |
 * | Capability effects | `builder: ConsumeOnCommit, kind=process_builder, rights=0x800` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Task` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_PROCESS_BUILDER_ABORT UINT64_C(262)

/**
 * Object-local method `process_request_stop`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `process: Borrow, kind=process, rights=0x800` |
 * | Operation | 256 |
 * | Feature | `Core` |
 * | Arguments | — |
 * | Results | — |
 * | Capability effects | `process: Borrow, kind=process, rights=0x800` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Task` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_PROCESS_REQUEST_STOP UINT64_C(256)

/**
 * Object-local method `object_wait_many`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `receiver: Borrow, kind=execution_context, rights=0x20` |
 * | Operation | 259 |
 * | Feature | `Core` |
 * | Arguments | `items: user_address`, `item_count: element_count`, `deadline: u64` |
 * | Results | `index: element_count`, `observed: u64` |
 * | Capability effects | `receiver: Borrow, kind=execution_context, rights=0x20` |
 * | User memory | `items: Read, len=item_count elements, max-elements=64, element-size=16, record=object_wait_item, borrowed-handles=(handle), required-rights=0x4; order=0` |
 * | Execution | `blocking=MayBlock, cancellation=Explicit, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Object` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_OBJECT_WAIT_MANY UINT64_C(259)

/**
 * Object-local method `process_get_info`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `process: Borrow, kind=process, rights=0x8` |
 * | Operation | 257 |
 * | Feature | `Core` |
 * | Arguments | `info: user_address`, `info_size: byte_count` |
 * | Results | `supported_size: byte_count` |
 * | Capability effects | `process: Borrow, kind=process, rights=0x8` |
 * | User memory | `info: Write, len=info_size bytes, max-bytes=4096, record=process_info; order=0` |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Task` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_PROCESS_GET_INFO UINT64_C(257)

/**
 * Object-local method `task_inspector_scan_processes`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `inspector: Borrow, kind=task_inspector, rights=0x8` |
 * | Operation | 256 |
 * | Feature | `Core` |
 * | Arguments | `cursor: u64`, `records: user_address`, `capacity: element_count` |
 * | Results | `count: element_count`, `next_cursor: u64` |
 * | Capability effects | `inspector: Borrow, kind=task_inspector, rights=0x8` |
 * | User memory | `records: Write, len=capacity elements, max-elements=8, element-size=96, record=task_process; order=0` |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Task` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_TASK_INSPECTOR_SCAN_PROCESSES UINT64_C(256)

/**
 * Object-local method `task_inspector_scan_threads`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `inspector: Borrow, kind=task_inspector, rights=0x8` |
 * | Operation | 257 |
 * | Feature | `Core` |
 * | Arguments | `cursor: u64`, `records: user_address`, `capacity: element_count` |
 * | Results | `count: element_count`, `next_cursor: u64` |
 * | Capability effects | `inspector: Borrow, kind=task_inspector, rights=0x8` |
 * | User memory | `records: Write, len=capacity elements, max-elements=8, element-size=104, record=task_thread; order=0` |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Task` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_TASK_INSPECTOR_SCAN_THREADS UINT64_C(257)

/**
 * Object-local method `task_inspector_derive_process`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `inspector: Borrow, kind=task_inspector, rights=0x1000000b` |
 * | Operation | 258 |
 * | Feature | `Core` |
 * | Arguments | `process: handle` |
 * | Results | `inspector: handle` |
 * | Capability effects | `inspector: Borrow, kind=task_inspector, rights=0x1000000b`, `process: Borrow, kind=process, rights=0x8`, `inspector: produce, kind=task_inspector, fixed=0x1000000b` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Task` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_TASK_INSPECTOR_DERIVE_PROCESS UINT64_C(258)

/**
 * Object-local method `object_inspector_scan_objects`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `inspector: Borrow, kind=object_inspector, rights=0x8` |
 * | Operation | 256 |
 * | Feature | `Core` |
 * | Arguments | `cursor: u64`, `records: user_address`, `capacity: element_count` |
 * | Results | `count: element_count`, `next_cursor: u64` |
 * | Capability effects | `inspector: Borrow, kind=object_inspector, rights=0x8` |
 * | User memory | `records: Write, len=capacity elements, max-elements=8, element-size=104, record=object_inspection; order=0` |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Object` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_OBJECT_INSPECTOR_SCAN_OBJECTS UINT64_C(256)

/**
 * Object-local method `object_inspector_scan_handles`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `inspector: Borrow, kind=object_inspector, rights=0x8` |
 * | Operation | 257 |
 * | Feature | `Core` |
 * | Arguments | `process_koid: u64`, `cursor: u64`, `records: user_address`, `capacity: element_count` |
 * | Results | `count: element_count`, `next_cursor: u64` |
 * | Capability effects | `inspector: Borrow, kind=object_inspector, rights=0x8` |
 * | User memory | `records: Write, len=capacity elements, max-elements=8, element-size=40, record=handle_inspection; order=0` |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Object` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_OBJECT_INSPECTOR_SCAN_HANDLES UINT64_C(257)

/**
 * Object-local method `object_inspector_derive_process`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `inspector: Borrow, kind=object_inspector, rights=0x1000000b` |
 * | Operation | 258 |
 * | Feature | `Core` |
 * | Arguments | `process: handle` |
 * | Results | `inspector: handle` |
 * | Capability effects | `inspector: Borrow, kind=object_inspector, rights=0x1000000b`, `process: Borrow, kind=process, rights=0x8`, `inspector: produce, kind=object_inspector, fixed=0x1000000b` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Object` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_OBJECT_INSPECTOR_DERIVE_PROCESS UINT64_C(258)

/**
 * Object-local method `task_inspector_derive_task_group`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `inspector: Borrow, kind=task_inspector, rights=0x1000000b` |
 * | Operation | 259 |
 * | Feature | `Core` |
 * | Arguments | `task_group: handle` |
 * | Results | `inspector: handle` |
 * | Capability effects | `inspector: Borrow, kind=task_inspector, rights=0x1000000b`, `task_group: Borrow, kind=task_group, rights=0x8`, `inspector: produce, kind=task_inspector, fixed=0x1000000b` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Task` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_TASK_INSPECTOR_DERIVE_TASK_GROUP UINT64_C(259)

/**
 * Object-local method `object_inspector_derive_task_group`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `inspector: Borrow, kind=object_inspector, rights=0x1000000b` |
 * | Operation | 259 |
 * | Feature | `Core` |
 * | Arguments | `task_group: handle` |
 * | Results | `inspector: handle` |
 * | Capability effects | `inspector: Borrow, kind=object_inspector, rights=0x1000000b`, `task_group: Borrow, kind=task_group, rights=0x8`, `inspector: produce, kind=object_inspector, fixed=0x1000000b` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Object` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_OBJECT_INSPECTOR_DERIVE_TASK_GROUP UINT64_C(259)

/**
 * Object-local method `task_inspector_derive_resource_domain`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `inspector: Borrow, kind=task_inspector, rights=0x1000000b` |
 * | Operation | 260 |
 * | Feature | `Core` |
 * | Arguments | `resource_domain: handle` |
 * | Results | `inspector: handle` |
 * | Capability effects | `inspector: Borrow, kind=task_inspector, rights=0x1000000b`, `resource_domain: Borrow, kind=resource_domain, rights=0x8`, `inspector: produce, kind=task_inspector, fixed=0x1000000b` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Task` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_TASK_INSPECTOR_DERIVE_RESOURCE_DOMAIN UINT64_C(260)

/**
 * Object-local method `object_inspector_derive_resource_domain`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `inspector: Borrow, kind=object_inspector, rights=0x1000000b` |
 * | Operation | 260 |
 * | Feature | `Core` |
 * | Arguments | `resource_domain: handle` |
 * | Results | `inspector: handle` |
 * | Capability effects | `inspector: Borrow, kind=object_inspector, rights=0x1000000b`, `resource_domain: Borrow, kind=resource_domain, rights=0x8`, `inspector: produce, kind=object_inspector, fixed=0x1000000b` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Object` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_OBJECT_INSPECTOR_DERIVE_RESOURCE_DOMAIN UINT64_C(260)

/**
 * Object-local method `directory_open_directory`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `directory: Borrow, kind=directory, rights=0x10` |
 * | Operation | 257 |
 * | Feature | `Core` |
 * | Arguments | `path: user_address`, `path_size: byte_count`, `requested_rights: rights`, `options: u32` |
 * | Results | `child: handle` |
 * | Capability effects | `directory: Borrow, kind=directory, rights=0x10`, `child: produce, kind=directory, exact-from(requested_rights), allowed=0x9800000bb, required-from=directory` |
 * | User memory | `path: Read, len=path_size bytes, max-bytes=4096; order=0` |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_DIRECTORY_OPEN_DIRECTORY UINT64_C(257)

/**
 * Object-local method `vmo_create`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `receiver: Borrow, kind=object_factory, rights=0x8000000000` |
 * | Operation | 259 |
 * | Feature | `Core` |
 * | Arguments | `size: byte_count` |
 * | Results | `vmo: handle` |
 * | Capability effects | `receiver: Borrow, kind=object_factory, rights=0x8000000000`, `vmo: produce, kind=vmo, fixed=0x7b` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Object` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_VMO_CREATE UINT64_C(259)

/**
 * Object-local method `vmo_read`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `vmo: Borrow, kind=vmo, rights=0x10` |
 * | Operation | 256 |
 * | Feature | `Core` |
 * | Arguments | `offset: u64`, `bytes: user_address`, `byte_count: byte_count` |
 * | Results | — |
 * | Capability effects | `vmo: Borrow, kind=vmo, rights=0x10` |
 * | User memory | `bytes: Write, len=byte_count bytes, max-bytes=65536; order=0` |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_VMO_READ UINT64_C(256)

/**
 * Object-local method `vmo_write`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `vmo: Borrow, kind=vmo, rights=0x20` |
 * | Operation | 257 |
 * | Feature | `Core` |
 * | Arguments | `offset: u64`, `bytes: user_address`, `byte_count: byte_count` |
 * | Results | — |
 * | Capability effects | `vmo: Borrow, kind=vmo, rights=0x20` |
 * | User memory | `bytes: Read, len=byte_count bytes, max-bytes=65536; order=0` |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_VMO_WRITE UINT64_C(257)

/**
 * Object-local method `vmar_allocate`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `vmar: Borrow, kind=vmar, rights=0x40` |
 * | Operation | 256 |
 * | Feature | `Core` |
 * | Arguments | `address: u64`, `size: byte_count`, `options: u64` |
 * | Results | `child: handle`, `address: u64` |
 * | Capability effects | `vmar: Borrow, kind=vmar, rights=0x40`, `child: produce, kind=vmar, fixed=0x4b, inherited-from(vmar), mask=0x1000000000` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_VMAR_ALLOCATE UINT64_C(256)

/**
 * Object-local method `vmar_map`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `vmar: Borrow, kind=vmar, rights=0x40` |
 * | Operation | 257 |
 * | Feature | `Core` |
 * | Arguments | `vmo: handle`, `vmo_offset: u64`, `address: u64`, `size: byte_count`, `permissions: u32` |
 * | Results | — |
 * | Capability effects | `vmar: Borrow, kind=vmar, rights=0x40`, `vmo: Borrow, kind=vmo, rights=0x40` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_VMAR_MAP UINT64_C(257)

/**
 * Object-local method `vmar_protect`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `vmar: Borrow, kind=vmar, rights=0x40` |
 * | Operation | 258 |
 * | Feature | `Core` |
 * | Arguments | `address: u64`, `size: byte_count`, `permissions: u32` |
 * | Results | — |
 * | Capability effects | `vmar: Borrow, kind=vmar, rights=0x40` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_VMAR_PROTECT UINT64_C(258)

/**
 * Object-local method `vmar_unmap`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `vmar: Borrow, kind=vmar, rights=0x40` |
 * | Operation | 259 |
 * | Feature | `Core` |
 * | Arguments | `address: u64`, `size: byte_count` |
 * | Results | — |
 * | Capability effects | `vmar: Borrow, kind=vmar, rights=0x40` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_VMAR_UNMAP UINT64_C(259)

/**
 * Object-local method `vmar_destroy`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `vmar: ConsumeOnCommit, kind=vmar, rights=0x40` |
 * | Operation | 260 |
 * | Feature | `Core` |
 * | Arguments | — |
 * | Results | — |
 * | Capability effects | `vmar: ConsumeOnCommit, kind=vmar, rights=0x40` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_VMAR_DESTROY UINT64_C(260)

/**
 * Object-local method `directory_read`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `stream: Borrow, kind=directory_stream, rights=0x10` |
 * | Operation | 256 |
 * | Feature | `Core` |
 * | Arguments | `cookie: u64`, `records: user_address`, `capacity: element_count`, `options: u32` |
 * | Results | `count: element_count`, `next_cookie: u64` |
 * | Capability effects | `stream: Borrow, kind=directory_stream, rights=0x10` |
 * | User memory | `records: Write, len=capacity elements, max-elements=4, element-size=280, record=directory_entry; order=0` |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_DIRECTORY_READ UINT64_C(256)

/**
 * Object-local method `memory_inspector_read`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `inspector: Borrow, kind=memory_inspector, rights=0x8` |
 * | Operation | 256 |
 * | Feature | `Core` |
 * | Arguments | `observation: user_address`, `observation_size: byte_count` |
 * | Results | `supported_size: byte_count` |
 * | Capability effects | `inspector: Borrow, kind=memory_inspector, rights=0x8` |
 * | User memory | `observation: Write, len=observation_size bytes, max-bytes=4096, record=memory_observation; order=0` |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Object` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_MEMORY_INSPECTOR_READ UINT64_C(256)

/**
 * Object-local method `cpu_inspector_profile_start`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `inspector: Borrow, kind=cpu_inspector, rights=0x80000000` |
 * | Operation | 257 |
 * | Feature | `Core` |
 * | Arguments | `period: u64`, `seconds: u64` |
 * | Results | `generation: u64` |
 * | Capability effects | `inspector: Borrow, kind=cpu_inspector, rights=0x80000000` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Object` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_CPU_INSPECTOR_PROFILE_START UINT64_C(257)

/**
 * Object-local method `cpu_inspector_profile_stop`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `inspector: Borrow, kind=cpu_inspector, rights=0x80000000` |
 * | Operation | 258 |
 * | Feature | `Core` |
 * | Arguments | `generation: u64` |
 * | Results | — |
 * | Capability effects | `inspector: Borrow, kind=cpu_inspector, rights=0x80000000` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Object` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_CPU_INSPECTOR_PROFILE_STOP UINT64_C(258)

/**
 * Object-local method `cpu_inspector_profile_read`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `inspector: Borrow, kind=cpu_inspector, rights=0x200000000` |
 * | Operation | 259 |
 * | Feature | `Core` |
 * | Arguments | `generation: u64`, `cpu: u64`, `cursor: u64`, `output: user_address`, `output_size: byte_count` |
 * | Results | `supported_size: byte_count` |
 * | Capability effects | `inspector: Borrow, kind=cpu_inspector, rights=0x200000000` |
 * | User memory | `output: Write, len=output_size bytes, max-bytes=192; order=0` |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Object` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_CPU_INSPECTOR_PROFILE_READ UINT64_C(259)

/**
 * Object-local method `cpu_inspector_trace_start`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `inspector: Borrow, kind=cpu_inspector, rights=0x80000000` |
 * | Operation | 260 |
 * | Feature | `Core` |
 * | Arguments | `configuration: user_address` |
 * | Results | `generation: u64` |
 * | Capability effects | `inspector: Borrow, kind=cpu_inspector, rights=0x80000000` |
 * | User memory | `configuration: Read, fixed-bytes=56, record=trace_config; order=0` |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Object` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_CPU_INSPECTOR_TRACE_START UINT64_C(260)

/**
 * Object-local method `cpu_inspector_trace_stop`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `inspector: Borrow, kind=cpu_inspector, rights=0x80000000` |
 * | Operation | 261 |
 * | Feature | `Core` |
 * | Arguments | `generation: u64` |
 * | Results | — |
 * | Capability effects | `inspector: Borrow, kind=cpu_inspector, rights=0x80000000` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Object` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_CPU_INSPECTOR_TRACE_STOP UINT64_C(261)

/**
 * Object-local method `cpu_inspector_trace_read`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `inspector: Borrow, kind=cpu_inspector, rights=0x200000000` |
 * | Operation | 262 |
 * | Feature | `Core` |
 * | Arguments | `generation: u64`, `cpu: u64`, `cursor: u64`, `output: user_address`, `output_size: byte_count` |
 * | Results | `supported_size: byte_count` |
 * | Capability effects | `inspector: Borrow, kind=cpu_inspector, rights=0x200000000` |
 * | User memory | `output: Write, len=output_size bytes, max-bytes=2656; order=0` |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Object` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_CPU_INSPECTOR_TRACE_READ UINT64_C(262)

/**
 * Object-local method `cpu_inspector_read`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `inspector: Borrow, kind=cpu_inspector, rights=0x8` |
 * | Operation | 256 |
 * | Feature | `Core` |
 * | Arguments | `observation: user_address`, `observation_size: byte_count` |
 * | Results | `supported_size: byte_count` |
 * | Capability effects | `inspector: Borrow, kind=cpu_inspector, rights=0x8` |
 * | User memory | `observation: Write, len=observation_size bytes, max-bytes=4096, record=cpu_observation; order=0` |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Object` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_CPU_INSPECTOR_READ UINT64_C(256)

/**
 * Object-local method `file_get_info`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `file: Borrow, kind=file, rights=0x8` |
 * | Operation | 257 |
 * | Feature | `Core` |
 * | Arguments | `output: user_address`, `output_size: byte_count` |
 * | Results | `supported_size: byte_count` |
 * | Capability effects | `file: Borrow, kind=file, rights=0x8` |
 * | User memory | `output: Write, len=output_size bytes, max-bytes=4096, record=file_info; order=0` |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Object` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_FILE_GET_INFO UINT64_C(257)

/**
 * Object-local method `directory_get_info`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `directory: Borrow, kind=directory, rights=0x8` |
 * | Operation | 258 |
 * | Feature | `Core` |
 * | Arguments | `output: user_address`, `output_size: byte_count` |
 * | Results | `supported_size: byte_count` |
 * | Capability effects | `directory: Borrow, kind=directory, rights=0x8` |
 * | User memory | `output: Write, len=output_size bytes, max-bytes=4096, record=directory_info; order=0` |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Object` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_DIRECTORY_GET_INFO UINT64_C(258)

/**
 * Object-local method `virtual_machine_creation_lease_create`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `authority: Borrow, kind=virtual_machine_creation_authority, rights=0x30000000` |
 * | Operation | 256 |
 * | Feature | `Core` |
 * | Arguments | `resource_domain: handle` |
 * | Results | `lease: handle` |
 * | Capability effects | `authority: Borrow, kind=virtual_machine_creation_authority, rights=0x30000000`, `resource_domain: Borrow, kind=resource_domain, rights=0x8000000`, `lease: produce, kind=virtual_machine_creation_lease, fixed=0x2000000a, inherited-from(authority), mask=0xf0000000000` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_VIRTUAL_MACHINE_CREATION_LEASE_CREATE UINT64_C(256)

/**
 * Object-local method `virtual_machine_create`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `lease: ConsumeOnCommit, kind=virtual_machine_creation_lease, rights=0x20000000` |
 * | Operation | 256 |
 * | Feature | `Core` |
 * | Arguments | `configuration: user_address`, `configuration_size: byte_count` |
 * | Results | `pending_virtual_machine: handle` |
 * | Capability effects | `lease: ConsumeOnCommit, kind=virtual_machine_creation_lease, rights=0x20000000`, `pending_virtual_machine: produce, kind=pending_virtual_machine, fixed=0xc2a, inherited-from(lease), mask=0xf0000000000` |
 * | User memory | `configuration: Read, len=configuration_size bytes, max-bytes=4096, record=virtual_machine_configuration; order=0` |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_VIRTUAL_MACHINE_CREATE UINT64_C(256)

/**
 * Object-local method `pending_virtual_machine_set_memory`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `pending_virtual_machine: Borrow, kind=pending_virtual_machine, rights=0x20` |
 * | Operation | 256 |
 * | Feature | `Core` |
 * | Arguments | `vmo: handle` |
 * | Results | — |
 * | Capability effects | `pending_virtual_machine: Borrow, kind=pending_virtual_machine, rights=0x20`, `vmo: Borrow, kind=vmo, rights=0x70` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_PENDING_VIRTUAL_MACHINE_SET_MEMORY UINT64_C(256)

/**
 * Object-local method `pending_virtual_machine_set_bootstrap`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `pending_virtual_machine: Borrow, kind=pending_virtual_machine, rights=0x20` |
 * | Operation | 257 |
 * | Feature | `Core` |
 * | Arguments | `bootstrap: user_address`, `bootstrap_size: byte_count` |
 * | Results | — |
 * | Capability effects | `pending_virtual_machine: Borrow, kind=pending_virtual_machine, rights=0x20` |
 * | User memory | `bootstrap: Read, len=bootstrap_size bytes, max-bytes=4096, record=virtual_cpu_bootstrap; order=0` |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_PENDING_VIRTUAL_MACHINE_SET_BOOTSTRAP UINT64_C(257)

/**
 * Object-local method `pending_virtual_machine_seal`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `pending_virtual_machine: Borrow, kind=pending_virtual_machine, rights=0x20` |
 * | Operation | 258 |
 * | Feature | `Core` |
 * | Arguments | — |
 * | Results | — |
 * | Capability effects | `pending_virtual_machine: Borrow, kind=pending_virtual_machine, rights=0x20` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_PENDING_VIRTUAL_MACHINE_SEAL UINT64_C(258)

/**
 * Object-local method `pending_virtual_machine_install`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `pending_virtual_machine: ConsumeOnCommit, kind=pending_virtual_machine, rights=0x400` |
 * | Operation | 259 |
 * | Feature | `Core` |
 * | Arguments | — |
 * | Results | `virtual_machine: handle`, `boot_virtual_cpu: handle` |
 * | Capability effects | `pending_virtual_machine: ConsumeOnCommit, kind=pending_virtual_machine, rights=0x400`, `virtual_machine: produce, kind=virtual_machine, fixed=0x82e, inherited-from(pending_virtual_machine), mask=0xf0000000000`, `boot_virtual_cpu: produce, kind=virtual_cpu, fixed=0x1000000042e, inherited-from(pending_virtual_machine), mask=0xe0000000000` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_PENDING_VIRTUAL_MACHINE_INSTALL UINT64_C(259)

/**
 * Object-local method `pending_virtual_machine_abort`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `pending_virtual_machine: ConsumeOnCommit, kind=pending_virtual_machine, rights=0x800` |
 * | Operation | 260 |
 * | Feature | `Core` |
 * | Arguments | — |
 * | Results | — |
 * | Capability effects | `pending_virtual_machine: ConsumeOnCommit, kind=pending_virtual_machine, rights=0x800` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_PENDING_VIRTUAL_MACHINE_ABORT UINT64_C(260)

/**
 * Object-local method `virtual_machine_request_stop`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `virtual_machine: Borrow, kind=virtual_machine, rights=0x800` |
 * | Operation | 256 |
 * | Feature | `Core` |
 * | Arguments | — |
 * | Results | — |
 * | Capability effects | `virtual_machine: Borrow, kind=virtual_machine, rights=0x800` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_VIRTUAL_MACHINE_REQUEST_STOP UINT64_C(256)

/**
 * Object-local method `virtual_machine_get_info`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `virtual_machine: Borrow, kind=virtual_machine, rights=0x8` |
 * | Operation | 257 |
 * | Feature | `Core` |
 * | Arguments | `info: user_address`, `info_size: byte_count` |
 * | Results | `supported_size: byte_count` |
 * | Capability effects | `virtual_machine: Borrow, kind=virtual_machine, rights=0x8` |
 * | User memory | `info: Write, len=info_size bytes, max-bytes=4096, record=virtual_machine_info; order=0` |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Object` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_VIRTUAL_MACHINE_GET_INFO UINT64_C(257)

/**
 * Object-local method `virtual_cpu_get_info`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `virtual_cpu: Borrow, kind=virtual_cpu, rights=0x8` |
 * | Operation | 256 |
 * | Feature | `Core` |
 * | Arguments | `info: user_address`, `info_size: byte_count` |
 * | Results | `supported_size: byte_count` |
 * | Capability effects | `virtual_cpu: Borrow, kind=virtual_cpu, rights=0x8` |
 * | User memory | `info: Write, len=info_size bytes, max-bytes=4096, record=virtual_cpu_info; order=0` |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Object` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_VIRTUAL_CPU_GET_INFO UINT64_C(256)

/**
 * Object-local method `resource_domain_create`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `parent: Borrow, kind=resource_domain, rights=0x800000` |
 * | Operation | 256 |
 * | Feature | `Core` |
 * | Arguments | `limits: user_address`, `limits_size: byte_count` |
 * | Results | `child: handle` |
 * | Capability effects | `parent: Borrow, kind=resource_domain, rights=0x800000`, `child: produce, kind=resource_domain, fixed=0x984000b` |
 * | User memory | `limits: Read, len=limits_size bytes, max-bytes=4096, record=resource_limits; order=0` |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_RESOURCE_DOMAIN_CREATE UINT64_C(256)

/**
 * Object-local method `task_group_create`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `factory: Borrow, kind=task_factory, rights=0x400000` |
 * | Operation | 257 |
 * | Feature | `Core` |
 * | Arguments | `resource_domain: handle` |
 * | Results | `group: handle` |
 * | Capability effects | `factory: Borrow, kind=task_factory, rights=0x400000`, `resource_domain: Borrow, kind=resource_domain, rights=0x8000000`, `group: produce, kind=task_group, fixed=0x400080f` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_TASK_GROUP_CREATE UINT64_C(257)

/**
 * Object-local method `virtual_cpu_start`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `virtual_cpu: Borrow, kind=virtual_cpu, rights=0x400` |
 * | Operation | 257 |
 * | Feature | `Core` |
 * | Arguments | — |
 * | Results | — |
 * | Capability effects | `virtual_cpu: Borrow, kind=virtual_cpu, rights=0x400` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_VIRTUAL_CPU_START UINT64_C(257)

/**
 * Object-local method `pending_virtual_machine_set_virtual_serial`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `pending_virtual_machine: Borrow, kind=pending_virtual_machine, rights=0x20` |
 * | Operation | 261 |
 * | Feature | `Core` |
 * | Arguments | `virtual_serial: handle` |
 * | Results | — |
 * | Capability effects | `pending_virtual_machine: Borrow, kind=pending_virtual_machine, rights=0x20`, `virtual_serial: ConsumeOnCommit, kind=virtual_serial, rights=0x8002` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_PENDING_VIRTUAL_MACHINE_SET_VIRTUAL_SERIAL UINT64_C(261)

/**
 * Object-local method `clock_get_monotonic`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `receiver: Borrow, kind=system_info, rights=0x10` |
 * | Operation | 257 |
 * | Feature | `Core` |
 * | Arguments | — |
 * | Results | `nanoseconds: u64` |
 * | Capability effects | `receiver: Borrow, kind=system_info, rights=0x10` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Abi` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_CLOCK_GET_MONOTONIC UINT64_C(257)

/**
 * Object-local method `virtual_serial_create`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `receiver: Borrow, kind=object_factory, rights=0x8000000000` |
 * | Operation | 260 |
 * | Feature | `Core` |
 * | Arguments | — |
 * | Results | `virtual_serial: handle` |
 * | Capability effects | `receiver: Borrow, kind=object_factory, rights=0x8000000000`, `virtual_serial: produce, kind=virtual_serial, fixed=0x803f` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_VIRTUAL_SERIAL_CREATE UINT64_C(260)

/**
 * Object-local method `virtual_serial_register_output`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `virtual_serial: Borrow, kind=virtual_serial, rights=0x20` |
 * | Operation | 256 |
 * | Feature | `Core` |
 * | Arguments | `buffer: handle` |
 * | Results | — |
 * | Capability effects | `virtual_serial: Borrow, kind=virtual_serial, rights=0x20`, `buffer: Borrow, kind=vmo, rights=0x70` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_VIRTUAL_SERIAL_REGISTER_OUTPUT UINT64_C(256)

/**
 * Object-local method `virtual_serial_write`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `virtual_serial: Borrow, kind=virtual_serial, rights=0x20` |
 * | Operation | 257 |
 * | Feature | `Core` |
 * | Arguments | `bytes: user_address`, `byte_count: byte_count` |
 * | Results | `actual_bytes: byte_count` |
 * | Capability effects | `virtual_serial: Borrow, kind=virtual_serial, rights=0x20` |
 * | User memory | `bytes: Read, len=byte_count bytes, max-bytes=4096; order=0` |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_VIRTUAL_SERIAL_WRITE UINT64_C(257)

/**
 * Object-local method `thread_create`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `receiver: Borrow, kind=execution_context, rights=0x200000` |
 * | Operation | 260 |
 * | Feature | `Core` |
 * | Arguments | `entry: u64`, `stack: u64`, `tls: u64`, `argument: u64`, `affinity_words: user_address`, `word_count: element_count` |
 * | Results | `thread: handle` |
 * | Capability effects | `receiver: Borrow, kind=execution_context, rights=0x200000`, `thread: produce, kind=thread, fixed=0x10000000c0f, inherited-from(receiver), mask=0xe0000000000` |
 * | User memory | `affinity_words: Read, len=word_count elements, max-elements=4, element-size=8; order=0` |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Task` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_THREAD_CREATE UINT64_C(260)

/**
 * Object-local method `thread_start`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `thread: Borrow, kind=thread, rights=0x400` |
 * | Operation | 256 |
 * | Feature | `Core` |
 * | Arguments | — |
 * | Results | — |
 * | Capability effects | `thread: Borrow, kind=thread, rights=0x400` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Task` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_THREAD_START UINT64_C(256)

/**
 * Object-local method `thread_request_stop`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `thread: Borrow, kind=thread, rights=0x800` |
 * | Operation | 257 |
 * | Feature | `Core` |
 * | Arguments | — |
 * | Results | — |
 * | Capability effects | `thread: Borrow, kind=thread, rights=0x800` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Task` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_THREAD_REQUEST_STOP UINT64_C(257)

/**
 * Object-local method `atomic_wait`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `receiver: Borrow, kind=execution_context, rights=0x20` |
 * | Operation | 261 |
 * | Feature | `Core` |
 * | Arguments | `address: u64`, `expected: u32`, `deadline: u64` |
 * | Results | — |
 * | Capability effects | `receiver: Borrow, kind=execution_context, rights=0x20` |
 * | User memory | — |
 * | Execution | `blocking=MayBlock, cancellation=Explicit, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Task` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_ATOMIC_WAIT UINT64_C(261)

/**
 * Object-local method `atomic_wake`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `receiver: Borrow, kind=execution_context, rights=0x20` |
 * | Operation | 262 |
 * | Feature | `Core` |
 * | Arguments | `address: u64`, `count: u32` |
 * | Results | `woken: u64` |
 * | Capability effects | `receiver: Borrow, kind=execution_context, rights=0x20` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Task` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_ATOMIC_WAKE UINT64_C(262)

/**
 * Object-local method `thread_sleep`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `receiver: Borrow, kind=execution_context, rights=0x20` |
 * | Operation | 263 |
 * | Feature | `Core` |
 * | Arguments | `deadline: u64` |
 * | Results | — |
 * | Capability effects | `receiver: Borrow, kind=execution_context, rights=0x20` |
 * | User memory | — |
 * | Execution | `blocking=MayBlock, cancellation=Explicit, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Task` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_THREAD_SLEEP UINT64_C(263)

/**
 * Object-local method `file_write_at`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `file: Borrow, kind=file, rights=0x20` |
 * | Operation | 258 |
 * | Feature | `Core` |
 * | Arguments | `options: u32`, `offset: u64`, `input: user_address`, `input_size: byte_count` |
 * | Results | `actual_bytes: byte_count`, `end_offset: u64` |
 * | Capability effects | `file: Borrow, kind=file, rights=0x20` |
 * | User memory | `input: Read, len=input_size bytes, max-bytes=2097152; order=0` |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_FILE_WRITE_AT UINT64_C(258)

/**
 * Object-local method `file_resize`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `file: Borrow, kind=file, rights=0x20` |
 * | Operation | 259 |
 * | Feature | `Core` |
 * | Arguments | `size: byte_count` |
 * | Results | — |
 * | Capability effects | `file: Borrow, kind=file, rights=0x20` |
 * | User memory | — |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_FILE_RESIZE UINT64_C(259)

/**
 * Object-local method `directory_create_file`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `directory: Borrow, kind=directory, rights=0x30` |
 * | Operation | 259 |
 * | Feature | `Core` |
 * | Arguments | `path: user_address`, `path_size: byte_count`, `requested_rights: rights`, `mode: u32` |
 * | Results | `file: handle` |
 * | Capability effects | `directory: Borrow, kind=directory, rights=0x30`, `file: produce, kind=file, exact-from(requested_rights), allowed=0x9800000bb, required-from=directory` |
 * | User memory | `path: Read, len=path_size bytes, max-bytes=4096; order=0` |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_DIRECTORY_CREATE_FILE UINT64_C(259)

/**
 * Object-local method `directory_create_directory`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `directory: Borrow, kind=directory, rights=0x30` |
 * | Operation | 260 |
 * | Feature | `Core` |
 * | Arguments | `path: user_address`, `path_size: byte_count`, `mode: u32` |
 * | Results | — |
 * | Capability effects | `directory: Borrow, kind=directory, rights=0x30` |
 * | User memory | `path: Read, len=path_size bytes, max-bytes=4096; order=0` |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_DIRECTORY_CREATE_DIRECTORY UINT64_C(260)

/**
 * Object-local method `directory_remove`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `directory: Borrow, kind=directory, rights=0x30` |
 * | Operation | 261 |
 * | Feature | `Core` |
 * | Arguments | `path: user_address`, `path_size: byte_count`, `options: u32` |
 * | Results | — |
 * | Capability effects | `directory: Borrow, kind=directory, rights=0x30` |
 * | User memory | `path: Read, len=path_size bytes, max-bytes=4096; order=0` |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_DIRECTORY_REMOVE UINT64_C(261)

/**
 * Object-local method `wait_set_create`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `receiver: Borrow, kind=object_factory, rights=0x8000000000` |
 * | Operation | 261 |
 * | Feature | `Core` |
 * | Arguments | `capacity: element_count` |
 * | Results | `wait_set: handle` |
 * | Capability effects | `receiver: Borrow, kind=object_factory, rights=0x8000000000`, `wait_set: produce, kind=wait_set, fixed=0x4000000d` |
 * | User memory | — |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_WAIT_SET_CREATE UINT64_C(261)

/**
 * Object-local method `wait_set_add`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `wait_set: Borrow, kind=wait_set, rights=0x40000000` |
 * | Operation | 256 |
 * | Feature | `Core` |
 * | Arguments | `object: handle`, `signals: u64` |
 * | Results | `registration: u64` |
 * | Capability effects | `wait_set: Borrow, kind=wait_set, rights=0x40000000`, `object: Borrow, any, rights=0x4` |
 * | User memory | — |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_WAIT_SET_ADD UINT64_C(256)

/**
 * Object-local method `wait_set_rearm`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `wait_set: Borrow, kind=wait_set, rights=0x40000000` |
 * | Operation | 257 |
 * | Feature | `Core` |
 * | Arguments | `registration: u64` |
 * | Results | — |
 * | Capability effects | `wait_set: Borrow, kind=wait_set, rights=0x40000000` |
 * | User memory | — |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_WAIT_SET_REARM UINT64_C(257)

/**
 * Object-local method `wait_set_remove`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `wait_set: Borrow, kind=wait_set, rights=0x40000000` |
 * | Operation | 258 |
 * | Feature | `Core` |
 * | Arguments | `registration: u64` |
 * | Results | — |
 * | Capability effects | `wait_set: Borrow, kind=wait_set, rights=0x40000000` |
 * | User memory | — |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_WAIT_SET_REMOVE UINT64_C(258)

/**
 * Object-local method `wait_set_wait`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `wait_set: Borrow, kind=wait_set, rights=0x4` |
 * | Operation | 259 |
 * | Feature | `Core` |
 * | Arguments | `deadline: u64`, `output: user_address`, `output_size: byte_count` |
 * | Results | — |
 * | Capability effects | `wait_set: Borrow, kind=wait_set, rights=0x4` |
 * | User memory | `output: Write, len=output_size bytes, max-bytes=24, record=wait_set_event; order=0` |
 * | Execution | `blocking=MayBlock, cancellation=Explicit, restart=Never, completion=Returns, flags=Strict` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_WAIT_SET_WAIT UINT64_C(259)

/**
 * Object-local method `process_get_current_id`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `receiver: Borrow, kind=execution_context, rights=0x10` |
 * | Operation | 264 |
 * | Feature | `Core` |
 * | Arguments | — |
 * | Results | `koid: u64` |
 * | Capability effects | `receiver: Borrow, kind=execution_context, rights=0x10` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
 * | Audit | `Task` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_PROCESS_GET_CURRENT_ID UINT64_C(264)

/**
 * Object-local method `virtual_serial_acknowledge_output`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `virtual_serial: Borrow, kind=virtual_serial, rights=0x10` |
 * | Operation | 258 |
 * | Feature | `Core` |
 * | Arguments | `consumed: u64` |
 * | Results | — |
 * | Capability effects | `virtual_serial: Borrow, kind=virtual_serial, rights=0x10` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_VIRTUAL_SERIAL_ACKNOWLEDGE_OUTPUT UINT64_C(258)

/**
 * Object-local method `directory_scope_create`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `root: Borrow, kind=directory, rights=0x10` |
 * | Operation | 262 |
 * | Feature | `Core` |
 * | Arguments | `start: handle`, `rights: rights` |
 * | Results | `directory: handle` |
 * | Capability effects | `root: Borrow, kind=directory, rights=0x10`, `start: Borrow, kind=directory, rights=0x10`, `directory: produce, kind=directory, exact-from(rights), allowed=0xd800000bb, required-from=root` |
 * | User memory | — |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_DIRECTORY_SCOPE_CREATE UINT64_C(262)

/**
 * Object-local method `directory_get_metadata`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `directory: Borrow, kind=directory, rights=0x18` |
 * | Operation | 263 |
 * | Feature | `Core` |
 * | Arguments | `path: user_address`, `path_length: byte_count`, `options: u32`, `output: user_address`, `output_size: byte_count` |
 * | Results | `supported_size: byte_count` |
 * | Capability effects | `directory: Borrow, kind=directory, rights=0x18` |
 * | User memory | `path: Read, len=path_length bytes, max-bytes=4096; order=0`, `output: Write, len=output_size bytes, max-bytes=4096, record=file_metadata; order=1` |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_DIRECTORY_GET_METADATA UINT64_C(263)

/**
 * Object-local method `file_get_metadata`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `file: Borrow, kind=file, rights=0x8` |
 * | Operation | 260 |
 * | Feature | `Core` |
 * | Arguments | `output: user_address`, `output_size: byte_count` |
 * | Results | `supported_size: byte_count` |
 * | Capability effects | `file: Borrow, kind=file, rights=0x8` |
 * | User memory | `output: Write, len=output_size bytes, max-bytes=4096, record=file_metadata; order=0` |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_FILE_GET_METADATA UINT64_C(260)

/**
 * Object-local method `directory_get_self_metadata`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `directory: Borrow, kind=directory, rights=0x8` |
 * | Operation | 264 |
 * | Feature | `Core` |
 * | Arguments | `output: user_address`, `output_size: byte_count` |
 * | Results | `supported_size: byte_count` |
 * | Capability effects | `directory: Borrow, kind=directory, rights=0x8` |
 * | User memory | `output: Write, len=output_size bytes, max-bytes=4096, record=file_metadata; order=0` |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_DIRECTORY_GET_SELF_METADATA UINT64_C(264)

/**
 * Object-local method `directory_set_metadata`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `directory: Borrow, kind=directory, rights=0x80000010` |
 * | Operation | 265 |
 * | Feature | `Core` |
 * | Arguments | `path: user_address`, `path_length: byte_count`, `options: u32`, `input: user_address`, `input_size: byte_count` |
 * | Results | — |
 * | Capability effects | `directory: Borrow, kind=directory, rights=0x80000010` |
 * | User memory | `path: Read, len=path_length bytes, max-bytes=4096; order=1`, `input: Read, len=input_size bytes, max-bytes=4096, record=file_metadata_update; order=0` |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_DIRECTORY_SET_METADATA UINT64_C(265)

/**
 * Object-local method `file_set_metadata`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `file: Borrow, kind=file, rights=0x80000000` |
 * | Operation | 261 |
 * | Feature | `Core` |
 * | Arguments | `input: user_address`, `input_size: byte_count` |
 * | Results | — |
 * | Capability effects | `file: Borrow, kind=file, rights=0x80000000` |
 * | User memory | `input: Read, len=input_size bytes, max-bytes=4096, record=file_metadata_update; order=0` |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_FILE_SET_METADATA UINT64_C(261)

/**
 * Object-local method `directory_rename`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `source: Borrow, kind=directory, rights=0x30` |
 * | Operation | 266 |
 * | Feature | `Core` |
 * | Arguments | `path: user_address`, `path_length: byte_count`, `destination: handle`, `new_path: user_address`, `new_path_length: byte_count` |
 * | Results | — |
 * | Capability effects | `source: Borrow, kind=directory, rights=0x30`, `destination: Borrow, kind=directory, rights=0x30` |
 * | User memory | `path: Read, len=path_length bytes, max-bytes=4096; order=0`, `new_path: Read, len=new_path_length bytes, max-bytes=4096; order=1` |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_DIRECTORY_RENAME UINT64_C(266)

/**
 * Object-local method `directory_link`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `source: Borrow, kind=directory, rights=0x30` |
 * | Operation | 267 |
 * | Feature | `Core` |
 * | Arguments | `path: user_address`, `path_length: byte_count`, `destination: handle`, `new_path: user_address`, `new_path_length: byte_count` |
 * | Results | — |
 * | Capability effects | `source: Borrow, kind=directory, rights=0x30`, `destination: Borrow, kind=directory, rights=0x30` |
 * | User memory | `path: Read, len=path_length bytes, max-bytes=4096; order=0`, `new_path: Read, len=new_path_length bytes, max-bytes=4096; order=1` |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_DIRECTORY_LINK UINT64_C(267)

/**
 * Object-local method `directory_symlink`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `directory: Borrow, kind=directory, rights=0x30` |
 * | Operation | 268 |
 * | Feature | `Core` |
 * | Arguments | `path: user_address`, `path_length: byte_count`, `target: user_address`, `target_length: byte_count` |
 * | Results | — |
 * | Capability effects | `directory: Borrow, kind=directory, rights=0x30` |
 * | User memory | `path: Read, len=path_length bytes, max-bytes=4096; order=0`, `target: Read, len=target_length bytes, max-bytes=4096; order=1` |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_DIRECTORY_SYMLINK UINT64_C(268)

/**
 * Object-local method `directory_read_link`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `directory: Borrow, kind=directory, rights=0x10` |
 * | Operation | 269 |
 * | Feature | `Core` |
 * | Arguments | `path: user_address`, `path_length: byte_count`, `output: user_address`, `capacity: byte_count` |
 * | Results | `actual: byte_count` |
 * | Capability effects | `directory: Borrow, kind=directory, rights=0x10` |
 * | User memory | `path: Read, len=path_length bytes, max-bytes=4096; order=0`, `output: Write, len=capacity bytes, max-bytes=4096; order=1` |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_DIRECTORY_READ_LINK UINT64_C(269)

/**
 * Object-local method `directory_canonicalize`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `directory: Borrow, kind=directory, rights=0x10` |
 * | Operation | 270 |
 * | Feature | `Core` |
 * | Arguments | `path: user_address`, `path_length: byte_count`, `output: user_address`, `capacity: byte_count` |
 * | Results | `actual: byte_count` |
 * | Capability effects | `directory: Borrow, kind=directory, rights=0x10` |
 * | User memory | `path: Read, len=path_length bytes, max-bytes=4096; order=0`, `output: Write, len=capacity bytes, max-bytes=4096; order=1` |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_DIRECTORY_CANONICALIZE UINT64_C(270)

/**
 * Object-local method `directory_remove_if`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `directory: Borrow, kind=directory, rights=0x30` |
 * | Operation | 271 |
 * | Feature | `Core` |
 * | Arguments | `path: user_address`, `path_length: byte_count`, `options: u32`, `expected_node_id: u64` |
 * | Results | — |
 * | Capability effects | `directory: Borrow, kind=directory, rights=0x30` |
 * | User memory | `path: Read, len=path_length bytes, max-bytes=4096; order=0` |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_DIRECTORY_REMOVE_IF UINT64_C(271)

/**
 * Object-local method `directory_open_directory_nofollow`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `directory: Borrow, kind=directory, rights=0x10` |
 * | Operation | 272 |
 * | Feature | `Core` |
 * | Arguments | `path: user_address`, `path_length: byte_count`, `rights: rights` |
 * | Results | `directory: handle` |
 * | Capability effects | `directory: Borrow, kind=directory, rights=0x10`, `directory: produce, kind=directory, exact-from(rights), allowed=0x9800000bb, required-from=directory` |
 * | User memory | `path: Read, len=path_length bytes, max-bytes=4096; order=0` |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_DIRECTORY_OPEN_DIRECTORY_NOFOLLOW UINT64_C(272)

/**
 * Object-local method `file_sync`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `file: Borrow, kind=file, rights=0x20` |
 * | Operation | 262 |
 * | Feature | `Core` |
 * | Arguments | `scope: u32` |
 * | Results | — |
 * | Capability effects | `file: Borrow, kind=file, rights=0x20` |
 * | User memory | — |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_FILE_SYNC UINT64_C(262)

/**
 * Object-local method `file_lock`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `file: Borrow, kind=file, rights=0x100000000` |
 * | Operation | 263 |
 * | Feature | `Core` |
 * | Arguments | `mode: u32`, `deadline: u64` |
 * | Results | — |
 * | Capability effects | `file: Borrow, kind=file, rights=0x100000000` |
 * | User memory | — |
 * | Execution | `blocking=MayBlock, cancellation=Explicit, restart=Never, completion=Returns, flags=Strict` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_FILE_LOCK UINT64_C(263)

/**
 * Object-local method `file_unlock`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `file: Borrow, kind=file, rights=0x100000000` |
 * | Operation | 264 |
 * | Feature | `Core` |
 * | Arguments | — |
 * | Results | — |
 * | Capability effects | `file: Borrow, kind=file, rights=0x100000000` |
 * | User memory | — |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_FILE_UNLOCK UINT64_C(264)

/**
 * Object-local method `clock_get_realtime`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `receiver: Borrow, kind=system_info, rights=0x10` |
 * | Operation | 258 |
 * | Feature | `Core` |
 * | Arguments | — |
 * | Results | `seconds: i64`, `nanoseconds: u32` |
 * | Capability effects | `receiver: Borrow, kind=system_info, rights=0x10` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_CLOCK_GET_REALTIME UINT64_C(258)

/**
 * Object-local method `directory_open_file_with_options`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `directory: Borrow, kind=directory, rights=0x10` |
 * | Operation | 273 |
 * | Feature | `Core` |
 * | Arguments | `path: user_address`, `path_length: byte_count`, `rights: rights`, `options: u32`, `mode: u32` |
 * | Results | `file: handle` |
 * | Capability effects | `directory: Borrow, kind=directory, rights=0x10`, `file: produce, kind=file, exact-from(rights), allowed=0x9800000bb, required-from=directory` |
 * | User memory | `path: Read, len=path_length bytes, max-bytes=4096; order=0` |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_DIRECTORY_OPEN_FILE_WITH_OPTIONS UINT64_C(273)

/**
 * Object-local method `virtual_machine_creation_lease_get_platform_info`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `lease: Borrow, kind=virtual_machine_creation_lease, rights=0x8` |
 * | Operation | 257 |
 * | Feature | `Core` |
 * | Arguments | `platform_profile: u32`, `info: user_address`, `info_size: byte_count` |
 * | Results | `supported_size: byte_count` |
 * | Capability effects | `lease: Borrow, kind=virtual_machine_creation_lease, rights=0x8` |
 * | User memory | `info: Write, len=info_size bytes, max-bytes=4096, record=virtual_machine_platform_info; order=0` |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Object` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_VIRTUAL_MACHINE_CREATION_LEASE_GET_PLATFORM_INFO UINT64_C(257)

/**
 * Object-local method `vmo_create_snapshot`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `vmo: Borrow, kind=vmo, rights=0x10` |
 * | Operation | 258 |
 * | Feature | `Core` |
 * | Arguments | — |
 * | Results | `vmo: handle`, `byte_size: byte_count` |
 * | Capability effects | `vmo: Borrow, kind=vmo, rights=0x10`, `vmo: produce, kind=vmo, fixed=0x5b` |
 * | User memory | — |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_VMO_CREATE_SNAPSHOT UINT64_C(258)

/**
 * Object-local method `vmar_map_private`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `vmar: Borrow, kind=vmar, rights=0x40` |
 * | Operation | 261 |
 * | Feature | `Core` |
 * | Arguments | `vmo: handle`, `mapping: user_address`, `mapping_size: byte_count` |
 * | Results | — |
 * | Capability effects | `vmar: Borrow, kind=vmar, rights=0x40`, `vmo: Borrow, kind=vmo, rights=0x50` |
 * | User memory | `mapping: Read, len=mapping_size bytes, max-bytes=4096, record=private_mapping; order=0` |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_VMAR_MAP_PRIVATE UINT64_C(261)

/**
 * Object-local method `virtual_machine_get_power_request`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `virtual_machine: Borrow, kind=virtual_machine, rights=0x20` |
 * | Operation | 258 |
 * | Feature | `Core` |
 * | Arguments | `request: user_address`, `request_size: byte_count` |
 * | Results | `supported_size: byte_count` |
 * | Capability effects | `virtual_machine: Borrow, kind=virtual_machine, rights=0x20` |
 * | User memory | `request: Write, len=request_size bytes, max-bytes=4096, record=virtual_machine_power_request; order=0` |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_VIRTUAL_MACHINE_GET_POWER_REQUEST UINT64_C(258)

/**
 * Object-local method `virtual_machine_complete_power_request`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `virtual_machine: Borrow, kind=virtual_machine, rights=0x20` |
 * | Operation | 259 |
 * | Feature | `Core` |
 * | Arguments | `request_id: u64`, `accept: u32` |
 * | Results | — |
 * | Capability effects | `virtual_machine: Borrow, kind=virtual_machine, rights=0x20` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_VIRTUAL_MACHINE_COMPLETE_POWER_REQUEST UINT64_C(259)

/**
 * Object-local method `virtual_machine_open_vcpu`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `virtual_machine: Borrow, kind=virtual_machine, rights=0x20` |
 * | Operation | 260 |
 * | Feature | `Core` |
 * | Arguments | `vcpu_id: u32` |
 * | Results | `virtual_cpu: handle` |
 * | Capability effects | `virtual_machine: Borrow, kind=virtual_machine, rights=0x20`, `virtual_cpu: produce, kind=virtual_cpu, fixed=0x1000000042e, inherited-from(virtual_machine), mask=0xe0000000000` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_VIRTUAL_MACHINE_OPEN_VCPU UINT64_C(260)

/**
 * Object-local method `virtual_machine_register_mmio`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `virtual_machine: Borrow, kind=virtual_machine, rights=0x20` |
 * | Operation | 261 |
 * | Feature | `Core` |
 * | Arguments | `base: u64`, `length: u64`, `device: u64` |
 * | Results | — |
 * | Capability effects | `virtual_machine: Borrow, kind=virtual_machine, rights=0x20` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_VIRTUAL_MACHINE_REGISTER_MMIO UINT64_C(261)

/**
 * Object-local method `virtual_cpu_get_mmio_request`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `virtual_cpu: Borrow, kind=virtual_cpu, rights=0x20` |
 * | Operation | 258 |
 * | Feature | `Core` |
 * | Arguments | `request: user_address`, `request_size: byte_count` |
 * | Results | `supported_size: byte_count` |
 * | Capability effects | `virtual_cpu: Borrow, kind=virtual_cpu, rights=0x20` |
 * | User memory | `request: Write, len=request_size bytes, max-bytes=4096, record=virtual_cpu_mmio_request; order=0` |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_VIRTUAL_CPU_GET_MMIO_REQUEST UINT64_C(258)

/**
 * Object-local method `virtual_cpu_complete_mmio`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `virtual_cpu: Borrow, kind=virtual_cpu, rights=0x20` |
 * | Operation | 259 |
 * | Feature | `Core` |
 * | Arguments | `request_id: u64`, `operation: u32`, `value: u64` |
 * | Results | — |
 * | Capability effects | `virtual_cpu: Borrow, kind=virtual_cpu, rights=0x20` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_VIRTUAL_CPU_COMPLETE_MMIO UINT64_C(259)

/**
 * Object-local method `guest_memory_create`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `vmo: Borrow, kind=vmo, rights=0x70` |
 * | Operation | 259 |
 * | Feature | `Core` |
 * | Arguments | — |
 * | Results | `memory: handle` |
 * | Capability effects | `vmo: Borrow, kind=vmo, rights=0x70`, `memory: produce, kind=guest_memory, fixed=0x7b` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_GUEST_MEMORY_CREATE UINT64_C(259)

/**
 * Object-local method `pending_virtual_machine_map_memory`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `pending_virtual_machine: Borrow, kind=pending_virtual_machine, rights=0x20` |
 * | Operation | 262 |
 * | Feature | `Core` |
 * | Arguments | `memory: handle`, `guest_offset: u64`, `source_offset: u64`, `length: u64` |
 * | Results | — |
 * | Capability effects | `pending_virtual_machine: Borrow, kind=pending_virtual_machine, rights=0x20`, `memory: Borrow, kind=guest_memory, rights=0x40` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_PENDING_VIRTUAL_MACHINE_MAP_MEMORY UINT64_C(262)

/**
 * Object-local method `vmo_create_contiguous`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `receiver: Borrow, kind=object_factory, rights=0x8000000000` |
 * | Operation | 262 |
 * | Feature | `Core` |
 * | Arguments | `size: byte_count` |
 * | Results | `vmo: handle` |
 * | Capability effects | `receiver: Borrow, kind=object_factory, rights=0x8000000000`, `vmo: produce, kind=vmo, fixed=0x7b` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_VMO_CREATE_CONTIGUOUS UINT64_C(262)

/**
 * Object-local method `device_claim`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `authority: Borrow, kind=device_assignment_authority, rights=0x8000` |
 * | Operation | 256 |
 * | Feature | `Core` |
 * | Arguments | `index: u32` |
 * | Results | `physical_device: handle` |
 * | Capability effects | `authority: Borrow, kind=device_assignment_authority, rights=0x8000`, `physical_device: produce, kind=physical_device, fixed=0x800f` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_DEVICE_CLAIM UINT64_C(256)

/**
 * Object-local method `physical_device_info`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `device: Borrow, kind=physical_device, rights=0x8` |
 * | Operation | 256 |
 * | Feature | `Core` |
 * | Arguments | `output: user_address`, `output_size: byte_count` |
 * | Results | `supported_size: byte_count` |
 * | Capability effects | `device: Borrow, kind=physical_device, rights=0x8` |
 * | User memory | `output: Write, len=output_size bytes, max-bytes=4096, record=physical_device_info; order=0` |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_PHYSICAL_DEVICE_INFO UINT64_C(256)

/**
 * Object-local method `vmo_get_dma_extent`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `authority: Borrow, kind=device_assignment_authority, rights=0x10000` |
 * | Operation | 257 |
 * | Feature | `Core` |
 * | Arguments | `vmo: handle`, `offset: u64`, `length: u64`, `output: user_address`, `output_size: byte_count` |
 * | Results | `supported_size: byte_count` |
 * | Capability effects | `authority: Borrow, kind=device_assignment_authority, rights=0x10000`, `vmo: Borrow, kind=vmo, rights=0x50` |
 * | User memory | `output: Write, len=output_size bytes, max-bytes=4096, record=dma_extent; order=0` |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_VMO_GET_DMA_EXTENT UINT64_C(257)

/**
 * Object-local method `pending_virtual_machine_assign_device`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `pending: Borrow, kind=pending_virtual_machine, rights=0x20` |
 * | Operation | 263 |
 * | Feature | `Core` |
 * | Arguments | `device: handle`, `guest_base: u64`, `guest_irq: u32` |
 * | Results | — |
 * | Capability effects | `pending: Borrow, kind=pending_virtual_machine, rights=0x20`, `device: Borrow, kind=physical_device, rights=0x8000` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_PENDING_VIRTUAL_MACHINE_ASSIGN_DEVICE UINT64_C(263)

/**
 * Object-local method `guest_mailbox_create`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `machine: Borrow, kind=virtual_machine, rights=0x20` |
 * | Operation | 262 |
 * | Feature | `Core` |
 * | Arguments | `base: u64`, `irq: u32` |
 * | Results | `guest_mailbox: handle` |
 * | Capability effects | `machine: Borrow, kind=virtual_machine, rights=0x20`, `guest_mailbox: produce, kind=guest_mailbox, fixed=0x3e` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_GUEST_MAILBOX_CREATE UINT64_C(262)

/**
 * Object-local method `guest_mailbox_send`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `mailbox: Borrow, kind=guest_mailbox, rights=0x20` |
 * | Operation | 256 |
 * | Feature | `Core` |
 * | Arguments | `bytes: user_address`, `length: byte_count` |
 * | Results | — |
 * | Capability effects | `mailbox: Borrow, kind=guest_mailbox, rights=0x20` |
 * | User memory | `bytes: Read, len=length bytes, max-bytes=256; order=0` |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_GUEST_MAILBOX_SEND UINT64_C(256)

/**
 * Object-local method `guest_mailbox_receive`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `mailbox: Borrow, kind=guest_mailbox, rights=0x10` |
 * | Operation | 257 |
 * | Feature | `Core` |
 * | Arguments | `bytes: user_address`, `capacity: byte_count` |
 * | Results | `bytes: byte_count` |
 * | Capability effects | `mailbox: Borrow, kind=guest_mailbox, rights=0x10` |
 * | User memory | `bytes: Write, len=capacity bytes, max-bytes=256; order=0` |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_GUEST_MAILBOX_RECEIVE UINT64_C(257)

/**
 * Object-local method `guest_notification_create`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `frontend: Borrow, kind=virtual_machine, rights=0x20` |
 * | Operation | 263 |
 * | Feature | `Core` |
 * | Arguments | `backend: handle`, `frontend_base: u64`, `backend_base: u64`, `frontend_irq: u32`, `backend_irq: u32` |
 * | Results | `guest_notification: handle` |
 * | Capability effects | `frontend: Borrow, kind=virtual_machine, rights=0x20`, `backend: Borrow, kind=virtual_machine, rights=0x20`, `guest_notification: produce, kind=guest_notification, fixed=0x2e` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_GUEST_NOTIFICATION_CREATE UINT64_C(263)

/**
 * Object-local method `guest_notification_control`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `notification: Borrow, kind=guest_notification, rights=0x20` |
 * | Operation | 256 |
 * | Feature | `Core` |
 * | Arguments | `operation: u32` |
 * | Results | `epoch: u32` |
 * | Capability effects | `notification: Borrow, kind=guest_notification, rights=0x20` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_GUEST_NOTIFICATION_CONTROL UINT64_C(256)

/**
 * Object-local method `native_block_create`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `memory: Borrow, kind=guest_memory, rights=0x40` |
 * | Operation | 256 |
 * | Feature | `Core` |
 * | Arguments | `backend: handle`, `guest_base: u64`, `notification_base: u64`, `notification_irq: u32` |
 * | Results | `block: handle` |
 * | Capability effects | `memory: Borrow, kind=guest_memory, rights=0x40`, `backend: Borrow, kind=virtual_machine, rights=0x20`, `block: produce, kind=native_block, fixed=0x7e` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_NATIVE_BLOCK_CREATE UINT64_C(256)

/**
 * Object-local method `native_block_activate`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `block: Borrow, kind=native_block, rights=0x20` |
 * | Operation | 256 |
 * | Feature | `Core` |
 * | Arguments | `readonly: u32` |
 * | Results | `sectors: u64`, `readonly: u32` |
 * | Capability effects | `block: Borrow, kind=native_block, rights=0x20` |
 * | User memory | — |
 * | Execution | `blocking=MayBlock, cancellation=Explicit, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_NATIVE_BLOCK_ACTIVATE UINT64_C(256)

/**
 * Object-local method `filesystem_mount`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `directory: Borrow, kind=directory, rights=0xa0` |
 * | Operation | 274 |
 * | Feature | `Core` |
 * | Arguments | `channel: handle`, `buffer: handle`, `path: user_address`, `path_length: byte_count`, `read_only: u64` |
 * | Results | — |
 * | Capability effects | `directory: Borrow, kind=directory, rights=0xa0`, `channel: Borrow, kind=byte_channel, rights=0x34`, `buffer: Borrow, kind=vmo, rights=0x70` |
 * | User memory | `path: Read, len=path_length bytes, max-bytes=4096; order=0` |
 * | Execution | `blocking=MayBlock, cancellation=Explicit, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_FILESYSTEM_MOUNT UINT64_C(274)

/**
 * Object-local method `guest_mapping_create`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `backend: Borrow, kind=virtual_machine, rights=0x20` |
 * | Operation | 264 |
 * | Feature | `Core` |
 * | Arguments | `memory: handle`, `frontend_base: u64` |
 * | Results | `mapping: handle`, `token: u64` |
 * | Capability effects | `backend: Borrow, kind=virtual_machine, rights=0x20`, `memory: Borrow, kind=guest_memory, rights=0x40`, `mapping: produce, kind=guest_mapping, fixed=0x2a` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_GUEST_MAPPING_CREATE UINT64_C(264)

/**
 * Object-local method `guest_mapping_release`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `mapping: Borrow, kind=guest_mapping, rights=0x20` |
 * | Operation | 256 |
 * | Feature | `Core` |
 * | Arguments | — |
 * | Results | — |
 * | Capability effects | `mapping: Borrow, kind=guest_mapping, rights=0x20` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_GUEST_MAPPING_RELEASE UINT64_C(256)

/**
 * Object-local method `device_claim_matching`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `authority: Borrow, kind=device_assignment_authority, rights=0x8000` |
 * | Operation | 258 |
 * | Feature | `Core` |
 * | Arguments | `profile: u32`, `identity_kind: u32`, `identity: user_address`, `length: byte_count` |
 * | Results | `physical_device: handle` |
 * | Capability effects | `authority: Borrow, kind=device_assignment_authority, rights=0x8000`, `physical_device: produce, kind=physical_device, fixed=0x800f` |
 * | User memory | `identity: Read, len=length bytes, max-bytes=512; order=0` |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_DEVICE_CLAIM_MATCHING UINT64_C(258)

/**
 * Object-local method `device_profile_info`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `device: Borrow, kind=physical_device, rights=0x8` |
 * | Operation | 257 |
 * | Feature | `Core` |
 * | Arguments | `output: user_address`, `output_size: byte_count` |
 * | Results | `supported_size: byte_count` |
 * | Capability effects | `device: Borrow, kind=physical_device, rights=0x8` |
 * | User memory | `output: Write, len=output_size bytes, max-bytes=4096, record=device_profile_info; order=0` |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_DEVICE_PROFILE_INFO UINT64_C(257)

/**
 * Object-local method `device_resource_info`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `device: Borrow, kind=physical_device, rights=0x8` |
 * | Operation | 258 |
 * | Feature | `Core` |
 * | Arguments | `index: u32`, `output: user_address`, `output_size: byte_count` |
 * | Results | `supported_size: byte_count` |
 * | Capability effects | `device: Borrow, kind=physical_device, rights=0x8` |
 * | User memory | `output: Write, len=output_size bytes, max-bytes=4096, record=device_resource_info; order=0` |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_DEVICE_RESOURCE_INFO UINT64_C(258)

/**
 * Object-local method `device_firmware_read`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `authority: Borrow, kind=device_assignment_authority, rights=0x8` |
 * | Operation | 259 |
 * | Feature | `Core` |
 * | Arguments | `query: user_address`, `output: user_address`, `capacity: byte_count` |
 * | Results | `value: u64` |
 * | Capability effects | `authority: Borrow, kind=device_assignment_authority, rights=0x8` |
 * | User memory | `query: Read, fixed-bytes=24, record=device_firmware_query; order=0`, `output: Write, len=capacity bytes, max-bytes=65536; order=1` |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_DEVICE_FIRMWARE_READ UINT64_C(259)

/**
 * Object-local method `device_claim_bundle`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `authority: Borrow, kind=device_assignment_authority, rights=0x8000` |
 * | Operation | 260 |
 * | Feature | `Core` |
 * | Arguments | `entries: user_address`, `count: element_count`, `irq_node: u32` |
 * | Results | `physical_device: handle` |
 * | Capability effects | `authority: Borrow, kind=device_assignment_authority, rights=0x8000`, `physical_device: produce, kind=physical_device, fixed=0x2803f` |
 * | User memory | `entries: Read, len=count elements, max-elements=8, element-size=16, record=device_bundle_entry; order=0` |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_DEVICE_CLAIM_BUNDLE UINT64_C(260)

/**
 * Object-local method `device_mmio`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `device: Borrow, kind=physical_device, rights=0x0` |
 * | Operation | 259 |
 * | Feature | `Core` |
 * | Arguments | `offset: u64`, `width: u32`, `operation: u32`, `value: u64` |
 * | Results | `value: u64` |
 * | Capability effects | `device: Borrow, kind=physical_device, rights=0x0` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_DEVICE_MMIO UINT64_C(259)

/**
 * Object-local method `device_irq_pending`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `device: Borrow, kind=physical_device, rights=0x4` |
 * | Operation | 260 |
 * | Feature | `Core` |
 * | Arguments | — |
 * | Results | `value: u64` |
 * | Capability effects | `device: Borrow, kind=physical_device, rights=0x4` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_DEVICE_IRQ_PENDING UINT64_C(260)

/**
 * Object-local method `device_irq_complete`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `device: Borrow, kind=physical_device, rights=0x20000` |
 * | Operation | 261 |
 * | Feature | `Core` |
 * | Arguments | `sequence: u64`, `asserted: u32` |
 * | Results | — |
 * | Capability effects | `device: Borrow, kind=physical_device, rights=0x20000` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_DEVICE_IRQ_COMPLETE UINT64_C(261)

/**
 * Object-local method `virtual_cpu_set_affinity`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `virtual_cpu: Borrow, kind=virtual_cpu, rights=0x20` |
 * | Operation | 260 |
 * | Feature | `Core` |
 * | Arguments | `affinity_words: user_address`, `word_count: element_count` |
 * | Results | — |
 * | Capability effects | `virtual_cpu: Borrow, kind=virtual_cpu, rights=0x20` |
 * | User memory | `affinity_words: Read, len=word_count elements, max-elements=4, element-size=8; order=0` |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_VIRTUAL_CPU_SET_AFFINITY UINT64_C(260)

/**
 * Object-local method `system_config`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `receiver: Borrow, kind=system_info, rights=0x10` |
 * | Operation | 259 |
 * | Feature | `Core` |
 * | Arguments | `key: u64` |
 * | Results | `value: u64` |
 * | Capability effects | `receiver: Borrow, kind=system_info, rights=0x10` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Abi` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_SYSTEM_CONFIG UINT64_C(259)

/**
 * Object-local method `object_inspector_read_details`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `inspector: Borrow, kind=object_inspector, rights=0x200000008` |
 * | Operation | 261 |
 * | Feature | `Core` |
 * | Arguments | `process_koid: u64`, `target: u64`, `cursor: u64`, `record: user_address`, `record_size: byte_count` |
 * | Results | `supported_size: byte_count` |
 * | Capability effects | `inspector: Borrow, kind=object_inspector, rights=0x200000008` |
 * | User memory | `record: Write, len=record_size bytes, max-bytes=4096, record=object_details; order=0` |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Object` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_OBJECT_INSPECTOR_READ_DETAILS UINT64_C(261)

/**
 * Object-local method `filesystem_unmount`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `directory: Borrow, kind=directory, rights=0xa0` |
 * | Operation | 275 |
 * | Feature | `Core` |
 * | Arguments | `path: user_address`, `path_length: byte_count` |
 * | Results | — |
 * | Capability effects | `directory: Borrow, kind=directory, rights=0xa0` |
 * | User memory | `path: Read, len=path_length bytes, max-bytes=4096; order=0` |
 * | Execution | `blocking=MayBlock, cancellation=Explicit, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_FILESYSTEM_UNMOUNT UINT64_C(275)

/**
 * Object-local method `filesystem_mount_ramfs`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `directory: Borrow, kind=directory, rights=0x4000000a0` |
 * | Operation | 276 |
 * | Feature | `Core` |
 * | Arguments | `path: user_address`, `path_length: byte_count` |
 * | Results | — |
 * | Capability effects | `directory: Borrow, kind=directory, rights=0x4000000a0` |
 * | User memory | `path: Read, len=path_length bytes, max-bytes=4096; order=0` |
 * | Execution | `blocking=MayBlock, cancellation=Explicit, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_FILESYSTEM_MOUNT_RAMFS UINT64_C(276)

/**
 * Object-local method `filesystem_switch_root`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `directory: Borrow, kind=directory, rights=0x4000000a0` |
 * | Operation | 277 |
 * | Feature | `Core` |
 * | Arguments | `request: user_address` |
 * | Results | — |
 * | Capability effects | `directory: Borrow, kind=directory, rights=0x4000000a0` |
 * | User memory | `request: Read, fixed-bytes=48, record=filesystem_root_switch; order=0` |
 * | Execution | `blocking=MayBlock, cancellation=Explicit, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_FILESYSTEM_SWITCH_ROOT UINT64_C(277)

/**
 * Object-local method `file_protect`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `file: Borrow, kind=file, rights=0x800000000` |
 * | Operation | 265 |
 * | Feature | `Core` |
 * | Arguments | — |
 * | Results | `protection: handle`, `byte_size: byte_count` |
 * | Capability effects | `file: Borrow, kind=file, rights=0x800000000`, `protection: produce, kind=file_protection, fixed=0x48` |
 * | User memory | — |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_FILE_PROTECT UINT64_C(265)

/**
 * Object-local method `vmar_map_file`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `vmar: Borrow, kind=vmar, rights=0x40` |
 * | Operation | 262 |
 * | Feature | `Core` |
 * | Arguments | `file: handle`, `protection: handle`, `mapping: user_address`, `mapping_size: byte_count` |
 * | Results | — |
 * | Capability effects | `vmar: Borrow, kind=vmar, rights=0x40`, `file: Borrow, kind=file, rights=0x10`, `protection: Borrow, kind=file_protection, rights=0x40` |
 * | User memory | `mapping: Read, len=mapping_size bytes, max-bytes=4096, record=private_mapping; order=0` |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_VMAR_MAP_FILE UINT64_C(262)

/**
 * Object-local method `directory_begin_read`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `directory: Borrow, kind=directory, rights=0x10` |
 * | Operation | 278 |
 * | Feature | `Core` |
 * | Arguments | — |
 * | Results | `stream: handle` |
 * | Capability effects | `directory: Borrow, kind=directory, rights=0x10`, `stream: produce, kind=directory_stream, inherited-from(directory), mask=0x1b` |
 * | User memory | — |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_DIRECTORY_BEGIN_READ UINT64_C(278)

/**
 * Object-local method `process_derive_observer`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `process: Borrow, kind=process, rights=0x806` |
 * | Operation | 258 |
 * | Feature | `Core` |
 * | Arguments | — |
 * | Results | `observer: handle` |
 * | Capability effects | `process: Borrow, kind=process, rights=0x806`, `observer: produce, kind=process, fixed=0x6` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Task` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_PROCESS_DERIVE_OBSERVER UINT64_C(258)

/**
 * Object-local method `task_group_request_stop`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `task_group: Borrow, kind=task_group, rights=0x800` |
 * | Operation | 256 |
 * | Feature | `Core` |
 * | Arguments | — |
 * | Results | — |
 * | Capability effects | `task_group: Borrow, kind=task_group, rights=0x800` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Task` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_TASK_GROUP_REQUEST_STOP UINT64_C(256)

/**
 * Object-local method `console_discard_input`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `console: Borrow, kind=console, rights=0x10` |
 * | Operation | 258 |
 * | Feature | `Core` |
 * | Arguments | — |
 * | Results | `discarded: byte_count` |
 * | Capability effects | `console: Borrow, kind=console, rights=0x10` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_CONSOLE_DISCARD_INPUT UINT64_C(258)

/**
 * Object-local method `guest_notification_configure`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `machine: Borrow, kind=virtual_machine, rights=0x20` |
 * | Operation | 265 |
 * | Feature | `Core` |
 * | Arguments | `original_base: u64`, `base: u64`, `flags: u32` |
 * | Results | — |
 * | Capability effects | `machine: Borrow, kind=virtual_machine, rights=0x20` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_GUEST_NOTIFICATION_CONFIGURE UINT64_C(265)

/**
 * Object-local method `guest_notification_access`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `machine: Borrow, kind=virtual_machine, rights=0x20` |
 * | Operation | 266 |
 * | Feature | `Core` |
 * | Arguments | `original_base: u64`, `operation: u32`, `value: u32` |
 * | Results | `value: u32` |
 * | Capability effects | `machine: Borrow, kind=virtual_machine, rights=0x20` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_GUEST_NOTIFICATION_ACCESS UINT64_C(266)

/**
 * Object-local method `vmar_lock_memory`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `vmar: Borrow, kind=vmar, rights=0x1000000000` |
 * | Operation | 263 |
 * | Feature | `Core` |
 * | Arguments | `address: u64`, `size: byte_count`, `flags: u32` |
 * | Results | — |
 * | Capability effects | `vmar: Borrow, kind=vmar, rights=0x1000000000` |
 * | User memory | — |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_VMAR_LOCK_MEMORY UINT64_C(263)

/**
 * Object-local method `vmar_unlock_memory`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `vmar: Borrow, kind=vmar, rights=0x1000000000` |
 * | Operation | 264 |
 * | Feature | `Core` |
 * | Arguments | `address: u64`, `size: byte_count`, `flags: u32` |
 * | Results | — |
 * | Capability effects | `vmar: Borrow, kind=vmar, rights=0x1000000000` |
 * | User memory | — |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_VMAR_UNLOCK_MEMORY UINT64_C(264)

/**
 * Object-local method `process_builder_set_memory_lock`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `builder: Borrow, kind=process_builder, rights=0x1000000020` |
 * | Operation | 263 |
 * | Feature | `Core` |
 * | Arguments | `enabled: u32` |
 * | Results | — |
 * | Capability effects | `builder: Borrow, kind=process_builder, rights=0x1000000020` |
 * | User memory | — |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_PROCESS_BUILDER_SET_MEMORY_LOCK UINT64_C(263)

/**
 * Object-local method `filesystem_begin_root_switch`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `directory: Borrow, kind=directory, rights=0x4000000a0` |
 * | Operation | 279 |
 * | Feature | `Core` |
 * | Arguments | `request: user_address` |
 * | Results | `transition: handle` |
 * | Capability effects | `directory: Borrow, kind=directory, rights=0x4000000a0`, `transition: produce, kind=root_transition, fixed=0x2a` |
 * | User memory | `request: Read, fixed-bytes=48, record=filesystem_root_switch; order=0` |
 * | Execution | `blocking=MayBlock, cancellation=Explicit, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_FILESYSTEM_BEGIN_ROOT_SWITCH UINT64_C(279)

/**
 * Object-local method `filesystem_finish_root_switch`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `transition: Borrow, kind=root_transition, rights=0x20` |
 * | Operation | 256 |
 * | Feature | `Core` |
 * | Arguments | `commit: u32` |
 * | Results | — |
 * | Capability effects | `transition: Borrow, kind=root_transition, rights=0x20` |
 * | User memory | — |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_FILESYSTEM_FINISH_ROOT_SWITCH UINT64_C(256)

/**
 * Object-local method `guest_memory_transfer`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `memory: Borrow, kind=guest_memory, rights=0x0` |
 * | Operation | 257 |
 * | Feature | `Core` |
 * | Arguments | `operation: u32`, `offset: u64`, `buffer: user_address`, `length: byte_count` |
 * | Results | — |
 * | Capability effects | `memory: Borrow, kind=guest_memory, rights=0x0` |
 * | User memory | `buffer: ReadWrite, len=length bytes, max-bytes=4096; order=0` |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_GUEST_MEMORY_TRANSFER UINT64_C(257)

/**
 * Object-local method `guest_memory_scalar`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `memory: Borrow, kind=guest_memory, rights=0x0` |
 * | Operation | 258 |
 * | Feature | `Core` |
 * | Arguments | `operation: u32`, `offset: u64`, `width: u32`, `value: u64` |
 * | Results | `value: u64` |
 * | Capability effects | `memory: Borrow, kind=guest_memory, rights=0x0` |
 * | User memory | — |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_GUEST_MEMORY_SCALAR UINT64_C(258)

/**
 * Object-local method `guest_interrupt_create`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `machine: Borrow, kind=virtual_machine, rights=0x20` |
 * | Operation | 267 |
 * | Feature | `Core` |
 * | Arguments | `irq: u32` |
 * | Results | `interrupt: handle` |
 * | Capability effects | `machine: Borrow, kind=virtual_machine, rights=0x20`, `interrupt: produce, kind=guest_interrupt, fixed=0x2a` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_GUEST_INTERRUPT_CREATE UINT64_C(267)

/**
 * Object-local method `guest_interrupt_set_level`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `interrupt: Borrow, kind=guest_interrupt, rights=0x20` |
 * | Operation | 256 |
 * | Feature | `Core` |
 * | Arguments | `asserted: u32` |
 * | Results | — |
 * | Capability effects | `interrupt: Borrow, kind=guest_interrupt, rights=0x20` |
 * | User memory | — |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_GUEST_INTERRUPT_SET_LEVEL UINT64_C(256)

/**
 * Object-local method `memory_inspector_read_statistics`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `inspector: Borrow, kind=memory_inspector, rights=0x8` |
 * | Operation | 257 |
 * | Feature | `Core` |
 * | Arguments | `topic: u32`, `index: u32`, `statistics: user_address`, `statistics_size: byte_count` |
 * | Results | `supported_size: byte_count` |
 * | Capability effects | `inspector: Borrow, kind=memory_inspector, rights=0x8` |
 * | User memory | `statistics: Write, len=statistics_size bytes, max-bytes=4096; order=0` |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Object` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_MEMORY_INSPECTOR_READ_STATISTICS UINT64_C(257)

/**
 * Object-local method `process_builder_retain`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `builder: Borrow, kind=process_builder, rights=0x2000000020` |
 * | Operation | 264 |
 * | Feature | `Core` |
 * | Arguments | `max_bytes: byte_count`, `timeout_ns: u64` |
 * | Results | `handle: handle` |
 * | Capability effects | `builder: Borrow, kind=process_builder, rights=0x2000000020`, `handle: produce, kind=process_retirement, fixed=0x420000000e` |
 * | User memory | — |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_PROCESS_BUILDER_RETAIN UINT64_C(264)

/**
 * Object-local method `process_retirement_get_info`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `object: Borrow, kind=process_retirement, rights=0x8` |
 * | Operation | 256 |
 * | Feature | `Core` |
 * | Arguments | `output: user_address`, `output_size: byte_count` |
 * | Results | `supported_size: byte_count` |
 * | Capability effects | `object: Borrow, kind=process_retirement, rights=0x8` |
 * | User memory | `output: Write, len=output_size bytes, max-bytes=4096, record=process_retirement_info; order=0` |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_PROCESS_RETIREMENT_GET_INFO UINT64_C(256)

/**
 * Object-local method `process_retirement_open_view`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `object: Borrow, kind=process_retirement, rights=0x200000000` |
 * | Operation | 257 |
 * | Feature | `Core` |
 * | Arguments | — |
 * | Results | `handle: handle` |
 * | Capability effects | `object: Borrow, kind=process_retirement, rights=0x200000000`, `handle: produce, kind=crash_view, fixed=0x1b` |
 * | User memory | — |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_PROCESS_RETIREMENT_OPEN_VIEW UINT64_C(257)

/**
 * Object-local method `process_retirement_release`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `object: Borrow, kind=process_retirement, rights=0x4000000000` |
 * | Operation | 258 |
 * | Feature | `Core` |
 * | Arguments | — |
 * | Results | — |
 * | Capability effects | `object: Borrow, kind=process_retirement, rights=0x4000000000` |
 * | User memory | — |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_PROCESS_RETIREMENT_RELEASE UINT64_C(258)

/**
 * Object-local method `crash_view_get_info`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `object: Borrow, kind=crash_view, rights=0x8` |
 * | Operation | 256 |
 * | Feature | `Core` |
 * | Arguments | `output: user_address`, `output_size: byte_count` |
 * | Results | `supported_size: byte_count` |
 * | Capability effects | `object: Borrow, kind=crash_view, rights=0x8` |
 * | User memory | `output: Write, len=output_size bytes, max-bytes=4096, record=process_retirement_info; order=0` |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_CRASH_VIEW_GET_INFO UINT64_C(256)

/**
 * Object-local method `crash_view_read`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `object: Borrow, kind=crash_view, rights=0x10` |
 * | Operation | 257 |
 * | Feature | `Core` |
 * | Arguments | `address: u64`, `output: user_address`, `length: byte_count` |
 * | Results | `actual_bytes: byte_count` |
 * | Capability effects | `object: Borrow, kind=crash_view, rights=0x10` |
 * | User memory | `output: Write, len=length bytes, max-bytes=65536; order=0` |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_CRASH_VIEW_READ UINT64_C(257)

/**
 * Object-local method `crash_view_thread`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `object: Borrow, kind=crash_view, rights=0x10` |
 * | Operation | 258 |
 * | Feature | `Core` |
 * | Arguments | `index: u64`, `output: user_address`, `output_size: byte_count` |
 * | Results | `supported_size: byte_count`, `next_index: u64` |
 * | Capability effects | `object: Borrow, kind=crash_view, rights=0x10` |
 * | User memory | `output: Write, len=output_size bytes, max-bytes=4096, record=crash_thread; order=0` |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_CRASH_VIEW_THREAD UINT64_C(258)

/**
 * Object-local method `crash_view_mapping`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `object: Borrow, kind=crash_view, rights=0x10` |
 * | Operation | 259 |
 * | Feature | `Core` |
 * | Arguments | `index: u64`, `output: user_address`, `output_size: byte_count` |
 * | Results | `supported_size: byte_count`, `next_index: u64` |
 * | Capability effects | `object: Borrow, kind=crash_view, rights=0x10` |
 * | User memory | `output: Write, len=output_size bytes, max-bytes=4096, record=crash_mapping; order=0` |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_CRASH_VIEW_MAPPING UINT64_C(259)

/**
 * Object-local method `crash_view_handle`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `object: Borrow, kind=crash_view, rights=0x10` |
 * | Operation | 260 |
 * | Feature | `Core` |
 * | Arguments | `index: u64`, `output: user_address`, `output_size: byte_count` |
 * | Results | `supported_size: byte_count`, `next_index: u64` |
 * | Capability effects | `object: Borrow, kind=crash_view, rights=0x10` |
 * | User memory | `output: Write, len=output_size bytes, max-bytes=4096, record=crash_handle; order=0` |
 * | Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_CRASH_VIEW_HANDLE UINT64_C(260)

/**
 * Object-local method `thread_set_current_name`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `receiver: Borrow, kind=execution_context, rights=0x20` |
 * | Operation | 265 |
 * | Feature | `Core` |
 * | Arguments | `name: user_address`, `name_size: byte_count` |
 * | Results | — |
 * | Capability effects | `receiver: Borrow, kind=execution_context, rights=0x20` |
 * | User memory | `name: Read, len=name_size bytes, max-bytes=64; order=0` |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Task` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_THREAD_SET_CURRENT_NAME UINT64_C(265)

/**
 * Object-local method `device_dma_fault_info`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `device: Borrow, kind=physical_device, rights=0x8008` |
 * | Operation | 262 |
 * | Feature | `Core` |
 * | Arguments | `output: user_address`, `output_size: byte_count` |
 * | Results | `supported_size: byte_count` |
 * | Capability effects | `device: Borrow, kind=physical_device, rights=0x8008` |
 * | User memory | `output: Write, len=output_size bytes, max-bytes=4096, record=device_dma_fault_info; order=0` |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_DEVICE_DMA_FAULT_INFO UINT64_C(262)

/**
 * Object-local method `native_block_transfer`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `block: Borrow, kind=native_block, rights=0x0` |
 * | Operation | 257 |
 * | Feature | `Core` |
 * | Arguments | `operation: u32`, `first_sector: u64`, `buffer: user_address`, `length: byte_count` |
 * | Results | — |
 * | Capability effects | `block: Borrow, kind=native_block, rights=0x0` |
 * | User memory | `buffer: ReadWrite, len=length bytes, max-bytes=524352; order=0` |
 * | Execution | `blocking=MayBlock, cancellation=Explicit, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Capability` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_NATIVE_BLOCK_TRANSFER UINT64_C(257)

/**
 * Object-local method `thread_get_current_scheduling`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `receiver: Borrow, kind=execution_context, rights=0x8` |
 * | Operation | 266 |
 * | Feature | `Core` |
 * | Arguments | `configuration: user_address` |
 * | Results | — |
 * | Capability effects | `receiver: Borrow, kind=execution_context, rights=0x8` |
 * | User memory | `configuration: Write, fixed-bytes=64, record=scheduling_parameters; order=0` |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Task` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_THREAD_GET_CURRENT_SCHEDULING UINT64_C(266)

/**
 * Object-local method `thread_set_current_scheduling`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `receiver: Borrow, kind=execution_context, rights=0x0` |
 * | Operation | 267 |
 * | Feature | `Core` |
 * | Arguments | `configuration: user_address` |
 * | Results | — |
 * | Capability effects | `receiver: Borrow, kind=execution_context, rights=0x0` |
 * | User memory | `configuration: Read, fixed-bytes=64, record=scheduling_parameters; order=0` |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Task` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_THREAD_SET_CURRENT_SCHEDULING UINT64_C(267)

/**
 * Object-local method `thread_get_scheduling`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `receiver: Borrow, kind=thread, rights=0x8` |
 * | Operation | 258 |
 * | Feature | `Core` |
 * | Arguments | `configuration: user_address` |
 * | Results | — |
 * | Capability effects | `receiver: Borrow, kind=thread, rights=0x8` |
 * | User memory | `configuration: Write, fixed-bytes=64, record=scheduling_parameters; order=0` |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Task` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_THREAD_GET_SCHEDULING UINT64_C(258)

/**
 * Object-local method `thread_set_scheduling`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `receiver: Borrow, kind=thread, rights=0x0` |
 * | Operation | 259 |
 * | Feature | `Core` |
 * | Arguments | `configuration: user_address` |
 * | Results | — |
 * | Capability effects | `receiver: Borrow, kind=thread, rights=0x0` |
 * | User memory | `configuration: Read, fixed-bytes=64, record=scheduling_parameters; order=0` |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Task` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_THREAD_SET_SCHEDULING UINT64_C(259)

/**
 * Object-local method `virtual_cpu_get_scheduling`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `receiver: Borrow, kind=virtual_cpu, rights=0x8` |
 * | Operation | 261 |
 * | Feature | `Core` |
 * | Arguments | `configuration: user_address` |
 * | Results | — |
 * | Capability effects | `receiver: Borrow, kind=virtual_cpu, rights=0x8` |
 * | User memory | `configuration: Write, fixed-bytes=64, record=scheduling_parameters; order=0` |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Task` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_VIRTUAL_CPU_GET_SCHEDULING UINT64_C(261)

/**
 * Object-local method `virtual_cpu_set_scheduling`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `receiver: Borrow, kind=virtual_cpu, rights=0x0` |
 * | Operation | 262 |
 * | Feature | `Core` |
 * | Arguments | `configuration: user_address` |
 * | Results | — |
 * | Capability effects | `receiver: Borrow, kind=virtual_cpu, rights=0x0` |
 * | User memory | `configuration: Read, fixed-bytes=64, record=scheduling_parameters; order=0` |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Task` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_VIRTUAL_CPU_SET_SCHEDULING UINT64_C(262)

/**
 * Object-local method `process_builder_set_scheduling`.
 *
 * | Property | Contract |
 * | --- | --- |
 * | Receiver | `receiver: Borrow, kind=process_builder, rights=0x0` |
 * | Operation | 265 |
 * | Feature | `Core` |
 * | Arguments | `configuration: user_address`, `delegated_rights: rights` |
 * | Results | — |
 * | Capability effects | `receiver: Borrow, kind=process_builder, rights=0x0` |
 * | User memory | `configuration: Read, fixed-bytes=64, record=scheduling_parameters; order=0` |
 * | Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
 * | Audit | `Task` |
 *
 * Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.
 *
 * Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.
 */
#define HYPER_NATIVE_OP_PROCESS_BUILDER_SET_SCHEDULING UINT64_C(265)

/**
 * Return the auxiliary-result registers defined on a failure status.
 * Bit N denotes auxiliary result N, excluding the separate signed status register.
 * Zero means no defined failure results (also returned for unknown methods and OK).
 * This is not the result-validity mask for a successful call.
 */
static inline uint64_t hyper_native_failure_result_mask(
    uint32_t object_kind, uint64_t operation, hyper_native_status_t status)
{
    if (object_kind == HYPER_NATIVE_OBJECT_BYTE_CHANNEL &&
        operation == HYPER_NATIVE_OP_BYTE_CHANNEL_READ &&
        status == HYPER_NATIVE_STATUS_BUFFER_TOO_SMALL) {
        return UINT64_C(1);
    }
    if (object_kind == HYPER_NATIVE_OBJECT_CONSOLE &&
        operation == HYPER_NATIVE_OP_CONSOLE_READ &&
        status == HYPER_NATIVE_STATUS_WOULD_BLOCK) {
        return UINT64_C(1);
    }
    if (object_kind == HYPER_NATIVE_OBJECT_CONSOLE &&
        operation == HYPER_NATIVE_OP_CONSOLE_WRITE &&
        status == HYPER_NATIVE_STATUS_WOULD_BLOCK) {
        return UINT64_C(1);
    }
    if (object_kind == HYPER_NATIVE_OBJECT_CAPABILITY_CHANNEL &&
        operation == HYPER_NATIVE_OP_CAPABILITY_CHANNEL_RECEIVE &&
        status == HYPER_NATIVE_STATUS_BUFFER_TOO_SMALL) {
        return UINT64_C(3);
    }
    return UINT64_C(0);
}

/**
 * Minimum supported prefix of `trace_config`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_TRACE_CONFIG_MIN_SIZE UINT64_C(56)
/**
 * Wire layout for `trace_config`.
 *
 * Size: 56 bytes; alignment: 8 bytes; supported minimum prefix: 56 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 *
 * Used by: `cpu_inspector_trace_start`.
 */
typedef struct hyper_native_trace_config_t {
    /**
     * Wire offset 0 bytes; width 8 bytes.
     */
    uint64_t categories;
    /**
     * Wire offset 8 bytes; width 8 bytes.
     */
    uint64_t duration_ms;
    /**
     * Wire offset 16 bytes; width 8 bytes.
     */
    uint64_t capacity;
    /**
     * Wire offset 24 bytes; width 32 bytes.
     */
    uint8_t cpu_mask[32];
} hyper_native_trace_config_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_trace_config_t) == 56, "trace_config size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_trace_config_t) == 8, "trace_config alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_trace_config_t, categories) == 0, "trace_config.categories offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_trace_config_t, duration_ms) == 8, "trace_config.duration_ms offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_trace_config_t, capacity) == 16, "trace_config.capacity offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_trace_config_t, cpu_mask) == 24, "trace_config.cpu_mask offset");

/**
 * Minimum supported prefix of `trace_info`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_TRACE_INFO_MIN_SIZE UINT64_C(160)
/**
 * Wire layout for `trace_info`.
 *
 * Size: 160 bytes; alignment: 8 bytes; supported minimum prefix: 160 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 */
typedef struct hyper_native_trace_info_t {
    /**
     * Wire offset 0 bytes; width 8 bytes.
     */
    uint64_t generation;
    /**
     * Wire offset 8 bytes; width 8 bytes.
     */
    uint64_t started_ns;
    /**
     * Wire offset 16 bytes; width 8 bytes.
     */
    uint64_t deadline_ns;
    /**
     * Wire offset 24 bytes; width 8 bytes.
     */
    uint64_t categories;
    /**
     * Wire offset 32 bytes; width 8 bytes.
     */
    uint64_t cpu_count;
    /**
     * Wire offset 40 bytes; width 8 bytes.
     */
    uint64_t capacity;
    /**
     * Wire offset 48 bytes; width 8 bytes.
     */
    uint64_t maximum_capacity;
    /**
     * Wire offset 56 bytes; width 8 bytes.
     */
    uint64_t running;
    /**
     * Wire offset 64 bytes; width 32 bytes.
     */
    uint8_t cpu_mask[32];
    /**
     * Wire offset 96 bytes; width 64 bytes.
     */
    uint8_t revision[64];
} hyper_native_trace_info_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_trace_info_t) == 160, "trace_info size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_trace_info_t) == 8, "trace_info alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_trace_info_t, generation) == 0, "trace_info.generation offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_trace_info_t, started_ns) == 8, "trace_info.started_ns offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_trace_info_t, deadline_ns) == 16, "trace_info.deadline_ns offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_trace_info_t, categories) == 24, "trace_info.categories offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_trace_info_t, cpu_count) == 32, "trace_info.cpu_count offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_trace_info_t, capacity) == 40, "trace_info.capacity offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_trace_info_t, maximum_capacity) == 48, "trace_info.maximum_capacity offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_trace_info_t, running) == 56, "trace_info.running offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_trace_info_t, cpu_mask) == 64, "trace_info.cpu_mask offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_trace_info_t, revision) == 96, "trace_info.revision offset");

/**
 * Minimum supported prefix of `trace_record`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_TRACE_RECORD_MIN_SIZE UINT64_C(328)
/**
 * Wire layout for `trace_record`.
 *
 * Size: 328 bytes; alignment: 8 bytes; supported minimum prefix: 328 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 */
typedef struct hyper_native_trace_record_t {
    /**
     * Wire offset 0 bytes; width 8 bytes.
     */
    uint64_t sequence;
    /**
     * Wire offset 8 bytes; width 8 bytes.
     */
    uint64_t timestamp_ns;
    /**
     * Wire offset 16 bytes; width 8 bytes.
     */
    uint64_t category;
    /**
     * Wire offset 24 bytes; width 8 bytes.
     */
    uint64_t field_count;
    /**
     * Wire offset 32 bytes; width 32 bytes.
     */
    uint8_t values[32];
    /**
     * Wire offset 64 bytes; width 64 bytes.
     */
    uint8_t name[64];
    /**
     * Wire offset 128 bytes; width 96 bytes.
     */
    uint8_t fields[96];
    /**
     * Wire offset 224 bytes; width 96 bytes.
     */
    uint8_t source[96];
    /**
     * Wire offset 320 bytes; width 8 bytes.
     */
    uint64_t line;
} hyper_native_trace_record_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_trace_record_t) == 328, "trace_record size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_trace_record_t) == 8, "trace_record alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_trace_record_t, sequence) == 0, "trace_record.sequence offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_trace_record_t, timestamp_ns) == 8, "trace_record.timestamp_ns offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_trace_record_t, category) == 16, "trace_record.category offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_trace_record_t, field_count) == 24, "trace_record.field_count offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_trace_record_t, values) == 32, "trace_record.values offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_trace_record_t, name) == 64, "trace_record.name offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_trace_record_t, fields) == 128, "trace_record.fields offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_trace_record_t, source) == 224, "trace_record.source offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_trace_record_t, line) == 320, "trace_record.line offset");

/**
 * Minimum supported prefix of `trace_batch`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_TRACE_BATCH_MIN_SIZE UINT64_C(2656)
/**
 * Wire layout for `trace_batch`.
 *
 * Size: 2656 bytes; alignment: 8 bytes; supported minimum prefix: 2656 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 */
typedef struct hyper_native_trace_batch_t {
    /**
     * Wire offset 0 bytes; width 8 bytes.
     */
    uint64_t count;
    /**
     * Wire offset 8 bytes; width 8 bytes.
     */
    uint64_t next;
    /**
     * Wire offset 16 bytes; width 8 bytes.
     */
    uint64_t overwritten;
    /**
     * Wire offset 24 bytes; width 8 bytes.
     */
    uint64_t incomplete;
    /**
     * Wire offset 32 bytes; width 2624 bytes.
     */
    uint8_t records[2624];
} hyper_native_trace_batch_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_trace_batch_t) == 2656, "trace_batch size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_trace_batch_t) == 8, "trace_batch alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_trace_batch_t, count) == 0, "trace_batch.count offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_trace_batch_t, next) == 8, "trace_batch.next offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_trace_batch_t, overwritten) == 16, "trace_batch.overwritten offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_trace_batch_t, incomplete) == 24, "trace_batch.incomplete offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_trace_batch_t, records) == 32, "trace_batch.records offset");

/**
 * Minimum supported prefix of `trace_thread`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_TRACE_THREAD_MIN_SIZE UINT64_C(72)
/**
 * Wire layout for `trace_thread`.
 *
 * Size: 72 bytes; alignment: 8 bytes; supported minimum prefix: 72 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 */
typedef struct hyper_native_trace_thread_t {
    /**
     * Wire offset 0 bytes; width 8 bytes.
     */
    uint64_t thread;
    /**
     * Wire offset 8 bytes; width 64 bytes.
     */
    uint8_t name[64];
} hyper_native_trace_thread_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_trace_thread_t) == 72, "trace_thread size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_trace_thread_t) == 8, "trace_thread alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_trace_thread_t, thread) == 0, "trace_thread.thread offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_trace_thread_t, name) == 8, "trace_thread.name offset");

/**
 * Minimum supported prefix of `scheduling_parameters`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_SCHEDULING_PARAMETERS_MIN_SIZE UINT64_C(64)
/**
 * Wire layout for `scheduling_parameters`.
 *
 * Size: 64 bytes; alignment: 8 bytes; supported minimum prefix: 64 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 *
 * Used by: `thread_get_current_scheduling`, `thread_set_current_scheduling`, `thread_get_scheduling`, `thread_set_scheduling`, `virtual_cpu_get_scheduling`, `virtual_cpu_set_scheduling`, `process_builder_set_scheduling`.
 */
typedef struct hyper_native_scheduling_parameters_t {
    /**
     * Wire offset 0 bytes; width 4 bytes.
     */
    uint32_t policy;
    /**
     * Wire offset 4 bytes; width 4 bytes.
     */
    uint32_t band;
    /**
     * Wire offset 8 bytes; width 4 bytes.
     */
    uint32_t role;
    /**
     * Wire offset 12 bytes; width 4 bytes.
     */
    uint32_t weight;
    /**
     * Wire offset 16 bytes; width 4 bytes.
     */
    uint32_t fifo_priority;
    /**
     * Wire offset 20 bytes; width 4 bytes.
     */
    uint32_t fair_backend;
    /**
     * Wire offset 24 bytes; width 8 bytes.
     */
    uint64_t quantum_ns;
    /**
     * Wire offset 32 bytes; width 8 bytes.
     */
    uint64_t budget_ns;
    /**
     * Wire offset 40 bytes; width 8 bytes.
     */
    uint64_t period_ns;
    /**
     * Wire offset 48 bytes; width 8 bytes.
     */
    uint64_t burst_ns;
    /**
     * Wire offset 56 bytes; width 8 bytes.
     */
    uint64_t reserved;
} hyper_native_scheduling_parameters_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_scheduling_parameters_t) == 64, "scheduling_parameters size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_scheduling_parameters_t) == 8, "scheduling_parameters alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_scheduling_parameters_t, policy) == 0, "scheduling_parameters.policy offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_scheduling_parameters_t, band) == 4, "scheduling_parameters.band offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_scheduling_parameters_t, role) == 8, "scheduling_parameters.role offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_scheduling_parameters_t, weight) == 12, "scheduling_parameters.weight offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_scheduling_parameters_t, fifo_priority) == 16, "scheduling_parameters.fifo_priority offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_scheduling_parameters_t, fair_backend) == 20, "scheduling_parameters.fair_backend offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_scheduling_parameters_t, quantum_ns) == 24, "scheduling_parameters.quantum_ns offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_scheduling_parameters_t, budget_ns) == 32, "scheduling_parameters.budget_ns offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_scheduling_parameters_t, period_ns) == 40, "scheduling_parameters.period_ns offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_scheduling_parameters_t, burst_ns) == 48, "scheduling_parameters.burst_ns offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_scheduling_parameters_t, reserved) == 56, "scheduling_parameters.reserved offset");

/**
 * Minimum supported prefix of `cpu_profile_info`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_CPU_PROFILE_INFO_MIN_SIZE UINT64_C(144)
/**
 * Wire layout for `cpu_profile_info`.
 *
 * Size: 144 bytes; alignment: 8 bytes; supported minimum prefix: 144 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 */
typedef struct hyper_native_cpu_profile_info_t {
    /**
     * Wire offset 0 bytes; width 8 bytes.
     */
    uint64_t generation;
    /**
     * Wire offset 8 bytes; width 8 bytes.
     */
    uint64_t period;
    /**
     * Wire offset 16 bytes; width 8 bytes.
     */
    uint64_t deadline_ns;
    /**
     * Wire offset 24 bytes; width 8 bytes.
     */
    uint64_t cpu_count;
    /**
     * Wire offset 32 bytes; width 8 bytes.
     */
    uint64_t running_generation;
    /**
     * Wire offset 40 bytes; width 8 bytes.
     */
    uint64_t armed_mask;
    /**
     * Wire offset 48 bytes; width 8 bytes.
     */
    uint64_t capacity;
    /**
     * Wire offset 56 bytes; width 8 bytes.
     */
    uint64_t depth;
    /**
     * Wire offset 64 bytes; width 8 bytes.
     */
    uint64_t active_cpus;
    /**
     * Wire offset 72 bytes; width 8 bytes.
     */
    uint64_t dropped;
    /**
     * Wire offset 80 bytes; width 64 bytes.
     */
    uint8_t revision[64];
} hyper_native_cpu_profile_info_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_cpu_profile_info_t) == 144, "cpu_profile_info size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_cpu_profile_info_t) == 8, "cpu_profile_info alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_cpu_profile_info_t, generation) == 0, "cpu_profile_info.generation offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_cpu_profile_info_t, period) == 8, "cpu_profile_info.period offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_cpu_profile_info_t, deadline_ns) == 16, "cpu_profile_info.deadline_ns offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_cpu_profile_info_t, cpu_count) == 24, "cpu_profile_info.cpu_count offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_cpu_profile_info_t, running_generation) == 32, "cpu_profile_info.running_generation offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_cpu_profile_info_t, armed_mask) == 40, "cpu_profile_info.armed_mask offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_cpu_profile_info_t, capacity) == 48, "cpu_profile_info.capacity offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_cpu_profile_info_t, depth) == 56, "cpu_profile_info.depth offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_cpu_profile_info_t, active_cpus) == 64, "cpu_profile_info.active_cpus offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_cpu_profile_info_t, dropped) == 72, "cpu_profile_info.dropped offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_cpu_profile_info_t, revision) == 80, "cpu_profile_info.revision offset");

/**
 * Minimum supported prefix of `cpu_profile_sample`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_CPU_PROFILE_SAMPLE_MIN_SIZE UINT64_C(192)
/**
 * Wire layout for `cpu_profile_sample`.
 *
 * Size: 192 bytes; alignment: 8 bytes; supported minimum prefix: 192 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 */
typedef struct hyper_native_cpu_profile_sample_t {
    /**
     * Wire offset 0 bytes; width 8 bytes.
     */
    uint64_t sequence;
    /**
     * Wire offset 8 bytes; width 8 bytes.
     */
    uint64_t timestamp_ns;
    /**
     * Wire offset 16 bytes; width 8 bytes.
     */
    uint64_t cpu;
    /**
     * Wire offset 24 bytes; width 8 bytes.
     */
    uint64_t world;
    /**
     * Wire offset 32 bytes; width 8 bytes.
     */
    uint64_t depth;
    /**
     * Wire offset 40 bytes; width 8 bytes.
     */
    uint64_t flags;
    /**
     * Wire offset 48 bytes; width 8 bytes.
     */
    uint64_t dropped;
    /**
     * Wire offset 56 bytes; width 8 bytes.
     */
    uint64_t period;
    /**
     * Wire offset 64 bytes; width 128 bytes.
     */
    uint8_t frames[128];
} hyper_native_cpu_profile_sample_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_cpu_profile_sample_t) == 192, "cpu_profile_sample size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_cpu_profile_sample_t) == 8, "cpu_profile_sample alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_cpu_profile_sample_t, sequence) == 0, "cpu_profile_sample.sequence offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_cpu_profile_sample_t, timestamp_ns) == 8, "cpu_profile_sample.timestamp_ns offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_cpu_profile_sample_t, cpu) == 16, "cpu_profile_sample.cpu offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_cpu_profile_sample_t, world) == 24, "cpu_profile_sample.world offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_cpu_profile_sample_t, depth) == 32, "cpu_profile_sample.depth offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_cpu_profile_sample_t, flags) == 40, "cpu_profile_sample.flags offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_cpu_profile_sample_t, dropped) == 48, "cpu_profile_sample.dropped offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_cpu_profile_sample_t, period) == 56, "cpu_profile_sample.period offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_cpu_profile_sample_t, frames) == 64, "cpu_profile_sample.frames offset");

/**
 * Minimum supported prefix of `process_retirement_info`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_PROCESS_RETIREMENT_INFO_MIN_SIZE UINT64_C(104)
/**
 * Wire layout for `process_retirement_info`.
 *
 * Size: 104 bytes; alignment: 8 bytes; supported minimum prefix: 104 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 *
 * Used by: `process_retirement_get_info`, `crash_view_get_info`.
 */
typedef struct hyper_native_process_retirement_info_t {
    /**
     * Wire offset 0 bytes; width 4 bytes.
     */
    uint32_t size;
    /**
     * Wire offset 4 bytes; width 4 bytes.
     */
    uint32_t phase;
    /**
     * Wire offset 8 bytes; width 4 bytes.
     */
    uint32_t flags;
    /**
     * Wire offset 12 bytes; width 4 bytes.
     */
    uint32_t release_reason;
    /**
     * Wire offset 16 bytes; width 8 bytes.
     */
    uint64_t process_koid;
    /**
     * Wire offset 24 bytes; width 8 bytes.
     */
    uint64_t deadline_ns;
    /**
     * Wire offset 32 bytes; width 8 bytes.
     */
    uint64_t retained_bytes;
    /**
     * Wire offset 40 bytes; width 8 bytes.
     */
    uint64_t memory_limit;
    /**
     * Wire offset 48 bytes; width 4 bytes.
     */
    uint32_t terminal_reason;
    /**
     * Wire offset 52 bytes; width 4 bytes.
     */
    uint32_t fault_class;
    /**
     * Wire offset 56 bytes; width 8 bytes.
     */
    int64_t exit_status;
    /**
     * Wire offset 64 bytes; width 8 bytes.
     */
    uint64_t fault_pc;
    /**
     * Wire offset 72 bytes; width 8 bytes.
     */
    uint64_t fault_address;
    /**
     * Wire offset 80 bytes; width 8 bytes.
     */
    uint64_t fault_syndrome;
    /**
     * Wire offset 88 bytes; width 4 bytes.
     */
    uint32_t thread_count;
    /**
     * Wire offset 92 bytes; width 4 bytes.
     */
    uint32_t mapping_count;
    /**
     * Wire offset 96 bytes; width 4 bytes.
     */
    uint32_t handle_count;
    /**
     * Wire offset 100 bytes; width 4 bytes.
     */
    uint32_t reserved;
} hyper_native_process_retirement_info_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_process_retirement_info_t) == 104, "process_retirement_info size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_process_retirement_info_t) == 8, "process_retirement_info alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_process_retirement_info_t, size) == 0, "process_retirement_info.size offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_process_retirement_info_t, phase) == 4, "process_retirement_info.phase offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_process_retirement_info_t, flags) == 8, "process_retirement_info.flags offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_process_retirement_info_t, release_reason) == 12, "process_retirement_info.release_reason offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_process_retirement_info_t, process_koid) == 16, "process_retirement_info.process_koid offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_process_retirement_info_t, deadline_ns) == 24, "process_retirement_info.deadline_ns offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_process_retirement_info_t, retained_bytes) == 32, "process_retirement_info.retained_bytes offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_process_retirement_info_t, memory_limit) == 40, "process_retirement_info.memory_limit offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_process_retirement_info_t, terminal_reason) == 48, "process_retirement_info.terminal_reason offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_process_retirement_info_t, fault_class) == 52, "process_retirement_info.fault_class offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_process_retirement_info_t, exit_status) == 56, "process_retirement_info.exit_status offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_process_retirement_info_t, fault_pc) == 64, "process_retirement_info.fault_pc offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_process_retirement_info_t, fault_address) == 72, "process_retirement_info.fault_address offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_process_retirement_info_t, fault_syndrome) == 80, "process_retirement_info.fault_syndrome offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_process_retirement_info_t, thread_count) == 88, "process_retirement_info.thread_count offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_process_retirement_info_t, mapping_count) == 92, "process_retirement_info.mapping_count offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_process_retirement_info_t, handle_count) == 96, "process_retirement_info.handle_count offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_process_retirement_info_t, reserved) == 100, "process_retirement_info.reserved offset");

/**
 * Minimum supported prefix of `crash_thread`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_CRASH_THREAD_MIN_SIZE UINT64_C(840)
/**
 * Wire layout for `crash_thread`.
 *
 * Size: 840 bytes; alignment: 8 bytes; supported minimum prefix: 840 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 *
 * Used by: `crash_view_thread`.
 */
typedef struct hyper_native_crash_thread_t {
    /**
     * Wire offset 0 bytes; width 4 bytes.
     */
    uint32_t size;
    /**
     * Wire offset 4 bytes; width 4 bytes.
     */
    uint32_t machine;
    /**
     * Wire offset 8 bytes; width 4 bytes.
     */
    uint32_t flags;
    /**
     * Wire offset 12 bytes; width 4 bytes.
     */
    uint32_t reserved;
    /**
     * Wire offset 16 bytes; width 8 bytes.
     */
    uint64_t thread_koid;
    /**
     * Wire offset 24 bytes; width 8 bytes.
     */
    uint64_t pc;
    /**
     * Wire offset 32 bytes; width 8 bytes.
     */
    uint64_t sp;
    /**
     * Wire offset 40 bytes; width 8 bytes.
     */
    uint64_t tls;
    /**
     * Wire offset 48 bytes; width 8 bytes.
     */
    uint64_t processor_state;
    /**
     * Wire offset 56 bytes; width 256 bytes.
     */
    uint8_t general[256];
    /**
     * Wire offset 312 bytes; width 512 bytes.
     */
    uint8_t vector[512];
    /**
     * Wire offset 824 bytes; width 16 bytes.
     */
    uint8_t fp_control[16];
} hyper_native_crash_thread_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_crash_thread_t) == 840, "crash_thread size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_crash_thread_t) == 8, "crash_thread alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_crash_thread_t, size) == 0, "crash_thread.size offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_crash_thread_t, machine) == 4, "crash_thread.machine offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_crash_thread_t, flags) == 8, "crash_thread.flags offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_crash_thread_t, reserved) == 12, "crash_thread.reserved offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_crash_thread_t, thread_koid) == 16, "crash_thread.thread_koid offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_crash_thread_t, pc) == 24, "crash_thread.pc offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_crash_thread_t, sp) == 32, "crash_thread.sp offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_crash_thread_t, tls) == 40, "crash_thread.tls offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_crash_thread_t, processor_state) == 48, "crash_thread.processor_state offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_crash_thread_t, general) == 56, "crash_thread.general offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_crash_thread_t, vector) == 312, "crash_thread.vector offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_crash_thread_t, fp_control) == 824, "crash_thread.fp_control offset");

/**
 * Minimum supported prefix of `crash_mapping`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_CRASH_MAPPING_MIN_SIZE UINT64_C(48)
/**
 * Wire layout for `crash_mapping`.
 *
 * Size: 48 bytes; alignment: 8 bytes; supported minimum prefix: 48 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 *
 * Used by: `crash_view_mapping`.
 */
typedef struct hyper_native_crash_mapping_t {
    /**
     * Wire offset 0 bytes; width 4 bytes.
     */
    uint32_t size;
    /**
     * Wire offset 4 bytes; width 4 bytes.
     */
    uint32_t flags;
    /**
     * Wire offset 8 bytes; width 8 bytes.
     */
    uint64_t base;
    /**
     * Wire offset 16 bytes; width 8 bytes.
     */
    uint64_t length;
    /**
     * Wire offset 24 bytes; width 8 bytes.
     */
    uint64_t object_offset;
    /**
     * Wire offset 32 bytes; width 8 bytes.
     */
    uint64_t object_koid;
    /**
     * Wire offset 40 bytes; width 4 bytes.
     */
    uint32_t permissions;
    /**
     * Wire offset 44 bytes; width 4 bytes.
     */
    uint32_t kind;
} hyper_native_crash_mapping_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_crash_mapping_t) == 48, "crash_mapping size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_crash_mapping_t) == 8, "crash_mapping alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_crash_mapping_t, size) == 0, "crash_mapping.size offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_crash_mapping_t, flags) == 4, "crash_mapping.flags offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_crash_mapping_t, base) == 8, "crash_mapping.base offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_crash_mapping_t, length) == 16, "crash_mapping.length offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_crash_mapping_t, object_offset) == 24, "crash_mapping.object_offset offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_crash_mapping_t, object_koid) == 32, "crash_mapping.object_koid offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_crash_mapping_t, permissions) == 40, "crash_mapping.permissions offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_crash_mapping_t, kind) == 44, "crash_mapping.kind offset");

/**
 * Minimum supported prefix of `crash_handle`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_CRASH_HANDLE_MIN_SIZE UINT64_C(40)
/**
 * Wire layout for `crash_handle`.
 *
 * Size: 40 bytes; alignment: 8 bytes; supported minimum prefix: 40 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 *
 * Used by: `crash_view_handle`.
 */
typedef struct hyper_native_crash_handle_t {
    /**
     * Wire offset 0 bytes; width 4 bytes.
     */
    uint32_t size;
    /**
     * Wire offset 4 bytes; width 4 bytes.
     */
    uint32_t kind;
    /**
     * Wire offset 8 bytes; width 8 bytes.
     */
    uint64_t value;
    /**
     * Wire offset 16 bytes; width 8 bytes.
     */
    uint64_t koid;
    /**
     * Wire offset 24 bytes; width 8 bytes.
     */
    uint64_t rights;
    /**
     * Wire offset 32 bytes; width 4 bytes.
     */
    uint32_t flags;
    /**
     * Wire offset 36 bytes; width 4 bytes.
     */
    uint32_t reserved;
} hyper_native_crash_handle_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_crash_handle_t) == 40, "crash_handle size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_crash_handle_t) == 8, "crash_handle alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_crash_handle_t, size) == 0, "crash_handle.size offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_crash_handle_t, kind) == 4, "crash_handle.kind offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_crash_handle_t, value) == 8, "crash_handle.value offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_crash_handle_t, koid) == 16, "crash_handle.koid offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_crash_handle_t, rights) == 24, "crash_handle.rights offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_crash_handle_t, flags) == 32, "crash_handle.flags offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_crash_handle_t, reserved) == 36, "crash_handle.reserved offset");

/**
 * Minimum supported prefix of `memory_heap_statistics`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_MEMORY_HEAP_STATISTICS_MIN_SIZE UINT64_C(176)
/**
 * Wire layout for `memory_heap_statistics`.
 *
 * Size: 176 bytes; alignment: 8 bytes; supported minimum prefix: 176 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 */
typedef struct hyper_native_memory_heap_statistics_t {
    /**
     * Wire offset 0 bytes; width 8 bytes.
     */
    uint64_t captured_at_ns;
    /**
     * Wire offset 8 bytes; width 8 bytes.
     */
    uint64_t live_objects;
    /**
     * Wire offset 16 bytes; width 8 bytes.
     */
    uint64_t live_slab_objects;
    /**
     * Wire offset 24 bytes; width 8 bytes.
     */
    uint64_t live_large_objects;
    /**
     * Wire offset 32 bytes; width 8 bytes.
     */
    uint64_t requested_bytes;
    /**
     * Wire offset 40 bytes; width 8 bytes.
     */
    uint64_t peak_objects;
    /**
     * Wire offset 48 bytes; width 8 bytes.
     */
    uint64_t peak_requested_bytes;
    /**
     * Wire offset 56 bytes; width 8 bytes.
     */
    uint64_t allocations;
    /**
     * Wire offset 64 bytes; width 8 bytes.
     */
    uint64_t failures;
    /**
     * Wire offset 72 bytes; width 8 bytes.
     */
    uint64_t cached_objects;
    /**
     * Wire offset 80 bytes; width 8 bytes.
     */
    uint64_t cache_hits;
    /**
     * Wire offset 88 bytes; width 8 bytes.
     */
    uint64_t cache_misses;
    /**
     * Wire offset 96 bytes; width 8 bytes.
     */
    uint64_t cache_refills;
    /**
     * Wire offset 104 bytes; width 8 bytes.
     */
    uint64_t cache_drains;
    /**
     * Wire offset 112 bytes; width 8 bytes.
     */
    uint64_t pressure_reclaims;
    /**
     * Wire offset 120 bytes; width 8 bytes.
     */
    uint64_t reclaimed_objects;
    /**
     * Wire offset 128 bytes; width 8 bytes.
     */
    uint64_t cache_cpus;
    /**
     * Wire offset 136 bytes; width 8 bytes.
     */
    uint64_t profiling_enabled;
    /**
     * Wire offset 144 bytes; width 8 bytes.
     */
    uint64_t page_allocations;
    /**
     * Wire offset 152 bytes; width 8 bytes.
     */
    uint64_t page_failures;
    /**
     * Wire offset 160 bytes; width 8 bytes.
     */
    uint64_t page_deallocations;
    /**
     * Wire offset 168 bytes; width 8 bytes.
     */
    uint64_t peak_allocated_pages;
} hyper_native_memory_heap_statistics_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_memory_heap_statistics_t) == 176, "memory_heap_statistics size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_memory_heap_statistics_t) == 8, "memory_heap_statistics alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_heap_statistics_t, captured_at_ns) == 0, "memory_heap_statistics.captured_at_ns offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_heap_statistics_t, live_objects) == 8, "memory_heap_statistics.live_objects offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_heap_statistics_t, live_slab_objects) == 16, "memory_heap_statistics.live_slab_objects offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_heap_statistics_t, live_large_objects) == 24, "memory_heap_statistics.live_large_objects offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_heap_statistics_t, requested_bytes) == 32, "memory_heap_statistics.requested_bytes offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_heap_statistics_t, peak_objects) == 40, "memory_heap_statistics.peak_objects offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_heap_statistics_t, peak_requested_bytes) == 48, "memory_heap_statistics.peak_requested_bytes offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_heap_statistics_t, allocations) == 56, "memory_heap_statistics.allocations offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_heap_statistics_t, failures) == 64, "memory_heap_statistics.failures offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_heap_statistics_t, cached_objects) == 72, "memory_heap_statistics.cached_objects offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_heap_statistics_t, cache_hits) == 80, "memory_heap_statistics.cache_hits offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_heap_statistics_t, cache_misses) == 88, "memory_heap_statistics.cache_misses offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_heap_statistics_t, cache_refills) == 96, "memory_heap_statistics.cache_refills offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_heap_statistics_t, cache_drains) == 104, "memory_heap_statistics.cache_drains offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_heap_statistics_t, pressure_reclaims) == 112, "memory_heap_statistics.pressure_reclaims offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_heap_statistics_t, reclaimed_objects) == 120, "memory_heap_statistics.reclaimed_objects offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_heap_statistics_t, cache_cpus) == 128, "memory_heap_statistics.cache_cpus offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_heap_statistics_t, profiling_enabled) == 136, "memory_heap_statistics.profiling_enabled offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_heap_statistics_t, page_allocations) == 144, "memory_heap_statistics.page_allocations offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_heap_statistics_t, page_failures) == 152, "memory_heap_statistics.page_failures offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_heap_statistics_t, page_deallocations) == 160, "memory_heap_statistics.page_deallocations offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_heap_statistics_t, peak_allocated_pages) == 168, "memory_heap_statistics.peak_allocated_pages offset");

/**
 * Minimum supported prefix of `memory_buddy_statistics`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_MEMORY_BUDDY_STATISTICS_MIN_SIZE UINT64_C(24)
/**
 * Wire layout for `memory_buddy_statistics`.
 *
 * Size: 24 bytes; alignment: 8 bytes; supported minimum prefix: 24 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 */
typedef struct hyper_native_memory_buddy_statistics_t {
    /**
     * Wire offset 0 bytes; width 8 bytes.
     */
    uint64_t captured_at_ns;
    /**
     * Wire offset 8 bytes; width 8 bytes.
     */
    uint64_t order;
    /**
     * Wire offset 16 bytes; width 8 bytes.
     */
    uint64_t free_blocks;
} hyper_native_memory_buddy_statistics_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_memory_buddy_statistics_t) == 24, "memory_buddy_statistics size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_memory_buddy_statistics_t) == 8, "memory_buddy_statistics alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_buddy_statistics_t, captured_at_ns) == 0, "memory_buddy_statistics.captured_at_ns offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_buddy_statistics_t, order) == 8, "memory_buddy_statistics.order offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_buddy_statistics_t, free_blocks) == 16, "memory_buddy_statistics.free_blocks offset");

/**
 * Minimum supported prefix of `memory_owner_statistics`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_MEMORY_OWNER_STATISTICS_MIN_SIZE UINT64_C(48)
/**
 * Wire layout for `memory_owner_statistics`.
 *
 * Size: 48 bytes; alignment: 8 bytes; supported minimum prefix: 48 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 */
typedef struct hyper_native_memory_owner_statistics_t {
    /**
     * Wire offset 0 bytes; width 8 bytes.
     */
    uint64_t captured_at_ns;
    /**
     * Wire offset 8 bytes; width 8 bytes.
     */
    uint64_t resident_pages;
    /**
     * Wire offset 16 bytes; width 8 bytes.
     */
    uint64_t peak_pages;
    /**
     * Wire offset 24 bytes; width 8 bytes.
     */
    uint64_t allocations;
    /**
     * Wire offset 32 bytes; width 8 bytes.
     */
    uint64_t failures;
    /**
     * Wire offset 40 bytes; width 8 bytes.
     */
    uint64_t deallocations;
} hyper_native_memory_owner_statistics_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_memory_owner_statistics_t) == 48, "memory_owner_statistics size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_memory_owner_statistics_t) == 8, "memory_owner_statistics alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_owner_statistics_t, captured_at_ns) == 0, "memory_owner_statistics.captured_at_ns offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_owner_statistics_t, resident_pages) == 8, "memory_owner_statistics.resident_pages offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_owner_statistics_t, peak_pages) == 16, "memory_owner_statistics.peak_pages offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_owner_statistics_t, allocations) == 24, "memory_owner_statistics.allocations offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_owner_statistics_t, failures) == 32, "memory_owner_statistics.failures offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_owner_statistics_t, deallocations) == 40, "memory_owner_statistics.deallocations offset");

/**
 * Minimum supported prefix of `memory_file_cache_statistics`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_MEMORY_FILE_CACHE_STATISTICS_MIN_SIZE UINT64_C(120)
/**
 * Wire layout for `memory_file_cache_statistics`.
 *
 * Size: 120 bytes; alignment: 8 bytes; supported minimum prefix: 120 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 */
typedef struct hyper_native_memory_file_cache_statistics_t {
    /**
     * Wire offset 0 bytes; width 8 bytes.
     */
    uint64_t captured_at_ns;
    /**
     * Wire offset 8 bytes; width 8 bytes.
     */
    uint64_t payload_pages;
    /**
     * Wire offset 16 bytes; width 8 bytes.
     */
    uint64_t metadata_slab_pages;
    /**
     * Wire offset 24 bytes; width 8 bytes.
     */
    uint64_t metadata_large_pages;
    /**
     * Wire offset 32 bytes; width 8 bytes.
     */
    uint64_t index_capacity;
    /**
     * Wire offset 40 bytes; width 8 bytes.
     */
    uint64_t clean_entries;
    /**
     * Wire offset 48 bytes; width 8 bytes.
     */
    uint64_t loading_entries;
    /**
     * Wire offset 56 bytes; width 8 bytes.
     */
    uint64_t live_payloads;
    /**
     * Wire offset 64 bytes; width 8 bytes.
     */
    uint64_t metadata_budget_pages;
    /**
     * Wire offset 72 bytes; width 8 bytes.
     */
    uint64_t table_present;
    /**
     * Wire offset 80 bytes; width 8 bytes.
     */
    uint64_t admission_enabled;
    /**
     * Wire offset 88 bytes; width 8 bytes.
     */
    uint64_t admission_paused;
    /**
     * Wire offset 96 bytes; width 8 bytes.
     */
    uint64_t maintenance_pending;
    /**
     * Wire offset 104 bytes; width 8 bytes.
     */
    uint64_t maintenance_running;
    /**
     * Wire offset 112 bytes; width 8 bytes.
     */
    uint64_t growth_requested;
} hyper_native_memory_file_cache_statistics_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_memory_file_cache_statistics_t) == 120, "memory_file_cache_statistics size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_memory_file_cache_statistics_t) == 8, "memory_file_cache_statistics alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_file_cache_statistics_t, captured_at_ns) == 0, "memory_file_cache_statistics.captured_at_ns offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_file_cache_statistics_t, payload_pages) == 8, "memory_file_cache_statistics.payload_pages offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_file_cache_statistics_t, metadata_slab_pages) == 16, "memory_file_cache_statistics.metadata_slab_pages offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_file_cache_statistics_t, metadata_large_pages) == 24, "memory_file_cache_statistics.metadata_large_pages offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_file_cache_statistics_t, index_capacity) == 32, "memory_file_cache_statistics.index_capacity offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_file_cache_statistics_t, clean_entries) == 40, "memory_file_cache_statistics.clean_entries offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_file_cache_statistics_t, loading_entries) == 48, "memory_file_cache_statistics.loading_entries offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_file_cache_statistics_t, live_payloads) == 56, "memory_file_cache_statistics.live_payloads offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_file_cache_statistics_t, metadata_budget_pages) == 64, "memory_file_cache_statistics.metadata_budget_pages offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_file_cache_statistics_t, table_present) == 72, "memory_file_cache_statistics.table_present offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_file_cache_statistics_t, admission_enabled) == 80, "memory_file_cache_statistics.admission_enabled offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_file_cache_statistics_t, admission_paused) == 88, "memory_file_cache_statistics.admission_paused offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_file_cache_statistics_t, maintenance_pending) == 96, "memory_file_cache_statistics.maintenance_pending offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_file_cache_statistics_t, maintenance_running) == 104, "memory_file_cache_statistics.maintenance_running offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_file_cache_statistics_t, growth_requested) == 112, "memory_file_cache_statistics.growth_requested offset");

/**
 * Minimum supported prefix of `memory_heap_class_statistics`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_MEMORY_HEAP_CLASS_STATISTICS_MIN_SIZE UINT64_C(88)
/**
 * Wire layout for `memory_heap_class_statistics`.
 *
 * Size: 88 bytes; alignment: 8 bytes; supported minimum prefix: 88 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 */
typedef struct hyper_native_memory_heap_class_statistics_t {
    /**
     * Wire offset 0 bytes; width 8 bytes.
     */
    uint64_t captured_at_ns;
    /**
     * Wire offset 8 bytes; width 8 bytes.
     */
    uint64_t pool;
    /**
     * Wire offset 16 bytes; width 8 bytes.
     */
    uint64_t object_size;
    /**
     * Wire offset 24 bytes; width 8 bytes.
     */
    uint64_t pages;
    /**
     * Wire offset 32 bytes; width 8 bytes.
     */
    uint64_t capacity;
    /**
     * Wire offset 40 bytes; width 8 bytes.
     */
    uint64_t reserved;
    /**
     * Wire offset 48 bytes; width 8 bytes.
     */
    uint64_t cached;
    /**
     * Wire offset 56 bytes; width 8 bytes.
     */
    uint64_t live;
    /**
     * Wire offset 64 bytes; width 8 bytes.
     */
    uint64_t requested_bytes;
    /**
     * Wire offset 72 bytes; width 8 bytes.
     */
    uint64_t allocations;
    /**
     * Wire offset 80 bytes; width 8 bytes.
     */
    uint64_t deallocations;
} hyper_native_memory_heap_class_statistics_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_memory_heap_class_statistics_t) == 88, "memory_heap_class_statistics size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_memory_heap_class_statistics_t) == 8, "memory_heap_class_statistics alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_heap_class_statistics_t, captured_at_ns) == 0, "memory_heap_class_statistics.captured_at_ns offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_heap_class_statistics_t, pool) == 8, "memory_heap_class_statistics.pool offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_heap_class_statistics_t, object_size) == 16, "memory_heap_class_statistics.object_size offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_heap_class_statistics_t, pages) == 24, "memory_heap_class_statistics.pages offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_heap_class_statistics_t, capacity) == 32, "memory_heap_class_statistics.capacity offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_heap_class_statistics_t, reserved) == 40, "memory_heap_class_statistics.reserved offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_heap_class_statistics_t, cached) == 48, "memory_heap_class_statistics.cached offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_heap_class_statistics_t, live) == 56, "memory_heap_class_statistics.live offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_heap_class_statistics_t, requested_bytes) == 64, "memory_heap_class_statistics.requested_bytes offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_heap_class_statistics_t, allocations) == 72, "memory_heap_class_statistics.allocations offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_heap_class_statistics_t, deallocations) == 80, "memory_heap_class_statistics.deallocations offset");

/**
 * Minimum supported prefix of `filesystem_root_switch`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_FILESYSTEM_ROOT_SWITCH_MIN_SIZE UINT64_C(48)
/**
 * Wire layout for `filesystem_root_switch`.
 *
 * Size: 48 bytes; alignment: 8 bytes; supported minimum prefix: 48 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 *
 * Used by: `filesystem_switch_root`, `filesystem_begin_root_switch`.
 */
typedef struct hyper_native_filesystem_root_switch_t {
    /**
     * Wire offset 0 bytes; width 8 bytes.
     */
    uint64_t new_root;
    /**
     * Wire offset 8 bytes; width 8 bytes.
     */
    uint64_t new_root_length;
    /**
     * Wire offset 16 bytes; width 8 bytes.
     */
    uint64_t put_old;
    /**
     * Wire offset 24 bytes; width 8 bytes.
     */
    uint64_t put_old_length;
    /**
     * Wire offset 32 bytes; width 8 bytes.
     */
    uint64_t moves;
    /**
     * Wire offset 40 bytes; width 8 bytes.
     */
    uint64_t move_count;
} hyper_native_filesystem_root_switch_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_filesystem_root_switch_t) == 48, "filesystem_root_switch size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_filesystem_root_switch_t) == 8, "filesystem_root_switch alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_filesystem_root_switch_t, new_root) == 0, "filesystem_root_switch.new_root offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_filesystem_root_switch_t, new_root_length) == 8, "filesystem_root_switch.new_root_length offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_filesystem_root_switch_t, put_old) == 16, "filesystem_root_switch.put_old offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_filesystem_root_switch_t, put_old_length) == 24, "filesystem_root_switch.put_old_length offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_filesystem_root_switch_t, moves) == 32, "filesystem_root_switch.moves offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_filesystem_root_switch_t, move_count) == 40, "filesystem_root_switch.move_count offset");

/**
 * Minimum supported prefix of `filesystem_mount_move`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_FILESYSTEM_MOUNT_MOVE_MIN_SIZE UINT64_C(32)
/**
 * Wire layout for `filesystem_mount_move`.
 *
 * Size: 32 bytes; alignment: 8 bytes; supported minimum prefix: 32 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 */
typedef struct hyper_native_filesystem_mount_move_t {
    /**
     * Wire offset 0 bytes; width 8 bytes.
     */
    uint64_t source;
    /**
     * Wire offset 8 bytes; width 8 bytes.
     */
    uint64_t source_length;
    /**
     * Wire offset 16 bytes; width 8 bytes.
     */
    uint64_t target;
    /**
     * Wire offset 24 bytes; width 8 bytes.
     */
    uint64_t target_length;
} hyper_native_filesystem_mount_move_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_filesystem_mount_move_t) == 32, "filesystem_mount_move size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_filesystem_mount_move_t) == 8, "filesystem_mount_move alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_filesystem_mount_move_t, source) == 0, "filesystem_mount_move.source offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_filesystem_mount_move_t, source_length) == 8, "filesystem_mount_move.source_length offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_filesystem_mount_move_t, target) == 16, "filesystem_mount_move.target offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_filesystem_mount_move_t, target_length) == 24, "filesystem_mount_move.target_length offset");

/**
 * Minimum supported prefix of `object_details`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_OBJECT_DETAILS_MIN_SIZE UINT64_C(88)
/**
 * Wire layout for `object_details`.
 *
 * Size: 88 bytes; alignment: 8 bytes; supported minimum prefix: 88 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 *
 * Used by: `object_inspector_read_details`.
 */
typedef struct hyper_native_object_details_t {
    /**
     * Wire offset 0 bytes; width 8 bytes.
     */
    uint64_t koid;
    /**
     * Wire offset 8 bytes; width 4 bytes.
     */
    uint32_t object_kind;
    /**
     * Wire offset 12 bytes; width 4 bytes.
     */
    uint32_t record_kind;
    /**
     * Wire offset 16 bytes; width 8 bytes.
     */
    uint64_t next_cursor;
    /**
     * Wire offset 24 bytes; width 64 bytes.
     */
    uint8_t payload[64];
} hyper_native_object_details_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_object_details_t) == 88, "object_details size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_object_details_t) == 8, "object_details alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_object_details_t, koid) == 0, "object_details.koid offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_object_details_t, object_kind) == 8, "object_details.object_kind offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_object_details_t, record_kind) == 12, "object_details.record_kind offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_object_details_t, next_cursor) == 16, "object_details.next_cursor offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_object_details_t, payload) == 24, "object_details.payload offset");

/**
 * Minimum supported prefix of `device_dma_fault_info`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_DEVICE_DMA_FAULT_INFO_MIN_SIZE UINT64_C(40)
/**
 * Wire layout for `device_dma_fault_info`.
 *
 * Size: 40 bytes; alignment: 8 bytes; supported minimum prefix: 40 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 *
 * Used by: `device_dma_fault_info`.
 */
typedef struct hyper_native_device_dma_fault_info_t {
    /**
     * Wire offset 0 bytes; width 8 bytes.
     */
    uint64_t timestamp_ns;
    /**
     * Wire offset 8 bytes; width 8 bytes.
     */
    uint64_t address;
    /**
     * Wire offset 16 bytes; width 4 bytes.
     */
    uint32_t stream_id;
    /**
     * Wire offset 20 bytes; width 4 bytes.
     */
    uint32_t source;
    /**
     * Wire offset 24 bytes; width 4 bytes.
     */
    uint32_t scope;
    /**
     * Wire offset 28 bytes; width 4 bytes.
     */
    uint32_t code;
    /**
     * Wire offset 32 bytes; width 4 bytes.
     */
    uint32_t flags;
    /**
     * Wire offset 36 bytes; width 4 bytes.
     */
    uint32_t reserved;
} hyper_native_device_dma_fault_info_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_device_dma_fault_info_t) == 40, "device_dma_fault_info size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_device_dma_fault_info_t) == 8, "device_dma_fault_info alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_device_dma_fault_info_t, timestamp_ns) == 0, "device_dma_fault_info.timestamp_ns offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_device_dma_fault_info_t, address) == 8, "device_dma_fault_info.address offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_device_dma_fault_info_t, stream_id) == 16, "device_dma_fault_info.stream_id offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_device_dma_fault_info_t, source) == 20, "device_dma_fault_info.source offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_device_dma_fault_info_t, scope) == 24, "device_dma_fault_info.scope offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_device_dma_fault_info_t, code) == 28, "device_dma_fault_info.code offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_device_dma_fault_info_t, flags) == 32, "device_dma_fault_info.flags offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_device_dma_fault_info_t, reserved) == 36, "device_dma_fault_info.reserved offset");

/**
 * Minimum supported prefix of `device_profile_info`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_DEVICE_PROFILE_INFO_MIN_SIZE UINT64_C(32)
/**
 * Wire layout for `device_profile_info`.
 *
 * Size: 32 bytes; alignment: 8 bytes; supported minimum prefix: 32 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 *
 * Used by: `device_profile_info`.
 */
typedef struct hyper_native_device_profile_info_t {
    /**
     * Wire offset 0 bytes; width 4 bytes.
     */
    uint32_t profile;
    /**
     * Wire offset 4 bytes; width 4 bytes.
     */
    uint32_t interrupt_count;
    /**
     * Wire offset 8 bytes; width 4 bytes.
     */
    uint32_t resource_count;
    /**
     * Wire offset 12 bytes; width 4 bytes.
     */
    uint32_t pci_identity;
    /**
     * Wire offset 16 bytes; width 8 bytes.
     */
    uint64_t dma_bus_offset;
    /**
     * Wire offset 24 bytes; width 8 bytes.
     */
    uint64_t aperture_size;
} hyper_native_device_profile_info_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_device_profile_info_t) == 32, "device_profile_info size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_device_profile_info_t) == 8, "device_profile_info alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_device_profile_info_t, profile) == 0, "device_profile_info.profile offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_device_profile_info_t, interrupt_count) == 4, "device_profile_info.interrupt_count offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_device_profile_info_t, resource_count) == 8, "device_profile_info.resource_count offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_device_profile_info_t, pci_identity) == 12, "device_profile_info.pci_identity offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_device_profile_info_t, dma_bus_offset) == 16, "device_profile_info.dma_bus_offset offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_device_profile_info_t, aperture_size) == 24, "device_profile_info.aperture_size offset");

/**
 * Minimum supported prefix of `device_firmware_query`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_DEVICE_FIRMWARE_QUERY_MIN_SIZE UINT64_C(24)
/**
 * Wire layout for `device_firmware_query`.
 *
 * Size: 24 bytes; alignment: 8 bytes; supported minimum prefix: 24 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 *
 * Used by: `device_firmware_read`.
 */
typedef struct hyper_native_device_firmware_query_t {
    /**
     * Wire offset 0 bytes; width 4 bytes.
     */
    uint32_t node;
    /**
     * Wire offset 4 bytes; width 4 bytes.
     */
    uint32_t field;
    /**
     * Wire offset 8 bytes; width 8 bytes.
     */
    uint64_t name_address;
    /**
     * Wire offset 16 bytes; width 8 bytes.
     */
    uint64_t name_length;
} hyper_native_device_firmware_query_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_device_firmware_query_t) == 24, "device_firmware_query size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_device_firmware_query_t) == 8, "device_firmware_query alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_device_firmware_query_t, node) == 0, "device_firmware_query.node offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_device_firmware_query_t, field) == 4, "device_firmware_query.field offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_device_firmware_query_t, name_address) == 8, "device_firmware_query.name_address offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_device_firmware_query_t, name_length) == 16, "device_firmware_query.name_length offset");

/**
 * Minimum supported prefix of `device_firmware_info`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_DEVICE_FIRMWARE_INFO_MIN_SIZE UINT64_C(32)
/**
 * Wire layout for `device_firmware_info`.
 *
 * Size: 32 bytes; alignment: 8 bytes; supported minimum prefix: 32 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 */
typedef struct hyper_native_device_firmware_info_t {
    /**
     * Wire offset 0 bytes; width 4 bytes.
     */
    uint32_t flags;
    /**
     * Wire offset 4 bytes; width 4 bytes.
     */
    uint32_t register_count;
    /**
     * Wire offset 8 bytes; width 4 bytes.
     */
    uint32_t irq_number;
    /**
     * Wire offset 12 bytes; width 4 bytes.
     */
    uint32_t irq_trigger;
    /**
     * Wire offset 16 bytes; width 8 bytes.
     */
    uint64_t reserved0;
    /**
     * Wire offset 24 bytes; width 8 bytes.
     */
    uint64_t reserved1;
} hyper_native_device_firmware_info_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_device_firmware_info_t) == 32, "device_firmware_info size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_device_firmware_info_t) == 8, "device_firmware_info alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_device_firmware_info_t, flags) == 0, "device_firmware_info.flags offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_device_firmware_info_t, register_count) == 4, "device_firmware_info.register_count offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_device_firmware_info_t, irq_number) == 8, "device_firmware_info.irq_number offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_device_firmware_info_t, irq_trigger) == 12, "device_firmware_info.irq_trigger offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_device_firmware_info_t, reserved0) == 16, "device_firmware_info.reserved0 offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_device_firmware_info_t, reserved1) == 24, "device_firmware_info.reserved1 offset");

/**
 * Minimum supported prefix of `device_bundle_entry`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_DEVICE_BUNDLE_ENTRY_MIN_SIZE UINT64_C(16)
/**
 * Wire layout for `device_bundle_entry`.
 *
 * Size: 16 bytes; alignment: 8 bytes; supported minimum prefix: 16 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 *
 * Used by: `device_claim_bundle`.
 */
typedef struct hyper_native_device_bundle_entry_t {
    /**
     * Wire offset 0 bytes; width 4 bytes.
     */
    uint32_t node;
    /**
     * Wire offset 4 bytes; width 4 bytes.
     */
    uint32_t resource;
    /**
     * Wire offset 8 bytes; width 8 bytes.
     */
    uint64_t offset;
} hyper_native_device_bundle_entry_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_device_bundle_entry_t) == 16, "device_bundle_entry size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_device_bundle_entry_t) == 8, "device_bundle_entry alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_device_bundle_entry_t, node) == 0, "device_bundle_entry.node offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_device_bundle_entry_t, resource) == 4, "device_bundle_entry.resource offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_device_bundle_entry_t, offset) == 8, "device_bundle_entry.offset offset");

/**
 * Minimum supported prefix of `device_resource_info`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_DEVICE_RESOURCE_INFO_MIN_SIZE UINT64_C(32)
/**
 * Wire layout for `device_resource_info`.
 *
 * Size: 32 bytes; alignment: 8 bytes; supported minimum prefix: 32 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 *
 * Used by: `device_resource_info`.
 */
typedef struct hyper_native_device_resource_info_t {
    /**
     * Wire offset 0 bytes; width 4 bytes.
     */
    uint32_t kind;
    /**
     * Wire offset 4 bytes; width 4 bytes.
     */
    uint32_t flags;
    /**
     * Wire offset 8 bytes; width 8 bytes.
     */
    uint64_t offset;
    /**
     * Wire offset 16 bytes; width 8 bytes.
     */
    uint64_t length;
    /**
     * Wire offset 24 bytes; width 8 bytes.
     */
    uint64_t bus_address;
} hyper_native_device_resource_info_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_device_resource_info_t) == 32, "device_resource_info size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_device_resource_info_t) == 8, "device_resource_info alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_device_resource_info_t, kind) == 0, "device_resource_info.kind offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_device_resource_info_t, flags) == 4, "device_resource_info.flags offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_device_resource_info_t, offset) == 8, "device_resource_info.offset offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_device_resource_info_t, length) == 16, "device_resource_info.length offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_device_resource_info_t, bus_address) == 24, "device_resource_info.bus_address offset");

/**
 * Minimum supported prefix of `physical_device_info`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_PHYSICAL_DEVICE_INFO_MIN_SIZE UINT64_C(16)
/**
 * Wire layout for `physical_device_info`.
 *
 * Size: 16 bytes; alignment: 8 bytes; supported minimum prefix: 16 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 *
 * Used by: `physical_device_info`.
 */
typedef struct hyper_native_physical_device_info_t {
    /**
     * Wire offset 0 bytes; width 4 bytes.
     */
    uint32_t device_id;
    /**
     * Wire offset 4 bytes; width 4 bytes.
     */
    uint32_t transport_version;
    /**
     * Wire offset 8 bytes; width 8 bytes.
     */
    uint64_t mmio_size;
} hyper_native_physical_device_info_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_physical_device_info_t) == 16, "physical_device_info size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_physical_device_info_t) == 8, "physical_device_info alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_physical_device_info_t, device_id) == 0, "physical_device_info.device_id offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_physical_device_info_t, transport_version) == 4, "physical_device_info.transport_version offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_physical_device_info_t, mmio_size) == 8, "physical_device_info.mmio_size offset");

/**
 * Minimum supported prefix of `dma_extent`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_DMA_EXTENT_MIN_SIZE UINT64_C(16)
/**
 * Wire layout for `dma_extent`.
 *
 * Size: 16 bytes; alignment: 8 bytes; supported minimum prefix: 16 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 *
 * Used by: `vmo_get_dma_extent`.
 */
typedef struct hyper_native_dma_extent_t {
    /**
     * Wire offset 0 bytes; width 8 bytes.
     */
    uint64_t physical_base;
    /**
     * Wire offset 8 bytes; width 8 bytes.
     */
    uint64_t length;
} hyper_native_dma_extent_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_dma_extent_t) == 16, "dma_extent size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_dma_extent_t) == 8, "dma_extent alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_dma_extent_t, physical_base) == 0, "dma_extent.physical_base offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_dma_extent_t, length) == 8, "dma_extent.length offset");

/**
 * Minimum supported prefix of `virtual_cpu_mmio_request`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_VIRTUAL_CPU_MMIO_REQUEST_MIN_SIZE UINT64_C(48)
/**
 * Wire layout for `virtual_cpu_mmio_request`.
 *
 * Size: 48 bytes; alignment: 8 bytes; supported minimum prefix: 48 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 *
 * Used by: `virtual_cpu_get_mmio_request`.
 */
typedef struct hyper_native_virtual_cpu_mmio_request_t {
    /**
     * Wire offset 0 bytes; width 8 bytes.
     */
    uint64_t id;
    /**
     * Wire offset 8 bytes; width 8 bytes.
     */
    uint64_t device;
    /**
     * Wire offset 16 bytes; width 8 bytes.
     */
    uint64_t address;
    /**
     * Wire offset 24 bytes; width 8 bytes.
     */
    uint64_t value;
    /**
     * Wire offset 32 bytes; width 4 bytes.
     */
    uint32_t operation;
    /**
     * Wire offset 36 bytes; width 4 bytes.
     */
    uint32_t width;
    /**
     * Wire offset 40 bytes; width 8 bytes.
     */
    uint64_t reserved;
} hyper_native_virtual_cpu_mmio_request_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_virtual_cpu_mmio_request_t) == 48, "virtual_cpu_mmio_request size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_virtual_cpu_mmio_request_t) == 8, "virtual_cpu_mmio_request alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_cpu_mmio_request_t, id) == 0, "virtual_cpu_mmio_request.id offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_cpu_mmio_request_t, device) == 8, "virtual_cpu_mmio_request.device offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_cpu_mmio_request_t, address) == 16, "virtual_cpu_mmio_request.address offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_cpu_mmio_request_t, value) == 24, "virtual_cpu_mmio_request.value offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_cpu_mmio_request_t, operation) == 32, "virtual_cpu_mmio_request.operation offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_cpu_mmio_request_t, width) == 36, "virtual_cpu_mmio_request.width offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_cpu_mmio_request_t, reserved) == 40, "virtual_cpu_mmio_request.reserved offset");

/**
 * Minimum supported prefix of `private_mapping`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_PRIVATE_MAPPING_MIN_SIZE UINT64_C(48)
/**
 * Wire layout for `private_mapping`.
 *
 * Size: 48 bytes; alignment: 8 bytes; supported minimum prefix: 48 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 *
 * Used by: `vmar_map_private`, `vmar_map_file`.
 */
typedef struct hyper_native_private_mapping_t {
    /**
     * Wire offset 0 bytes; width 8 bytes.
     */
    uint64_t source_offset;
    /**
     * Wire offset 8 bytes; width 8 bytes.
     */
    uint64_t source_length;
    /**
     * Wire offset 16 bytes; width 8 bytes.
     */
    uint64_t address;
    /**
     * Wire offset 24 bytes; width 8 bytes.
     */
    uint64_t size;
    /**
     * Wire offset 32 bytes; width 8 bytes.
     */
    uint64_t data_offset;
    /**
     * Wire offset 40 bytes; width 4 bytes.
     */
    uint32_t permissions;
    /**
     * Wire offset 44 bytes; width 4 bytes.
     */
    uint32_t mode;
} hyper_native_private_mapping_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_private_mapping_t) == 48, "private_mapping size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_private_mapping_t) == 8, "private_mapping alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_private_mapping_t, source_offset) == 0, "private_mapping.source_offset offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_private_mapping_t, source_length) == 8, "private_mapping.source_length offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_private_mapping_t, address) == 16, "private_mapping.address offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_private_mapping_t, size) == 24, "private_mapping.size offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_private_mapping_t, data_offset) == 32, "private_mapping.data_offset offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_private_mapping_t, permissions) == 40, "private_mapping.permissions offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_private_mapping_t, mode) == 44, "private_mapping.mode offset");

/**
 * Minimum supported prefix of `virtual_machine_platform_info`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_VIRTUAL_MACHINE_PLATFORM_INFO_MIN_SIZE UINT64_C(32)
/**
 * Wire layout for `virtual_machine_platform_info`.
 *
 * Size: 32 bytes; alignment: 8 bytes; supported minimum prefix: 32 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 *
 * Used by: `virtual_machine_creation_lease_get_platform_info`.
 */
typedef struct hyper_native_virtual_machine_platform_info_t {
    /**
     * Wire offset 0 bytes; width 4 bytes.
     */
    uint32_t architecture;
    /**
     * Wire offset 4 bytes; width 4 bytes.
     */
    uint32_t platform_profile;
    /**
     * Wire offset 8 bytes; width 8 bytes.
     */
    uint64_t counter_frequency_hz;
    /**
     * Wire offset 16 bytes; width 8 bytes.
     */
    uint64_t riscv_isa;
    /**
     * Wire offset 24 bytes; width 8 bytes.
     */
    uint64_t aarch64_gic_version;
} hyper_native_virtual_machine_platform_info_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_virtual_machine_platform_info_t) == 32, "virtual_machine_platform_info size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_virtual_machine_platform_info_t) == 8, "virtual_machine_platform_info alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_machine_platform_info_t, architecture) == 0, "virtual_machine_platform_info.architecture offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_machine_platform_info_t, platform_profile) == 4, "virtual_machine_platform_info.platform_profile offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_machine_platform_info_t, counter_frequency_hz) == 8, "virtual_machine_platform_info.counter_frequency_hz offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_machine_platform_info_t, riscv_isa) == 16, "virtual_machine_platform_info.riscv_isa offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_machine_platform_info_t, aarch64_gic_version) == 24, "virtual_machine_platform_info.aarch64_gic_version offset");

/**
 * Minimum supported prefix of `file_metadata`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_FILE_METADATA_MIN_SIZE UINT64_C(112)
/**
 * Wire layout for `file_metadata`.
 *
 * Size: 112 bytes; alignment: 8 bytes; supported minimum prefix: 112 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 *
 * Used by: `directory_get_metadata`, `file_get_metadata`, `directory_get_self_metadata`.
 */
typedef struct hyper_native_file_metadata_t {
    /**
     * Wire offset 0 bytes; width 8 bytes.
     */
    uint64_t filesystem_id;
    /**
     * Wire offset 8 bytes; width 8 bytes.
     */
    uint64_t mount_id;
    /**
     * Wire offset 16 bytes; width 8 bytes.
     */
    uint64_t node_id;
    /**
     * Wire offset 24 bytes; width 8 bytes.
     */
    uint64_t size;
    /**
     * Wire offset 32 bytes; width 4 bytes.
     */
    uint32_t mode;
    /**
     * Wire offset 36 bytes; width 4 bytes.
     */
    uint32_t kind;
    /**
     * Wire offset 40 bytes; width 4 bytes.
     */
    uint32_t valid_times;
    /**
     * Wire offset 44 bytes; width 4 bytes.
     */
    uint32_t reserved;
    /**
     * Wire offset 48 bytes; width 8 bytes.
     */
    int64_t accessed_seconds;
    /**
     * Wire offset 56 bytes; width 4 bytes.
     */
    uint32_t accessed_nanoseconds;
    /**
     * Wire offset 60 bytes; width 4 bytes.
     */
    uint32_t accessed_reserved;
    /**
     * Wire offset 64 bytes; width 8 bytes.
     */
    int64_t modified_seconds;
    /**
     * Wire offset 72 bytes; width 4 bytes.
     */
    uint32_t modified_nanoseconds;
    /**
     * Wire offset 76 bytes; width 4 bytes.
     */
    uint32_t modified_reserved;
    /**
     * Wire offset 80 bytes; width 8 bytes.
     */
    int64_t created_seconds;
    /**
     * Wire offset 88 bytes; width 4 bytes.
     */
    uint32_t created_nanoseconds;
    /**
     * Wire offset 92 bytes; width 4 bytes.
     */
    uint32_t created_reserved;
    /**
     * Wire offset 96 bytes; width 8 bytes.
     */
    int64_t changed_seconds;
    /**
     * Wire offset 104 bytes; width 4 bytes.
     */
    uint32_t changed_nanoseconds;
    /**
     * Wire offset 108 bytes; width 4 bytes.
     */
    uint32_t changed_reserved;
} hyper_native_file_metadata_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_file_metadata_t) == 112, "file_metadata size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_file_metadata_t) == 8, "file_metadata alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_t, filesystem_id) == 0, "file_metadata.filesystem_id offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_t, mount_id) == 8, "file_metadata.mount_id offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_t, node_id) == 16, "file_metadata.node_id offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_t, size) == 24, "file_metadata.size offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_t, mode) == 32, "file_metadata.mode offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_t, kind) == 36, "file_metadata.kind offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_t, valid_times) == 40, "file_metadata.valid_times offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_t, reserved) == 44, "file_metadata.reserved offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_t, accessed_seconds) == 48, "file_metadata.accessed_seconds offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_t, accessed_nanoseconds) == 56, "file_metadata.accessed_nanoseconds offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_t, accessed_reserved) == 60, "file_metadata.accessed_reserved offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_t, modified_seconds) == 64, "file_metadata.modified_seconds offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_t, modified_nanoseconds) == 72, "file_metadata.modified_nanoseconds offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_t, modified_reserved) == 76, "file_metadata.modified_reserved offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_t, created_seconds) == 80, "file_metadata.created_seconds offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_t, created_nanoseconds) == 88, "file_metadata.created_nanoseconds offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_t, created_reserved) == 92, "file_metadata.created_reserved offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_t, changed_seconds) == 96, "file_metadata.changed_seconds offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_t, changed_nanoseconds) == 104, "file_metadata.changed_nanoseconds offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_t, changed_reserved) == 108, "file_metadata.changed_reserved offset");

/**
 * Minimum supported prefix of `file_metadata_update`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_FILE_METADATA_UPDATE_MIN_SIZE UINT64_C(40)
/**
 * Wire layout for `file_metadata_update`.
 *
 * Size: 40 bytes; alignment: 8 bytes; supported minimum prefix: 40 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 *
 * Used by: `directory_set_metadata`, `file_set_metadata`.
 */
typedef struct hyper_native_file_metadata_update_t {
    /**
     * Wire offset 0 bytes; width 4 bytes.
     */
    uint32_t mask;
    /**
     * Wire offset 4 bytes; width 4 bytes.
     */
    uint32_t mode;
    /**
     * Wire offset 8 bytes; width 8 bytes.
     */
    int64_t accessed_seconds;
    /**
     * Wire offset 16 bytes; width 4 bytes.
     */
    uint32_t accessed_nanoseconds;
    /**
     * Wire offset 20 bytes; width 4 bytes.
     */
    uint32_t accessed_reserved;
    /**
     * Wire offset 24 bytes; width 8 bytes.
     */
    int64_t modified_seconds;
    /**
     * Wire offset 32 bytes; width 4 bytes.
     */
    uint32_t modified_nanoseconds;
    /**
     * Wire offset 36 bytes; width 4 bytes.
     */
    uint32_t modified_reserved;
} hyper_native_file_metadata_update_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_file_metadata_update_t) == 40, "file_metadata_update size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_file_metadata_update_t) == 8, "file_metadata_update alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_update_t, mask) == 0, "file_metadata_update.mask offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_update_t, mode) == 4, "file_metadata_update.mode offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_update_t, accessed_seconds) == 8, "file_metadata_update.accessed_seconds offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_update_t, accessed_nanoseconds) == 16, "file_metadata_update.accessed_nanoseconds offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_update_t, accessed_reserved) == 20, "file_metadata_update.accessed_reserved offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_update_t, modified_seconds) == 24, "file_metadata_update.modified_seconds offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_update_t, modified_nanoseconds) == 32, "file_metadata_update.modified_nanoseconds offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_update_t, modified_reserved) == 36, "file_metadata_update.modified_reserved offset");

/**
 * Minimum supported prefix of `wait_set_event`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_WAIT_SET_EVENT_MIN_SIZE UINT64_C(24)
/**
 * Wire layout for `wait_set_event`.
 *
 * Size: 24 bytes; alignment: 8 bytes; supported minimum prefix: 24 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 *
 * Used by: `wait_set_wait`.
 */
typedef struct hyper_native_wait_set_event_t {
    /**
     * Wire offset 0 bytes; width 8 bytes.
     */
    uint64_t registration;
    /**
     * Wire offset 8 bytes; width 8 bytes.
     */
    uint64_t signals;
    /**
     * Wire offset 16 bytes; width 8 bytes.
     */
    uint64_t sequence;
} hyper_native_wait_set_event_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_wait_set_event_t) == 24, "wait_set_event size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_wait_set_event_t) == 8, "wait_set_event alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_wait_set_event_t, registration) == 0, "wait_set_event.registration offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_wait_set_event_t, signals) == 8, "wait_set_event.signals offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_wait_set_event_t, sequence) == 16, "wait_set_event.sequence offset");

/**
 * Minimum supported prefix of `handle_info`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_HANDLE_INFO_MIN_SIZE UINT64_C(16)
/**
 * Wire layout for `handle_info`.
 *
 * Size: 16 bytes; alignment: 8 bytes; supported minimum prefix: 16 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 *
 * Used by: `handle_get_info`.
 */
typedef struct hyper_native_handle_info_t {
    /**
     * Wire offset 0 bytes; width 4 bytes.
     */
    uint32_t object_kind;
    /**
     * Wire offset 4 bytes; width 4 bytes.
     */
    uint32_t flags;
    /**
     * Wire offset 8 bytes; width 8 bytes.
     */
    uint64_t rights;
} hyper_native_handle_info_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_handle_info_t) == 16, "handle_info size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_handle_info_t) == 8, "handle_info alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_handle_info_t, object_kind) == 0, "handle_info.object_kind offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_handle_info_t, flags) == 4, "handle_info.flags offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_handle_info_t, rights) == 8, "handle_info.rights offset");

/**
 * Minimum supported prefix of `object_basic_info`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_OBJECT_BASIC_INFO_MIN_SIZE UINT64_C(16)
/**
 * Wire layout for `object_basic_info`.
 *
 * Size: 16 bytes; alignment: 8 bytes; supported minimum prefix: 16 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 *
 * Used by: `object_get_basic_info`.
 */
typedef struct hyper_native_object_basic_info_t {
    /**
     * Wire offset 0 bytes; width 8 bytes.
     */
    uint64_t koid;
    /**
     * Wire offset 8 bytes; width 4 bytes.
     */
    uint32_t object_kind;
    /**
     * Wire offset 12 bytes; width 4 bytes.
     */
    uint32_t reserved;
} hyper_native_object_basic_info_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_object_basic_info_t) == 16, "object_basic_info size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_object_basic_info_t) == 8, "object_basic_info alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_object_basic_info_t, koid) == 0, "object_basic_info.koid offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_object_basic_info_t, object_kind) == 8, "object_basic_info.object_kind offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_object_basic_info_t, reserved) == 12, "object_basic_info.reserved offset");

/**
 * Minimum supported prefix of `object_wait_item`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_OBJECT_WAIT_ITEM_MIN_SIZE UINT64_C(16)
/**
 * Wire layout for `object_wait_item`.
 *
 * Size: 16 bytes; alignment: 8 bytes; supported minimum prefix: 16 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 *
 * Used by: `object_wait_many`.
 */
typedef struct hyper_native_object_wait_item_t {
    /**
     * Wire offset 0 bytes; width 8 bytes.
     */
    uint64_t handle;
    /**
     * Wire offset 8 bytes; width 8 bytes.
     */
    uint64_t signals;
} hyper_native_object_wait_item_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_object_wait_item_t) == 16, "object_wait_item size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_object_wait_item_t) == 8, "object_wait_item alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_object_wait_item_t, handle) == 0, "object_wait_item.handle offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_object_wait_item_t, signals) == 8, "object_wait_item.signals offset");

/**
 * Minimum supported prefix of `process_info`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_PROCESS_INFO_MIN_SIZE UINT64_C(32)
/**
 * Wire layout for `process_info`.
 *
 * Size: 32 bytes; alignment: 8 bytes; supported minimum prefix: 32 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 *
 * Used by: `process_get_info`.
 */
typedef struct hyper_native_process_info_t {
    /**
     * Wire offset 0 bytes; width 4 bytes.
     */
    uint32_t phase;
    /**
     * Wire offset 4 bytes; width 4 bytes.
     */
    uint32_t terminal_reason;
    /**
     * Wire offset 8 bytes; width 8 bytes.
     */
    uint64_t detail0;
    /**
     * Wire offset 16 bytes; width 8 bytes.
     */
    uint64_t detail1;
    /**
     * Wire offset 24 bytes; width 8 bytes.
     */
    uint64_t reserved;
} hyper_native_process_info_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_process_info_t) == 32, "process_info size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_process_info_t) == 8, "process_info alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_process_info_t, phase) == 0, "process_info.phase offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_process_info_t, terminal_reason) == 4, "process_info.terminal_reason offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_process_info_t, detail0) == 8, "process_info.detail0 offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_process_info_t, detail1) == 16, "process_info.detail1 offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_process_info_t, reserved) == 24, "process_info.reserved offset");

/**
 * Minimum supported prefix of `capability_disposition`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_CAPABILITY_DISPOSITION_MIN_SIZE UINT64_C(24)
/**
 * Wire layout for `capability_disposition`.
 *
 * Size: 24 bytes; alignment: 8 bytes; supported minimum prefix: 24 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 *
 * Used by: `capability_channel_try_send`.
 */
typedef struct hyper_native_capability_disposition_t {
    /**
     * Wire offset 0 bytes; width 8 bytes.
     */
    uint64_t handle;
    /**
     * Wire offset 8 bytes; width 8 bytes.
     */
    uint64_t rights;
    /**
     * Wire offset 16 bytes; width 4 bytes.
     */
    uint32_t expected_kind;
    /**
     * Wire offset 20 bytes; width 4 bytes.
     */
    uint32_t operation;
} hyper_native_capability_disposition_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_capability_disposition_t) == 24, "capability_disposition size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_capability_disposition_t) == 8, "capability_disposition alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_capability_disposition_t, handle) == 0, "capability_disposition.handle offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_capability_disposition_t, rights) == 8, "capability_disposition.rights offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_capability_disposition_t, expected_kind) == 16, "capability_disposition.expected_kind offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_capability_disposition_t, operation) == 20, "capability_disposition.operation offset");

/**
 * Minimum supported prefix of `capability_receive_slot`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_CAPABILITY_RECEIVE_SLOT_MIN_SIZE UINT64_C(24)
/**
 * Wire layout for `capability_receive_slot`.
 *
 * Size: 24 bytes; alignment: 8 bytes; supported minimum prefix: 24 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 *
 * Used by: `capability_channel_receive`.
 */
typedef struct hyper_native_capability_receive_slot_t {
    /**
     * Wire offset 0 bytes; width 8 bytes.
     */
    uint64_t handle;
    /**
     * Wire offset 8 bytes; width 8 bytes.
     */
    uint64_t rights;
    /**
     * Wire offset 16 bytes; width 4 bytes.
     */
    uint32_t expected_kind;
    /**
     * Wire offset 20 bytes; width 4 bytes.
     */
    uint32_t flags;
} hyper_native_capability_receive_slot_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_capability_receive_slot_t) == 24, "capability_receive_slot size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_capability_receive_slot_t) == 8, "capability_receive_slot alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_capability_receive_slot_t, handle) == 0, "capability_receive_slot.handle offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_capability_receive_slot_t, rights) == 8, "capability_receive_slot.rights offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_capability_receive_slot_t, expected_kind) == 16, "capability_receive_slot.expected_kind offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_capability_receive_slot_t, flags) == 20, "capability_receive_slot.flags offset");

/**
 * Minimum supported prefix of `loader_startup`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_LOADER_STARTUP_MIN_SIZE UINT64_C(96)
/**
 * Wire layout for `loader_startup`.
 *
 * Size: 96 bytes; alignment: 8 bytes; supported minimum prefix: 96 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 */
typedef struct hyper_native_loader_startup_t {
    /**
     * Wire offset 0 bytes; width 4 bytes.
     */
    uint32_t size;
    /**
     * Wire offset 4 bytes; width 4 bytes.
     */
    uint32_t handle_count;
    /**
     * Wire offset 8 bytes; width 4 bytes.
     */
    uint32_t data_size;
    /**
     * Wire offset 12 bytes; width 4 bytes.
     */
    uint32_t flags;
    /**
     * Wire offset 16 bytes; width 8 bytes.
     */
    uint64_t executable;
    /**
     * Wire offset 24 bytes; width 8 bytes.
     */
    uint64_t root_vmar;
    /**
     * Wire offset 32 bytes; width 8 bytes.
     */
    uint64_t stack_vmar;
    /**
     * Wire offset 40 bytes; width 8 bytes.
     */
    uint64_t stack_base;
    /**
     * Wire offset 48 bytes; width 8 bytes.
     */
    uint64_t stack_size;
    /**
     * Wire offset 56 bytes; width 8 bytes.
     */
    uint64_t loader_base;
    /**
     * Wire offset 64 bytes; width 8 bytes.
     */
    uint64_t loader_size;
    /**
     * Wire offset 72 bytes; width 8 bytes.
     */
    uint64_t execution_context;
    /**
     * Wire offset 80 bytes; width 8 bytes.
     */
    uint64_t object_factory;
    /**
     * Wire offset 88 bytes; width 8 bytes.
     */
    uint64_t system_info;
} hyper_native_loader_startup_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_loader_startup_t) == 96, "loader_startup size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_loader_startup_t) == 8, "loader_startup alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_loader_startup_t, size) == 0, "loader_startup.size offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_loader_startup_t, handle_count) == 4, "loader_startup.handle_count offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_loader_startup_t, data_size) == 8, "loader_startup.data_size offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_loader_startup_t, flags) == 12, "loader_startup.flags offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_loader_startup_t, executable) == 16, "loader_startup.executable offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_loader_startup_t, root_vmar) == 24, "loader_startup.root_vmar offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_loader_startup_t, stack_vmar) == 32, "loader_startup.stack_vmar offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_loader_startup_t, stack_base) == 40, "loader_startup.stack_base offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_loader_startup_t, stack_size) == 48, "loader_startup.stack_size offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_loader_startup_t, loader_base) == 56, "loader_startup.loader_base offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_loader_startup_t, loader_size) == 64, "loader_startup.loader_size offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_loader_startup_t, execution_context) == 72, "loader_startup.execution_context offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_loader_startup_t, object_factory) == 80, "loader_startup.object_factory offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_loader_startup_t, system_info) == 88, "loader_startup.system_info offset");

/**
 * Minimum supported prefix of `startup_handle`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_STARTUP_HANDLE_MIN_SIZE UINT64_C(16)
/**
 * Wire layout for `startup_handle`.
 *
 * Size: 16 bytes; alignment: 8 bytes; supported minimum prefix: 16 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 */
typedef struct hyper_native_startup_handle_t {
    /**
     * Wire offset 0 bytes; width 4 bytes.
     */
    uint32_t purpose;
    /**
     * Wire offset 4 bytes; width 4 bytes.
     */
    uint32_t flags;
    /**
     * Wire offset 8 bytes; width 8 bytes.
     */
    uint64_t handle;
} hyper_native_startup_handle_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_startup_handle_t) == 16, "startup_handle size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_startup_handle_t) == 8, "startup_handle alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_startup_handle_t, purpose) == 0, "startup_handle.purpose offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_startup_handle_t, flags) == 4, "startup_handle.flags offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_startup_handle_t, handle) == 8, "startup_handle.handle offset");

/**
 * Minimum supported prefix of `task_process`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_TASK_PROCESS_MIN_SIZE UINT64_C(96)
/**
 * Wire layout for `task_process`.
 *
 * Size: 96 bytes; alignment: 8 bytes; supported minimum prefix: 96 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 *
 * Used by: `task_inspector_scan_processes`.
 */
typedef struct hyper_native_task_process_t {
    /**
     * Wire offset 0 bytes; width 8 bytes.
     */
    uint64_t koid;
    /**
     * Wire offset 8 bytes; width 4 bytes.
     */
    uint32_t phase;
    /**
     * Wire offset 12 bytes; width 4 bytes.
     */
    uint32_t terminal_reason;
    /**
     * Wire offset 16 bytes; width 4 bytes.
     */
    uint32_t pending_threads;
    /**
     * Wire offset 20 bytes; width 4 bytes.
     */
    uint32_t active_threads;
    /**
     * Wire offset 24 bytes; width 4 bytes.
     */
    uint32_t name_length;
    /**
     * Wire offset 28 bytes; width 4 bytes.
     */
    uint32_t reserved;
    /**
     * Wire offset 32 bytes; width 64 bytes.
     */
    uint8_t name[64];
} hyper_native_task_process_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_task_process_t) == 96, "task_process size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_task_process_t) == 8, "task_process alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_task_process_t, koid) == 0, "task_process.koid offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_task_process_t, phase) == 8, "task_process.phase offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_task_process_t, terminal_reason) == 12, "task_process.terminal_reason offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_task_process_t, pending_threads) == 16, "task_process.pending_threads offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_task_process_t, active_threads) == 20, "task_process.active_threads offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_task_process_t, name_length) == 24, "task_process.name_length offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_task_process_t, reserved) == 28, "task_process.reserved offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_task_process_t, name) == 32, "task_process.name offset");

/**
 * Minimum supported prefix of `task_thread`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_TASK_THREAD_MIN_SIZE UINT64_C(104)
/**
 * Wire layout for `task_thread`.
 *
 * Size: 104 bytes; alignment: 8 bytes; supported minimum prefix: 104 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 *
 * Used by: `task_inspector_scan_threads`.
 */
typedef struct hyper_native_task_thread_t {
    /**
     * Wire offset 0 bytes; width 8 bytes.
     */
    uint64_t koid;
    /**
     * Wire offset 8 bytes; width 8 bytes.
     */
    uint64_t process_koid;
    /**
     * Wire offset 16 bytes; width 4 bytes.
     */
    uint32_t role;
    /**
     * Wire offset 20 bytes; width 4 bytes.
     */
    uint32_t registry_phase;
    /**
     * Wire offset 24 bytes; width 4 bytes.
     */
    uint32_t name_length;
    /**
     * Wire offset 28 bytes; width 4 bytes.
     */
    uint32_t reserved;
    /**
     * Wire offset 32 bytes; width 8 bytes.
     */
    uint64_t runtime_ns;
    /**
     * Wire offset 40 bytes; width 64 bytes.
     */
    uint8_t name[64];
} hyper_native_task_thread_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_task_thread_t) == 104, "task_thread size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_task_thread_t) == 8, "task_thread alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_task_thread_t, koid) == 0, "task_thread.koid offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_task_thread_t, process_koid) == 8, "task_thread.process_koid offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_task_thread_t, role) == 16, "task_thread.role offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_task_thread_t, registry_phase) == 20, "task_thread.registry_phase offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_task_thread_t, name_length) == 24, "task_thread.name_length offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_task_thread_t, reserved) == 28, "task_thread.reserved offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_task_thread_t, runtime_ns) == 32, "task_thread.runtime_ns offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_task_thread_t, name) == 40, "task_thread.name offset");

/**
 * Minimum supported prefix of `memory_observation`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_MEMORY_OBSERVATION_MIN_SIZE UINT64_C(144)
/**
 * Wire layout for `memory_observation`.
 *
 * Size: 144 bytes; alignment: 8 bytes; supported minimum prefix: 144 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 *
 * Used by: `memory_inspector_read`.
 */
typedef struct hyper_native_memory_observation_t {
    /**
     * Wire offset 0 bytes; width 8 bytes.
     */
    uint64_t captured_at_ns;
    /**
     * Wire offset 8 bytes; width 8 bytes.
     */
    uint64_t page_size;
    /**
     * Wire offset 16 bytes; width 8 bytes.
     */
    uint64_t total_bytes;
    /**
     * Wire offset 24 bytes; width 8 bytes.
     */
    uint64_t managed_bytes;
    /**
     * Wire offset 32 bytes; width 8 bytes.
     */
    uint64_t free_bytes;
    /**
     * Wire offset 40 bytes; width 8 bytes.
     */
    uint64_t kernel_bytes;
    /**
     * Wire offset 48 bytes; width 8 bytes.
     */
    uint64_t slab_bytes;
    /**
     * Wire offset 56 bytes; width 8 bytes.
     */
    uint64_t large_heap_bytes;
    /**
     * Wire offset 64 bytes; width 8 bytes.
     */
    uint64_t page_table_bytes;
    /**
     * Wire offset 72 bytes; width 8 bytes.
     */
    uint64_t vmo_bytes;
    /**
     * Wire offset 80 bytes; width 8 bytes.
     */
    uint64_t guest_loaned_bytes;
    /**
     * Wire offset 88 bytes; width 8 bytes.
     */
    uint64_t guest_private_bytes;
    /**
     * Wire offset 96 bytes; width 8 bytes.
     */
    uint64_t file_cache_bytes;
    /**
     * Wire offset 104 bytes; width 8 bytes.
     */
    uint64_t slab_reclaimable_bytes;
    /**
     * Wire offset 112 bytes; width 8 bytes.
     */
    uint64_t slab_sample_complete;
    /**
     * Wire offset 120 bytes; width 8 bytes.
     */
    uint64_t file_cache_reclaimable_bytes;
    /**
     * Wire offset 128 bytes; width 8 bytes.
     */
    uint64_t file_cache_sample_complete;
    /**
     * Wire offset 136 bytes; width 8 bytes.
     */
    uint64_t ramfs_capacity_bytes;
} hyper_native_memory_observation_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_memory_observation_t) == 144, "memory_observation size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_memory_observation_t) == 8, "memory_observation alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_observation_t, captured_at_ns) == 0, "memory_observation.captured_at_ns offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_observation_t, page_size) == 8, "memory_observation.page_size offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_observation_t, total_bytes) == 16, "memory_observation.total_bytes offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_observation_t, managed_bytes) == 24, "memory_observation.managed_bytes offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_observation_t, free_bytes) == 32, "memory_observation.free_bytes offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_observation_t, kernel_bytes) == 40, "memory_observation.kernel_bytes offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_observation_t, slab_bytes) == 48, "memory_observation.slab_bytes offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_observation_t, large_heap_bytes) == 56, "memory_observation.large_heap_bytes offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_observation_t, page_table_bytes) == 64, "memory_observation.page_table_bytes offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_observation_t, vmo_bytes) == 72, "memory_observation.vmo_bytes offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_observation_t, guest_loaned_bytes) == 80, "memory_observation.guest_loaned_bytes offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_observation_t, guest_private_bytes) == 88, "memory_observation.guest_private_bytes offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_observation_t, file_cache_bytes) == 96, "memory_observation.file_cache_bytes offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_observation_t, slab_reclaimable_bytes) == 104, "memory_observation.slab_reclaimable_bytes offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_observation_t, slab_sample_complete) == 112, "memory_observation.slab_sample_complete offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_observation_t, file_cache_reclaimable_bytes) == 120, "memory_observation.file_cache_reclaimable_bytes offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_observation_t, file_cache_sample_complete) == 128, "memory_observation.file_cache_sample_complete offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_observation_t, ramfs_capacity_bytes) == 136, "memory_observation.ramfs_capacity_bytes offset");

/**
 * Minimum supported prefix of `cpu_observation`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_CPU_OBSERVATION_MIN_SIZE UINT64_C(64)
/**
 * Wire layout for `cpu_observation`.
 *
 * Size: 64 bytes; alignment: 8 bytes; supported minimum prefix: 64 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 *
 * Used by: `cpu_inspector_read`.
 */
typedef struct hyper_native_cpu_observation_t {
    /**
     * Wire offset 0 bytes; width 8 bytes.
     */
    uint64_t captured_at_ns;
    /**
     * Wire offset 8 bytes; width 8 bytes.
     */
    uint64_t reserved_clock;
    /**
     * Wire offset 16 bytes; width 8 bytes.
     */
    uint64_t online_cpus;
    /**
     * Wire offset 24 bytes; width 8 bytes.
     */
    uint64_t idle_ns;
    /**
     * Wire offset 32 bytes; width 8 bytes.
     */
    uint64_t kernel_thread_ns;
    /**
     * Wire offset 40 bytes; width 8 bytes.
     */
    uint64_t user_thread_ns;
    /**
     * Wire offset 48 bytes; width 8 bytes.
     */
    uint64_t vcpu_ns;
    /**
     * Wire offset 56 bytes; width 8 bytes.
     */
    uint64_t reserved;
} hyper_native_cpu_observation_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_cpu_observation_t) == 64, "cpu_observation size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_cpu_observation_t) == 8, "cpu_observation alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_cpu_observation_t, captured_at_ns) == 0, "cpu_observation.captured_at_ns offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_cpu_observation_t, reserved_clock) == 8, "cpu_observation.reserved_clock offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_cpu_observation_t, online_cpus) == 16, "cpu_observation.online_cpus offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_cpu_observation_t, idle_ns) == 24, "cpu_observation.idle_ns offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_cpu_observation_t, kernel_thread_ns) == 32, "cpu_observation.kernel_thread_ns offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_cpu_observation_t, user_thread_ns) == 40, "cpu_observation.user_thread_ns offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_cpu_observation_t, vcpu_ns) == 48, "cpu_observation.vcpu_ns offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_cpu_observation_t, reserved) == 56, "cpu_observation.reserved offset");

/**
 * Minimum supported prefix of `object_inspection`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_OBJECT_INSPECTION_MIN_SIZE UINT64_C(104)
/**
 * Wire layout for `object_inspection`.
 *
 * Size: 104 bytes; alignment: 8 bytes; supported minimum prefix: 104 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 *
 * Used by: `object_inspector_scan_objects`.
 */
typedef struct hyper_native_object_inspection_t {
    /**
     * Wire offset 0 bytes; width 8 bytes.
     */
    uint64_t koid;
    /**
     * Wire offset 8 bytes; width 4 bytes.
     */
    uint32_t object_kind;
    /**
     * Wire offset 12 bytes; width 4 bytes.
     */
    uint32_t handle_state;
    /**
     * Wire offset 16 bytes; width 8 bytes.
     */
    uint64_t active_handles;
    /**
     * Wire offset 24 bytes; width 8 bytes.
     */
    uint64_t supported_rights;
    /**
     * Wire offset 32 bytes; width 8 bytes.
     */
    uint64_t strong_references;
    /**
     * Wire offset 40 bytes; width 8 bytes.
     */
    uint64_t kernel_service_references;
    /**
     * Wire offset 48 bytes; width 8 bytes.
     */
    uint64_t scheduler_references;
    /**
     * Wire offset 56 bytes; width 8 bytes.
     */
    uint64_t operation_references;
    /**
     * Wire offset 64 bytes; width 8 bytes.
     */
    uint64_t user_authority_references;
    /**
     * Wire offset 72 bytes; width 8 bytes.
     */
    uint64_t publication_references;
    /**
     * Wire offset 80 bytes; width 8 bytes.
     */
    uint64_t diagnostic_references;
    /**
     * Wire offset 88 bytes; width 8 bytes.
     */
    uint64_t retirement_references;
    /**
     * Wire offset 96 bytes; width 8 bytes.
     */
    uint64_t vm_device_binding_references;
} hyper_native_object_inspection_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_object_inspection_t) == 104, "object_inspection size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_object_inspection_t) == 8, "object_inspection alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_object_inspection_t, koid) == 0, "object_inspection.koid offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_object_inspection_t, object_kind) == 8, "object_inspection.object_kind offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_object_inspection_t, handle_state) == 12, "object_inspection.handle_state offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_object_inspection_t, active_handles) == 16, "object_inspection.active_handles offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_object_inspection_t, supported_rights) == 24, "object_inspection.supported_rights offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_object_inspection_t, strong_references) == 32, "object_inspection.strong_references offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_object_inspection_t, kernel_service_references) == 40, "object_inspection.kernel_service_references offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_object_inspection_t, scheduler_references) == 48, "object_inspection.scheduler_references offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_object_inspection_t, operation_references) == 56, "object_inspection.operation_references offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_object_inspection_t, user_authority_references) == 64, "object_inspection.user_authority_references offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_object_inspection_t, publication_references) == 72, "object_inspection.publication_references offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_object_inspection_t, diagnostic_references) == 80, "object_inspection.diagnostic_references offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_object_inspection_t, retirement_references) == 88, "object_inspection.retirement_references offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_object_inspection_t, vm_device_binding_references) == 96, "object_inspection.vm_device_binding_references offset");

/**
 * Minimum supported prefix of `handle_inspection`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_HANDLE_INSPECTION_MIN_SIZE UINT64_C(40)
/**
 * Wire layout for `handle_inspection`.
 *
 * Size: 40 bytes; alignment: 8 bytes; supported minimum prefix: 40 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 *
 * Used by: `object_inspector_scan_handles`.
 */
typedef struct hyper_native_handle_inspection_t {
    /**
     * Wire offset 0 bytes; width 8 bytes.
     */
    uint64_t process_koid;
    /**
     * Wire offset 8 bytes; width 8 bytes.
     */
    uint64_t handle;
    /**
     * Wire offset 16 bytes; width 8 bytes.
     */
    uint64_t object_koid;
    /**
     * Wire offset 24 bytes; width 8 bytes.
     */
    uint64_t rights;
    /**
     * Wire offset 32 bytes; width 4 bytes.
     */
    uint32_t object_kind;
    /**
     * Wire offset 36 bytes; width 4 bytes.
     */
    uint32_t flags;
} hyper_native_handle_inspection_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_handle_inspection_t) == 40, "handle_inspection size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_handle_inspection_t) == 8, "handle_inspection alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_handle_inspection_t, process_koid) == 0, "handle_inspection.process_koid offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_handle_inspection_t, handle) == 8, "handle_inspection.handle offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_handle_inspection_t, object_koid) == 16, "handle_inspection.object_koid offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_handle_inspection_t, rights) == 24, "handle_inspection.rights offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_handle_inspection_t, object_kind) == 32, "handle_inspection.object_kind offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_handle_inspection_t, flags) == 36, "handle_inspection.flags offset");

/**
 * Minimum supported prefix of `directory_entry`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_DIRECTORY_ENTRY_MIN_SIZE UINT64_C(280)
/**
 * Wire layout for `directory_entry`.
 *
 * Size: 280 bytes; alignment: 8 bytes; supported minimum prefix: 280 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 *
 * Used by: `directory_read`.
 */
typedef struct hyper_native_directory_entry_t {
    /**
     * Wire offset 0 bytes; width 8 bytes.
     */
    uint64_t size;
    /**
     * Wire offset 8 bytes; width 4 bytes.
     */
    uint32_t mode;
    /**
     * Wire offset 12 bytes; width 4 bytes.
     */
    uint32_t kind;
    /**
     * Wire offset 16 bytes; width 4 bytes.
     */
    uint32_t name_length;
    /**
     * Wire offset 20 bytes; width 4 bytes.
     */
    uint32_t reserved;
    /**
     * Wire offset 24 bytes; width 256 bytes.
     */
    uint8_t name[256];
} hyper_native_directory_entry_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_directory_entry_t) == 280, "directory_entry size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_directory_entry_t) == 8, "directory_entry alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_directory_entry_t, size) == 0, "directory_entry.size offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_directory_entry_t, mode) == 8, "directory_entry.mode offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_directory_entry_t, kind) == 12, "directory_entry.kind offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_directory_entry_t, name_length) == 16, "directory_entry.name_length offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_directory_entry_t, reserved) == 20, "directory_entry.reserved offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_directory_entry_t, name) == 24, "directory_entry.name offset");

/**
 * Minimum supported prefix of `file_info`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_FILE_INFO_MIN_SIZE UINT64_C(40)
/**
 * Wire layout for `file_info`.
 *
 * Size: 40 bytes; alignment: 8 bytes; supported minimum prefix: 40 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 *
 * Used by: `file_get_info`.
 */
typedef struct hyper_native_file_info_t {
    /**
     * Wire offset 0 bytes; width 8 bytes.
     */
    uint64_t filesystem_id;
    /**
     * Wire offset 8 bytes; width 8 bytes.
     */
    uint64_t mount_id;
    /**
     * Wire offset 16 bytes; width 8 bytes.
     */
    uint64_t node_id;
    /**
     * Wire offset 24 bytes; width 8 bytes.
     */
    uint64_t size;
    /**
     * Wire offset 32 bytes; width 4 bytes.
     */
    uint32_t mode;
    /**
     * Wire offset 36 bytes; width 4 bytes.
     */
    uint32_t reserved;
} hyper_native_file_info_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_file_info_t) == 40, "file_info size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_file_info_t) == 8, "file_info alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_info_t, filesystem_id) == 0, "file_info.filesystem_id offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_info_t, mount_id) == 8, "file_info.mount_id offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_info_t, node_id) == 16, "file_info.node_id offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_info_t, size) == 24, "file_info.size offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_info_t, mode) == 32, "file_info.mode offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_info_t, reserved) == 36, "file_info.reserved offset");

/**
 * Minimum supported prefix of `directory_info`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_DIRECTORY_INFO_MIN_SIZE UINT64_C(32)
/**
 * Wire layout for `directory_info`.
 *
 * Size: 32 bytes; alignment: 8 bytes; supported minimum prefix: 32 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 *
 * Used by: `directory_get_info`.
 */
typedef struct hyper_native_directory_info_t {
    /**
     * Wire offset 0 bytes; width 8 bytes.
     */
    uint64_t filesystem_id;
    /**
     * Wire offset 8 bytes; width 8 bytes.
     */
    uint64_t mount_id;
    /**
     * Wire offset 16 bytes; width 8 bytes.
     */
    uint64_t node_id;
    /**
     * Wire offset 24 bytes; width 4 bytes.
     */
    uint32_t mode;
    /**
     * Wire offset 28 bytes; width 4 bytes.
     */
    uint32_t reserved;
} hyper_native_directory_info_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_directory_info_t) == 32, "directory_info size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_directory_info_t) == 8, "directory_info alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_directory_info_t, filesystem_id) == 0, "directory_info.filesystem_id offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_directory_info_t, mount_id) == 8, "directory_info.mount_id offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_directory_info_t, node_id) == 16, "directory_info.node_id offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_directory_info_t, mode) == 24, "directory_info.mode offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_directory_info_t, reserved) == 28, "directory_info.reserved offset");

/**
 * Minimum supported prefix of `virtual_machine_configuration`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_VIRTUAL_MACHINE_CONFIGURATION_MIN_SIZE UINT64_C(32)
/**
 * Wire layout for `virtual_machine_configuration`.
 *
 * Size: 32 bytes; alignment: 8 bytes; supported minimum prefix: 32 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 *
 * Used by: `virtual_machine_create`.
 */
typedef struct hyper_native_virtual_machine_configuration_t {
    /**
     * Wire offset 0 bytes; width 8 bytes.
     */
    uint64_t guest_physical_base;
    /**
     * Wire offset 8 bytes; width 8 bytes.
     */
    uint64_t memory_size;
    /**
     * Wire offset 16 bytes; width 4 bytes.
     */
    uint32_t vcpu_count;
    /**
     * Wire offset 20 bytes; width 4 bytes.
     */
    uint32_t architecture;
    /**
     * Wire offset 24 bytes; width 4 bytes.
     */
    uint32_t platform_profile;
    /**
     * Wire offset 28 bytes; width 4 bytes.
     */
    uint32_t flags;
} hyper_native_virtual_machine_configuration_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_virtual_machine_configuration_t) == 32, "virtual_machine_configuration size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_virtual_machine_configuration_t) == 8, "virtual_machine_configuration alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_machine_configuration_t, guest_physical_base) == 0, "virtual_machine_configuration.guest_physical_base offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_machine_configuration_t, memory_size) == 8, "virtual_machine_configuration.memory_size offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_machine_configuration_t, vcpu_count) == 16, "virtual_machine_configuration.vcpu_count offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_machine_configuration_t, architecture) == 20, "virtual_machine_configuration.architecture offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_machine_configuration_t, platform_profile) == 24, "virtual_machine_configuration.platform_profile offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_machine_configuration_t, flags) == 28, "virtual_machine_configuration.flags offset");

/**
 * Minimum supported prefix of `virtual_cpu_bootstrap`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_VIRTUAL_CPU_BOOTSTRAP_MIN_SIZE UINT64_C(64)
/**
 * Wire layout for `virtual_cpu_bootstrap`.
 *
 * Size: 64 bytes; alignment: 8 bytes; supported minimum prefix: 64 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 *
 * Used by: `pending_virtual_machine_set_bootstrap`.
 */
typedef struct hyper_native_virtual_cpu_bootstrap_t {
    /**
     * Wire offset 0 bytes; width 8 bytes.
     */
    uint64_t entry;
    /**
     * Wire offset 8 bytes; width 8 bytes.
     */
    uint64_t stack;
    /**
     * Wire offset 16 bytes; width 8 bytes.
     */
    uint64_t argument0;
    /**
     * Wire offset 24 bytes; width 8 bytes.
     */
    uint64_t argument1;
    /**
     * Wire offset 32 bytes; width 8 bytes.
     */
    uint64_t argument2;
    /**
     * Wire offset 40 bytes; width 8 bytes.
     */
    uint64_t argument3;
    /**
     * Wire offset 48 bytes; width 4 bytes.
     */
    uint32_t vcpu_id;
    /**
     * Wire offset 52 bytes; width 4 bytes.
     */
    uint32_t flags;
    /**
     * Wire offset 56 bytes; width 8 bytes.
     */
    uint64_t reserved;
} hyper_native_virtual_cpu_bootstrap_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_virtual_cpu_bootstrap_t) == 64, "virtual_cpu_bootstrap size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_virtual_cpu_bootstrap_t) == 8, "virtual_cpu_bootstrap alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_cpu_bootstrap_t, entry) == 0, "virtual_cpu_bootstrap.entry offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_cpu_bootstrap_t, stack) == 8, "virtual_cpu_bootstrap.stack offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_cpu_bootstrap_t, argument0) == 16, "virtual_cpu_bootstrap.argument0 offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_cpu_bootstrap_t, argument1) == 24, "virtual_cpu_bootstrap.argument1 offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_cpu_bootstrap_t, argument2) == 32, "virtual_cpu_bootstrap.argument2 offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_cpu_bootstrap_t, argument3) == 40, "virtual_cpu_bootstrap.argument3 offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_cpu_bootstrap_t, vcpu_id) == 48, "virtual_cpu_bootstrap.vcpu_id offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_cpu_bootstrap_t, flags) == 52, "virtual_cpu_bootstrap.flags offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_cpu_bootstrap_t, reserved) == 56, "virtual_cpu_bootstrap.reserved offset");

/**
 * Minimum supported prefix of `virtual_machine_power_request`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_VIRTUAL_MACHINE_POWER_REQUEST_MIN_SIZE UINT64_C(40)
/**
 * Wire layout for `virtual_machine_power_request`.
 *
 * Size: 40 bytes; alignment: 8 bytes; supported minimum prefix: 40 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 *
 * Used by: `virtual_machine_get_power_request`.
 */
typedef struct hyper_native_virtual_machine_power_request_t {
    /**
     * Wire offset 0 bytes; width 8 bytes.
     */
    uint64_t id;
    /**
     * Wire offset 8 bytes; width 4 bytes.
     */
    uint32_t vcpu;
    /**
     * Wire offset 12 bytes; width 4 bytes.
     */
    uint32_t operation;
    /**
     * Wire offset 16 bytes; width 4 bytes.
     */
    uint32_t target;
    /**
     * Wire offset 20 bytes; width 4 bytes.
     */
    uint32_t reserved;
    /**
     * Wire offset 24 bytes; width 8 bytes.
     */
    uint64_t entry;
    /**
     * Wire offset 32 bytes; width 8 bytes.
     */
    uint64_t context;
} hyper_native_virtual_machine_power_request_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_virtual_machine_power_request_t) == 40, "virtual_machine_power_request size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_virtual_machine_power_request_t) == 8, "virtual_machine_power_request alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_machine_power_request_t, id) == 0, "virtual_machine_power_request.id offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_machine_power_request_t, vcpu) == 8, "virtual_machine_power_request.vcpu offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_machine_power_request_t, operation) == 12, "virtual_machine_power_request.operation offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_machine_power_request_t, target) == 16, "virtual_machine_power_request.target offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_machine_power_request_t, reserved) == 20, "virtual_machine_power_request.reserved offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_machine_power_request_t, entry) == 24, "virtual_machine_power_request.entry offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_machine_power_request_t, context) == 32, "virtual_machine_power_request.context offset");

/**
 * Minimum supported prefix of `virtual_machine_info`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_VIRTUAL_MACHINE_INFO_MIN_SIZE UINT64_C(32)
/**
 * Wire layout for `virtual_machine_info`.
 *
 * Size: 40 bytes; alignment: 8 bytes; supported minimum prefix: 32 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 *
 * Used by: `virtual_machine_get_info`.
 */
typedef struct hyper_native_virtual_machine_info_t {
    /**
     * Wire offset 0 bytes; width 4 bytes.
     */
    uint32_t phase;
    /**
     * Wire offset 4 bytes; width 4 bytes.
     */
    uint32_t vcpu_count;
    /**
     * Wire offset 8 bytes; width 8 bytes.
     */
    uint64_t guest_physical_base;
    /**
     * Wire offset 16 bytes; width 8 bytes.
     */
    uint64_t memory_size;
    /**
     * Wire offset 24 bytes; width 4 bytes.
     */
    uint32_t architecture;
    /**
     * Wire offset 28 bytes; width 4 bytes.
     */
    uint32_t platform_profile;
    /**
     * Wire offset 32 bytes; width 8 bytes.
     */
    uint64_t resident_memory_bytes;
} hyper_native_virtual_machine_info_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_virtual_machine_info_t) == 40, "virtual_machine_info size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_virtual_machine_info_t) == 8, "virtual_machine_info alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_machine_info_t, phase) == 0, "virtual_machine_info.phase offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_machine_info_t, vcpu_count) == 4, "virtual_machine_info.vcpu_count offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_machine_info_t, guest_physical_base) == 8, "virtual_machine_info.guest_physical_base offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_machine_info_t, memory_size) == 16, "virtual_machine_info.memory_size offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_machine_info_t, architecture) == 24, "virtual_machine_info.architecture offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_machine_info_t, platform_profile) == 28, "virtual_machine_info.platform_profile offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_machine_info_t, resident_memory_bytes) == 32, "virtual_machine_info.resident_memory_bytes offset");

/**
 * Minimum supported prefix of `virtual_cpu_info`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_VIRTUAL_CPU_INFO_MIN_SIZE UINT64_C(24)
/**
 * Wire layout for `virtual_cpu_info`.
 *
 * Size: 32 bytes; alignment: 8 bytes; supported minimum prefix: 24 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 *
 * Used by: `virtual_cpu_get_info`.
 */
typedef struct hyper_native_virtual_cpu_info_t {
    /**
     * Wire offset 0 bytes; width 4 bytes.
     */
    uint32_t vcpu_id;
    /**
     * Wire offset 4 bytes; width 4 bytes.
     */
    uint32_t phase;
    /**
     * Wire offset 8 bytes; width 8 bytes.
     */
    uint64_t scheduler_thread_id;
    /**
     * Wire offset 16 bytes; width 4 bytes.
     */
    uint32_t terminal_reason;
    /**
     * Wire offset 20 bytes; width 4 bytes.
     */
    uint32_t reserved;
    /**
     * Wire offset 24 bytes; width 4 bytes.
     */
    uint32_t host_cpu;
    /**
     * Wire offset 28 bytes; width 4 bytes.
     */
    uint32_t migration_target;
} hyper_native_virtual_cpu_info_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_virtual_cpu_info_t) == 32, "virtual_cpu_info size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_virtual_cpu_info_t) == 8, "virtual_cpu_info alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_cpu_info_t, vcpu_id) == 0, "virtual_cpu_info.vcpu_id offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_cpu_info_t, phase) == 4, "virtual_cpu_info.phase offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_cpu_info_t, scheduler_thread_id) == 8, "virtual_cpu_info.scheduler_thread_id offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_cpu_info_t, terminal_reason) == 16, "virtual_cpu_info.terminal_reason offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_cpu_info_t, reserved) == 20, "virtual_cpu_info.reserved offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_cpu_info_t, host_cpu) == 24, "virtual_cpu_info.host_cpu offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_cpu_info_t, migration_target) == 28, "virtual_cpu_info.migration_target offset");

/**
 * Minimum supported prefix of `resource_limits`, in bytes; individual calls define their buffer lengths.
 */
#define HYPER_NATIVE_RESOURCE_LIMITS_MIN_SIZE UINT64_C(160)
/**
 * Wire layout for `resource_limits`.
 *
 * Size: 160 bytes; alignment: 8 bytes; supported minimum prefix: 160 bytes.
 * Scalar widths and member offsets are fixed across supported architectures.
 * The minimum prefix describes extensibility, not permission to truncate every call's buffer;
 * the consuming operation defines its accepted length.
 *
 * Used by: `resource_domain_create`.
 */
typedef struct hyper_native_resource_limits_t {
    /**
     * Wire offset 0 bytes; width 8 bytes.
     */
    uint64_t kernel_memory_bytes;
    /**
     * Wire offset 8 bytes; width 8 bytes.
     */
    uint64_t processes;
    /**
     * Wire offset 16 bytes; width 8 bytes.
     */
    uint64_t threads;
    /**
     * Wire offset 24 bytes; width 8 bytes.
     */
    uint64_t handles;
    /**
     * Wire offset 32 bytes; width 8 bytes.
     */
    uint64_t kernel_objects;
    /**
     * Wire offset 40 bytes; width 8 bytes.
     */
    uint64_t committed_pages;
    /**
     * Wire offset 48 bytes; width 8 bytes.
     */
    uint64_t pinned_pages;
    /**
     * Wire offset 56 bytes; width 8 bytes.
     */
    uint64_t guest_pages;
    /**
     * Wire offset 64 bytes; width 8 bytes.
     */
    uint64_t ipc_messages;
    /**
     * Wire offset 72 bytes; width 8 bytes.
     */
    uint64_t ipc_bytes;
    /**
     * Wire offset 80 bytes; width 8 bytes.
     */
    uint64_t ipc_handles;
    /**
     * Wire offset 88 bytes; width 8 bytes.
     */
    uint64_t subscriptions;
    /**
     * Wire offset 96 bytes; width 8 bytes.
     */
    uint64_t timers;
    /**
     * Wire offset 104 bytes; width 8 bytes.
     */
    uint64_t virtual_machines;
    /**
     * Wire offset 112 bytes; width 8 bytes.
     */
    uint64_t virtual_cpus;
    /**
     * Wire offset 120 bytes; width 8 bytes.
     */
    uint64_t device_leases;
    /**
     * Wire offset 128 bytes; width 8 bytes.
     */
    uint64_t dma_mappings;
    /**
     * Wire offset 136 bytes; width 8 bytes.
     */
    uint64_t user_address_spaces;
    /**
     * Wire offset 144 bytes; width 8 bytes.
     */
    uint64_t user_mappings;
    /**
     * Wire offset 152 bytes; width 8 bytes.
     */
    uint64_t reserved;
} hyper_native_resource_limits_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_resource_limits_t) == 160, "resource_limits size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_resource_limits_t) == 8, "resource_limits alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_resource_limits_t, kernel_memory_bytes) == 0, "resource_limits.kernel_memory_bytes offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_resource_limits_t, processes) == 8, "resource_limits.processes offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_resource_limits_t, threads) == 16, "resource_limits.threads offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_resource_limits_t, handles) == 24, "resource_limits.handles offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_resource_limits_t, kernel_objects) == 32, "resource_limits.kernel_objects offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_resource_limits_t, committed_pages) == 40, "resource_limits.committed_pages offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_resource_limits_t, pinned_pages) == 48, "resource_limits.pinned_pages offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_resource_limits_t, guest_pages) == 56, "resource_limits.guest_pages offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_resource_limits_t, ipc_messages) == 64, "resource_limits.ipc_messages offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_resource_limits_t, ipc_bytes) == 72, "resource_limits.ipc_bytes offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_resource_limits_t, ipc_handles) == 80, "resource_limits.ipc_handles offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_resource_limits_t, subscriptions) == 88, "resource_limits.subscriptions offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_resource_limits_t, timers) == 96, "resource_limits.timers offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_resource_limits_t, virtual_machines) == 104, "resource_limits.virtual_machines offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_resource_limits_t, virtual_cpus) == 112, "resource_limits.virtual_cpus offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_resource_limits_t, device_leases) == 120, "resource_limits.device_leases offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_resource_limits_t, dma_mappings) == 128, "resource_limits.dma_mappings offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_resource_limits_t, user_address_spaces) == 136, "resource_limits.user_address_spaces offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_resource_limits_t, user_mappings) == 144, "resource_limits.user_mappings offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_resource_limits_t, reserved) == 152, "resource_limits.reserved offset");

#undef HYPER_ABI_ALIGNOF
#undef HYPER_ABI_STATIC_ASSERT

/** @} */

#endif /* HYPER_NATIVE_H */
