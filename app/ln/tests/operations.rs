// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::*;

struct Directory(std::path::PathBuf);
impl Directory {
    fn new() -> std::io::Result<Self> {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("hyper-ln-test-{}-{id}", std::process::id()));
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
fn hard_and_relative_dangling_links() -> io::Result<()> {
    let d = Directory::new()?;
    fs::write(d.path("file"), b"data")?;
    link(&d.path("file"), &d.path("hard"), false)?;
    fs::write(d.path("hard"), b"updated")?;
    assert_eq!(fs::read(d.path("file"))?, b"updated");
    link(Path::new("file"), &d.path("soft"), true)?;
    assert_eq!(fs::read(d.path("soft"))?, b"updated");
    link(Path::new("missing"), &d.path("dangling"), true)?;
    assert_eq!(fs::read_link(d.path("dangling"))?, Path::new("missing"));
    assert!(link(&d.path("file"), &d.path("soft"), false).is_err());
    assert_eq!(fs::read_link(d.path("soft"))?, Path::new("file"));
    Ok(())
}
#[test]
fn directory_destination() -> io::Result<()> {
    let d = Directory::new()?;
    fs::write(d.path("file"), b"data")?;
    fs::create_dir(d.path("out"))?;
    assert!(run(Args {
        symbolic: false,
        no_target_directory: false,
        target: d.path("file"),
        link: d.path("out")
    }));
    assert_eq!(fs::read(d.path("out/file"))?, b"data");
    Ok(())
}
