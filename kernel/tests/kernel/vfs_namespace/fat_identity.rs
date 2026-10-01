// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Cache-only file identity through the real FAT namespace adapter.

#[path = "fat_identity/quota.rs"]
mod quota;

use alloc::vec::Vec;
use core::num::NonZeroU64;

use hyper::fs::block::{BlockDevice, Error as BlockError, validate_range};
use hyper::fs::{Name, NodeKind};
use hyper::mm::FallibleArc;

use crate::kernel::accounting::ResourceDomain;
use crate::kernel::io_cache::{
    CacheAccess, CacheKey, FileDataCache, FileIdentity, FilePageIndex, FilesystemGeneration,
    NodeIdentity, Refill,
};

use super::super::fat::{Fatfs, Node};
use super::super::file_record::CachePage;
use super::super::instance::{Creation, EntryName, Error};
use super::{TestError, check};

static CONTENTS: [u8; 4096] = [0x3d; 4096];

pub(super) fn run(domain: &ResourceDomain) -> Result<(), TestError> {
    super::super::fat::test_record_storage(domain)?;
    let filesystem = Fatfs::mount(Disk::fresh()?, domain.clone())?;
    let root = filesystem.root();
    let parent = create(&filesystem, &root, "before", NodeKind::Directory)?;
    let node = create(&filesystem, &parent, "identity.bin", NodeKind::File)?;
    filesystem.write(&node, Some(0), &CONTENTS)?;
    let identity = node.id();
    let active = node.downgrade();
    let content = node.record.downgrade();
    let revision = {
        let mut guard = node.record.content.lock()?;
        guard.set_length(CONTENTS.len() as u64);
        guard.revision()
    };
    let cache = FileDataCache::try_new(1).map_err(|_| Error::Allocation)?;
    let key = CacheKey::new(
        FileIdentity::new(
            FilesystemGeneration::new(NonZeroU64::MAX),
            NodeIdentity::new(NonZeroU64::new(identity).ok_or(Error::InvalidBackendResult)?),
            revision,
        ),
        FilePageIndex::new(0),
    );
    let reservation = match cache.access(key) {
        Ok(CacheAccess::Load(reservation)) => reservation,
        _ => return Err(TestError::Contract("FAT identity cache admission")),
    };
    drop(
        reservation
            .fill_and_publish(
                || CachePage::try_copy(&CONTENTS, node.record.clone()),
                |_| Refill::Recreate,
            )
            .map_err(|_| Error::Allocation)?,
    );
    drop(node);
    check(
        active.upgrade().is_none() && content.upgrade().is_some(),
        "cached identity survives actual final active-node destruction",
    )?;

    let reopened = lookup(&filesystem, &parent, "identity.bin")?;
    let alias = lookup(&filesystem, &parent, "IDENTITY.BIN")?;
    check(
        reopened.id() == identity && alias.id() == identity,
        "reopened canonical and case alias retain the cached incarnation",
    )?;
    check(
        filesystem.remove(&parent, entry("identity.bin")?, NodeKind::File, None, None)
            == Err(Error::Busy),
        "live opens still prevent FAT unlink",
    )?;
    let reopened_active = reopened.downgrade();
    drop(alias);
    drop(reopened);
    drop(parent);
    check(
        reopened_active.upgrade().is_none(),
        "all reopened active owners are synchronously gone",
    )?;
    check(
        filesystem.reclaim_records(1) == (1, 0),
        "housekeeping moves the cached binding before rename",
    )?;
    filesystem.rename(
        &root,
        entry("before")?,
        &root,
        entry("after")?,
        filesystem.epoch(),
    )?;
    let parent = lookup(&filesystem, &root, "after")?;
    let renamed = lookup(&filesystem, &parent, "identity.bin")?;
    check(
        renamed.id() == identity,
        "rename updates idle cached descendants",
    )?;
    drop(renamed);
    filesystem.remove(
        &parent,
        entry("identity.bin")?,
        NodeKind::File,
        Some(identity),
        None,
    )?;
    check(
        content.upgrade().is_some(),
        "unlink does not need to destroy cached content",
    )?;
    let replacement = create(&filesystem, &parent, "identity.bin", NodeKind::File)?;
    check(
        replacement.id() != identity,
        "recreated path receives a fresh incarnation",
    )?;
    check(
        filesystem.attributes(&replacement)?.size() == 0,
        "recreated file cannot inherit cached length",
    )?;
    drop(replacement);
    check(
        cache.reclaim_batch(1) == 1,
        "reclaim releases the retained old page",
    )?;
    check(
        content.upgrade().is_none(),
        "unlinked incarnation retires with its final page",
    )?;

    let failed: Result<(), Error> = filesystem.create(
        &parent,
        entry("failed.bin")?,
        creation(NodeKind::File),
        None,
        |_| Err(Error::InvalidInput),
    );
    check(
        failed == Err(Error::InvalidInput),
        "FAT create preparation failure is reported",
    )?;
    check(
        filesystem.lookup(&parent, name("failed.bin")?)?.is_none(),
        "failed create publishes no name binding",
    )?;
    let created = create(&filesystem, &parent, "failed.bin", NodeKind::File)?;
    check(
        created.id() != identity,
        "create after failed preparation has a valid fresh owner",
    )?;
    quota::run()?;
    Ok(())
}

