#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

"""Check compiler-reported local frames, not a whole-callgraph stack bound.

Assembly entry frames, indirect calls, recursion and runtime watermark evidence
must be reviewed separately. Metadata must be emitted by the tested build.
"""
import argparse
import json
from pathlib import Path
import re
import subprocess
import sys


def check(metadata, policy):
    if not isinstance(metadata, list) or len(metadata) != 1:
        raise ValueError('expected exactly one ELF metadata object')
    elf = metadata[0]
    if elf.get('FileSummary', {}).get('Arch') != policy['architecture']:
        raise ValueError('ELF architecture does not match policy')
    sections = [item['Section'] for item in elf.get('Sections', [])
                if item['Section']['Name']['Name'] == '.stack_sizes']
    if not sections or not any(section['Size'] > 0 for section in sections):
        raise ValueError('missing or empty .stack_sizes section')
    if any(section['Flags']['Value'] & 2 for section in sections):
        raise ValueError('.stack_sizes must not have SHF_ALLOC')
    entries = elf.get('StackSizes', [])
    if not entries:
        raise ValueError('empty stack metadata')
    rules = policy['rules']
    patterns = [re.compile(rule['pattern']) for rule in rules]
    hits = [0] * len(rules)
    report, failures = [], []
    for item in entries:
        entry = item['Entry']
        size, functions = entry['Size'], entry['Functions']
        if type(size) is not int or size < 0 or not functions:
            raise ValueError('invalid stack metadata entry')
        for name in functions:
            selected = [index for index, pattern in enumerate(patterns) if pattern.fullmatch(name)]
            if len(selected) > 1:
                raise ValueError(f'ambiguous stack rules for {name}')
            budget, reason = policy['default_budget'], 'default local-frame ceiling'
            if selected:
                index = selected[0]
                hits[index] += 1
                budget, reason = rules[index]['budget'], rules[index]['reason']
            report.append(dict(function=name, size=size, budget=budget, reason=reason))
            if size > budget:
                failures.append(f'{name}: {size} bytes exceeds {budget}')
    for rule, count in zip(rules, hits):
        if rule.get('required', False) and not count:
            failures.append('required symbol rule unmatched: ' + rule['pattern'])
    report.sort(key=lambda row: (-row['size'], row['function']))
    return dict(architecture=policy['architecture'],
                scope='compiler local frames only; not a whole-callgraph bound',
                functions=report, failures=failures)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('elf')
    parser.add_argument('--readobj', default='llvm-readobj')
    parser.add_argument('--policy', type=Path,
                        default=Path(__file__).with_name('stack-budget-aarch64.json'))
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    try:
        raw = subprocess.check_output([args.readobj, '--elf-output-style=JSON',
                                       '--stack-sizes', '--demangle', '--sections', args.elf], text=True)
        report = check(json.loads(raw), json.loads(args.policy.read_text()))
        if args.output:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(json.dumps(report, indent=2) + '\n')
        for row in report['functions'][:20]:
            print(f"{row['size']:6} / {row['budget']:6} {row['function']}")
        if report['failures']:
            raise ValueError('\n'.join(report['failures']))
        print(f"checked {len(report['functions'])} compiler local frames")
    except (OSError, ValueError, KeyError, TypeError, subprocess.CalledProcessError) as error:
        sys.exit(str(error))


if __name__ == '__main__':
    main()
