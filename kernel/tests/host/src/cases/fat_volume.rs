// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use crate::require_ok;
use hyper::fs::block::{BlockDevice, Error as BlockError, SECTOR_SIZE, validate_range};
use hyper::fs::fat::{Error, FatVolume};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct State {
    sectors: BTreeMap<u64, [u8; 512]>,
    reads: usize,
    writes: usize,
    flushes: usize,
    fail_flush: bool,
    fail_reads: bool,
    readonly: bool,
}
#[derive(Clone)]
struct Disk(Arc<Mutex<State>>);
impl Disk {
    fn fresh() -> Self {
        let mut state = State::default();
        let mut boot = [0; 512];
        boot[0..3].copy_from_slice(&[0xeb, 0x58, 0x90]);
        boot[3..11].copy_from_slice(b"HYPER   ");
        boot[11..13].copy_from_slice(&512u16.to_le_bytes());
        boot[13] = 1;
        boot[14..16].copy_from_slice(&32u16.to_le_bytes());
        boot[16] = 2;
        boot[21] = 0xf8;
        boot[32..36].copy_from_slice(&70000u32.to_le_bytes());
        boot[36..40].copy_from_slice(&600u32.to_le_bytes());
        boot[44..48].copy_from_slice(&2u32.to_le_bytes());
        boot[48..50].copy_from_slice(&1u16.to_le_bytes());
        boot[50..52].copy_from_slice(&6u16.to_le_bytes());
        boot[64] = 0x80;
        boot[66] = 0x29;
        boot[71..82].copy_from_slice(b"HYPER DATA ");
        boot[82..90].copy_from_slice(b"FAT32   ");
        boot[510..512].copy_from_slice(&[0x55, 0xaa]);
        state.sectors.insert(0, boot);
        state.sectors.insert(6, boot);
        let mut info = [0; 512];
        info[..4].copy_from_slice(&0x41615252u32.to_le_bytes());
        info[484..488].copy_from_slice(&0x61417272u32.to_le_bytes());
        info[488..492].copy_from_slice(&u32::MAX.to_le_bytes());
        info[492..496].copy_from_slice(&u32::MAX.to_le_bytes());
        info[508..512].copy_from_slice(&0xaa550000u32.to_le_bytes());
        state.sectors.insert(1, info);
        let mut fat = [0; 512];
        fat[..4].copy_from_slice(&0x0ffffff8u32.to_le_bytes());
        fat[4..8].copy_from_slice(&0xffffffffu32.to_le_bytes());
        fat[8..12].copy_from_slice(&0x0fffffffu32.to_le_bytes());
        state.sectors.insert(32, fat);
        state.sectors.insert(632, fat);
        Self(Arc::new(Mutex::new(state)))
    }
}
impl BlockDevice for Disk {
    fn is_read_only(&self) -> bool {
        require_ok(self.0.lock()).readonly
    }

