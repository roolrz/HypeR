// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::cli::Ldd;
use crate::fixtures::{Directory, elf};
use crate::output;
use clap::Parser;

#[test]
fn shared_transitive_dependencies_are_listed_once_and_cycles_terminate() -> io::Result<()> {
    let files = Directory::new()?;
    let root = files.add("libroot.so", &["liba.so", "libb.so"])?;
    files.add("liba.so", &["libshared.so"])?;
    files.add("libb.so", &["libshared.so"])?;
    files.add("libshared.so", &["liba.so"])?;
    let graph = inspect(&root, Some(&files.0), false)?;
    assert!(!graph.failed());
    assert_eq!(graph.objects.len(), 4);
    let mut text = Vec::new();
    output::write(&graph, false, false, &mut text)?;
    let text = String::from_utf8_lossy(&text);
    assert_eq!(text.matches("libshared.so =>").count(), 1);
    let mut tree = Vec::new();
    output::write(&graph, true, true, &mut tree)?;
    let tree = String::from_utf8_lossy(&tree);
    assert!(tree.contains("[cycle]"));
    assert!(tree.contains("[already shown]"));
    assert!(tree.contains("SONAME libshared.so"));
    Ok(())
}

#[test]
fn missing_library_reports_failure_but_does_not_hide_its_siblings() -> io::Result<()> {
    let files = Directory::new()?;
    let root = files.add("libroot.so", &["libmissing.so", "libpresent.so"])?;
    files.add("libpresent.so", &[])?;
    let graph = inspect(&root, Some(&files.0), false)?;
    assert!(graph.failed());
    assert_eq!(graph.objects.len(), 3);
    assert!(matches!(graph.objects[1].image, Err(Failure::NotFound)));
    assert!(graph.objects[2].image.is_ok());
    let mut text = Vec::new();
    output::write(&graph, false, false, &mut text)?;
    let text = String::from_utf8_lossy(&text);
    assert!(text.contains("libmissing.so => not found"));
    assert!(text.contains("libpresent.so =>"));
    Ok(())
}

#[test]
fn direct_mode_checks_immediate_dependencies_without_opening_grandchildren() -> io::Result<()> {
    let files = Directory::new()?;
    let root = files.add("libroot.so", &["liba.so"])?;
    files.add("liba.so", &["missing.so"])?;
    let direct = inspect(&root, Some(&files.0), true)?;
    assert!(!direct.failed());
    assert_eq!(direct.objects.len(), 2);
    assert!(inspect(&root, Some(&files.0), false)?.failed());
    Ok(())
}

#[test]
fn reports_incompatible_and_malformed_libraries() -> io::Result<()> {
    let files = Directory::new()?;
    let root = files.add("libroot.so", &["libwrong.so", "libabi.so", "libbroken.so"])?;
    fs::write(files.0.join("libwrong.so"), elf(&[], None, None, 243))?;
    let mut foreign = elf(&[], None, None, 183);
    foreign[7] = 0;
    fs::write(files.0.join("libabi.so"), foreign)?;
    fs::write(files.0.join("libbroken.so"), b"ordinary text")?;
    let graph = inspect(&root, Some(&files.0), false)?;
    assert!(graph.failed());
    let mut text = Vec::new();
    output::write(&graph, false, false, &mut text)?;
    let text = String::from_utf8_lossy(&text);
    assert!(text.contains("wrong architecture: riscv64, expected aarch64"));
    assert!(text.contains("OS ABI or ABI version differs"));
    assert!(text.contains("ELF range exceeds file size"));
    Ok(())
}

#[test]
fn interpreter_is_reported_once_and_missing_interpreter_is_an_error() -> io::Result<()> {
    let files = Directory::new()?;
    let loader = files.add("ld-hyper-aarch64.so", &[])?;
    let libraries = files.0.join("aarch64-hyper-hyper");
    fs::create_dir(&libraries)?;
    files.add("aarch64-hyper-hyper/libruntime.so", &[])?;
    let root = files.0.join("program");
    fs::write(
        &root,
        elf(
            &["ld-hyper-aarch64.so", "libruntime.so"],
            None,
            Some(&loader.to_string_lossy()),
            183,
        ),
    )?;
    let graph = inspect(&root, Some(&libraries), false)?;
    assert_eq!(graph.objects.len(), 3);
    assert!(graph.objects[1].interpreter);
    assert!(!graph.failed());
    fs::remove_file(loader)?;
    assert!(inspect(&root, Some(&libraries), false)?.failed());
    Ok(())
}

