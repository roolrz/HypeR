// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn parents_stop_at_nonempty_ancestor_and_report_completed_removals() -> io::Result<()> {
    let d = Directory::new()?;
    fs::write(d.path("keep"), b"keep")?;
    fs::create_dir_all(d.path("a/b"))?;
    let mut output = Vec::new();
    assert!(remove(&d.path("a/b"), true, true, &mut output).is_err());
    assert!(!d.path("a").exists());
    assert_eq!(fs::read(d.path("keep"))?, b"keep");
    assert_eq!(output.iter().filter(|byte| **byte == b'\n').count(), 2);
    assert!(remove(&d.path("a/.."), true, false, &mut output).is_err());
    assert!(remove(Path::new("/"), false, false, &mut output).is_err());
    Ok(())
}

struct Directory(std::path::PathBuf);
impl Directory {
    fn new() -> std::io::Result<Self> {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("hyper-rmdir-test-{}-{id}", std::process::id()));
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
fn removes_only_empty_directories_and_continues() -> std::io::Result<()> {
    let d = Directory::new()?;
    fs::create_dir(d.path("full"))?;
    fs::write(d.path("full/file"), b"keep")?;
    fs::create_dir(d.path("empty"))?;
    assert!(!run(Args {
        paths: vec![d.path("full"), d.path("empty")],
        ..Args::default()
    }));
    assert_eq!(fs::read(d.path("full/file"))?, b"keep");
    assert!(!d.path("empty").exists());
    std::os::unix::fs::symlink("full", d.path("link"))?;
    assert!(!run(Args {
        paths: vec![d.path("link")],
        ..Args::default()
    }));
    assert!(Args::try_parse_from(["rmdir"]).is_err());
    Ok(())
}
