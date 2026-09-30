// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Cached identities yield ancestor quota to live namespace preparation.

use alloc::vec::Vec;
use core::num::NonZeroU64;

use crate::kernel::accounting::{
    ResourceAmount, ResourceDomain, ResourceError, ResourceKind, ResourceLimits,
};
use crate::kernel::io_cache::{
    CacheAccess, CacheKey, FileIdentity, FilePageIndex, FilesystemGeneration, NodeIdentity, Refill,
};

use super::{CONTENTS, Disk, Error, Fatfs, NodeKind, TestError, check, create, lookup};

pub(super) fn run() -> Result<(), TestError> {
    let cache = crate::kernel::vfs::file_cache()
        .ok_or(TestError::Contract("system cache for FAT quota recovery"))?;
    // Retain more than one emergency batch without incidental eviction.
    progress(|| {
        if cache.usage().capacity < 128 {
            let _ = cache.maintain_capacity(128);
        }
        cache.usage().capacity >= 128
    })?;
    let parent =
        ResourceDomain::try_new_root(ResourceLimits::UNLIMITED).map_err(Error::Resource)?;
    let domain = parent
        .try_new_child(ResourceLimits::UNLIMITED)
        .map_err(Error::Resource)?;
    let filesystem = Fatfs::mount(Disk::fresh()?, domain.clone())?;
    let root = filesystem.root();
    let baseline = parent.usage().committed(ResourceKind::KernelMemoryBytes);
    let mut identities = Vec::new();
    identities
        .try_reserve_exact(5)
        .map_err(|_| Error::Allocation)?;
    let mut actual = Vec::new();
    actual
        .try_reserve_exact(CONTENTS.len())
        .map_err(|_| Error::Allocation)?;
    actual.resize(CONTENTS.len(), 0);
    let mut old_id = 0;
    for (index, name) in ["first.bin", "two.bin", "three.bin", "four.bin", "five.bin"]
        .into_iter()
        .enumerate()
    {
        let node = create(&filesystem, &root, name, NodeKind::File)?;
        filesystem.write(&node, Some(0), &CONTENTS)?;
        if index == 0 {
            old_id = node.id();
        }
        identities.push(node.record.downgrade());
        // Eighty old revisions all retain the first identity. Evicting only
        // the oldest 64 pages cannot release even one namespace quota charge.
        for _ in 0..if index == 0 { 80 } else { 1 } {
            let revision = {
                let mut content = node.record.content.lock()?;
                content.begin_mutation()?;
                content.set_length(CONTENTS.len() as u64);
                content.revision()
            };
            let key = CacheKey::new(
                FileIdentity::new(
                    FilesystemGeneration::new(
                        NonZeroU64::new(u64::MAX - 1).ok_or(Error::InvalidBackendResult)?,
                    ),
                    NodeIdentity::new(
                        NonZeroU64::new(node.id()).ok_or(Error::InvalidBackendResult)?,
                    ),
                    revision,
                ),
                FilePageIndex::new(0),
            );
            progress(|| match cache.access(key) {
                Ok(CacheAccess::Hit(_)) => true,
                Ok(CacheAccess::Load(reservation)) => reservation
                    .fill_and_publish(
                        || super::CachePage::try_copy(&CONTENTS, node.record.clone()),
                        |_| Refill::Recreate,
                    )
                    .is_ok(),
                _ => false,
            })?;
        }
        drop(node);
    }
    check(
        identities
            .iter()
            .all(|identity| identity.upgrade().is_some()),
        "all closed FAT identities are still cache-owned before quota failure",
    )?;
    let cached = parent.usage().committed(ResourceKind::KernelMemoryBytes);
    parent
        .set_local_limits(ResourceLimits::UNLIMITED.with(ResourceKind::KernelMemoryBytes, cached))
        .map_err(Error::Resource)?;
    let denied = domain.reserve(ResourceAmount::ZERO.with(
        ResourceKind::KernelMemoryBytes,
        (hyper::fs::MAX_PATH_BYTES * 3) as u64,
    ));
    check(
        matches!(denied, Err(ResourceError::LimitExceeded { domain, .. }) if domain == parent.id()),
        "the ancestor, not the unlimited mount sponsor, denies new scratch",
    )?;

    // This holds the FAT sleeping mutex while waiting for the worker. Domain
    // cleanup must skip it; local pruning precedes the preparation-only retry.
    let reopened = lookup(&filesystem, &root, "first.bin")?;
    check(
        reopened.id() != old_id,
        "quota recovery discards the expired identity before reopening",
    )?;
    check(
        identities
            .iter()
            .all(|identity| identity.upgrade().is_none()),
        "finite domain reclaim releases every cache-only identity",
    )?;
    check(
        filesystem.read(&reopened, 0, &mut actual, false)? == actual.len() && actual == CONTENTS,
        "quota retry preserves on-disk file contents",
    )?;
    drop(reopened);
    filesystem.reclaim_idle_records(Some(parent.id()));
    check(
        parent.usage().committed(ResourceKind::KernelMemoryBytes) == baseline,
        "binding reclamation releases all storage with no retained table peak",
    )?;
    Ok(())
}

fn progress(mut condition: impl FnMut() -> bool) -> Result<(), TestError> {
    let completed = crate::kernel::task::wait_for_test_progress(
        crate::kernel::task::TEST_PROGRESS_TIMEOUT_NS,
        || Ok::<_, crate::kernel::task::SleepError>(condition()),
    )
    .map_err(|_| TestError::Contract("FAT quota cache progress"))?;
    check(completed, "FAT quota cache progress")
}
