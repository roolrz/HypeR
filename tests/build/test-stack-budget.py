#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0
"""Check rejection and exception behavior using llvm-readobj's JSON shape."""
import importlib.util
from pathlib import Path
import sys
import unittest

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location('budget', Path(__file__).resolve().parents[2] / 'kernel/tools/stack-budget.py')
budget = importlib.util.module_from_spec(spec)
spec.loader.exec_module(budget)


def metadata(size=64, flags=0, entries=True, section=True):
    return [{'FileSummary': {'Arch': 'aarch64'}, 'Sections': [
        {'Section': {'Name': {'Name': '.stack_sizes'}, 'Size': 10, 'Flags': {'Value': flags}}}
    ] if section else [], 'StackSizes': [
        {'Entry': {'Functions': ['function'], 'Size': size}}
    ] if entries else []}]


def policy(rules=None):
    return {'architecture': 'aarch64', 'default_budget': 4096, 'rules': rules or []}


class BudgetTests(unittest.TestCase):
    def test_default_violation(self):
        self.assertTrue(budget.check(metadata(5000), policy())['failures'])

    def test_exception(self):
        rule = {'pattern': 'function', 'budget': 6000, 'reason': 'boot scratch', 'required': True}
        self.assertFalse(budget.check(metadata(5000), policy([rule]))['failures'])

    def test_missing_metadata(self):
        with self.assertRaisesRegex(ValueError, 'missing'):
            budget.check(metadata(section=False), policy())

    def test_empty_metadata(self):
        with self.assertRaisesRegex(ValueError, 'empty'):
            budget.check(metadata(entries=False), policy())

    def test_empty_section_rejected_even_with_stale_entries(self):
        data = metadata()
        data[0]['Sections'][0]['Section']['Size'] = 0
        with self.assertRaisesRegex(ValueError, 'empty'):
            budget.check(data, policy())

    def test_architecture_specific_policy(self):
        data = metadata()
        data[0]['FileSummary']['Arch'] = 'riscv64'
        with self.assertRaisesRegex(ValueError, 'architecture'):
            budget.check(data, policy())

    def test_allocated_metadata(self):
        with self.assertRaisesRegex(ValueError, 'SHF_ALLOC'):
            budget.check(metadata(flags=2), policy())

    def test_required_rule_cannot_silently_disappear(self):
        rule = {'pattern': 'missing', 'budget': 32, 'reason': 'guard', 'required': True}
        self.assertTrue(budget.check(metadata(), policy([rule]))['failures'])

    def test_alias_cannot_hide_large_frame(self):
        data = metadata(5000)
        data[0]['StackSizes'][0]['Entry']['Functions'].append('alias')
        rule = {'pattern': 'function', 'budget': 6000, 'reason': 'boot', 'required': True}
        self.assertEqual(len(budget.check(data, policy([rule]))['failures']), 1)

    def test_ambiguous_rules_rejected(self):
        rule = {'pattern': 'function', 'budget': 6000, 'reason': 'boot'}
        with self.assertRaisesRegex(ValueError, 'ambiguous'):
            budget.check(metadata(), policy([rule, rule]))

    def test_zero_frame_is_valid(self):
        self.assertFalse(budget.check(metadata(0), policy())['failures'])


if __name__ == '__main__':
    unittest.main()
