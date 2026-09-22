<!-- SPDX-FileCopyrightText: 2026 roolrz -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Pi 5 image distribution

The selected distribution model separates HypeR development, Linux appliance
publication and final SD-image releases. The
[image-distribution repository](https://github.com/roolrz/HypeR-pi5-images) builds
compressed images and creates draft releases on updates to its main branch.
Public release publication is manual. Users only need a HypeR checkout: its Make
targets download pinned external inputs and compose images. The optional
distribution repository publishes versioned outputs using these same targets;
it is not a build prerequisite.

## Ownership

- HypeR owns its Apache-2.0 kernel, Native applications, board policy, guest
  device trees, external-input download locks and reusable image composition tools.
- HypeR-io-vm owns Linux/BusyBox builds, Linux modules, appliance startup policy
  and publication with matching corresponding sources.
- The distribution repository pins a HypeR commit, inherits its dependency locks,
  includes the official Pi
  DTBs/overlays and optional Alpine payloads, and releases the assembled image.
  It does not update the board's EEPROM.

The combined image contains independently licensed components; it is not
licensed wholesale under Apache-2.0. HypeR retains its own license. Preserve
component license texts, copyright notices and applicable NOTICE files, and
provide the corresponding source/build materials required by each component.
A “mixed licenses” label or repository separation alone does not discharge
these obligations.

## Release inputs and outputs

The distribution repository pins the HypeR commit and records the selected board
profile and build configuration. It must consume the I/O VM, official Pi boot
inputs and Alpine versions selected by that HypeR revision, rather than maintain
independent dependency pins or silently override them. Dependency upgrades land
in HypeR first; the distribution repository then advances its HypeR pin.

Generate a resolved release inventory containing the HypeR binary checksums,
I/O VM OCI digest and matching source artifact, official Pi firmware revision
and DTB/overlay checksums, and Alpine checksums and required source materials.
Record qualification results. This inventory records what was assembled; it
is not a second set of dependency selections. Board EEPROM remains an external
prerequisite and its tested version must be recorded separately.

Adopt a HypeR revision after merge, successful CI and required integration
checks. Local working trees and development reassemblies remain development
inputs, not release identities.

Release assets contain compressed whole-disk images (`.img.xz`), checksums,
input locks, component inventories, notices and companion source archives.
The distribution pipeline collects Rust dependency/std sources for every
profile, matching libc materials for I/O profiles, and Alpine package/kernel
sources for the full SD profile. See the
[artifact guide](https://github.com/roolrz/HypeR-pi5-images/blob/main/ARTIFACTS.md)
for image selection and the
[component and source inventory](https://github.com/roolrz/HypeR-pi5-images/blob/main/DISTRIBUTION.md)
for the supplied materials.

A boot-file update bundle is also planned. It must preserve Linux Image runtime padding
and state its required disk layout; it is not yet a dedicated Make target.
Replacing individual boot files is appropriate only when the partition and
configuration contracts remain compatible.

Hardware results currently establish Native boot, Linux appliance userspace
and SD-backed configuration-directory reads on Pi 5 D0. A two-vCPU Alpine
guest has also passed three secondary-CPU off/on cycles, ordinary reboot with
a synced file preserved on its ext4 root, and ordinary poweroff while the I/O
VM remained available. Write durability under power loss, device-reset recovery
and networking still need qualification.
