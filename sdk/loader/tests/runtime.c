/* SPDX-FileCopyrightText: 2026 roolrz
 * SPDX-License-Identifier: Apache-2.0
 */
/* Execute the real freestanding loader, mocking only its Native syscall edge.
 * Load host headers before selecting the ELF machine: host libc types stay host
 * types even when testing the other guest ISA's relocation rules. */
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <hyper/syscall.h>
#if defined(TEST_RISCV)
#undef __aarch64__
#define __riscv 1
#define __riscv_xlen 64
#else
#undef __riscv
#define __aarch64__ 1
#endif
#include "../src/rtld.c"

typedef struct {
	uint64_t padding;
	Elf64_Dyn dynamic[12];
	uint32_t hash[4];
	Elf64_Sym symbols[2];
	char strings[16];
	uintptr_t targets[130];
} Image;

static Image image;
static Elf64_Phdr segments[2];
static Object view;
static unsigned effects[32];
static size_t effect_count;
static unsigned reads;
static Elf64_Ehdr file_header;
static Elf64_Phdr file_segments[2];
static int fail_mapping;
static size_t file_length;
static int allow_open;

static void effect(unsigned value)
{
	assert(effect_count < sizeof(effects) / sizeof(effects[0]));
	effects[effect_count++] = value;
}

static void reset(void)
{
	memset(objects, 0, sizeof(objects));
	object_count = 0;
	next_library_address = LIBRARY_BASE;
	root_vmar = 1;
	loader_lock = 0;
	last_error = NULL;
	effect_count = 0;
	reads = 0;
	allow_open = 0;
	fail_mapping = 0;
	memset(&image, 0, sizeof(image));
	segments[0] = (Elf64_Phdr){.type = PT_LOAD, .flags = PF_R | PF_W, .memsz = sizeof(image)};
	segments[1] = (Elf64_Phdr){.type = PT_DYNAMIC,
				   .vaddr = offsetof(Image, dynamic),
				   .memsz = 5 * sizeof(Elf64_Dyn)};
	view = (Object){.base = (uintptr_t)&image, .phdr = segments, .phnum = 2};
	image.dynamic[0] = (Elf64_Dyn){DT_HASH, offsetof(Image, hash)};
	image.dynamic[1] = (Elf64_Dyn){DT_STRTAB, offsetof(Image, strings)};
	image.dynamic[2] = (Elf64_Dyn){DT_STRSZ, sizeof(image.strings)};
	image.dynamic[3] = (Elf64_Dyn){DT_SYMTAB, offsetof(Image, symbols)};
	image.hash[1] = 2;
	memcpy(image.strings, "\0known\0", 7);
}

static void parsing(void)
{
	reset();
	assert(parse_dynamic(&view));
	assert(dynamic_string(&view, sizeof(image.strings)) == NULL);
	memset(image.strings, 'x', sizeof(image.strings));
	assert(dynamic_string(&view, 0) == NULL);
	reset();
	segments[1].memsz--;
	assert(!parse_dynamic(&view));
	reset();
	segments[1].vaddr++;
	assert(!parse_dynamic(&view));
	reset();
	segments[1].memsz = 4 * sizeof(Elf64_Dyn); /* No terminator in bounded table. */
	assert(!parse_dynamic(&view));
	reset();
	image.dynamic[1].value = UINT64_MAX;
	assert(!parse_dynamic(&view));
	reset();
	image.hash[1] = UINT32_MAX;
	assert(!parse_dynamic(&view));
	reset();
	image.dynamic[3].value++;
	assert(!parse_dynamic(&view));
	reset();
	image.dynamic[4] = (Elf64_Dyn){DT_RELA, sizeof(image) - 8};
	image.dynamic[5] = (Elf64_Dyn){DT_RELASZ, sizeof(Elf64_Rela)};
	image.dynamic[6] = (Elf64_Dyn){DT_RELAENT, sizeof(Elf64_Rela)};
	segments[1].memsz = 8 * sizeof(Elf64_Dyn);
	assert(!parse_dynamic(&view));
	reset();
	assert(!object_pointer(&view, UINT64_MAX, 1));
	assert(!address_in_object(&view, UINTPTR_MAX - 1, 8, PF_R));
}

