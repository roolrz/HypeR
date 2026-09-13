#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

"""Package and verify cross-VM virtio-scsi against a disposable QEMU disk."""

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import selectors
import subprocess
import sys
import tempfile
import time
import zlib

MIB = 1024 * 1024
DISK_BYTES = 8 * MIB
WRITE_OFFSET = 8 * 512
FINAL_PATTERN = bytes((index + 31 * 17) & 255 for index in range(512))
HOST_LOG = re.compile(rb'<[0-7]>\[[ \t]*[0-9]+\.[0-9]+\] [^\n]*\n')
FAILURE_MARKERS = (b'IO-VM-SMOKE: FAIL', b'HypeR: fatal', b'kernel panic',
                   b'Kernel panic', b'HypeR KERNEL PANIC', b'HypeR crash monitor',
                   b'kernel startup failed', b'HypeR business disk: FAIL')


def bounded_read(path, limit):
    with path.open('rb') as stream:
        data = stream.read(limit + 1)
    if not data or len(data) > limit:
        raise ValueError(f'empty or oversized artifact: {path}')
    return data


def package_payloads(package):
    """Accept a complete external boot generation or a verified OCI import."""
    boot = package / 'boot-artifacts.json'
    if boot.exists():
        metadata = json.loads(bounded_read(boot, MIB))
        if metadata.get('format') != 1 or metadata.get('architecture') != 'aarch64':
            raise ValueError('unsupported boot artifact format or architecture')
        paths = []
        for field, pattern, limit in (
                ('kernel', r'Image-([0-9a-f]{64})', 32 * MIB),
                ('initramfs', r'initramfs-([0-9a-f]{64})\.cpio\.gz', 8 * MIB)):
            match = re.fullmatch(pattern, metadata[field])
            if match is None:
                raise ValueError('invalid content-addressed boot filename')
            path = package / metadata[field]
            data = bounded_read(path, limit)
            if hashlib.sha256(data).hexdigest() != match[1]:
                raise ValueError(f'boot artifact checksum mismatch: {field}')
            paths.append(path)
        image, initramfs = paths
    else:
        # Reuse the importer contract rather than independently interpreting
        # its layer names and source-material requirements.
        source = Path(__file__).resolve().parents[2] / 'scripts/fetch-io-vm.py'
        spec = importlib.util.spec_from_file_location('hyper_io_package', source)
        importer = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(importer)
        layers = importer.validate_manifest(package / 'oci-manifest.json',
                                            'sha256:' + package.name, 'qemu')
        importer.validate_runtime(package, layers)
        image, initramfs = package / 'Image', package / 'initramfs.cpio.gz'
    if bounded_read(image, 32 * MIB)[56:60] != b'ARM\x64':
        raise ValueError('not an AArch64 Linux Image')
    decoder = zlib.decompressobj(16 + zlib.MAX_WBITS)
    expanded = decoder.decompress(bounded_read(initramfs, 8 * MIB), 32 * MIB + 1)
    if (len(expanded) > 32 * MIB or not decoder.eof or decoder.unused_data
            or not expanded.startswith(b'070701')):
        raise ValueError('invalid complete initramfs')
    return image.resolve(), initramfs.resolve()


def prepare(package, fit_pack, output, test="basic"):
    image, initramfs = package_payloads(package)
    output.mkdir(parents=True, exist_ok=True)
    for role in ('io', 'business'):
        bootargs = ('console=ttyAMA0 earlycon=pl011,mmio32,0x09000000 '
                    f'rdinit=/init loglevel=7 hyper.role={role} hyper.test={test}')
        subprocess.run([str(fit_pack), str(output / f'{role}.itb'), 'arm64',
                        str(64 * MIB), '1', str(image), '0x40200000', '0x40200000',
                        str(initramfs), bootargs], check=True)


def verify_disk(path):
    """Check the guest's final round and reject writes outside the test extent."""
    if path.stat().st_size != DISK_BYTES:
        raise ValueError('unexpected physical disk size')
    with path.open('rb') as stream:
        if any(stream.read(WRITE_OFFSET)):
            raise ValueError('disk bytes before LBA 8 changed')
        if stream.read(len(FINAL_PATTERN)) != FINAL_PATTERN:
            raise ValueError('physical disk does not contain the final business-guest write')
        while chunk := stream.read(MIB):
            if any(chunk):
                raise ValueError('disk bytes after the test extent changed')


def append_output(pending, data):
    pending.extend(data.replace(b'\r', b''))
    if any(marker in pending for marker in FAILURE_MARKERS):
        raise RuntimeError('cross-VM fixture reported a failure')
    pending[:] = HOST_LOG.sub(b'', pending)
    if any(marker in pending for marker in FAILURE_MARKERS):
        raise RuntimeError('cross-VM fixture reported a failure')
    passed = b'IO-VM-SMOKE: PASS\n' in pending
    if len(pending) > 65536:
        del pending[:-65536]
    return passed


