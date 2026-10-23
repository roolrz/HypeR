<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# Configuring a product

HypeR separates kernel capabilities, deployed userspace components and board
resources. A product can omit applications, background services and filesystem
workers without changing the kernel's startup contract or embedding a new
deployment policy in the Linux I/O VM.

| Input | Owns |
| --- | --- |
| `CONFIG_FILE` | Kernel architecture, address widths, stack sizes and numeric limits |
| `KERNEL_FEATURES` | Optional production kernel capabilities |
| `SYSTEM_CONFIG` | Executable selection, selectable boot profiles, formal storage successors and optional kernel features |
| `BOARD_CONFIG` | Physical devices, guest definitions, disk partitions and mount policy |
| Component and service declarations | Build targets, required child programs, startup and capability grants |

The root [Kconfig](../../../Kconfig) sources the configuration definitions owned by
[bootstrap/stage1](../../../bootstrap/stage1/Kconfig), [app](../../../app/Kconfig),
[svc](../../../svc/Kconfig) and [tests/app](../../../tests/app/Kconfig). These files own
program defaults, deployment dependencies, menu labels and help.
`component.mk` binds those symbols to Cargo targets and installed paths; it does
not repeat their normal-product defaults or executable dependencies.

The saved `.config` is the product selection used by both builds and packaging.
[`configs/default_defconfig`](../../../configs/default_defconfig) is a minimal preset;
omitted values take their defaults from Kconfig. Without a local `.config`,
ordinary builds use the same Kconfig defaults. Test programs default to disabled.

Default and saved products use the same selection and admission code. Saving
an unchanged default selection does not change the executable set or startup
graph. The formal mount job is derived from the formal fstab, independently of
whether a product file was supplied. Packaging checks required startup images
for the default product as well as explicitly trimmed products.

Generated `target/.../deployment.json`, bootstrap profiles, fstab, component
manifests and build records are projections of these inputs, not additional
editable configuration sources. Hardware-specific board policy and capability
grants remain separate from executable selection: choosing a program does not
grant it a device or decide when it starts.

Board resources and boot profiles describe how the selected programs are used.
They do not add missing programs back into the selection. In particular, an
`io-vm` boot profile still requires its declared bootstrap provider and actual
I/O VM payload in the archive. Every shipped profile is validated at build time,
including profiles not selected by the default boot command line.

## Interactive configuration

The repository root configures the complete product; the kernel also retains
an independent menu:

```sh
make menuconfig
make -C kernel menuconfig
```

The top-level menu separates these owners:

- **Kernel** opens the kernel's own menu for architecture, address widths,
  stacks, scheduler limits and other Kconfig settings.
- **Bootstrap stage1** selects its filesystem workers, services and tools.
- **System** selects formal applications, filesystem workers, background
  services and optional test programs.

Program checkboxes come from the domain Kconfig files. They control both Cargo
executable targets and image membership. Shared libraries follow actual Cargo
and ELF dependencies; there is no second library selection list to maintain.
Stage1 and formal copies are independent: disabling `CONFIG_BOOT_APP_FS_FAT` does not
disable `CONFIG_SYSTEM_APP_FS_FAT_SYSTEM`. Filesystem support selection changes available
workers, not fstab or partition formats.

The product menu saves to the ignored local `.config`. Subsequent root
`make` commands automatically select it, including its separate product output
directories. An explicit `SYSTEM_CONFIG` takes precedence; `SYSTEM_CONFIG=`
uses the full default product without reading the local file. To edit another
product instead:

```sh
make menuconfig SYSTEM_CONFIG=/tmp/hyper-product.config
make board-build SYSTEM_CONFIG=/tmp/hyper-product.config
```

Only explicit saves change files. The kernel submenu saves `kernel/.config`
(or `CONFIG_FILE`) independently; discarding product edits does not undo a
kernel save. **Build policy** selects optional production Cargo features and
the path to structured runtime boot policy. `BOARD_CONFIG` remains a separate
hardware and mount policy input.