    fn sector_count(&self) -> u64 {
        70000
    }
    fn read_sectors(&mut self, first: u64, output: &mut [u8]) -> Result<(), BlockError> {
        validate_range(self.sector_count(), first, output.len())?;
        let mut state = require_ok(self.0.lock());
        state.reads += 1;
        if state.fail_reads {
            return Err(BlockError::Io);
        }
        for (i, sector) in output.chunks_exact_mut(512).enumerate() {
            sector.copy_from_slice(state.sectors.get(&(first + i as u64)).unwrap_or(&[0; 512]));
        }
        Ok(())
    }
    fn write_sectors(&mut self, first: u64, input: &[u8]) -> Result<(), BlockError> {
        validate_range(self.sector_count(), first, input.len())?;
        let mut state = require_ok(self.0.lock());
        state.writes += 1;
        if state.readonly {
            return Err(BlockError::ReadOnly);
        }
        for (i, sector) in input.chunks_exact(512).enumerate() {
            let mut copy = [0; 512];
            copy.copy_from_slice(sector);
            state.sectors.insert(first + i as u64, copy);
        }
        Ok(())
    }
    fn flush(&mut self) -> Result<(), BlockError> {
        let mut state = require_ok(self.0.lock());
        state.flushes += 1;
        if state.fail_flush {
            Err(BlockError::Io)
        } else {
            Ok(())
        }
    }
}
#[test]
fn fat_file_timestamps_use_utc_and_survive_remount() {
    use hyper::time::Timestamp;
    let disk = Disk::fresh();
    let mut fs = require_ok(FatVolume::mount_with_clock(disk.clone(), || {
        Timestamp::new(1709251201, 120_000_000)
    }));
    require_ok(fs.create("time.txt", false));
    let created = require_ok(fs.stat("time.txt")).created;
    assert_eq!(created, Timestamp::new(1709251201, 120_000_000));
    // 2000-02-29 is a leap day; modification time has FAT's two-second precision.
    let update = Timestamp::new(951827697, 123_000_000);
    require_ok(fs.set_times("time.txt", update, update));
    require_ok(fs.sync());
    drop(fs);
    let mut fs = require_ok(FatVolume::mount(disk));
    let entry = require_ok(fs.stat("time.txt"));
    assert_eq!(entry.created, created);
    assert_eq!(entry.modified, Timestamp::new(951827696, 0));
    assert_eq!(entry.accessed, Timestamp::new(951782400, 0));
    assert_eq!(
        fs.set_times("time.txt", None, Timestamp::new(0, 0)),
        Err(Error::InvalidInput)
    );
    assert_eq!(require_ok(fs.stat("time.txt")).modified, entry.modified);
    // End of the representable FAT range, including the non-leap year 2100.
    require_ok(fs.set_times("time.txt", None, Timestamp::new(4354819199, 0)));
    assert_eq!(
        require_ok(fs.stat("time.txt")).modified,
        Timestamp::new(4354819198, 0)
    );
    assert_eq!(
        fs.set_times("time.txt", None, Timestamp::new(4354819200, 0)),
        Err(Error::InvalidInput)
    );
}

#[test]
fn fat_persists_long_names_sparse_writes_and_resize_across_mounts() {
    let disk = Disk::fresh();
    let mut fs = require_ok(FatVolume::mount(disk.clone()));
    require_ok(fs.create("virtual machines", true));
    require_ok(fs.create("virtual machines/configuration.json", false));
    require_ok(fs.write_at("virtual machines/configuration.json", 700, b"persisted"));
    require_ok(fs.sync());
    drop(fs);
    let mut fs = require_ok(FatVolume::mount(disk));
    let mut output = [0xff; 709];
    assert_eq!(
        require_ok(fs.read_at("virtual machines/configuration.json", 0, &mut output)),
        709
    );
    assert!(output[..700].iter().all(|b| *b == 0));
    assert_eq!(&output[700..], b"persisted");
    require_ok(fs.resize("virtual machines/configuration.json", 3));
    require_ok(fs.rename("virtual machines/configuration.json", "configuration.json"));
    require_ok(fs.remove("virtual machines"));
    assert_eq!(
        require_ok(fs.read_at("configuration.json", 0, &mut output)),
        3
    );
    require_ok(fs.remove("configuration.json"));
    require_ok(fs.sync());
}
#[test]
fn fat_drop_never_issues_device_io() {
    let disk = Disk::fresh();
    let mut fs = require_ok(FatVolume::mount(disk.clone()));
    require_ok(fs.create("test.txt", false));
    require_ok(fs.write_at("test.txt", 0, b"hello"));
    let before = {
        let state = require_ok(disk.0.lock());
        (state.reads, state.writes, state.flushes)
    };
    drop(fs);
    let state = require_ok(disk.0.lock());
    assert_eq!((state.reads, state.writes, state.flushes), before);
}
#[test]
fn fat_flush_error_is_reported_and_volume_is_not_reused() {
    let disk = Disk::fresh();
    let mut fs = require_ok(FatVolume::mount(disk.clone()));
    require_ok(fs.create("dirty.txt", false));
    require_ok(fs.write_at("dirty.txt", 0, b"pending"));
    require_ok(disk.0.lock()).fail_flush = true;
    assert_eq!(fs.sync(), Err(Error::Block(BlockError::Io)));
    let before = {
        let mut state = require_ok(disk.0.lock());
        assert_eq!(state.flushes, 1);
        // Recovery of the backend must not reopen a failed metadata view.
        state.fail_flush = false;
        (state.reads, state.writes, state.flushes)
    };
    assert!(matches!(fs.entry("", 0), Err(Error::Block(BlockError::Io))));
    assert_eq!(
        fs.write_at("dirty.txt", 0, b"lost"),
        Err(Error::Block(BlockError::Io))
    );
    assert_eq!(fs.sync(), Err(Error::Block(BlockError::Io)));
    drop(fs);
    let state = require_ok(disk.0.lock());
    assert_eq!((state.reads, state.writes, state.flushes), before);
}

