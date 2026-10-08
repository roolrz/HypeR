// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn directory_option_removes_only_empty_directories() -> io::Result<()> {
    let d = Directory::new()?;
    fs::create_dir(d.path("empty"))?;
    fs::create_dir(d.path("full"))?;
    fs::write(d.path("full/file"), b"keep")?;
    remove(&d.path("empty"), false, true)?;
    assert!(remove(&d.path("full"), false, true).is_err());
    assert_eq!(fs::read(d.path("full/file"))?, b"keep");
    assert!(remove(Path::new("/"), false, true).is_err());
    Ok(())
}

struct Directory(std::path::PathBuf);
impl Directory {
    fn new() -> std::io::Result<Self> {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("hyper-rm-test-{}-{id}", std::process::id()));
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
fn recursive_removal_does_not_follow_links() -> io::Result<()> {
    let d = Directory::new()?;
    fs::create_dir(d.path("outside"))?;
    fs::write(d.path("outside/keep"), b"keep")?;
    fs::create_dir(d.path("tree"))?;
    std::os::unix::fs::symlink("../outside", d.path("tree/link"))?;
    assert!(remove(&d.path("tree"), false, false).is_err());
    remove(&d.path("tree"), true, false)?;
    assert_eq!(fs::read(d.path("outside/keep"))?, b"keep");
    std::os::unix::fs::symlink("outside", d.path("link"))?;
    remove(&d.path("link/"), true, false)?;
    assert_eq!(fs::read(d.path("outside/keep"))?, b"keep");
    Ok(())
}
#[test]
fn force_ignores_only_missing_paths_and_protects_root() -> io::Result<()> {
    let d = Directory::new()?;
    assert!(run(Args {
        recursive: false,
        force: true,
        paths: vec![d.path("missing")],
        ..Args::default()
    }));
    assert!(!run(Args {
        recursive: false,
        force: true,
        paths: vec![d.0.clone()],
        ..Args::default()
    }));
    for path in ["/", "//", ".", "..", "/tmp/.", "/tmp/../"] {
        assert!(remove(Path::new(path), true, false).is_err());
    }
    assert!(Args::try_parse_from(["rm"]).is_err());
    assert!(Args::try_parse_from(["rm", "-f"]).is_ok());
    Ok(())
}
