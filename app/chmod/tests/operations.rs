// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::*;

struct Directory(std::path::PathBuf);
impl Directory {
    fn new() -> std::io::Result<Self> {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("hyper-chmod-test-{}-{id}", std::process::id()));
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
fn octal_symbolic_conditional_and_copy_modes() -> Result<(), Box<dyn std::error::Error>> {
    for (text, old, directory, wanted) in [
        ("0750", 0o777, false, 0o750),
        ("u+rw,go-rwx", 0o444, false, 0o600),
        ("a+X", 0o644, false, 0o644),
        ("a+X", 0o644, true, 0o755),
        ("a+X", 0o744, false, 0o755),
        ("g=u,o=", 0o640, false, 0o660),
        ("u+s,g+s,o+t", 0o755, false, 0o7755),
        ("a=rw", 0o7777, false, 0o666),
    ] {
        assert_eq!(
            text.parse::<Mode>()?.apply(old, directory),
            wanted,
            "{text}"
        );
    }
    for text in ["", "888", "10000", "u", "u+q", "u+rw,", "g=ur"] {
        assert!(text.parse::<Mode>().is_err(), "{text}");
    }
    Ok(())
}
#[test]
fn recursion_skips_external_symlink_targets() -> Result<(), Box<dyn std::error::Error>> {
    let d = Directory::new()?;
    fs::write(d.path("outside"), b"keep")?;
    fs::set_permissions(d.path("outside"), fs::Permissions::from_mode(0o640))?;
    fs::create_dir(d.path("tree"))?;
    fs::write(d.path("tree/file"), b"data")?;
    std::os::unix::fs::symlink("../outside", d.path("tree/link"))?;
    change(&d.path("tree"), &"u=rwX,go=".parse()?, true)?;
    assert_eq!(
        fs::metadata(d.path("tree"))?.permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(d.path("tree/file"))?.permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        fs::metadata(d.path("outside"))?.permissions().mode() & 0o777,
        0o640
    );
    Ok(())
}
