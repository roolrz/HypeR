<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# Native syscall families

The compiler-checked [schema](../../sdk/abi/schema/native.rs) owns syscall
numbers, arguments, completion and blocking classes. Numbers identify operations;
they are not an object taxonomy. Related operations need not occupy adjacent
numbers. Renumbering alone does not simplify the kernel or SDK.

## Group by contract, not by count

The current interface separates these responsibilities:

| Family | Boundary that must remain explicit |
| --- | --- |
| Handles and object observation | Process-local authority versus object identity and waitable state |
| Process builders and task accounting | Reversible construction versus publication, execution and stop |
| Threads, atomic waits and WaitSets | Creation versus runnable admission; distinct wait ownership and completion |
| Directories and files | Namespace authority versus an opened file; positional data versus metadata |
| VMO and VMAR | Backing ownership and provenance versus address-space placement |
| Pending VM, VM and vCPU | Construction versus installed lifecycle; per-vCPU execution and exit completion |
| Physical devices and DMA | Claim authority, mapped resources and lifetime-bound physical access |
| Guest transport and block service | Memory grants, notifications and service publication are separate capabilities |
| Inspectors and clocks | Authority-scoped observation versus time reads |

A shared noun is insufficient reason for an `object_control(op, bytes)` syscall.
Such a dispatcher still needs all the operation-specific permission, size,
blocking and cancellation checks, but hides them behind another discriminator.
In particular, immediate-call admission must remain conservative for every
operation: a family-level allowlist must not make a later blocking operation
legal inside masked exception entry.

## Consolidation candidates reviewed

Directory open variants are the clearest overlap. `directory_open_file` and
`directory_open_file_with_options` share the object and path model; the latter
adds atomic create/truncate/exclusive behavior. The SDK can offer one options
builder while retaining typed wire entry points. A future wire consolidation
must preserve the old operation's rejection of nonzero reserved arguments and
validate options before namespace mutation. Similarly, no-follow directory
opening is a path-resolution policy, not a new object type.

`directory_remove_if` additionally checks the expected node identity. It is not
interchangeable with unconditional removal: erasing that check recreates a
lookup/remove race. Consolidation would have to represent the condition
explicitly, including whether an identity of zero means no condition.

Task and object inspector derivations look similar but produce different
visibility capabilities. Their target Process, TaskGroup and ResourceDomain
handles are separately typed and validated. A shared internal parser already
handles their wire arguments; replacing the public calls with an untyped
selector would not remove the authority checks.

File, directory and VM information calls produce different typed records.
They must not be collapsed into a generic query solely because they all copy
records to userspace. Shared record validation and copying are the appropriate
internal reuse boundary. Likewise, file locking/unlocking and atomic wait/wake
have different blocking and completion behavior despite sharing a resource.

The review retains existing numbers and typed operations. The directory-open
family is a valid future consolidation candidate, but no application behavior
currently requires a wire migration. Prefer one coherent SDK API where useful;
do not accumulate another parallel family just to rename the existing one.
Any later consolidation must update schema, generated C/Rust bindings, kernel
routing, installed-SDK consumers and contract tests together. Pre-release ABI
revision remains zero; that does not make partial producer/consumer migrations
safe.

## Reserved executable conversion

`ExecutableAuthority` and `CREATE_EXECUTABLE` are reserved foundations, not an
available Native issuance/conversion API. The current callable path derives an
executable VMO from a file using `file_create_executable_vmo`. Converting arbitrary
writable bytes requires a separately issued authority, immutable snapshot and
architecture instruction-publication proof. Do not expose it implicitly through
ordinary VMO write/map rights. There is no current app consumer that justifies
adding a new authority-distribution policy as part of syscall regrouping.
