ARCH := aarch64
TARGET := $(ARCH)-unknown-none
PROFILE ?= debug
QEMU ?= qemu-system-aarch64
QEMU_CPU ?= cortex-a72
QEMU_CPUS ?= 4
QEMU_MEMORY ?= 512M
QEMU_BOOTARGS ?= earlycon=pl011,mmio32,0x09000000
HOST_TARGET ?= $(shell rustc -vV | sed -n 's/^host: //p')
RUST_HOST := $(shell rustc -vV | sed -n 's/^host: //p')
LLVM_BIN := $(shell rustc --print sysroot)/lib/rustlib/$(RUST_HOST)/bin
OBJCOPY ?= $(LLVM_BIN)/llvm-objcopy
READOBJ ?= $(LLVM_BIN)/llvm-readobj
NM ?= $(LLVM_BIN)/llvm-nm
OBJDUMP ?= $(LLVM_BIN)/llvm-objdump

ifeq ($(PROFILE),release)
CARGO_PROFILE := --release
KALLSYMS_STORAGE_SIZE := 131072
else
KALLSYMS_STORAGE_SIZE := 786432
endif

CARGO_FEATURES ?=

KERNEL_ELF := target/$(TARGET)/$(PROFILE)/hyper
KERNEL_IMAGE := target/$(TARGET)/$(PROFILE)/hyper.img
KCONFIG_MANIFEST := tools/kconfig/Cargo.toml
KALLSYMS_MANIFEST := tools/kallsyms/Cargo.toml
KALLSYMS_BLOB := target/$(TARGET)/$(PROFILE)/hyper.kallsyms
KALLSYMS_ELF := target/$(TARGET)/$(PROFILE)/hyper.with-kallsyms
HOST_TEST_MANIFEST := tests/host/Cargo.toml
DEFCONFIG := configs/qemu_aarch64_defconfig
GUEST_OUTPUT := target/guest
GUEST_ASSET_STAMP := $(GUEST_OUTPUT)/.alpine-3.23.5.stamp
HOST_INITRD := $(GUEST_OUTPUT)/hypervisor-initrd.cpio

.PHONY: all config defconfig olddefconfig guest-assets clean-guest-assets build image check test test-image test-timer test-qemu verify verify-image verify-boot verify-smp run clean

all: image

.config: Kconfig $(DEFCONFIG) $(KCONFIG_MANIFEST) tools/kconfig/src/lib.rs tools/kconfig/src/main.rs
	cargo run --quiet --manifest-path $(KCONFIG_MANIFEST) --target $(HOST_TARGET) -- defconfig $(DEFCONFIG) .config

defconfig:
	cargo run --quiet --manifest-path $(KCONFIG_MANIFEST) --target $(HOST_TARGET) -- defconfig $(DEFCONFIG) .config

olddefconfig:
	cargo run --quiet --manifest-path $(KCONFIG_MANIFEST) --target $(HOST_TARGET) -- olddefconfig .config .config

config:
	cargo run --quiet --manifest-path $(KCONFIG_MANIFEST) --target $(HOST_TARGET) -- config .config .config

$(GUEST_ASSET_STAMP): tools/guest/fetch-alpine.sh tools/guest/init tools/guest/boot.conf tools/guest/alpine.manifest
	sh tools/guest/fetch-alpine.sh

guest-assets: $(GUEST_ASSET_STAMP)

clean-guest-assets:
	rm -f $(GUEST_OUTPUT)/Image $(GUEST_OUTPUT)/initramfs.cpio.gz $(GUEST_OUTPUT)/alpine.cpio $(HOST_INITRD) $(GUEST_ASSET_STAMP)

build: .config
	cargo build $(CARGO_PROFILE) $(CARGO_FEATURES)

image: build
	cargo run --quiet --manifest-path $(KALLSYMS_MANIFEST) --target $(HOST_TARGET) -- $(NM) $(KERNEL_ELF) $(KALLSYMS_BLOB) $(KALLSYMS_STORAGE_SIZE)
	$(OBJCOPY) --update-section=.kallsyms=$(KALLSYMS_BLOB) $(KERNEL_ELF) $(KALLSYMS_ELF)
	mv $(KALLSYMS_ELF) $(KERNEL_ELF)
	$(OBJCOPY) --output-target=binary $(KERNEL_ELF) $(KERNEL_IMAGE)

check: .config
	cargo check --lib --bins --target $(TARGET)
	cargo clippy --target $(TARGET) -- -D warnings
	cargo clippy --target $(TARGET) --features kernel-self-test -- -D warnings
	cargo clippy --manifest-path $(HOST_TEST_MANIFEST) --target $(HOST_TARGET) -- -D warnings
	cargo clippy --manifest-path $(KCONFIG_MANIFEST) --target $(HOST_TARGET) -- -D warnings
	cargo clippy --manifest-path $(KALLSYMS_MANIFEST) --target $(HOST_TARGET) -- -D warnings

test: .config
	cargo test --manifest-path $(HOST_TEST_MANIFEST) --target $(HOST_TARGET)
	cargo test --manifest-path $(KCONFIG_MANIFEST) --target $(HOST_TARGET)
	cargo test --manifest-path $(KALLSYMS_MANIFEST) --target $(HOST_TARGET)

verify: check test
	cargo fmt -- --check
	cargo fmt --manifest-path $(HOST_TEST_MANIFEST) -- --check
	cargo fmt --manifest-path $(KCONFIG_MANIFEST) -- --check
	cargo fmt --manifest-path $(KALLSYMS_MANIFEST) -- --check
	$(MAKE) image PROFILE=debug
	$(MAKE) image PROFILE=release
	$(MAKE) test-image PROFILE=debug
	$(MAKE) test-image PROFILE=release
	$(MAKE) test-qemu PROFILE=debug QEMU_CPU=cortex-a72
	$(MAKE) test-qemu PROFILE=debug QEMU_CPU=max

test-image:
	sh tests/image/verify-image.sh $(READOBJ) $(NM) $(OBJDUMP) $(KERNEL_ELF) $(KERNEL_IMAGE)

test-timer: image guest-assets
	sh tests/qemu/verify-timer.sh $(QEMU) $(KERNEL_IMAGE) $(HOST_INITRD) $(QEMU_CPU) $(QEMU_MEMORY) "$(QEMU_BOOTARGS)"

test-timer: CARGO_FEATURES=--features kernel-self-test

test-qemu: image guest-assets
	sh tests/qemu/verify-smp.sh $(QEMU) $(KERNEL_IMAGE) $(HOST_INITRD) $(QEMU_CPU) $(QEMU_MEMORY) "$(QEMU_BOOTARGS)"

test-qemu: CARGO_FEATURES=--features kernel-self-test

# Compatibility targets build the image before running the corresponding test.
verify-image: image
	$(MAKE) test-image PROFILE=$(PROFILE)

verify-boot: image
	$(MAKE) test-timer PROFILE=$(PROFILE) QEMU_CPU=$(QEMU_CPU)

verify-smp: image
	$(MAKE) test-qemu PROFILE=$(PROFILE) QEMU_CPU=$(QEMU_CPU)

run: image guest-assets
	$(QEMU) \
		-machine virt,virtualization=on,gic-version=3,dtb-randomness=on \
		-cpu $(QEMU_CPU) \
		-smp $(QEMU_CPUS) \
		-m $(QEMU_MEMORY) \
		-nodefaults \
		-display none \
		-serial stdio \
		-monitor none \
		-no-reboot \
		-append "$(QEMU_BOOTARGS)" \
		-initrd $(HOST_INITRD) \
		-kernel $(KERNEL_IMAGE)

clean:
	cargo clean
