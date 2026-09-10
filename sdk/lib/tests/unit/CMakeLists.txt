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

add_executable(hyper-syscall-test
    ../../src/syscall.c
    syscall.c
)
target_compile_features(hyper-syscall-test PRIVATE c_std_17)
target_compile_options(hyper-syscall-test PRIVATE
    -Wall
    -Wextra
    -Werror
)
target_include_directories(hyper-syscall-test PRIVATE
    ../../include
    ${HYPER_ABI_INCLUDE_DIR}
)

add_test(NAME syscall-veneer COMMAND hyper-syscall-test)

# Rename only the C allocation facade so the fixture never intercepts host libc.
find_package(Threads REQUIRED)
add_executable(hyper-heap-test ../../src/heap.c ../../src/stdlib.c ../../src/startup.c heap.c)
target_compile_features(hyper-heap-test PRIVATE c_std_17)
target_compile_options(hyper-heap-test PRIVATE -fno-builtin -Wall -Wextra -Werror
    -include ${CMAKE_CURRENT_SOURCE_DIR}/heap-host.h)
target_compile_definitions(hyper-heap-test PRIVATE
    HYPER_HEAP_BASE=hyper_heap_test_base
    malloc=hyper_test_malloc calloc=hyper_test_calloc realloc=hyper_test_realloc
    free=hyper_test_free aligned_alloc=hyper_test_aligned_alloc
)
target_include_directories(hyper-heap-test PRIVATE ../../include ${HYPER_ABI_INCLUDE_DIR})
target_link_libraries(hyper-heap-test PRIVATE Threads::Threads)
add_test(NAME process-heap COMMAND hyper-heap-test)

add_executable(hyper-thread-test ../../src/thread.c thread.c atomic-wait-host.c)
target_compile_features(hyper-thread-test PRIVATE c_std_17)
target_compile_definitions(hyper-thread-test PRIVATE HYPER_THREAD_HOST_TEST _POSIX_C_SOURCE=200809L)
target_compile_options(hyper-thread-test PRIVATE -Wall -Wextra -Werror)
target_include_directories(hyper-thread-test PRIVATE ../../include ${HYPER_ABI_INCLUDE_DIR})
target_link_libraries(hyper-thread-test PRIVATE Threads::Threads)
add_test(NAME thread-runtime COMMAND hyper-thread-test)

add_executable(hyper-stdio-test ../../src/stdio.c ../../src/startup.c stdio.c)
target_compile_features(hyper-stdio-test PRIVATE c_std_17)
target_compile_options(hyper-stdio-test PRIVATE -Wall -Wextra -Werror)
target_include_directories(hyper-stdio-test PRIVATE ../../include ${HYPER_ABI_INCLUDE_DIR})
add_test(NAME standard-streams COMMAND hyper-stdio-test)
