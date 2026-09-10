// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::*;

struct Directory(std::path::PathBuf);
impl Directory {
    fn new() -> std::io::Result<Self> {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("hyper-mkdir-test-{}-{id}", std::process::id()));
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
fn parents_modes_and_existing_paths() -> io::Result<()> {
    let d = Directory::new()?;
    assert!(create(&d.path("a/b"), false, None).is_err());
    create(&d.path("a/b"), true, Some(0o750))?;
    assert_eq!(
        fs::metadata(d.path("a/b"))?.permissions().mode() & 0o7777,
        0o750
    );
    create(&d.path("a/b"), true, Some(0o700))?;
    assert_eq!(
        fs::metadata(d.path("a/b"))?.permissions().mode() & 0o7777,
        0o750
    );
    assert!(create(&d.path("a/b"), false, None).is_err());
    fs::write(d.path("file"), b"data")?;
    assert!(create(&d.path("file/child"), true, None).is_err());
    assert!(create(&d.path("file"), true, None).is_err());
    Ok(())
}
#[test]
fn validates_octal_mode_and_operands() {
    for invalid in ["", "888", "10000", "-1", "u+r"] {
        assert!(parse_mode(invalid).is_err());
    }
    assert_eq!(parse_mode("0750"), Ok(0o750));
    assert!(Args::try_parse_from(["mkdir"]).is_err());
}
