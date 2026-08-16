use std::env;
use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

#[path = "src/arch/aarch64/registers.rs"]
mod aarch64_registers;
#[allow(dead_code)]
#[path = "src/arch/riscv64/registers.rs"]
mod riscv64_registers;
// The build script uses only validation and export; the host configurator uses
// the interactive and serialization portions of the shared implementation.
#[allow(dead_code)]
#[path = "tools/kconfig/src/lib.rs"]
mod kconfig;

type AssemblySource<'a> = (&'a str, &'a str);
type ArchitectureBuild<'a> = (&'a str, &'a [(&'a str, u64)], &'a [AssemblySource<'a>]);

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/arch/aarch64/boot.S");
    println!("cargo:rerun-if-changed=src/arch/aarch64/vectors.S");
    println!("cargo:rerun-if-changed=src/arch/aarch64/context.S");
    println!("cargo:rerun-if-changed=src/arch/aarch64/registers.rs");
    println!("cargo:rerun-if-changed=src/arch/aarch64/linker.ld");
    println!("cargo:rerun-if-changed=src/arch/riscv64/boot.S");
    println!("cargo:rerun-if-changed=src/arch/riscv64/trap.S");
    println!("cargo:rerun-if-changed=src/arch/riscv64/context.S");
    println!("cargo:rerun-if-changed=src/arch/riscv64/guest.S");
    println!("cargo:rerun-if-changed=src/arch/riscv64/registers.rs");
    println!("cargo:rerun-if-changed=src/arch/riscv64/linker.ld");
    println!("cargo:rerun-if-changed=Kconfig");
    println!("cargo:rerun-if-env-changed=HYPER_CONFIG");
    println!("cargo:rerun-if-env-changed=HYPER_KALLSYMS_BLOB");
    println!("cargo:rustc-check-cfg=cfg(hyper_embed_kallsyms)");

    let output_directory = PathBuf::from(env::var("OUT_DIR")?);
    let target = env::var("TARGET")?;
    let configuration_path = kernel_configuration_path()?;
    println!("cargo:rerun-if-changed={}", configuration_path.display());
    export_kernel_configuration(&output_directory, &target, &configuration_path)?;
    configure_kallsyms_embedding()?;

    let header_path = output_directory.join("asm_constants.h");
    let (clang_target, constants, sources): ArchitectureBuild<'_> = match target.as_str() {
        "aarch64-unknown-none" => (
            "aarch64-none-elf",
            aarch64_registers::ASM_CONSTANTS,
            &[
                ("src/arch/aarch64/boot.S", "aarch64_boot.o"),
                ("src/arch/aarch64/vectors.S", "aarch64_vectors.o"),
                ("src/arch/aarch64/context.S", "aarch64_context.o"),
            ],
        ),
        "riscv64imac-unknown-none-elf" => (
            "riscv64-none-elf",
            riscv64_registers::ASM_CONSTANTS,
            &[
                ("src/arch/riscv64/boot.S", "riscv64_boot.o"),
                ("src/arch/riscv64/trap.S", "riscv64_trap.o"),
                ("src/arch/riscv64/context.S", "riscv64_context.o"),
                ("src/arch/riscv64/guest.S", "riscv64_guest.o"),
            ],
        ),
        _ => return Ok(()),
    };
    write_assembly_header(&header_path, constants)?;
    for &(source, object_name) in sources {
        let object_path = output_directory.join(object_name);
        compile_assembly(
            clang_target,
            (target == "riscv64imac-unknown-none-elf").then_some("rv64imafdc_h_zicsr_zifencei"),
            source,
            &output_directory,
            &object_path,
        )?;
        println!("cargo:rustc-link-arg-bin=hyper={}", object_path.display());
    }
    Ok(())
}

fn kernel_configuration_path() -> Result<PathBuf, Box<dyn Error>> {
    match env::var_os("HYPER_CONFIG") {
        Some(path) if !path.is_empty() => Ok(PathBuf::from(path)),
        Some(_) | None => Ok(PathBuf::from(".config")),
    }
}

fn configure_kallsyms_embedding() -> Result<(), Box<dyn Error>> {
    let Some(path) = env::var_os("HYPER_KALLSYMS_BLOB").filter(|path| !path.is_empty()) else {
        return Ok(());
    };
    let path = PathBuf::from(path).canonicalize()?;
    if !path.is_file() {
        return Err(format!("kallsyms blob is not a regular file: {}", path.display()).into());
    }
    println!("cargo:rerun-if-changed={}", path.display());
    println!("cargo:rustc-cfg=hyper_embed_kallsyms");
    println!("cargo:rustc-env=HYPER_KALLSYMS_BLOB={}", path.display());
    Ok(())
}

fn export_kernel_configuration(
    output_directory: &Path,
    target: &str,
    configuration_path: &Path,
) -> Result<(), Box<dyn Error>> {
    let (schema, configuration) =
        kconfig::load_and_validate(Path::new("Kconfig"), configuration_path).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "invalid kernel configuration {}: {error}; run `make defconfig`",
                    configuration_path.display()
                ),
            )
        })?;
    validate_architecture_configuration(&configuration, target)?;
    let mut rust_source = String::from("// Generated kernel configuration. Do not edit.\n\n");
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

fn validate_architecture_configuration(
    configuration: &kconfig::Configuration,
    target: &str,
) -> Result<(), Box<dyn Error>> {
    let aarch64 = configuration.value("ARCH_AARCH64") == Some("y");
    let riscv64 = configuration.value("ARCH_RISCV64") == Some("y");
    if aarch64 == riscv64 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "exactly one of CONFIG_ARCH_AARCH64 and CONFIG_ARCH_RISCV64 must be enabled",
        )
        .into());
    }
    let matches_target = match target {
        "aarch64-unknown-none" => aarch64,
        "riscv64imac-unknown-none-elf" => riscv64,
        _ => true,
    };
    if !matches_target {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("kernel architecture configuration does not match target {target}"),
        )
        .into());
    }
    Ok(())
}

fn write_assembly_header(path: &Path, constants: &[(&str, u64)]) -> io::Result<()> {
    let mut header = String::from(
        "/* Generated from the selected architecture register definitions. Do not edit. */\n\
         #ifndef HYPER_ASM_CONSTANTS_H\n\
         #define HYPER_ASM_CONSTANTS_H\n\n",
    );

    for &(name, value) in constants {
        header.push_str(&format!("#define {name} 0x{value:016x}\n"));
    }
    header.push_str("\n#endif /* HYPER_ASM_CONSTANTS_H */\n");

    fs::write(path, header)
}

fn compile_assembly(
    target: &str,
    march: Option<&str>,
    source: &str,
    include_directory: &Path,
    object_path: &Path,
) -> io::Result<()> {
    let compiler = match env::var("CLANG") {
        Ok(compiler) => compiler,
        Err(env::VarError::NotPresent) => String::from("clang"),
        Err(error) => return Err(io::Error::new(io::ErrorKind::InvalidInput, error)),
    };
    let mut command = Command::new(&compiler);
    command.args([
        &format!("--target={target}"),
        "-ffreestanding",
        "-x",
        "assembler-with-cpp",
        "-c",
        source,
    ]);
    if let Some(march) = march {
        command.arg(format!("-march={march}"));
        command.arg("-mabi=lp64");
    }
    let status = command
        .arg("-o")
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