#[test]
fn fat_sync_retains_clean_view_and_persists_later_allocations() {
    let disk = Disk::fresh();
    let mut fs = require_ok(FatVolume::mount(disk.clone()));
    require_ok(fs.create("continued.txt", false));
    require_ok(fs.write_at("continued.txt", 0, b"first"));
    require_ok(fs.sync());
    let before = {
        let state = require_ok(disk.0.lock());
        assert_eq!(state.sectors[&0][0x41] & 1, 0);
        (state.reads, state.writes, state.flushes)
    };
    require_ok(fs.sync());
    {
        let state = require_ok(disk.0.lock());
        // A clean sync fences the device without reparsing the filesystem.
        assert_eq!(
            (state.reads, state.writes, state.flushes),
            (before.0, before.1, before.2 + 1)
        );
    }
    require_ok(fs.write_at("continued.txt", 700, b"second"));
    assert_ne!(require_ok(disk.0.lock()).sectors[&0][0x41] & 1, 0);
    require_ok(fs.create("later.txt", false));
    require_ok(fs.write_at("later.txt", 0, &[0x5a; 1024]));
    require_ok(fs.sync());
    assert_eq!(require_ok(disk.0.lock()).sectors[&0][0x41] & 1, 0);
    drop(fs);
    let mut fs = require_ok(FatVolume::mount(disk));
    let mut bytes = [0xff; 706];
    assert_eq!(
        require_ok(fs.read_at("continued.txt", 0, &mut bytes)),
        bytes.len()
    );
    assert_eq!(&bytes[..5], b"first");
    assert!(bytes[5..700].iter().all(|byte| *byte == 0));
    assert_eq!(&bytes[700..], b"second");
    let mut later = [0; 1024];
    assert_eq!(
        require_ok(fs.read_at("later.txt", 0, &mut later)),
        later.len()
    );
    assert_eq!(later, [0x5a; 1024]);
}
#[test]
fn fat_rejects_cyclic_root_and_out_of_bounds_directory_cluster() {
    let disk = Disk::fresh();
    {
        let mut state = require_ok(disk.0.lock());
        let fat = crate::require_some(state.sectors.get_mut(&32));
        fat[8..12].copy_from_slice(&2u32.to_le_bytes());
    }
    assert!(matches!(FatVolume::mount(disk), Err(Error::Corrupt)));
    let disk = Disk::fresh();
    {
        let mut state = require_ok(disk.0.lock());
        let mut root = [0; 512];
        root[..11].copy_from_slice(b"BAD     TXT");
        root[20..22].copy_from_slice(&0xffffu16.to_le_bytes());
        root[26..28].copy_from_slice(&0xffffu16.to_le_bytes());
        state.sectors.insert(1232, root);
    }
    assert!(matches!(FatVolume::mount(disk), Err(Error::Corrupt)));
}
#[test]
fn block_ranges_reject_overflow_and_partial_sectors() {
    assert_eq!(
        validate_range(10, 9, 2 * SECTOR_SIZE),
        Err(BlockError::InvalidRange)
    );
    assert_eq!(
        validate_range(u64::MAX, u64::MAX, SECTOR_SIZE),
        Err(BlockError::InvalidRange)
    );
    assert_eq!(validate_range(10, 0, 1), Err(BlockError::InvalidRange));
    assert_eq!(validate_range(10, 10, 0), Ok(()));
}

#[test]
fn fat_metadata_reads_do_not_flush_the_physical_device() {
    let disk = Disk::fresh();
    let mut fs = require_ok(FatVolume::mount(disk.clone()));
    require_ok(fs.create("config.json", false));
    require_ok(fs.write_at("config.json", 0, b"{}"));
    let _ = require_ok(fs.stat("config.json"));
    let mut data = [0; 2];
    require_ok(fs.read_at("config.json", 0, &mut data));
    assert_eq!(require_ok(disk.0.lock()).flushes, 0);
    require_ok(fs.sync());
    assert_eq!(require_ok(disk.0.lock()).flushes, 1);
}