Kconfig `depends on` relations constrain deployment. For example, the formal
FAT worker depends on fs-backend, which depends on component-manager. Bootstrap
workers depend on the bootstrap filesystem backend, which depends on stage1
init. Keeping a dispatcher does not force every filesystem worker into the
image. Multiple prerequisites use `&&`; transitive dependencies are resolved
independently of declaration order and cycles are rejected.

Each program remains independently selectable. Dependencies live in its Kconfig
definition and can change without redefining a fixed bundle. Component-manager
itself does not depend on a particular startup coordinator or journal provider.
The shipped system-startup coordinator does require console-raw, journal and
bootstrap-retire. Separately, the selected profile's actual startup images and
capability sources are validated during composition, so changing a program
selection alone cannot leave a dangling reference in a bootable image.

The audited deployment prerequisites are:

| Component | Required deployment |
| --- | --- |
| Stage1 console-raw | Stage1 init (console capability and channel setup) |
| Stage1 fallback-sh, fs-backend, io-runtime | Stage1 console-raw (recovery I/O or service log transport) |
| Stage1 FS workers, mount, umount | Stage1 fs-backend |
| Formal console-raw, journal, bootstrap-retire | Component-manager |
| system-startup | Component-manager, console-raw, journal, bootstrap-retire |
| Formal fs-backend, io-runtime, vm-runtime | Component-manager |
| Formal FS workers, mount, umount | Formal fs-backend |
| Formal terminal-service | Formal console-raw |
| Formal session-manager | Component-manager, journal, terminal-service, sh |
| Formal vm-manager | Component-manager, vm-runtime |
| componentctl, journalctl, vmm | Component-manager, journal, vm-manager respectively |
| editor | Terminal-service |
| FS worker, retirement/service, terminal probes | FS backend, component-manager, terminal-service respectively |

`cat`, `ls` and other ordinary byte/file tools do not require a terminal service.
`less` retains its nonterminal copy path; shell retains script execution without
a terminal. Their interactive use still requires delegated terminal capabilities.
Keeping the shell does not force vm-manager or fs-backend into the product:
those connections are optional. Similarly, I/O VM is not a fixed prerequisite
of filesystem services: the selected boot profile supplies the block provider.
The packer also checks bootstrap console grants for formal journal/session
startup, storage grants, successor images and every shipped boot profile.

Disabling a prerequisite in the menu also disables dependent programs. Enabling
a program without its prerequisites produces a dependency error. A hand-edited
file that explicitly enables an unavailable program is rejected by the build;
the build never silently adds omitted programs back. Full boot-profile,
service-image and board admission still runs during image composition. Kconfig
does not replace runtime component routing, startup order or filesystem probing.

Run configuration commands separately from builds: `make menuconfig all` is rejected because
Make reads product selection before executing targets. Navigate with arrows,
Enter and Escape; Space or Y/N toggles programs, `/` searches, `?` shows the
installation path and dependencies, and S saves. Both menus use Python's
standard `curses` module and require an interactive terminal of at least 56x12.

## Userspace selection

The product uses the same `CONFIG_NAME=value` / `# CONFIG_NAME is not set`
syntax and Rust parser as the kernel. `SYSTEM_CONFIG` selects another complete
configuration, not an overlay. For example, a defconfig can disable both FAT
workers and two tools:

```text
# CONFIG_BOOT_APP_FS_FAT is not set
# CONFIG_SYSTEM_APP_FS_FAT_SYSTEM is not set
# CONFIG_SYSTEM_APP_EDITOR is not set
# CONFIG_SYSTEM_APP_LESS is not set
```

The workflow is:

```sh
make defconfig                          # configs/default_defconfig -> .config
make menuconfig                         # edit product; Kernel opens its own menu
make olddefconfig                       # fill new defaults, report removed settings
make savedefconfig                      # export differences from defaults to ./defconfig
make defconfig SYSTEM_DEFCONFIG=configs/custom_defconfig
make savedefconfig SYSTEM_DEFCONFIG_OUTPUT=configs/custom_defconfig
# Work on a separate product without replacing the local one:
make defconfig SYSTEM_CONFIG=/tmp/product.config
```