static void relocations(void)
{
	reset();
	assert(parse_dynamic(&view));
	Elf64_Rela rela = {offsetof(Image, targets), HYPER_RELOC_RELATIVE, 7};
	assert(apply_rela(&view, &rela, 1));
	assert(image.targets[0] == view.base + 7);
	image.targets[0] = 123;
	rela.offset++;
	assert(!apply_rela(&view, &rela, 1));
	assert(image.targets[0] == 123);
	rela.offset = offsetof(Image, targets);
	segments[0].flags = PF_R;
	assert(!apply_rela(&view, &rela, 1));
	segments[0].flags |= PF_W;
#if defined(TEST_RISCV)
	const uint32_t symbol_kind = 2;
#else
	const uint32_t symbol_kind = 257;
#endif
	rela.info = (UINT64_C(2) << 32) | symbol_kind;
	assert(!apply_rela(&view, &rela, 1)); /* First out-of-range symbol. */
	assert(image.targets[0] == 123);
	rela.info = (UINT64_C(1) << 32) | symbol_kind;
	image.symbols[1] = (Elf64_Sym){.section = SHN_ABS, .value = UINTPTR_MAX};
	assert(!apply_rela(&view, &rela, 1)); /* Addend overflow. */
	assert(image.targets[0] == 123);

	reset();
	uint64_t relr[] = {offsetof(Image, targets), 1 | (UINT64_C(1) << 1) | (UINT64_C(1) << 63),
			   3};
	view.dynamic.relr = relr;
	view.dynamic.relr_count = 3;
	assert(apply_relr(&view));
	assert(image.targets[0] == view.base && image.targets[1] == view.base);
	assert(image.targets[63] == view.base && image.targets[64] == view.base);
	assert(image.targets[2] == 0 && image.targets[65] == 0);
	reset();
	relr[0] = 1; /* Even an empty bitmap needs an address predecessor. */
	view.dynamic.relr = relr;
	view.dynamic.relr_count = 1;
	assert(!apply_relr(&view));
	relr[0] = offsetof(Image, targets);
	image.targets[0] = UINTPTR_MAX;
	assert(!apply_relr(&view));
	assert(image.targets[0] == UINTPTR_MAX);
	image.targets[0] = 0;
	relr[0] = sizeof(image) - sizeof(uintptr_t);
	relr[1] = 3;
	view.dynamic.relr_count = 2;
	assert(!apply_relr(&view)); /* Bitmap extends past writable segment. */

	/* An absolute-only stream must obey the same work budget as bitmaps.
	 * base=0 avoids arithmetic overflow obscuring the budget boundary. */
	uint64_t *many = calloc(MAX_RELOCATIONS + 1, sizeof(*many));
	assert(many != NULL);
	reset();
	view.base = 0;
	segments[0].vaddr = (uintptr_t)&image;
	for (size_t i = 0; i <= MAX_RELOCATIONS; ++i)
		many[i] = (uintptr_t)&image.targets[0];
	view.dynamic.relr = many;
	view.dynamic.relr_count = MAX_RELOCATIONS;
	assert(apply_relr(&view));
	view.dynamic.relr_count++;
	assert(!apply_relr(&view));
	assert(strcmp(last_error, "too many RELR relocations") == 0);
	free(many);
}

static void rollback(void)
{
	reset();
	objects[0].references = 4;
	objects[0].flags = HYPER_RTLD_LOCAL;
	object_count = 1;
	LoadTransaction transaction = begin_load_transaction();
	objects[0].references++;
	objects[0].flags = HYPER_RTLD_GLOBAL;
	/* Parent then dependency were admitted before a later dependency failed. */
	for (size_t i = 1; i <= 2; ++i) {
		objects[i].vmar = 10 + i;
		objects[i].map_start = LIBRARY_BASE + i * PAGE_SIZE;
		objects[i].map_end = objects[i].map_start + PAGE_SIZE;
	}
	object_count = 3;
	next_library_address += 8 * PAGE_SIZE;
	rollback_load_transaction(&transaction);
	assert(object_count == 1 && next_library_address == LIBRARY_BASE);
	assert(objects[0].references == 4 && objects[0].flags == HYPER_RTLD_LOCAL);
	const unsigned expected[] = {112, 212, 312, 111, 211};
	assert(effect_count == 5 && memcmp(effects, expected, sizeof(expected)) == 0);
	assert(objects[1].vmar == 0 && objects[2].vmar == 0);

	/* Exercise public dlopen's failure path, not only its transaction helper. */
	reset();
	assert(parse_dynamic(&view));
	objects[0] = view;
	copy_name(objects[0].name, "existing.so");
	objects[0].references = 2;
	objects[0].flags = HYPER_RTLD_LOCAL;
	Elf64_Rela bad = {offsetof(Image, targets) + 1, HYPER_RELOC_RELATIVE, 0};
	objects[0].dynamic.rela = &bad;
	objects[0].dynamic.rela_count = 1;
	object_count = 1;
	assert(hyper_dlopen_at(1, "existing.so", HYPER_RTLD_NOW | HYPER_RTLD_GLOBAL) == NULL);
	assert(objects[0].references == 2 && objects[0].flags == HYPER_RTLD_LOCAL);
	assert(object_count == 1 && effect_count == 0 && loader_lock == 0);
}