#[test]
fn fat_create_is_exclusive_and_zero_write_does_not_extend() {
    let disk = Disk::fresh();
    let mut fs = require_ok(FatVolume::mount(disk));
    require_ok(fs.create("config.json", false));
    assert_eq!(fs.create("config.json", false), Err(Error::Exists));
    assert_eq!(require_ok(fs.write_at("config.json", 512, &[])), 0);
    assert_eq!(require_ok(fs.stat("config.json")).size, 0);
    let mut byte = [0];
    assert_eq!(
        require_ok(fs.read_at("config.json", u64::MAX, &mut byte)),
        0
    );
}

#[test]
fn fat_rejects_file_chains_crosslinked_to_directory() {
    let disk = Disk::fresh();
    {
        let mut state = require_ok(disk.0.lock());
        let mut root = [0; 512];
        root[..11].copy_from_slice(b"BAD     TXT");
        root[26..28].copy_from_slice(&2u16.to_le_bytes());
        root[28..32].copy_from_slice(&512u32.to_le_bytes());
        state.sectors.insert(1232, root);
    }
    assert!(matches!(FatVolume::mount(disk), Err(Error::Corrupt)));
}

#[test]
fn fat_rename_full_destination_preserves_source() {
    let disk = Disk::fresh();
    {
        let mut state = require_ok(disk.0.lock());
        let mut allocated = [0u8; 512];
        for value in allocated.chunks_exact_mut(4) {
            value.copy_from_slice(&0x0fffffffu32.to_le_bytes());
        }
        for sector in 32..1232 {
            state.sectors.insert(sector, allocated);
        }
        let mut root = [0u8; 512];
        root[..11].copy_from_slice(b"OLD     TXT");
        root[11] = 0x20;
        root[32..43].copy_from_slice(b"DEST       ");
        root[43] = 0x10;
        root[58..60].copy_from_slice(&3u16.to_le_bytes());
        state.sectors.insert(1232, root);
        let mut destination = [0u8; 512];
        for (index, entry) in destination.chunks_exact_mut(32).enumerate() {
            let name = format!("F{index:07}TXT");
            entry[..11].copy_from_slice(name.as_bytes());
            entry[11] = 0x20;
        }
        state.sectors.insert(1233, destination);
    }
    let mut fs = require_ok(FatVolume::mount(disk));
    assert_eq!(
        fs.rename("OLD.TXT", "DEST/long-destination-name.txt"),
        Err(Error::NoSpace)
    );
    assert!(fs.stat("OLD.TXT").is_ok(), "ENOSPC must not unlink source");
}

#[test]
fn fat_directory_move_updates_on_disk_parent() {
    let disk = Disk::fresh();
    let mut fs = require_ok(FatVolume::mount(disk.clone()));
    require_ok(fs.create("a", true));
    require_ok(fs.create("b", true));
    require_ok(fs.create("a/child", true));
    require_ok(fs.rename("a/child", "b/child"));
    assert!(matches!(fs.stat("a/child"), Err(Error::Missing)));
    require_ok(fs.sync());
    let state = require_ok(disk.0.lock());
    let cluster = |sector: u64, name: &[u8; 11]| {
        state.sectors[&sector]
            .chunks_exact(32)
            .find(|entry| &entry[..11] == name)
            .map(|entry| {
                u32::from(u16::from_le_bytes([entry[26], entry[27]]))
                    | (u32::from(u16::from_le_bytes([entry[20], entry[21]])) << 16)
            })
            .unwrap_or(0)
    };
    let parent = cluster(1232, b"B          ");
    assert!(parent >= 2);
    let child = cluster(1232 + u64::from(parent) - 2, b"CHILD      ");
    assert!(child >= 2);
    assert_eq!(cluster(1232 + u64::from(child) - 2, b"..         "), parent);
    drop(state);
    require_ok(fs.rename("b/child", "child"));
    require_ok(fs.sync());
    drop(fs);
    let _remounted = require_ok(FatVolume::mount(disk.clone()));
    let state = require_ok(disk.0.lock());
    let moved = &state.sectors[&(1232 + u64::from(child) - 2)];
    assert_eq!(&moved[32..43], b"..         ");
    assert_eq!(&moved[52..54], &[0, 0]);
    assert_eq!(&moved[58..60], &[0, 0]);
}

