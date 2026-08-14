ARCH := aarch64
TARGET := $(ARCH)-unknown-none
PROFILE ?= debug
QEMU ?= qemu-system-aarch64
QEMU_CPU ?= cortex-a72
QEMU_CPUS ?= 4
QEMU_MEMORY ?= 512M
HOST_TARGET ?= $(shell rustc -vV | sed -n 's/^host: //p')
RUST_HOST := $(shell rustc -vV | sed -n 's/^host: //p')
LLVM_BIN := $(shell rustc --print sysroot)/lib/rustlib/$(RUST_HOST)/bin
OBJCOPY ?= $(LLVM_BIN)/llvm-objcopy
READOBJ ?= $(LLVM_BIN)/llvm-readobj
NM ?= $(LLVM_BIN)/llvm-nm
OBJDUMP ?= $(LLVM_BIN)/llvm-objdump

ifeq ($(PROFILE),release)
CARGO_PROFILE := --release
endif

KERNEL_ELF := target/$(TARGET)/$(PROFILE)/hyper
KERNEL_IMAGE := target/$(TARGET)/$(PROFILE)/hyper.img
KCONFIG_MANIFEST := tools/kconfig/Cargo.toml
HOST_TEST_MANIFEST := tests/host/Cargo.toml
DEFCONFIG := configs/qemu_aarch64_defconfig

.PHONY: all config defconfig olddefconfig build image check test test-image test-timer test-qemu verify verify-image verify-boot verify-smp run clean

all: image

.config: Kconfig $(DEFCONFIG) $(KCONFIG_MANIFEST) tools/kconfig/src/lib.rs tools/kconfig/src/main.rs
	cargo run --quiet --manifest-path $(KCONFIG_MANIFEST) --target $(HOST_TARGET) -- defconfig $(DEFCONFIG) .config

defconfig:
	cargo run --quiet --manifest-path $(KCONFIG_MANIFEST) --target $(HOST_TARGET) -- defconfig $(DEFCONFIG) .config

olddefconfig:
	cargo run --quiet --manifest-path $(KCONFIG_MANIFEST) --target $(HOST_TARGET) -- olddefconfig .config .config

config:
	cargo run --quiet --manifest-path $(KCONFIG_MANIFEST) --target $(HOST_TARGET) -- config .config .config

build: .config
	cargo build $(CARGO_PROFILE)

image: build
	$(OBJCOPY) --output-target=binary $(KERNEL_ELF) $(KERNEL_IMAGE)

check: .config
	cargo check --lib --bins --target $(TARGET)
	cargo clippy --target $(TARGET) -- -D warnings
	cargo clippy --manifest-path $(HOST_TEST_MANIFEST) --target $(HOST_TARGET) -- -D warnings
	cargo clippy --manifest-path $(KCONFIG_MANIFEST) --target $(HOST_TARGET) -- -D warnings

test: .config
	cargo test --manifest-path $(HOST_TEST_MANIFEST) --target $(HOST_TARGET)
	cargo test --manifest-path $(KCONFIG_MANIFEST) --target $(HOST_TARGET)

verify: check test
	cargo fmt -- --check
	cargo fmt --manifest-path $(HOST_TEST_MANIFEST) -- --check
	cargo fmt --manifest-path $(KCONFIG_MANIFEST) -- --check
	$(MAKE) image PROFILE=debug
	$(MAKE) image PROFILE=release
	$(MAKE) test-image PROFILE=debug
	$(MAKE) test-image PROFILE=release
	$(MAKE) test-qemu PROFILE=debug QEMU_CPU=cortex-a72
	$(MAKE) test-qemu PROFILE=debug QEMU_CPU=max

test-image:
	sh tests/image/verify-image.sh $(READOBJ) $(NM) $(OBJDUMP) $(KERNEL_ELF) $(KERNEL_IMAGE)

test-timer:
	sh tests/qemu/verify-timer.sh $(QEMU) $(KERNEL_IMAGE) $(QEMU_CPU) $(QEMU_MEMORY)

test-qemu:
	sh tests/qemu/verify-smp.sh $(QEMU) $(KERNEL_IMAGE) $(QEMU_CPU) $(QEMU_MEMORY)

# Compatibility targets build the image before running the corresponding test.
verify-image: image
	$(MAKE) test-image PROFILE=$(PROFILE)

verify-boot: image
	$(MAKE) test-timer PROFILE=$(PROFILE) QEMU_CPU=$(QEMU_CPU)

verify-smp: image
	$(MAKE) test-qemu PROFILE=$(PROFILE) QEMU_CPU=$(QEMU_CPU)

run: image
	$(QEMU) \
		-machine virt,virtualization=on,gic-version=3 \
		-cpu $(QEMU_CPU) \
		-smp $(QEMU_CPUS) \
		-m $(QEMU_MEMORY) \
		-nographic \
		-no-reboot \
		-kernel $(KERNEL_IMAGE)

clean:
	cargo clean
