// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! QEMU acceptance executable, excluded from ordinary application builds.

use std::fs::{self, File, FileTimes, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::process::ExitCode;
use std::time::{Duration, UNIX_EPOCH};

const DIRECTORY: &str = "/data/acceptance";
const PENDING: &str = "/data/acceptance/pending.bin";
const PAYLOAD: &str = "/data/acceptance/persistent 数据.bin";
const COPY: &str = "/data/acceptance/copy.bin";
#[cfg(not(feature = "userspace-device-test"))]
const LENGTH: usize = 3 * 1024 * 1024 + 37;
// Still exceeds the Native block transfer window and crosses cluster boundaries;
// this fixture targets the physical userspace IRQ/MMIO path, not FAT capacity.
#[cfg(feature = "userspace-device-test")]
const LENGTH: usize = 256 * 1024 + 37;
const GAP: usize = 4096;

fn verify(path: &str) -> io::Result<()> {
    println!("BOARD-STORAGE: opening {path}");
    let mut file = File::open(path)?;
    println!("BOARD-STORAGE: checking metadata {path}");
    if file.metadata()?.len() != (LENGTH + GAP + 1) as u64 {
        return Err(io::Error::other("incorrect persisted file length"));
    }
    // Cross all four request queues, including unaligned heads and tails.
    let mut buffer = vec![0; 512 * 1024];
    println!("BOARD-STORAGE: reading {path}");
    let mut offset = 0;
    while offset < LENGTH {
        let count = if offset == 0 {
            13
        } else {
            buffer.len().min(LENGTH - offset)
        };
        file.read_exact(&mut buffer[..count])?;
        if buffer[..count]
            .iter()
            .enumerate()
            .any(|(i, byte)| *byte != ((offset + i) % 251) as u8)
        {
            return Err(io::Error::other("persisted data mismatch"));
        }
        offset += count;
    }
    file.read_exact(&mut buffer[..GAP + 1])?;
    if buffer[..GAP].iter().any(|byte| *byte != 0) || buffer[GAP] != 0xa5 {
        return Err(io::Error::other("sparse extension mismatch"));
    }
    if file.read(&mut buffer[..1])? != 0 {
        return Err(io::Error::other("read passed end of file"));
    }
    Ok(())
}

fn run(mode: &str) -> io::Result<()> {
    println!("BOARD-STORAGE: {mode} started");
    if mode == "write" {
        fs::create_dir_all(DIRECTORY)?;
        for path in [PENDING, PAYLOAD, COPY] {
            match fs::remove_file(path) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
        let mut file = File::create(PENDING)?;
        println!("BOARD-STORAGE: writing payload");
        let mut buffer = vec![0; 64 * 1024];
        let mut offset = 0;
        while offset < LENGTH {
            let count = buffer.len().min(LENGTH - offset);
            for (i, byte) in buffer[..count].iter_mut().enumerate() {
                *byte = ((offset + i) % 251) as u8;
            }
            file.write_all(&buffer[..count])?;
            offset += count;
        }
        file.seek(SeekFrom::Start((LENGTH + GAP) as u64))?;
        file.write_all(&[0xa5])?;
        file.set_times(
            FileTimes::new().set_modified(UNIX_EPOCH + Duration::from_secs(1_750_000_000)),
        )?;
        file.sync_all()?;
        println!("BOARD-STORAGE: payload synchronized");
        drop(file);
        fs::rename(PENDING, PAYLOAD)?;
        fs::copy(PAYLOAD, COPY)?;
        // The second fence includes the rename and copy's directory entries.
        OpenOptions::new().write(true).open(COPY)?.sync_all()?;
        println!("BOARD-STORAGE: copy synchronized");
    } else if mode != "verify" {
        return Err(io::Error::other("expected write or verify"));
    }
    verify(PAYLOAD)?;
    verify(COPY)?;
    if fs::metadata(PAYLOAD)?.modified()? != UNIX_EPOCH + Duration::from_secs(1_750_000_000) {
        return Err(io::Error::other("persisted modification time mismatch"));
    }
    if fs::read_dir(DIRECTORY)?.try_fold(0, |count, entry| entry.map(|_| count + 1))? != 2 {
        return Err(io::Error::other("directory entries mismatch"));
    }
    Ok(())
}

fn main() -> ExitCode {
    let mode = std::env::args().nth(1).unwrap_or_default();
    match run(&mode) {
        Ok(()) => {
            println!("BOARD-STORAGE: {mode} PASS");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("BOARD-STORAGE: {mode} FAIL: {error}");
            ExitCode::FAILURE
        }
    }
}
