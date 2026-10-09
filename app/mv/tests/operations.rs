// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn exact_destination_renames_instead_of_nesting() -> io::Result<()> {
    let d = Directory::new()?;
    fs::create_dir(d.path("source"))?;
    fs::create_dir(d.path("destination"))?;
    fs::write(d.path("source/file"), b"data")?;
    assert!(run(Args {
        no_target_directory: true,
        paths: vec![d.path("source"), d.path("destination")],
        ..Args::default()
    }));
    assert_eq!(fs::read(d.path("destination/file"))?, b"data");
    assert!(!d.path("destination/source").exists());
    Ok(())
}

struct Directory(std::path::PathBuf);
impl Directory {
    fn new() -> std::io::Result<Self> {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("hyper-mv-test-{}-{id}", std::process::id()));
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
fn rename_preserves_links_and_replaces_files() -> io::Result<()> {
    let d = Directory::new()?;
    fs::write(d.path("file"), b"data")?;
    fs::write(d.path("old"), b"old")?;
    move_one(&d.path("file"), &d.path("old"))?;
    assert!(!d.path("file").exists());
    assert_eq!(fs::read(d.path("old"))?, b"data");
    std::os::unix::fs::symlink("absent", d.path("link"))?;
    move_one(&d.path("link"), &d.path("moved-link"))?;
    assert_eq!(fs::read_link(d.path("moved-link"))?, Path::new("absent"));
    fs::create_dir(d.path("dir"))?;
    assert!(move_one(&d.path("dir"), &d.path("dir/sub")).is_err());
    Ok(())
}
#[test]
fn multiple_sources_and_errors() -> io::Result<()> {
    let d = Directory::new()?;
    fs::create_dir(d.path("out"))?;
    fs::write(d.path("file"), b"data")?;
    assert!(!run(Args {
        paths: vec![d.path("missing"), d.path("file"), d.path("out")],
        ..Args::default()
    }));
    assert_eq!(fs::read(d.path("out/file"))?, b"data");
    assert!(Args::try_parse_from(["mv", "one"]).is_err());
    Ok(())
}