static void file_bounds(void)
{
	reset();
	allow_open = 1;
	file_length = sizeof(file_header);
	file_header = (Elf64_Ehdr){.ident = {0x7f, 'E', 'L', 'F', 2, 1, 1, HYPER_NATIVE_ELF_OSABI,
					     HYPER_NATIVE_ELF_ABI_VERSION},
				   .type = ET_DYN,
				   .machine = HYPER_ELF_MACHINE,
				   .version = 1,
				   .ehsize = sizeof(Elf64_Ehdr),
				   .phentsize = sizeof(Elf64_Phdr),
				   .phnum = 1,
				   .phoff = sizeof(Elf64_Ehdr)};
#if defined(TEST_RISCV)
	file_header.flags = 4;
#endif
	assert(validate_header(&file_header));
	assert(hyper_dlopen_at(1, "truncated.so", HYPER_RTLD_NOW) == NULL);
	assert(strcmp(last_error, "truncated program headers") == 0);
	assert(reads == 1 && object_count == 0 && effect_count == 2);
	assert(effects[0] == 320 && effects[1] == 321);
	file_header.phoff = UINT64_MAX;
	effect_count = reads = 0;
	assert(hyper_dlopen_at(1, "overflow.so", HYPER_RTLD_NOW) == NULL);
	assert(reads == 1 && object_count == 0 && effect_count == 2);

	/* A real load admitted a VMAR, then mapping failed: the caller owns cleanup. */
	file_header.phoff = sizeof(file_header);
	file_header.phnum = 2;
	file_length = PAGE_SIZE;
	file_segments[0] = (Elf64_Phdr){.type = PT_LOAD,
					.flags = PF_R,
					.filesz = PAGE_SIZE,
					.memsz = PAGE_SIZE,
					.align = PAGE_SIZE};
	file_segments[1] =
		(Elf64_Phdr){.type = PT_DYNAMIC, .vaddr = 512, .memsz = sizeof(Elf64_Dyn)};
	fail_mapping = 1;
	effect_count = reads = 0;
	assert(hyper_dlopen_at(1, "map-fails.so", HYPER_RTLD_NOW) == NULL);
	assert(strcmp(last_error, "read-only shared-object mapping failed") == 0);
	const unsigned expected[] = {320, 400, 500, 321, 142, 242};
	assert(effect_count == 6 && memcmp(effects, expected, sizeof(expected)) == 0);
	assert(object_count == 0 && next_library_address == LIBRARY_BASE);
}

/* Unexpected Native effects fail closed so added loader behavior is reviewed. */
hyper_call_result_t hyper_native_call6(uint64_t n, uint64_t a, uint64_t b, uint64_t c, uint64_t d,
				       uint64_t e, uint64_t f)
{
	(void)n;
	(void)a;
	(void)b;
	(void)c;
	(void)d;
	(void)e;
	(void)f;
	abort();
}

hyper_call_result_t hyper_console_write(hyper_native_handle_t h, const void *b, size_t n)
{
	(void)h;
	(void)b;
	(void)n;
	abort();
}

_Noreturn void hyper_process_exit(int64_t status)
{
	(void)status;
	abort();
}

hyper_call_result_t hyper_directory_open_file(hyper_native_handle_t h, const void *p, size_t n,
					      uint64_t r)
{
	(void)h;
	(void)p;
	(void)n;
	(void)r;
	assert(allow_open);
	return (hyper_call_result_t){0, 20, 0};
}

hyper_call_result_t hyper_file_create_executable_vmo(hyper_native_handle_t h)
{
	assert(h == 20);
	return (hyper_call_result_t){0, 21, file_length};
}

hyper_native_status_t hyper_handle_close(hyper_native_handle_t h)
{
	effect(300 + (unsigned)h);
	return HYPER_NATIVE_STATUS_OK;
}

hyper_native_status_t hyper_vmo_read(hyper_native_handle_t h, uint64_t off, void *out, size_t n)
{
	assert(h == 21);
	reads++;
	if (off == 0 && n == sizeof(file_header))
		memcpy(out, &file_header, n);
	else if (off == sizeof(file_header) && n == sizeof(file_segments))
		memcpy(out, file_segments, n);
	else
		abort();
	return HYPER_NATIVE_STATUS_OK;
}

hyper_call_result_t hyper_vmar_allocate(hyper_native_handle_t h, uintptr_t a, size_t n)
{
	assert(fail_mapping && h == root_vmar && a == LIBRARY_BASE && n == PAGE_SIZE);
	effect(400);
	return (hyper_call_result_t){0, 42, 0};
}

hyper_native_status_t hyper_vmar_map_private(hyper_native_handle_t h, hyper_native_handle_t s,
					     const hyper_native_private_mapping_t *m, size_t n)
{
	assert(fail_mapping && h == 42 && s == 21 && n == sizeof(*m));
	effect(500);
	return HYPER_NATIVE_STATUS_NO_MEMORY;
}

hyper_native_status_t hyper_vmar_protect(hyper_native_handle_t h, uintptr_t a, size_t n, uint32_t p)
{
	(void)h;
	(void)a;
	(void)n;
	(void)p;
	abort();
}

hyper_native_status_t hyper_vmar_unmap(hyper_native_handle_t h, uintptr_t a, size_t n)
{
	assert(a >= LIBRARY_BASE && n == PAGE_SIZE);
	effect(100 + (unsigned)h);
	return 0;
}

hyper_native_status_t hyper_vmar_destroy(hyper_native_handle_t h)
{
	effect(200 + (unsigned)h);
	return h == 12 ? HYPER_NATIVE_STATUS_BAD_STATE : 0;
}

int main(void)
{
	parsing();
	relocations();
	rollback();
	file_bounds();
	puts("loader production parsing, relocation and rollback passed");
	return 0;
}