#[test]
fn fat_deep_paths_do_not_recurse_on_the_kernel_stack() {
    let worker = require_ok(
        std::thread::Builder::new()
            .stack_size(128 * 1024)
            .spawn(|| {
                let mut fs = require_ok(FatVolume::mount(Disk::fresh()));
                let mut directory = String::new();
                for _ in 0..192 {
                    if !directory.is_empty() {
                        directory.push('/');
                    }
                    directory.push('a');
                    require_ok(fs.create(&directory, true));
                }
                let file = format!("{directory}/file");
                let renamed = format!("{directory}/renamed");
                require_ok(fs.create(&file, false));
                assert_eq!(require_ok(fs.write_at(&file, 0, b"deep")), 4);
                require_ok(fs.rename(&file, &renamed));
                let mut output = [0; 4];
                assert_eq!(require_ok(fs.read_at(&renamed, 0, &mut output)), 4);
                assert_eq!(&output, b"deep");
                require_ok(fs.remove(&renamed));
                require_ok(fs.remove(&directory));
            }),
    );
    require_ok(worker.join());
}

#[test]
fn fat_read_only_rejections_preserve_read_access_without_device_writes() {
    let disk = Disk::fresh();
    let mut volume = require_ok(FatVolume::mount(disk.clone()));
    require_ok(volume.create("keep.txt", false));
    require_ok(volume.write_at("keep.txt", 0, b"keep"));
    require_ok(volume.sync());
    drop(volume);
    let (writes, flushes) = {
        let mut state = require_ok(disk.0.lock());
        state.readonly = true;
        (state.writes, state.flushes)
    };
    let mut volume = require_ok(FatVolume::mount(disk.clone()));
    assert!(volume.is_read_only());
    assert!(require_ok(volume.stat("")).read_only);
    assert!(require_ok(volume.stat("keep.txt")).read_only);
    let error = Error::Block(BlockError::ReadOnly);
    assert_eq!(volume.write_at("keep.txt", 0, b"bad"), Err(error));
    assert_eq!(volume.create("new.txt", false), Err(error));
    assert_eq!(volume.create("directory", true), Err(error));
    assert_eq!(volume.remove("keep.txt"), Err(error));
    assert_eq!(volume.rename("keep.txt", "lost.txt"), Err(error));
    assert_eq!(volume.resize("keep.txt", 0), Err(error));
    assert_eq!(volume.set_times("keep.txt", None, None), Err(error));
    let mut bytes = [0; 4];
    assert_eq!(require_ok(volume.read_at("keep.txt", 0, &mut bytes)), 4);
    assert_eq!(&bytes, b"keep");
    require_ok(volume.sync());
    drop(volume);
    let state = require_ok(disk.0.lock());
    assert_eq!(state.writes, writes);
    assert_eq!(state.flushes, flushes);
}

#[test]
fn fat_short_alias_is_canonical_and_exclusive_create_preserves_contents() {
    let disk = Disk::fresh();
    let mut fs = require_ok(FatVolume::mount(disk.clone()));
    require_ok(fs.create("longfilename.txt", false));
    require_ok(fs.write_at("longfilename.txt", 0, b"original"));
    let alias = first_root_alias(&disk);
    assert_eq!(require_ok(fs.stat(&alias)).name(), "longfilename.txt");
    assert_eq!(fs.create(&alias, false), Err(Error::Exists));
    let mut bytes = [0; 8];
    assert_eq!(require_ok(fs.read_at(&alias, 0, &mut bytes)), 8);
    assert_eq!(&bytes, b"original");
    require_ok(fs.sync());
    drop(fs);
    let mut fs = require_ok(FatVolume::mount(disk));
    assert_eq!(require_ok(fs.stat(&alias)).name(), "longfilename.txt");
    assert_eq!(require_ok(fs.read_at("longfilename.txt", 0, &mut bytes)), 8);
    assert_eq!(&bytes, b"original");
}

