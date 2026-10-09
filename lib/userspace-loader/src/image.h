/* SPDX-FileCopyrightText: 2026 roolrz
 * SPDX-License-Identifier: Apache-2.0
 */
#ifndef HYPER_USERSPACE_IMAGE_H
#define HYPER_USERSPACE_IMAGE_H
#include <hyper/elf.h>
#include <hyper/syscall.h>

/* Fixed bounds keep bootstrap allocation-free. These are SDK format policy,
 * not syscall limits; changing them requires no kernel implementation change. */
#define IMAGE_MAX_PHDR 32
#define IMAGE_PAGE 4096u

typedef struct {
	Elf64_Ehdr header;
	Elf64_Phdr phdr[IMAGE_MAX_PHDR];
	uintptr_t base, end, entry, program_headers, stack_size;
	char interpreter[256];
} Image;

hyper_native_status_t image_load(Image *image, hyper_native_handle_t root,
				 hyper_native_handle_t file, uint64_t file_size, uintptr_t start,
				 uintptr_t limit, int interpreter);
#endif