`defconfig` replaces the selected product file. `olddefconfig` preserves valid
selections, drops obsolete or unavailable ones with warnings, and rejects invalid
values of known symbols. `savedefconfig` writes a minimal fragment, not a second
active configuration. None of these root commands modifies `kernel/.config`;
use `make -C kernel defconfig` to reset the kernel. Normal builds reject unknown
symbols, duplicate assignments and invalid types. No JSON product selection is
accepted. The internal JSON communication between configuration tools and the
generated image manifests are derived records, not editable alternatives.

Program symbols use `CONFIG_BOOT_APP_` and `CONFIG_SYSTEM_APP_` followed by the
uppercased staged name with punctuation replaced by underscores. For example,
`fs-fat-system` is `CONFIG_SYSTEM_APP_FS_FAT_SYSTEM`, and `svc/journal` is
`CONFIG_SYSTEM_APP_SVC_JOURNAL`. The registry rejects missing definitions,
ambiguous symbol names and options with no registered program.

Selection controls Cargo executable targets, installation and final image
membership together. Shared-library providers are built from the selected
Cargo dependency graph; the packer follows each executable's actual `DT_NEEDED`
closure. A previous build's unused executables or libraries do not become part
of a trimmed image. Source-owned bootstrap executables remain statically linked
and cannot acquire formal userspace libraries through product configuration.
The existing debug-symbol policy is unchanged.
Cargo fetch and lockfiles remain workspace-wide: a trimmed product avoids
compiling and installing omitted programs, but does not maintain a separate
dependency lockfile or restrict downloads to its selected executable graph.

Removing an optional formal service removes its startup declaration as well.
Programs that launch other programs declare their runtime requirements: for
example, session-manager needs terminal-service and sh. A selection that keeps
a launcher while omitting a required executable fails with a dependency error;
the build does not silently add an excluded program back. The bootstrap
supervisor and formal ownership-adoption path remain necessary for this product
model. Their required executables cannot be removed while still selecting that
startup path.

Filesystem support remains discovered at runtime. The filesystem manager
recognizes the volume magic and locates the corresponding worker. Omitting
`fs-fat` does not make FAT mount syntax invalid at build time: requesting a FAT
mount without any reachable FAT worker produces an explicit runtime error.
Change the board's fstab when an omitted filesystem is no longer intended to be
mounted. The generated boot profile declares the worker for the board's known
system filesystem: omitting that worker now fails archive admission before
boot. The builder reports the missing dependency instead of silently restoring
an excluded program. Custom profiles declare their own early worker dependencies.

Ordinary `system` configurations exclude test binaries. Setting a test program
symbol to `y` explicitly opts in;
runtime acceptance generates its own trimmed selection with the command reporter
under the test output directory; it is not a second production configuration.
Development fixtures may select their test profile explicitly when no saved
product constrains the executable set. A saved product never implicitly enables
test binaries just because a development profile was requested. Neither
mechanism turns on kernel fault injection or self-tests.

## Selectable boot profiles

Set `CONFIG_BOOT_POLICY="path/to/boot-policy.json"` to select a structured
runtime policy file. Paths are relative to the repository root, or absolute;
exporting a defconfig does not change their base directory. The policy accepts
only `boot-profiles` and `storage-providers`, not build selections. Its validated
contents contribute to the product identity. It may define up to 16 `boot-profiles`. The fixed `default` entry is
mandatory. Each is installed as `/etc/hyper/boot-profiles/NAME.json` in the
initial archive. Without this section, composition generates `default` from
the selected bootstrap service graph and board resources, preserving ordinary
I/O VM boot. Profile names contain 1–63 ASCII letters, digits, `_` or `-`.

```json
{
  "boot-profiles": {
    "default": {
      "version": 1,
      "services": "/etc/hyper/services.json",
      "io": {
        "kind": "io-vm",
        "service": "io-runtime",
        "successor": "/io-runtime"
      },
      "requires": [
        "/etc/fstab",
        "/etc/hyper/filesystem.json",
        "/etc/hyper/storage.json",
        "/etc/hyper/io-clients.conf",
        "/svc/fs-erofs"
      ],
      "handoff": {
        "system-root": "/system",
        "put-old": "/boot",
        "system-init": "/svc/component-manager"
      }
    }
  }
}
```