#[test]
fn fat_mkdir_full_parent_returns_unpublished_cluster() {
    let disk = Disk::fresh();
    {
        let mut state = require_ok(disk.0.lock());
        let mut allocated = [0u8; 512];
        for value in allocated.chunks_exact_mut(4) {
            value.copy_from_slice(&0x0fffffffu32.to_le_bytes());
        }
        for sector in 32..1232 {
            state.sectors.insert(sector, allocated);
        }
        for sector in [32, 632] {
            require_ok(state.sectors.get_mut(&sector).ok_or("FAT"))[12..16].fill(0);
        }
        let mut root = [0; 512];
        for (index, entry) in root.chunks_exact_mut(32).enumerate() {
            entry[..11].copy_from_slice(format!("F{index:07}TXT").as_bytes());
            entry[11] = 0x20;
        }
        state.sectors.insert(1232, root);
    }
    let mut fs = require_ok(FatVolume::mount(disk.clone()));
    for _ in 0..2 {
        assert_eq!(fs.create("child", true), Err(Error::NoSpace));
        assert!(matches!(fs.stat("child"), Err(Error::Missing)));
        let state = require_ok(disk.0.lock());
        for sector in [32, 632] {
            assert_eq!(
                &require_ok(state.sectors.get(&sector).ok_or("FAT"))[12..16],
                &[0; 4]
            );
        }
    }
    require_ok(fs.sync());
    drop(fs);
    let mut fs = require_ok(FatVolume::mount(disk));
    assert!(matches!(fs.stat("child"), Err(Error::Missing)));
    assert!(fs.stat("F0000000.TXT").is_ok());
}

#[test]
fn fat_iterator_preserves_names_end_and_io_errors() {
    let disk = Disk::fresh();
    let mut fs = require_ok(FatVolume::mount(disk.clone()));
    require_ok(fs.create("deleted-long-name.txt", false));
    require_ok(fs.create("SHORT.TXT", false));
    let unicode = "é".repeat(100) + ".txt";
    require_ok(fs.create(&unicode, false));
    require_ok(fs.remove("deleted-long-name.txt"));
    let mut names = Vec::new();
    for index in 0..3 {
        if let Some(entry) = require_ok(fs.entry("", index)) {
            names.push(entry.name().to_owned());
        } else {
            assert_eq!(index, 2);
        }
    }
    assert_eq!(names, ["SHORT.TXT".to_owned(), unicode]);
    assert!(require_ok(fs.entry("", 3)).is_none());
    require_ok(fs.sync());
    require_ok(disk.0.lock()).fail_reads = true;
    assert!(matches!(fs.entry("", 0), Err(Error::Block(BlockError::Io))));
    let reads = require_ok(disk.0.lock()).reads;
    assert!(matches!(fs.entry("", 0), Err(Error::Block(BlockError::Io))));
    assert_eq!(require_ok(disk.0.lock()).reads, reads);
}

// Decode the first actual on-disk SFN instead of assuming its collision suffix.
fn first_root_alias(disk: &Disk) -> String {
    let state = require_ok(disk.0.lock());
    let root = require_ok(state.sectors.get(&1232).ok_or("root sector"));
    let short = require_ok(
        root.chunks_exact(32)
            .find(|e| e[0] != 0 && e[0] != 0xe5 && e[11] != 0xf)
            .ok_or("SFN"),
    );
    let base = require_ok(std::str::from_utf8(&short[..8])).trim_end();
    let ext = require_ok(std::str::from_utf8(&short[8..11])).trim_end();
    format!("{base}.{ext}")
}

#[test]
fn fat_compact_mutation_cursor_preserves_collisions_aliases_and_data() {
    let disk = Disk::fresh();
    let mut fs = require_ok(FatVolume::mount(disk.clone()));
    for index in 0..12u8 {
        let name = format!("long-collision-name-{index}.txt");
        require_ok(fs.create(&name, false));
        require_ok(fs.write_at(&name, 0, &[index; 600]));
    }
    let alias = first_root_alias(&disk);
    assert_eq!(
        fs.rename(&alias, "long-collision-name-1.txt"),
        Err(Error::Exists)
    );
    require_ok(fs.rename(&alias, "renamed-long-name.txt"));
    assert!(matches!(
        fs.stat("long-collision-name-0.txt"),
        Err(Error::Missing)
    ));
    for index in (2..12u8).step_by(2) {
        require_ok(fs.remove(&format!("long-collision-name-{index}.txt")));
    }
    // Reuse deleted directory slots and freed clusters with different metadata.
    require_ok(fs.create("replacement-long-name.txt", false));
    require_ok(fs.write_at("replacement-long-name.txt", 0, &[99; 600]));
    require_ok(fs.sync());
    drop(fs);
    let mut fs = require_ok(FatVolume::mount(disk));
    let mut bytes = [0; 600];
    for (name, value) in [
        ("renamed-long-name.txt", 0),
        ("replacement-long-name.txt", 99),
    ] {
        assert_eq!(require_ok(fs.read_at(name, 0, &mut bytes)), bytes.len());
        assert_eq!(bytes, [value; 600]);
    }
    for index in (1..12u8).step_by(2) {
        assert_eq!(
            require_ok(fs.read_at(&format!("long-collision-name-{index}.txt"), 0, &mut bytes)),
            bytes.len()
        );
        assert_eq!(bytes, [index; 600]);
    }
}

