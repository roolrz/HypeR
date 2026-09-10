// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::*;

struct Directory(std::path::PathBuf);
impl Directory {
    fn new() -> std::io::Result<Self> {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("hyper-cp-test-{}-{id}", std::process::id()));
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
fn copies_data_and_rejects_aliases_without_truncating() -> io::Result<()> {
    let d = Directory::new()?;
    fs::write(d.path("source"), b"keep me")?;
    fs::write(d.path("copy"), b"long old contents")?;
    copy(&d.path("source"), &d.path("copy"), false)?;
    assert_eq!(fs::read(d.path("copy"))?, b"keep me");
    fs::hard_link(d.path("source"), d.path("hard"))?;
    symlink("source", d.path("soft"))?;
    for name in ["source", "hard", "soft"] {
        assert!(copy(&d.path("source"), &d.path(name), false).is_err());
        assert_eq!(fs::read(d.path("source"))?, b"keep me");
    }
    symlink("absent", d.path("dangling"))?;
    assert!(copy(&d.path("source"), &d.path("dangling"), false).is_err());
    assert!(!d.path("absent").exists());
    Ok(())
}

#[test]
fn recursive_copy_preserves_links_and_rejects_descendants() -> io::Result<()> {
    let d = Directory::new()?;
    fs::create_dir_all(d.path("source/sub"))?;
    fs::write(d.path("source/sub/file"), b"data")?;
    symlink("sub/file", d.path("source/link"))?;
    symlink("missing", d.path("source/dangling"))?;
    assert!(copy(&d.path("source"), &d.path("plain"), false).is_err());
    assert!(copy(&d.path("source"), &d.path("source/new"), true).is_err());
    symlink("source/sub", d.path("alias"))?;
    assert!(copy(&d.path("source"), &d.path("alias/new"), true).is_err());
    copy(&d.path("source"), &d.path("copy"), true)?;
    assert_eq!(fs::read(d.path("copy/sub/file"))?, b"data");
    assert_eq!(fs::read_link(d.path("copy/link"))?, Path::new("sub/file"));
    assert_eq!(
        fs::read_link(d.path("copy/dangling"))?,
        Path::new("missing")
    );
    Ok(())
}

#[test]
fn cli_rejects_missing_destination_and_continues_after_errors()
-> Result<(), Box<dyn std::error::Error>> {
    assert!(Args::try_parse_from(["cp", "one"]).is_err());
    let d = Directory::new()?;
    fs::create_dir(d.path("out"))?;
    fs::write(d.path("file"), b"data")?;
    assert!(!run(Args {
        recursive: false,
        paths: vec![d.path("missing"), d.path("file"), d.path("out")]
    }));
    assert_eq!(fs::read(d.path("out/file"))?, b"data");
    Ok(())
}

#[test]
fn trailing_dot_copies_contents_into_existing_directory() -> io::Result<()> {
    let d = Directory::new()?;
    fs::create_dir(d.path("source"))?;
    fs::create_dir(d.path("out"))?;
    fs::write(d.path("source/file"), b"data")?;
    assert!(run(Args {
        recursive: true,
        paths: vec![d.path("source/."), d.path("out")]
    }));
    assert_eq!(fs::read(d.path("out/file"))?, b"data");
    assert!(!d.path("out/source").exists());
    Ok(())
}
