// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Real ramfs namespace and publication contracts in scheduled kernel context.

static EMPTY_ARCHIVE: &[u8] = b"07070100000001000000000000000000000000000000010000000000000000000000000000000000000000000000000000000b00000000TRAILER!!!\0\0\0\0";

use super::{DirectoryObject, Error, FileObject, FileOpenOptions, MetadataUpdate, ScratchBudget};
use crate::kernel::accounting::{ResourceDomain, ResourceLimits};
use crate::kernel::authority::Rights;
use hyper::fs::{NodeKind, ramfs::RamFs};
use hyper::mm::FallibleArc;

#[derive(Debug)]
pub(crate) enum TestError {
    Vfs(Error),
    Contract(&'static str),
}
impl From<Error> for TestError {
    fn from(error: Error) -> Self {
        Self::Vfs(error)
    }
}
fn check(condition: bool, label: &'static str) -> Result<(), TestError> {
    if condition {
        Ok(())
    } else {
        Err(TestError::Contract(label))
    }
}

pub(crate) fn run() -> Result<(), TestError> {
    let domain =
        ResourceDomain::try_new_root(ResourceLimits::UNLIMITED).map_err(Error::Resource)?;
    let scratch = ScratchBudget::new(&domain);
    let archive = RamFs::from_newc(EMPTY_ARCHIVE)
        .map_err(|error| Error::Backend(super::instance::Error::RamFs(error)))?;
    let filesystem =
        super::instance::FilesystemInstance::try_from_ramfs(archive).map_err(Error::from)?;
    let cache = crate::kernel::io_cache::FileDataCache::try_new_system().map_err(Error::Cache)?;
    let cache = FallibleArc::try_new(cache).map_err(|_| Error::Allocation)?;
    let namespace =
        super::instance::MountNamespace::try_new(filesystem, cache).map_err(Error::from)?;
    let root = DirectoryObject::try_root(namespace, &domain)?;
    snapshot_cache(&root, &domain, &scratch)?;
    root.create("left", NodeKind::Directory, 0o755, &scratch)?;
    root.create("right", NodeKind::Directory, 0o755, &scratch)?;
    let file = root.create_file("left/item", 0o666, &domain, Ok::<FileObject, Error>)?;
    file.write(Some(0), b"retained")?;
    let identity = file.metadata()?.location.node_id;
    root.link("left/item", &root, "other", &scratch)?;
    root.rename("left/item", &root, "other", &scratch)?;
    check(
        root.metadata("left/item", false, &scratch)?
            .location
            .node_id
            == identity
            && root.metadata("other", false, &scratch)?.location.node_id == identity,
        "rename aliases of one node is a no-op",
    )?;
    root.rename("left/item", &root, "right/moved", &scratch)?;
    check(
        root.metadata("other", true, &scratch)?.location.node_id == identity,
        "hardlink identity",
    )?;
    let left = root.open_directory("left", &domain)?;
    let cwd = DirectoryObject::scope(&root, &left, &domain)?;
    root.rename("left", &root, "right/inside", &scratch)?;
    check(
        cwd.canonicalize(".", &scratch)? == "/right/inside",
        "cwd follows rename",
    )?;
    check(
        cwd.metadata("../moved", true, &scratch)?.location.node_id == identity,
        "rooted parent traversal",
    )?;
    check(
        root.rename("right", &root, "right/inside/loop", &scratch) == Err(Error::InvalidInput),
        "directory cycle rejection",
    )?;
    root.symlink("right/inside/absolute", "/other", &scratch)?;
    check(
        cwd.metadata("absolute", true, &scratch)?.location.node_id == identity,
        "rooted absolute symlink",
    )?;
    check(
        left.metadata("absolute", true, &scratch) == Err(Error::Missing),
        "confined absolute symlink",
    )?;
    check(
        root.metadata("right/inside/absolute", false, &scratch)?
            .attributes
            .kind()
            == NodeKind::Symlink,
        "nofollow metadata",
    )?;
    root.symlink("loop", "loop", &scratch)?;
    check(
        root.metadata("loop", true, &scratch) == Err(Error::SymlinkLoop),
        "symlink loop classification",
    )?;
    check(
        &root.read_link("loop", &scratch)?[..] == b"loop",
        "readlink preserves target",
    )?;
    root.symlink("dangling", "right/new", &scratch)?;
    let created = root.open_file_with_options(
        "dangling",
        &FileOpenOptions::new(Rights::READ.union(Rights::WRITE), 1, 0o666)?,
        &domain,
        Ok::<FileObject, Error>,
    )?;
    check(
        root.metadata("right/new", true, &scratch)?.location.node_id
            == created.metadata()?.location.node_id,
        "dangling symlink creation",
    )?;
    check(
        matches!(
            root.open_file_with_options(
                "dangling",
                &FileOpenOptions::new(Rights::WRITE, 2, 0o666)?,
                &domain,
                Ok::<FileObject, Error>
            ),
            Err(Error::AlreadyExists)
        ),
        "exclusive create does not follow final symlink",
    )?;
    check(
        root.remove_if("other", false, identity + 1, &scratch) == Err(Error::Busy),
        "conditional removal identity",
    )?;
    root.remove_if("other", false, identity, &scratch)?;
    root.remove("right/moved", NodeKind::File, &scratch)?;
    let mut bytes = [0; 8];
    check(
        file.read(0, &mut bytes)? == 8 && &bytes == b"retained",
        "open lease survives final unlink",
    )?;
    check(
        root.metadata("right/new/..", true, &scratch) == Err(Error::NotDirectory),
        "regular file cannot be traversed",
    )?;
    check(
        root.metadata("right/new/", true, &scratch) == Err(Error::NotDirectory),
        "trailing slash requires directory",
    )?;
    check(
        root.canonicalize("//right///inside/.", &scratch)? == "/right/inside",
        "separator normalization",
    )?;
    created.write(Some(0), b"before")?;
    let failed: Result<(), Error> = root.open_file_with_options(
        "right/new",
        &FileOpenOptions::new(Rights::WRITE, 4, 0o666)?,
        &domain,
        |_| Err(Error::InvalidInput),
    );
    check(
        failed == Err(Error::InvalidInput) && created.len()? == 6,
        "publication failure cannot truncate",
    )?;
    let failed: Result<(), Error> =
        root.create_file("failed", 0o666, &domain, |_| Err(Error::InvalidInput));
    check(
        failed == Err(Error::InvalidInput)
            && root.metadata("failed", false, &scratch) == Err(Error::Missing),
        "publication failure cannot create name",
    )?;
    created.set_metadata(MetadataUpdate {
        mode: Some(0o444),
        ..MetadataUpdate::default()
    })?;
    let readonly = root.open_file("right/new", &domain)?;
    check(
        readonly.validate_access(Rights::WRITE) == Err(Error::AccessDenied),
        "new write admission honors mode",
    )?;
    check(
        created.write(Some(0), b"after!")?.0 == 6,
        "metadata does not revoke existing authority",
    )?;
    readonly.set_metadata(MetadataUpdate {
        mode: Some(0o666),
        ..MetadataUpdate::default()
    })?;
    check(
        root.open_file("right/new", &domain)?
            .validate_access(Rights::WRITE)
            .is_ok(),
        "metadata mutation independent of write",
    )?;
    let displaced = root.create_file("target", 0o666, &domain, Ok::<FileObject, Error>)?;
    displaced.write(Some(0), b"old")?;
    let replacement = root.create_file("source", 0o666, &domain, Ok::<FileObject, Error>)?;
    replacement.write(Some(0), b"new")?;
    root.rename("source", &root, "target", &scratch)?;
    let mut previous = [0; 3];
    check(
        displaced.read(0, &mut previous)? == 3 && &previous == b"old",
        "rename replacement preserves open target lease",
    )?;
    check(
        root.metadata("target", true, &scratch)?.location.node_id
            == replacement.metadata()?.location.node_id,
        "rename target atomically changes identity",
    )?;
    request_budget_follows_caller(&root, &domain)?;
    trailing_directory_mutations(&root, &domain, &scratch)?;
    deep_directory_rename(&root, &domain, &scratch)?;
    active_handle_lock_lifetime(&root, &domain)?;
    Ok(())
}

fn active_handle_lock_lifetime(
    root: &DirectoryObject,
    domain: &ResourceDomain,
) -> Result<(), TestError> {
    use super::locks::{LockError, LockMode};
    use crate::kernel::object::ObjectPublication;
    let first = root.open_file("right/new", domain)?;
    let second = root.open_file("right/new", domain)?;
    let active = ObjectPublication::try_new(first)
        .map_err(Error::Object)?
        .activate()
        .map_err(|_| TestError::Contract("activate file"))?;
    let pin = active
        .pin::<FileObject>()
        .ok_or(TestError::Contract("pin file"))?;
    let duplicate = active
        .try_duplicate()
        .map_err(|_| TestError::Contract("duplicate file"))?;
    pin.object()
        .lock(LockMode::Exclusive, 0, domain, || false)
        .map_err(|_| TestError::Contract("acquire first lock"))?;
    drop(active);
    check(
        matches!(
            second.lock(LockMode::Exclusive, 0, domain, || false),
            Err(LockError::WouldBlock)
        ),
        "duplicate retains advisory lock",
    )?;
    drop(duplicate);
    second
        .lock(LockMode::Exclusive, 0, domain, || false)
        .map_err(|_| TestError::Contract("last close releases advisory lock"))?;
    check(
        matches!(
            pin.object().lock(LockMode::Exclusive, 0, domain, || false),
            Err(LockError::Closed)
        ),
        "operation pin cannot resurrect closed lock owner",
    )?;
    second
        .unlock()
        .map_err(|_| TestError::Contract("unlock second file"))?;
    Ok(())
}

fn request_budget_follows_caller(
    root: &DirectoryObject,
    creator: &ResourceDomain,
) -> Result<(), TestError> {
    use crate::kernel::accounting::ResourceKind;
    let caller = ResourceDomain::try_new_root(
        ResourceLimits::UNLIMITED.with(ResourceKind::KernelMemoryBytes, 4096),
    )
    .map_err(Error::Resource)?;
    let budget = ScratchBudget::new(&caller);
    let creator_before = creator.usage();
    check(
        root.metadata("right/new", true, &budget)?.attributes.kind() == NodeKind::File,
        "small caller quota supports metadata",
    )?;
    check(
        caller.usage().total(ResourceKind::KernelMemoryBytes) == 0,
        "metadata scratch releases on return",
    )?;
    let canonical = root.canonicalize("right/new", &budget)?;
    check(
        canonical == "/right/new",
        "small caller quota supports canonicalize",
    )?;
    check(
        caller.usage().total(ResourceKind::KernelMemoryBytes) == canonical.capacity() as u64,
        "returned path retains exact caller charge",
    )?;
    check(
        creator.usage() == creator_before,
        "transferred directory never charges creator for request",
    )?;
    drop(canonical);
    check(
        caller.usage().total(ResourceKind::KernelMemoryBytes) == 0,
        "returned path drop releases caller charge",
    )?;
    let constrained = ResourceDomain::try_new_root(
        ResourceLimits::UNLIMITED.with(ResourceKind::KernelMemoryBytes, 16),
    )
    .map_err(Error::Resource)?;
    let constrained_budget = ScratchBudget::new(&constrained);
    check(
        matches!(
            root.canonicalize("right/new", &constrained_budget),
            Err(Error::Resource(_))
        ),
        "scratch quota enforced before growth",
    )?;
    check(
        constrained.usage().total(ResourceKind::KernelMemoryBytes) == 0,
        "failed request releases partial scratch",
    )?;
    check(
        creator.usage() == creator_before,
        "failed caller request preserves creator accounting",
    )?;
    Ok(())
}

fn trailing_directory_mutations(
    root: &DirectoryObject,
    domain: &ResourceDomain,
    scratch: &ScratchBudget,
) -> Result<(), TestError> {
    root.create("trailing/", NodeKind::Directory, 0o755, scratch)?;
    root.rename("trailing/", root, "renamed-trailing/", scratch)?;
    root.symlink("trailing-link", "renamed-trailing", scratch)?;
    check(
        root.remove("trailing-link/", NodeKind::Directory, scratch) == Err(Error::NotDirectory),
        "trailing slash never follows removal symlink",
    )?;
    check(
        root.rename("trailing-link/", root, "trailing-link/", scratch) == Err(Error::NotDirectory),
        "directory requirement precedes same-node rename no-op",
    )?;
    check(
        root.rename("renamed-trailing", root, "trailing-link/", scratch)
            == Err(Error::NotDirectory),
        "rename cannot replace directory-required symlink",
    )?;
    check(
        root.metadata("trailing-link", false, scratch)?
            .attributes
            .kind()
            == NodeKind::Symlink
            && root
                .metadata("renamed-trailing", false, scratch)?
                .attributes
                .kind()
                == NodeKind::Directory,
        "failed trailing symlink mutations preserve link and target",
    )?;
    check(
        root.remove("right/new/", NodeKind::File, scratch) == Err(Error::NotDirectory),
        "trailing slash cannot unlink regular file",
    )?;
    check(
        root.rename("right/new/", root, "bad-rename", scratch) == Err(Error::NotDirectory),
        "rename source directory requirement",
    )?;
    check(
        root.rename("right/new", root, "bad-rename/", scratch) == Err(Error::NotDirectory),
        "rename destination directory requirement",
    )?;
    check(
        matches!(
            root.create_file("bad-file/", 0o666, domain, Ok::<FileObject, Error>),
            Err(Error::NotDirectory)
        ),
        "trailing slash cannot create regular file",
    )?;
    check(
        root.metadata("bad-file", false, scratch) == Err(Error::Missing),
        "failed file directory requirement leaves no name",
    )?;
    check(
        root.link("right/new", root, "bad-link/", scratch) == Err(Error::NotDirectory),
        "hardlink destination directory requirement",
    )?;
    check(
        root.metadata("bad-link", false, scratch) == Err(Error::Missing),
        "failed link directory requirement leaves no name",
    )?;
    root.remove("renamed-trailing/", NodeKind::Directory, scratch)?;
    root.remove("trailing-link", NodeKind::File, scratch)?;
    Ok(())
}

fn deep_directory_rename(
    root: &DirectoryObject,
    domain: &ResourceDomain,
    scratch: &ScratchBudget,
) -> Result<(), TestError> {
    root.create("deep", NodeKind::Directory, 0o755, scratch)?;
    let mut ancestors = super::ScratchVec::new(scratch.clone());
    ancestors
        .push(root.open_directory("deep", domain)?)
        .map_err(Error::from)?;
    let result = (|| -> Result<(), TestError> {
        for _ in 0..260 {
            let current = ancestors.last().ok_or(TestError::Contract("deep parent"))?;
            current.create("d", NodeKind::Directory, 0o755, scratch)?;
            let child = current.open_directory("d", domain)?;
            ancestors.push(child).map_err(Error::from)?;
        }
        let current = ancestors.last().ok_or(TestError::Contract("deep leaf"))?;
        current.create("first", NodeKind::Directory, 0o755, scratch)?;
        current.create("second", NodeKind::Directory, 0o755, scratch)?;
        current.rename("first", current, "renamed", scratch)?;
        let second = current.open_directory("second", domain)?;
        current.rename("renamed", &second, "moved", scratch)?;
        let moved = second.open_directory("moved", domain)?;
        check(
            second.rename("moved", &moved, "cycle", scratch) == Err(Error::InvalidInput),
            "deep directory cycle remains rejected",
        )?;
        Ok(())
    })();
    // Explicit bottom-up cleanup also runs after a failed assertion. A deep
    // test fixture must never depend on recursive tree destruction at teardown.
    let mut cleanup = Ok(());
    while let Some(directory) = ancestors.pop() {
        for name in ["second/moved", "second", "renamed", "first", "d"] {
            match directory.remove(name, NodeKind::Directory, scratch) {
                Ok(()) | Err(Error::Missing) => {}
                Err(error) => {
                    if cleanup.is_ok() {
                        cleanup = Err(TestError::Vfs(error));
                    }
                }
            }
        }
    }
    match root.remove("deep", NodeKind::Directory, scratch) {
        Ok(()) | Err(Error::Missing) => {}
        Err(error) => {
            if cleanup.is_ok() {
                cleanup = Err(TestError::Vfs(error));
            }
        }
    }
    result.and(cleanup)
}

fn snapshot_cache(
    root: &DirectoryObject,
    domain: &ResourceDomain,
    scratch: &ScratchBudget,
) -> Result<(), TestError> {
    let file = root.create_file("snapshot", 0o666, domain, Ok::<FileObject, Error>)?;
    file.write(Some(0), b"original")?;
    root.link("snapshot", root, "snapshot-alias", scratch)?;
    let alias = root.open_file("snapshot-alias", domain)?;
    let first = file.readable_snapshot(domain)?;
    let second = alias.readable_snapshot(domain)?;
    let physical = |snapshot: &super::ExecutableSnapshot| {
        snapshot
            .storage()
            .resident_physical_page(0)
            .map_err(|_| TestError::Contract("snapshot physical backing"))
    };
    check(
        physical(&first)? == physical(&second)?,
        "hardlink snapshots share physical pages",
    )?;
    let old_weak = first.storage().downgrade();
    file.write(Some(0), b"modified")?;
    let changed = alias.readable_snapshot(domain)?;
    check(
        physical(&first)? != physical(&changed)?,
        "write creates a new physical generation",
    )?;
    let mut bytes = [0u8; 8];
    first
        .storage()
        .read(0, &mut bytes)
        .map_err(|_| TestError::Contract("snapshot read"))?;
    check(
        &bytes == b"original",
        "old snapshot remains immutable after file write",
    )?;
    drop(first);
    drop(second);
    check(
        old_weak.upgrade().is_none(),
        "weak file cache releases last detached generation",
    )?;
    file.resize(4)?;
    let resized = file.readable_snapshot(domain)?;
    check(
        physical(&changed)? != physical(&resized)? && resized.bytes() == b"modi",
        "resize replaces cached generation",
    )?;
    let _truncated = root.open_file_with_options(
        "snapshot-alias",
        &FileOpenOptions::new(Rights::WRITE, 4, 0o666)?,
        domain,
        Ok::<FileObject, Error>,
    )?;
    let truncated = file.readable_snapshot(domain)?;
    check(
        truncated.bytes().is_empty(),
        "truncate invalidates cached generation",
    )?;
    let weak = truncated.storage().downgrade();
    drop(truncated);
    check(
        weak.upgrade().is_none(),
        "file cache never strongly pins unused pages",
    )?;
    root.remove("snapshot-alias", NodeKind::File, scratch)?;
    root.remove("snapshot", NodeKind::File, scratch)?;
    Ok(())
}