`services` names an archive-local service manifest. The ordinary manifest at
`/etc/hyper/services.json` is generated from the configured service graph;
alternate manifests must be explicitly packaged as boot files. Profile paths
are canonical absolute paths; `requires` lists up to 128 additional files the
provider or its children need before handoff. The builder checks these against
the archive itself, including archive-local symlinks. A file present only in the
formal system or build directory cannot satisfy a bootstrap dependency.

Every shipped profile is checked, including unselected profiles. Their service
graphs use the production stage1 parser and capability policy. Each service
must have a packed, executable static ELF matching the bootstrap architecture.
For `io-vm`, the packed board configuration and the referenced I/O VM FIT must
also exist. Dynamic runtime choices, such as a format discovered only by
reading disk magic, still need explicit `requires` declarations when they are
critical to reaching the formal root; this is dependency admission, not a
simulation of arbitrary service code.

The kernel consumes `earlycon` and `nokaslr`, then forwards unrecognized boot
arguments without interpreting profile policy.
Stage1 selects `default` unless it sees `hyper.boot-profile=NAME`. Unknown,
malformed or repeated selectors produce a bootstrap error instead of selecting
another profile. QEMU accepts this token in the kernel `-append` command line;
Pi firmware accepts it in `cmdline.txt`. Stage1 forwards all arguments other than
its selector to system init in a read-only startup VMO. Unknown arguments are not
validated against a cross-stage registry; each stage recognizes only its own
parameters. See [boot argument forwarding](boot-policy.md#boot-profiles).

The I/O source chooses the bootstrap provider independently of root handoff:

| `io.kind` | Meaning |
| --- | --- |
| `io-vm` | Start the named provider with the explicit VM creation capability; the shipped implementation is `io-runtime` |
| `service` | Start a named userspace provider with no VM creation capability; its executable may implement a different storage source |
| `none` | No selected storage provider; a root needed for handoff must already be independently prepared |

The generic service option defines the integration boundary; this patch does
not add a Native physical block driver. A provider receives only the declared
capabilities, supplies readiness when it supplies filesystem volumes, and owns
its private migration protocol. A recovery profile may omit `handoff`; a
provider's `successor` is required only when handoff is configured.

Formal storage candidates can be declared in the same boot policy file:

```json
{
  "storage-providers": [
    {"component": "/io-runtime", "image": "/svc/io-runtime"},
    {"component": "/disk-driver", "image": "/svc/disk-driver"}
  ]
}
```

The example's custom driver must be implemented and explicitly deployed before
that candidate is usable. Composition declares these as prepared resident
storage components; only the selected profile's `successor` is adopted. An
unlisted successor is rejected. If the list is absent, composition declares the
shipped `/io-runtime` successor only when a handoff profile uses `io-vm`.
The common handoff transports an opaque migration endpoint; it does not parse
the I/O VM's private description. See [the handoff contract](../../design/boot/bootstrap-handoff.md).

Profiles still have to satisfy the chosen formal services. For example, the
current VM manager's I/O broker grant requires a bootstrap provider offering
that broker; a generic driver does not acquire one automatically. Such known
missing grants fail composition. Use a compatible formal service selection.

The board's `io-vm` section is optional for products whose sources do not need
an I/O VM; such products must define explicit boot profiles for their alternate
source rather than relying on the generated I/O VM default. All included
profiles contribute to the appliance requirement: if
any profile needs one, the board section and image are required, even when it
is not the default. If none does, board composition skips appliance fetching
and packaging, including stale appliance entries left by an earlier build.
The current guest disk/network declarations use the I/O VM backend and still
require its board section. Removing that section alone does not implement a
replacement guest backend.

## Kernel features

`KERNEL_FEATURES` is an exact comma-separated production feature set. An empty
value builds without these optional capabilities. A product can carry the same
selection in its JSON document:

```json
{
  "version": 1,
  "kernel": {
    "features": ["runtime-symbols"]
  }
}
```

| Feature | Behavior when disabled |
| --- | --- |
| `rtc` | No hardware wall-clock initialization; monotonic time remains available |
| `runtime-symbols` | No embedded runtime symbol table; crash reports retain raw addresses |
| `crash-console` | No interactive kernel crash command loop; fatal reporting remains |
| `pmu-profiling` | No AArch64 PMUv3 IRQ sampler or sample buffers; PMU queries report unsupported |
| `heap-profiling` | No per-size-class heap occupancy and traffic counters; general memory accounting remains |

The default feature set preserves the existing production behavior.
It is declared once in `kernel/Cargo.toml`'s `features.default`; kernel Make
derives the defaults from that graph, filtering capabilities by the manifest's
architecture availability metadata. The product does not carry another default
feature list. An explicit feature set replaces these defaults.
`pmu-profiling` is opt-in; see [PMU sampling](../diagnostics/profiling.md).
`kernel-trace` is opt-in; see [Perfetto tracing](../diagnostics/tracing.md).
`heap-profiling` is opt-in; see [memory diagnostics](../diagnostics/memory-diagnostics.md).
Memory ownership, capability checks, device isolation, IPC and the scheduler are
not optional security switches. The architecture and limit configuration stays
in the kernel's Kconfig file; a capability has one feature owner instead of
competing Cargo and Kconfig toggles.

Older local kernel configurations may contain `CONFIG_CRASH_CONSOLE` or
`CONFIG_HEAP_PROFILING`. Run `make -C kernel olddefconfig ARCH=aarch64` to remove obsolete
settings with an explicit warning while retaining recognized address, stack and
resource limits. Select the corresponding Cargo features separately if needed;
the removed values do not silently add features. Do not reset an otherwise
customized configuration merely to change optional functionality.

### Kernel architecture and resource configuration

`make -C kernel defconfig ARCH=aarch64` resolves the selected
[`kernel/configs/`](../../../kernel/configs/) fragment against
[`kernel/Kconfig`](../../../kernel/Kconfig). Common defaults live in Kconfig rather
than being copied into every architecture fragment. `make -C kernel config` edits the
existing selection as line-by-line prompts; enter `?` at a value prompt for help.

For a full-screen terminal menu, run:

```sh
make -C kernel menuconfig ARCH=aarch64
# Edit a separate configuration without touching kernel/.config:
make -C kernel menuconfig ARCH=riscv64 CONFIG_FILE=/tmp/hyper-riscv.config
```

The menu requires Python 3 with its standard `curses` module and an interactive
terminal of at least 56 columns by 12 rows. It is also available under **Kernel**
in the top-level product menu. An existing configuration is preserved on entry;
`ARCH` supplies the initial defconfig only when that file does not exist.
Use Up/Down to navigate, Enter to open categories or edit values, Space or Y/N
to toggle booleans, `?` for help, and `/` to search visible settings. PageUp,
PageDown, Home and End navigate long lists. Ctrl-U clears an edit field.
Save explicitly with S; Q/Left/Escape returns to the parent menu and offers
save/discard at the root. Ctrl-C discards unsaved edits and restores the terminal.
Opening the menu or cancelling never creates or overwrites the destination.
Saves replace the file atomically after validation. Changing architecture hides
unsupported settings immediately; KASLR cannot be selected on RISC-V or x86-64.
The selected architecture must still match `ARCH` in subsequent build commands.

The terminal frontend uses the same Rust resolver and kernel constraints as
normal builds. It does not maintain a second schema or relax dependency checks.
This submenu edits kernel Kconfig settings; the enclosing product menu edits
userspace selection. Optional kernel Cargo features remain the separate
`KERNEL_FEATURES` or product JSON input described above.

Architecture is a required `choice`: select exactly one of
`CONFIG_ARCH_AARCH64=y`, `CONFIG_ARCH_RISCV64=y`, or `CONFIG_ARCH_X86_64=y`.
Generated configurations contain only the selected architecture assignment.
Multiple enabled architectures fail validation. The selected architecture must
also match the bare-metal Cargo target; host tests may compile a different
platform's portable models.

Dependent settings are absent when their architecture is not selected, both
in the generated configuration and in exported Rust constants. For example,
RISC-V and x86-64 configurations do not contain `CONFIG_ARM64_*` settings.
`CONFIG_RANDOMIZE_BASE` is AArch64-only: the other backends currently select
fixed kernel addresses. Explicitly enabling an unavailable option or assigning
an unavailable numeric/string setting is an error during normal configuration
or build. Disabled boolean assignments are accepted and omitted from generated
output. `olddefconfig` removes unavailable or obsolete settings with a warning
and preserves applicable tuning.

The configuration tool and Cargo share validation of supported PA widths,
power-of-two stack sizes, and scheduler budgets. Duplicate assignments,
unknown keys, dependency cycles, and conflicting choices are errors, rather
than silently taking the last value. Generated files retain menu headings to
separate architecture, scheduler, stack-size and logging settings.

The scheduler has no configured global Thread-count limit. Registry segments,
stack-slot metadata and physical stacks grow on demand; existing slot addresses
stay stable across growth. Retiring Threads carry their own queue links, so exit
does not allocate. Creation remains subject to memory availability, resource-domain
quotas and representable IDs/virtual addresses. Vacant registry and stack slots
are reused; published registry segments retain their high-water metadata backing.

The scheduler menu also selects the Fair algorithm (`CONFIG_SCHED_FAIR_STRIDE`
or `CONFIG_SCHED_FAIR_RR`), the common Budget period and vCPU defaults, the shared
priority allowance and protected execution floor, and the guest/native preference.
These are kernel policy defaults; per-component and per-VM settings stay in their
existing deployment declarations and require scheduling capabilities to apply.
See [scheduling policy and tickless execution](../../../kernel/docs/design/scheduling.md),
[component scheduling authority](../../design/services/components.md#scheduling-authority),
and [VM scheduling configuration](../applications/vmm.md#scheduling).

Both `CONFIG_MAX_KERNEL_STACKS` and `CONFIG_SCHED_MAX_THREADS` have been removed.
Run `olddefconfig` to discard these obsolete settings from a saved configuration.

`CONFIG_ARM64_MAX_GIC_REDISTRIBUTOR_REGIONS` has also been removed. The
allocation-free discovery scratch list retains its internal capacity of 16,
like the other firmware range lists. This is a count of contiguous GICR MMIO
ranges, not a CPU count: one region can contain Redistributors for many CPUs.
It is intentionally independent of `CONFIG_MAX_CPUS`.

## Test builds

All kernel test selectors require two explicit gates: `kernel-test` and the
individual selector. Neither gate is enabled by production defaults, and a
selector never enables the master gate as a dependency. Selecting a test
without `kernel-test` is a build error. Test code also uses both conditions at
its compilation boundary.

| Selector | Purpose |
| --- | --- |
| `kernel-self-test` | Standalone kernel mechanism tests instead of Native startup |
| `kernel-smmuv3-test` | Standalone SMMUv3 isolation tests, including the self-test harness |
| `kernel-vgic-tc-test` | Force the GICv3 common-register trap path |
| `kernel-stack-audit` | Stack watermark instrumentation used by runtime acceptance |
| `io-dma-revoke-test` | Test-only revocation of a live production DMA buffer |

The supported test targets supply both gates. For an explicit test build:

```sh
make image ARCH=aarch64 \
  CARGO_FEATURES='--features kernel-test,kernel-self-test'
```

`kernel-test` alone does not replace production startup with self-tests. Ordinary
host `#[cfg(test)]` unit tests remain a separate compiler test mode; they do not
enter a delivered kernel image. Product JSON accepts production feature names,
not test selectors. Avoid Cargo's `--all-features` for a production image.

## Incremental builds and persistent disks

Custom product selections use configuration-specific kernel, application and
board output directories. Identity follows normalized configuration contents, so
editing a file in place selects new outputs; reordering a set does not create a
different build. Default invocations keep the existing output locations.

Normal board builds continue to update generated system contents while
preserving persistent partition bytes. Selecting a smaller image does not grant
permission to repartition or erase an existing data volume. To change storage
geometry, use a separate output directory or the explicitly destructive rebuild
target. See [board storage](board-storage.md) and
[component builds](../../../mk/README.md) for output and packaging details.
