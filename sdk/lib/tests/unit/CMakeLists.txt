# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

cmake_minimum_required(VERSION 3.25)
project(HypeRLibUnitTests LANGUAGES C)

if(NOT CMAKE_C_COMPILER_ID MATCHES "Clang")
    message(FATAL_ERROR "HypeR Lib unit tests require Clang")
endif()

enable_testing()

add_executable(hyper-memory-test
    ../../src/memory.c
    ../../src/string.c
    memory.c
)
target_compile_features(hyper-memory-test PRIVATE c_std_17)
target_compile_options(hyper-memory-test PRIVATE
    -ffreestanding
    -fno-builtin
    -Wall
    -Wextra
    -Werror
)
target_include_directories(hyper-memory-test PRIVATE ../../include)

add_test(NAME memory-primitives COMMAND hyper-memory-test)

add_executable(hyper-startup-test
    ../../src/startup.c
    startup.c
)
target_compile_features(hyper-startup-test PRIVATE c_std_17)
target_compile_options(hyper-startup-test PRIVATE
    -Wall
    -Wextra
    -Werror
)
target_include_directories(hyper-startup-test PRIVATE
    ../../include
    ${HYPER_ABI_INCLUDE_DIR}
)
if(NOT EXISTS "${HYPER_ABI_INCLUDE_DIR}/hyper/native.h")
    message(FATAL_ERROR "HYPER_ABI_INCLUDE_DIR must contain hyper/native.h")
endif()

add_test(NAME startup-stack COMMAND hyper-startup-test)
