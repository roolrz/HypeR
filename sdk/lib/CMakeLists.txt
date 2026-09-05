# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

cmake_minimum_required(VERSION 3.25)
set(CMAKE_TRY_COMPILE_TARGET_TYPE STATIC_LIBRARY)
if(NOT DEFINED CMAKE_AR)
    find_program(CMAKE_AR NAMES llvm-ar REQUIRED)
endif()
if(NOT DEFINED CMAKE_RANLIB)
    find_program(CMAKE_RANLIB NAMES llvm-ranlib REQUIRED)
endif()
project(HypeRLib VERSION 0.0.0 LANGUAGES C ASM)

if(NOT CMAKE_C_COMPILER_ID MATCHES "Clang")
    message(FATAL_ERROR "HypeR Lib requires Clang")
endif()

set(HYPER_ARCH "aarch64" CACHE STRING "HypeR target architecture")
set(HYPER_ABI_INCLUDE_DIR "" CACHE PATH "Directory containing hyper/native.h")

if(NOT HYPER_ARCH STREQUAL "aarch64")
    message(FATAL_ERROR "Native userspace is currently implemented only for AArch64")
endif()
if(NOT EXISTS "${HYPER_ABI_INCLUDE_DIR}/hyper/native.h")
    message(FATAL_ERROR "HYPER_ABI_INCLUDE_DIR must contain hyper/native.h")
endif()

add_library(hyper STATIC
    src/arch/aarch64/syscall.S
    src/crt.c
    src/memory.c
    src/startup.c
    src/string.c
    src/syscall.c
)

add_library(hyper_crt1 OBJECT src/arch/aarch64/crt1.S)

target_compile_features(hyper PRIVATE c_std_17)
target_compile_options(hyper PRIVATE
    $<$<COMPILE_LANGUAGE:C>:-ffreestanding;-fno-builtin;-fvisibility=hidden;-Wall;-Wextra;-Werror>
)
target_include_directories(hyper
    PUBLIC
        $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/include>
        $<BUILD_INTERFACE:${HYPER_ABI_INCLUDE_DIR}>
        $<INSTALL_INTERFACE:include>
)
set_target_properties(hyper PROPERTIES OUTPUT_NAME hyper)

install(TARGETS hyper ARCHIVE DESTINATION lib)
install(FILES $<TARGET_OBJECTS:hyper_crt1> DESTINATION lib RENAME crt1.o)
install(DIRECTORY include/ DESTINATION include)
