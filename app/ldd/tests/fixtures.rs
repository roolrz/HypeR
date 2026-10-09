// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

pub fn put16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}
pub fn put32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
pub fn put64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

pub fn elf(
    needed: &[&str],
    soname: Option<&str>,
    interpreter: Option<&str>,
    machine: u16,
) -> Vec<u8> {
    let mut strings = vec![0];
    let mut name = |name: &str| {
        let offset = strings.len() as u64;
        strings.extend_from_slice(name.as_bytes());
        strings.push(0);
        offset
    };
    let mut tags: Vec<(u64, u64)> = needed.iter().map(|value| (1, name(value))).collect();
    if let Some(soname) = soname {
        tags.push((14, name(soname)));
    }
    tags.extend([(5, 0x400800), (10, strings.len() as u64), (0, 0)]);
    let mut bytes = vec![0; 4096];
    bytes[..9].copy_from_slice(b"\x7fELF\x02\x01\x01\x3f\x00");
    put16(&mut bytes, 16, 3);
    put16(&mut bytes, 18, machine);
    put32(&mut bytes, 20, 1);
    put64(&mut bytes, 24, if soname.is_some() { 0 } else { 0x400100 });
    put64(&mut bytes, 32, 64);
    put32(&mut bytes, 48, if machine == 243 { 4 } else { 0 });
    put16(&mut bytes, 52, 64);
    put16(&mut bytes, 54, 56);
    put16(&mut bytes, 56, if interpreter.is_some() { 3 } else { 2 });
    let mut segment = |index: usize, kind, offset, address, size| {
        let start = 64 + index * 56;
        put32(&mut bytes, start, kind);
        put64(&mut bytes, start + 8, offset);
        put64(&mut bytes, start + 16, address);
        put64(&mut bytes, start + 32, size);
        put64(&mut bytes, start + 40, size);
    };
    segment(0, 1, 0, 0x400000, 4096);
    segment(1, 2, 512, 0x400200, tags.len() as u64 * 16);
    if let Some(interpreter) = interpreter {
        segment(2, 3, 3072, 0x400c00, interpreter.len() as u64 + 1);
        bytes[3072..3072 + interpreter.len()].copy_from_slice(interpreter.as_bytes());
    }
    for (index, (tag, value)) in tags.into_iter().enumerate() {
        put64(&mut bytes, 512 + index * 16, tag);
        put64(&mut bytes, 520 + index * 16, value);
    }
    bytes[2048..2048 + strings.len()].copy_from_slice(&strings);
    bytes
}

pub struct Directory(pub PathBuf);

impl Directory {
    pub fn new() -> io::Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "hyper-ldd-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path)?;
        Ok(Self(path))
    }

    pub fn add(&self, name: &str, needed: &[&str]) -> io::Result<PathBuf> {
        let path = self.0.join(name);
        fs::write(&path, elf(needed, Some(name), None, 183))?;
        Ok(path)
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
