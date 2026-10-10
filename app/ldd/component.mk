# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

$(eval $(call native-rust-app,app/ldd,hyper-ldd,ldd,bin/ldd,system development))