#[cfg(unix)]
#[test]
fn symlinks_resolve_to_real_files_and_dangling_links_are_reported() -> io::Result<()> {
    let files = Directory::new()?;
    let root = files.add("libroot.so", &["libalias.so", "libmissing.so"])?;
    let real = files.add("libactual.so.1", &[])?;
    std::os::unix::fs::symlink("libactual.so.1", files.0.join("libalias.so"))?;
    std::os::unix::fs::symlink("absent", files.0.join("libmissing.so"))?;
    let graph = inspect(&root, Some(&files.0), false)?;
    assert!(graph.failed());
    assert_eq!(graph.objects[1].path, fs::canonicalize(real)?);
    // SONAME does not rename a DT_NEEDED lookup in the Native loader.
    assert!(graph.objects[1].image.is_ok());
    assert!(matches!(graph.objects[2].image, Err(Failure::NotFound)));
    Ok(())
}

#[cfg(unix)]
#[test]
fn inspecting_a_symlinked_library_keeps_its_lookup_name_for_back_edges() -> io::Result<()> {
    let files = Directory::new()?;
    files.add("libroot.so.1", &["libleaf.so"])?;
    files.add("libleaf.so", &["libroot.so"])?;
    let alias = files.0.join("libroot.so");
    std::os::unix::fs::symlink("libroot.so.1", &alias)?;
    let graph = inspect(&alias, Some(&files.0), false)?;
    assert!(!graph.failed());
    assert_eq!(graph.objects.len(), 2);
    assert_eq!(graph.objects[1].dependencies, [0]);
    Ok(())
}

#[test]
fn excessive_graphs_stop_at_the_native_loader_limit() -> io::Result<()> {
    let files = Directory::new()?;
    for index in 0..16 {
        files.add(
            &format!("lib{index}.so"),
            &[&format!("lib{}.so", index + 1)],
        )?;
    }
    assert!(inspect(&files.0.join("lib0.so"), Some(&files.0), false).is_err());
    Ok(())
}

#[test]
fn cli_reports_multiple_operands_and_returns_failure_if_any_failed() -> io::Result<()> {
    let files = Directory::new()?;
    let static_file = files.0.join("static");
    fs::write(&static_file, elf(&[], None, None, 183))?;
    let args = Ldd::try_parse_from([
        "ldd",
        "/nonexistent-hyper-ldd-file",
        &static_file.to_string_lossy(),
    ])
    .map_err(io::Error::other)?;
    let mut output = Vec::new();
    let mut errors = Vec::new();
    assert!(crate::run(&args, &mut output, &mut errors)?);
    assert!(String::from_utf8_lossy(&output).contains("statically linked"));
    assert!(String::from_utf8_lossy(&errors).contains("nonexistent-hyper-ldd-file"));
    assert!(Ldd::try_parse_from(["ldd"]).is_err());
    assert!(Ldd::try_parse_from(["ldd", "--tree", "--direct", "file"]).is_err());
    Ok(())
}

#[test]
fn broken_output_is_propagated_and_metadata_cannot_inject_terminal_controls() -> io::Result<()> {
    struct Closed;
    impl io::Write for Closed {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::ErrorKind::BrokenPipe.into())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let args = Ldd::try_parse_from(["ldd", "file"]).map_err(io::Error::other)?;
    assert_eq!(
        crate::run(&args, &mut Closed, &mut Vec::new())
            .err()
            .map(|error| error.kind()),
        Some(io::ErrorKind::BrokenPipe)
    );
    assert_eq!(output::escaped("lib\x1b[2J\n.so"), "lib\\u{1b}[2J\\n.so");
    Ok(())
}