fn create(
    filesystem: &Fatfs<Disk>,
    directory: &FallibleArc<Node>,
    value: &str,
    kind: NodeKind,
) -> Result<FallibleArc<Node>, Error> {
    filesystem.create(
        directory,
        entry(value)?,
        creation(kind),
        None,
        Ok::<_, Error>,
    )
}

fn lookup(
    filesystem: &Fatfs<Disk>,
    directory: &Node,
    value: &str,
) -> Result<FallibleArc<Node>, Error> {
    filesystem
        .lookup(directory, name(value)?)?
        .ok_or(Error::Missing)
}

fn creation(kind: NodeKind) -> Creation<'static> {
    Creation {
        kind,
        mode: 0o777,
        target: None,
    }
}

fn name(value: &str) -> Result<Name<'_>, Error> {
    Name::new(value).map_err(|_| Error::InvalidInput)
}

fn entry(value: &str) -> Result<EntryName<'_>, Error> {
    Ok(EntryName {
        name: name(value)?,
        directory_required: false,
    })
}

/// The same FAT32 geometry used by volume host tests, with a bounded sparse
/// backing. Empty sectors read as zero; no allocation occurs during media I/O.
struct Disk {
    sectors: Vec<(u64, [u8; 512])>,
}

impl Disk {
    #[inline(never)]
    fn fresh() -> Result<Self, Error> {
        let mut sectors = Vec::new();
        sectors
            .try_reserve_exact(128)
            .map_err(|_| Error::Allocation)?;
        let mut boot = [0; 512];
        boot[0..3].copy_from_slice(&[0xeb, 0x58, 0x90]);
        boot[3..11].copy_from_slice(b"HYPER   ");
        boot[11..13].copy_from_slice(&512_u16.to_le_bytes());
        boot[13] = 1;
        boot[14..16].copy_from_slice(&32_u16.to_le_bytes());
        boot[16] = 2;
        boot[21] = 0xf8;
        boot[32..36].copy_from_slice(&70000_u32.to_le_bytes());
        boot[36..40].copy_from_slice(&600_u32.to_le_bytes());
        boot[44..48].copy_from_slice(&2_u32.to_le_bytes());
        boot[48..50].copy_from_slice(&1_u16.to_le_bytes());
        boot[50..52].copy_from_slice(&6_u16.to_le_bytes());
        boot[64] = 0x80;
        boot[66] = 0x29;
        boot[71..82].copy_from_slice(b"HYPER DATA ");
        boot[82..90].copy_from_slice(b"FAT32   ");
        boot[510..512].copy_from_slice(&[0x55, 0xaa]);
        sectors.push((0, boot));
        sectors.push((6, boot));
        let mut info = [0; 512];
        info[..4].copy_from_slice(&0x41615252_u32.to_le_bytes());
        info[484..488].copy_from_slice(&0x61417272_u32.to_le_bytes());
        info[488..492].copy_from_slice(&u32::MAX.to_le_bytes());
        info[492..496].copy_from_slice(&u32::MAX.to_le_bytes());
        info[508..512].copy_from_slice(&0xaa550000_u32.to_le_bytes());
        sectors.push((1, info));
        let mut fat = [0; 512];
        fat[..4].copy_from_slice(&0x0ffffff8_u32.to_le_bytes());
        fat[4..8].copy_from_slice(&0xffffffff_u32.to_le_bytes());
        fat[8..12].copy_from_slice(&0x0fffffff_u32.to_le_bytes());
        sectors.push((32, fat));
        sectors.push((632, fat));
        Ok(Self { sectors })
    }
}

impl BlockDevice for Disk {
    fn sector_count(&self) -> u64 {
        70000
    }

    fn read_sectors(&mut self, first: u64, output: &mut [u8]) -> Result<(), BlockError> {
        validate_range(self.sector_count(), first, output.len())?;
        for (index, output) in output.chunks_exact_mut(512).enumerate() {
            match self
                .sectors
                .iter()
                .find(|(sector, _)| *sector == first + index as u64)
            {
                Some((_, bytes)) => output.copy_from_slice(bytes),
                None => output.fill(0),
            }
        }
        Ok(())
    }

    fn write_sectors(&mut self, first: u64, input: &[u8]) -> Result<(), BlockError> {
        validate_range(self.sector_count(), first, input.len())?;
        for (index, input) in input.chunks_exact(512).enumerate() {
            let number = first + index as u64;
            if let Some((_, bytes)) = self
                .sectors
                .iter_mut()
                .find(|(sector, _)| *sector == number)
            {
                bytes.copy_from_slice(input);
            } else {
                if self.sectors.len() == self.sectors.capacity() {
                    return Err(BlockError::Exhausted);
                }
                let mut bytes = [0; 512];
                bytes.copy_from_slice(input);
                self.sectors.push((number, bytes));
            }
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<(), BlockError> {
        Ok(())
    }
}