#[test]
fn fat_inplace_lfn_parser_preserves_malformed_sequence_behavior() {
    for defect in ["checksum", "order", "deleted", "end", "unicode"] {
        let disk = Disk::fresh();
        let mut fs = require_ok(FatVolume::mount(disk.clone()));
        require_ok(fs.create("longfilename.txt", false));
        require_ok(fs.write_at("longfilename.txt", 0, b"original"));
        require_ok(fs.create("NEXT.TXT", false));
        require_ok(fs.sync());
        let alias = first_root_alias(&disk);
        drop(fs);
        {
            let mut state = require_ok(disk.0.lock());
            let root = require_ok(state.sectors.get_mut(&1232).ok_or("root sector"));
            // This name occupies two LFN slots followed by its authoritative SFN.
            assert_eq!(root[0], 0x42);
            assert_eq!(root[32], 1);
            assert_ne!(root[64 + 11], 0xf);
            match defect {
                "checksum" => root[13] ^= 1,
                "order" => root[32] = 2,
                "deleted" => root[32] = 0xe5,
                "end" => root[64] = 0,
                "unicode" => root[33..35].copy_from_slice(&0xd800u16.to_le_bytes()),
                _ => unreachable!(),
            }
        }
        let mut fs = require_ok(FatVolume::mount(disk.clone()));
        if defect == "end" {
            assert!(require_ok(fs.entry("", 0)).is_none());
        } else if defect == "unicode" {
            let writes = require_ok(disk.0.lock()).writes;
            assert!(matches!(
                fs.create("NEXT.TXT", false),
                Err(Error::Block(BlockError::Corrupt))
            ));
            assert_eq!(require_ok(disk.0.lock()).writes, writes);
            assert!(matches!(
                fs.entry("", 0),
                Err(Error::Block(BlockError::Corrupt))
            ));
        } else {
            assert!(matches!(fs.create(&alias, false), Err(Error::Exists)));
            let entry = require_ok(require_ok(fs.entry("", 0)).ok_or("missing short entry"));
            assert_eq!(entry.name(), alias);
            let next = require_ok(require_ok(fs.entry("", 1)).ok_or("missing next entry"));
            assert_eq!(next.name(), "NEXT.TXT");
            let mut data = [0; 8];
            assert_eq!(require_ok(fs.read_at(&alias, 0, &mut data)), 8);
            assert_eq!(&data, b"original");
        }
    }
}

#[test]
fn fat_restarted_lfn_chain_cannot_retain_an_older_long_suffix() {
    let disk = Disk::fresh();
    let mut fs = require_ok(FatVolume::mount(disk.clone()));
    let old = "old-long-name-".repeat(7) + ".txt";
    require_ok(fs.create(&old, false));
    require_ok(fs.create("new-name.txt", false));
    require_ok(fs.write_at("new-name.txt", 0, b"new"));
    require_ok(fs.sync());
    drop(fs);
    {
        let mut state = require_ok(disk.0.lock());
        let root = require_ok(state.sectors.get_mut(&1232).ok_or("root sector"));
        let old_short = require_ok(
            root.chunks_exact(32)
                .position(|entry| entry[11] != 0xf)
                .ok_or("old SFN"),
        );
        let mut replacement = [0; 64];
        replacement.copy_from_slice(&root[(old_short + 1) * 32..(old_short + 3) * 32]);
        assert_eq!(replacement[0], 0x41);
        assert_ne!(replacement[32 + 11], 0xf);
        // Keep the old chain's first LAST fragment, then begin a different,
        // shorter valid chain before the old sequence can finish.
        assert!(root[0] > 0x41);
        root[32..96].copy_from_slice(&replacement);
        root[96] = 0;
    }
    let mut fs = require_ok(FatVolume::mount(disk));
    let entry = require_ok(require_ok(fs.entry("", 0)).ok_or("new entry"));
    assert_eq!(entry.name(), "new-name.txt");
    assert!(require_ok(fs.entry("", 1)).is_none());
    let mut data = [0; 3];
    assert_eq!(require_ok(fs.read_at("new-name.txt", 0, &mut data)), 3);
    assert_eq!(&data, b"new");
}
