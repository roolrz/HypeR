use std::env;
use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

#[path = "src/arch/aarch64/registers.rs"]
mod aarch64_registers;
// The build script uses only validation and export; the host configurator uses
// the interactive and serialization portions of the shared implementation.
#[allow(dead_code)]
#[path = "tools/kconfig/src/lib.rs"]
mod kconfig;

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/arch/aarch64/boot.S");
    println!("cargo:rerun-if-changed=src/arch/aarch64/vectors.S");
    println!("cargo:rerun-if-changed=src/arch/aarch64/context.S");
    println!("cargo:rerun-if-changed=src/arch/aarch64/registers.rs");
    println!("cargo:rerun-if-changed=Kconfig");
    println!("cargo:rerun-if-changed=.config");

    let output_directory = PathBuf::from(env::var("OUT_DIR")?);
    export_kernel_configuration(&output_directory)?;

    let target = env::var("TARGET")?;
    if target != "aarch64-unknown-none" {
        return Ok(());
    }

    let header_path = output_directory.join("asm_constants.h");
    write_assembly_header(&header_path)?;
    for (source, object_name) in [
        ("src/arch/aarch64/boot.S", "aarch64_boot.o"),
        ("src/arch/aarch64/vectors.S", "aarch64_vectors.o"),
        ("src/arch/aarch64/context.S", "aarch64_context.o"),
    ] {
        let object_path = output_directory.join(object_name);
        compile_assembly(source, &output_directory, &object_path)?;
        println!("cargo:rustc-link-arg-bin=hyper={}", object_path.display());
    }
    Ok(())
}

fn export_kernel_configuration(output_directory: &Path) -> Result<(), Box<dyn Error>> {
    let (schema, configuration) =
        kconfig::load_and_validate(Path::new("Kconfig"), Path::new(".config")).map_err(
            |error| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("invalid kernel configuration: {error}; run `make defconfig`"),
                )
            },
        )?;
    let mut rust_source = String::from("// Generated from .config. Do not edit.\n\n");
    for symbol in &schema.symbols {
        let rust_name = format!("CONFIG_{}", symbol.name);
        let value = configuration.value(&symbol.name).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{rust_name} is missing from the resolved configuration"),
            )
        })?;
        println!("cargo:rustc-env={rust_name}={value}");
        match symbol.kind {
            kconfig::ValueKind::Bool => {
                println!("cargo:rustc-check-cfg=cfg({rust_name})");
                if value == "y" {
                    println!("cargo:rustc-cfg={rust_name}");
                }
                rust_source.push_str(&format!(
                    "pub const {}: bool = {};\n",
                    symbol.name,
                    value == "y"
                ));
            }
            kconfig::ValueKind::Int => {
                println!("cargo:rustc-check-cfg=cfg({rust_name}, values({value:?}))");
                println!("cargo:rustc-cfg={rust_name}={value:?}");
                let integer = value.parse::<i64>().map_err(|error| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("{rust_name} is not a valid integer: {error}"),
                    )
                })?;
                rust_source.push_str(&format!("pub const {}: i64 = {integer};\n", symbol.name));
            }
            kconfig::ValueKind::String => {
                println!("cargo:rustc-check-cfg=cfg({rust_name}, values({value:?}))");
                println!("cargo:rustc-cfg={rust_name}={value:?}");
                rust_source.push_str(&format!("pub const {}: &str = {value:?};\n", symbol.name));
            }
        }
    }
    fs::write(output_directory.join("kernel_config.rs"), rust_source)?;
    Ok(())
}

fn write_assembly_header(path: &Path) -> io::Result<()> {
    let mut header = String::from(
        "/* Generated from src/arch/aarch64/registers.rs. Do not edit. */\n\
         #ifndef HYPER_AARCH64_ASM_CONSTANTS_H\n\
         #define HYPER_AARCH64_ASM_CONSTANTS_H\n\n",
    );

    for &(name, value) in aarch64_registers::ASM_CONSTANTS {
        header.push_str(&format!("#define {name} 0x{value:016x}\n"));
    }
    header.push_str("\n#endif /* HYPER_AARCH64_ASM_CONSTANTS_H */\n");

    fs::write(path, header)
}

fn compile_assembly(source: &str, include_directory: &Path, object_path: &Path) -> io::Result<()> {
    let compiler = match env::var("CLANG") {
        Ok(compiler) => compiler,
        Err(env::VarError::NotPresent) => String::from("clang"),
        Err(error) => return Err(io::Error::new(io::ErrorKind::InvalidInput, error)),
    };
    let status = Command::new(&compiler)
        .args([
            "--target=aarch64-none-elf",
            "-ffreestanding",
            "-x",
            "assembler-with-cpp",
            "-c",
            source,
            "-o",
        ])
        .arg(object_path)
        .arg("-I")
        .arg(include_directory)
        .status()?;

    if !status.success() {
        return Err(io::Error::other(format!(
            "{compiler} failed to compile {source}"
        )));
    }
    Ok(())
}