def run(qemu, image, initramfs, logfile, timeout, test="basic"):
    logfile.parent.mkdir(parents=True, exist_ok=True)
    # Keep the uniquely named disk as evidence. Never reuse an old disk whose
    # final pattern could let a run with no actual I/O falsely pass.
    evidence = Path(tempfile.mkdtemp(prefix=logfile.stem + '-', dir=logfile.parent))
    disk = evidence / 'physical-disk.img'
    with disk.open('xb') as stream:
        stream.truncate(DISK_BYTES)
    command = [qemu, '-machine', os.environ.get('QEMU_MACHINE',
               'virt,virtualization=on,gic-version=3'),
               '-cpu', os.environ.get('QEMU_CPU', 'max'),
               '-smp', os.environ.get('QEMU_CPUS', '4'),
               '-m', os.environ.get('QEMU_MEMORY', '512M'),
               '-nodefaults', '-display', 'none', '-serial', 'stdio',
               '-monitor', 'none', '-nic', 'none', '-no-reboot', '-kernel', str(image),
               '-initrd', str(initramfs), '-append', os.environ.get('QEMU_BOOTARGS',
               'earlycon=pl011,mmio32,0x09000000'),
               '-global', 'virtio-mmio.force-legacy=false',
               '-drive', 'if=none,id=physicaldisk,format=raw,file='
               + str(disk).replace(',', ',,') + ',cache=writeback',
               '-device', 'virtio-scsi-device,id=physicalscsi,iommu_platform=on',
               '-device', 'scsi-hd,drive=physicaldisk,bus=physicalscsi.0,scsi-id=0,lun=0']
    started = time.monotonic()
    pending = bytearray()
    with logfile.open('wb') as log, selectors.DefaultSelector() as selector:
        process = subprocess.Popen(command, stdin=subprocess.DEVNULL,
                                   stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        try:
            selector.register(process.stdout, selectors.EVENT_READ)
            deadline = started + timeout
            while time.monotonic() < deadline:
                for key, _ in selector.select(0.2):
                    data = os.read(key.fd, 65536)
                    if not data:
                        raise RuntimeError('QEMU closed output before fixture completion')
                    log.write(data)
                    log.flush()
                    if append_output(pending, data):
                        break
                else:
                    if process.poll() is not None:
                        raise RuntimeError(f'QEMU exited before completion: {process.returncode}')
                    continue
                break
            else:
                raise TimeoutError(f'cross-VM fixture exceeded {timeout:g} seconds')
        except Exception:
            sys.stderr.write(pending[-16384:].decode(errors='replace'))
            sys.stderr.write(f'\nRetained physical disk: {disk}\n')
            raise
        finally:
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
            process.stdout.close()
    if test == 'reset' and b'HypeR business disk: reset/rebind PASS' not in pending:
        raise RuntimeError('appliance did not confirm reset/rebind acceptance')
    verify_disk(disk)
    result = {'passed': True, 'disk': str(disk), 'log': str(logfile),
              'test': test, 'rounds': 64 if test == 'reset' else 32, 'offset': WRITE_OFFSET, 'bytes': len(FINAL_PATTERN),
              'elapsed_seconds': round(time.monotonic() - started, 3),
              'pattern_sha256': hashlib.sha256(FINAL_PATTERN).hexdigest()}
    (evidence / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
    print(f'Cross-VM physical-disk read/write/flush acceptance passed: {logfile}')
    print(f'Host disk verification and evidence: {evidence}')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    packing = commands.add_parser('prepare')
    packing.add_argument('--package', type=Path, required=True)
    packing.add_argument('--fit-pack', type=Path, required=True)
    packing.add_argument('--output', type=Path, required=True)
    packing.add_argument('--test', choices=('basic', 'reset'), default='basic')
    running = commands.add_parser('run')
    running.add_argument('--qemu', required=True)
    running.add_argument('--test', choices=('basic', 'reset'), default='basic')
    running.add_argument('--image', type=Path, required=True)
    running.add_argument('--initramfs', type=Path, required=True)
    running.add_argument('--log', type=Path, required=True)
    running.add_argument('--timeout', type=float,
                         default=float(os.environ.get('QEMU_IO_VM_TIMEOUT_SECONDS', '600')))
    args = parser.parse_args()
    if args.command == 'prepare':
        prepare(args.package, args.fit_pack, args.output, args.test)
    else:
        if not 0 < args.timeout <= 3600:
            parser.error('timeout must be between 0 and 3600 seconds')
        run(args.qemu, args.image, args.initramfs, args.log, args.timeout, args.test)


if __name__ == '__main__':
    main()
