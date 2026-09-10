// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::*;

struct Directory(std::path::PathBuf);
impl Directory {
    fn new() -> std::io::Result<Self> {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("hyper-touch-test-{}-{id}", std::process::id()));
        std::fs::create_dir(&path)?;
        Ok(Self(path))
    }
    fn path(&self, name: &str) -> std::path::PathBuf {
        self.0.join(name)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn creates_and_updates_without_truncating_including_directories() -> io::Result<()> {
    let d = Directory::new()?;
    let time = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(123456);
    let times = FileTimes::new().set_accessed(time).set_modified(time);
    touch(&d.path("missing"), true, times)?;
    assert!(!d.path("missing").exists());
    touch(&d.path("file"), false, times)?;
    fs::write(d.path("file"), b"keep")?;
    touch(&d.path("file"), false, times)?;
    assert_eq!(fs::read(d.path("file"))?, b"keep");
    assert_eq!(fs::metadata(d.path("file"))?.modified()?, time);
    touch(&d.0, false, times)?;
    assert_eq!(fs::metadata(&d.0)?.modified()?, time);
    Ok(())
}
#[test]
fn reference_and_selective_timestamps() -> Result<(), Box<dyn std::error::Error>> {
    let d = Directory::new()?;
    let a = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(123456);
    let b = a + std::time::Duration::from_secs(100);
    touch(
        &d.path("ref"),
        false,
        FileTimes::new().set_accessed(a).set_modified(b),
    )?;
    touch(
        &d.path("file"),
        false,
        FileTimes::new().set_accessed(b).set_modified(a),
    )?;
    assert!(run(Args {
        access: true,
        modification: false,
        no_create: false,
        reference: Some(d.path("ref")),
        paths: vec![d.path("file")]
    }));
    let m = fs::metadata(d.path("file"))?;
    assert_eq!(m.accessed()?, a);
    assert_eq!(m.modified()?, a);
    assert!(Args::try_parse_from(["touch"]).is_err());
    Ok(())
}
