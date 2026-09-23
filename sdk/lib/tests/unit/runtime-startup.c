/* SPDX-FileCopyrightText: 2026 roolrz
 * SPDX-License-Identifier: Apache-2.0
 */
#include <hyper/heap.h>
#include <hyper/stack.h>
#include "../../src/stack-internal.h"
#include <assert.h>
#include <string.h>

struct hyper_stack {
	unsigned cookie;
};
static struct hyper_stack initial = {123};
static unsigned heap_calls, stack_calls, attach_calls, capability_calls;
static const hyper_native_startup_handle_t *original_handles;

static void check_original(const hyper_startup_t *startup)
{
	assert(startup->handles == original_handles && startup->handle_count == 3);
	hyper_native_handle_t handle = 0;
	assert(hyper_startup_find_handle(startup,
					 HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_INITIAL_STACK_VMAR,
					 &handle) == HYPER_NATIVE_STATUS_OK);
	assert(handle == 22);
}

hyper_native_status_t hyper_heap_initialize(const hyper_startup_t *startup)
{
	check_original(startup);
	assert(!heap_calls++ && !stack_calls && !attach_calls && !capability_calls);
	return HYPER_NATIVE_STATUS_OK;
}

hyper_native_status_t hyper_stack_initialize(const hyper_startup_t *startup, hyper_stack_t **output)
{
	check_original(startup);
	assert(heap_calls == 1 && !stack_calls++ && !attach_calls && !capability_calls);
	*output = &initial;
	return HYPER_NATIVE_STATUS_OK;
}

hyper_native_status_t hyper_runtime_thread_attach_stack(hyper_stack_t *stack)
{
	assert(stack == &initial && stack->cookie == 123);
	assert(heap_calls == 1 && stack_calls == 1 && !attach_calls++ && !capability_calls);
	return HYPER_NATIVE_STATUS_OK;
}

hyper_native_status_t hyper_runtime_capabilities_initialize(const hyper_startup_t *startup)
{
	check_original(startup);
	assert(heap_calls == 1 && stack_calls == 1 && attach_calls == 1 && !capability_calls++);
	return HYPER_NATIVE_STATUS_OK;
}

int main(void)
{
	assert(hyper_runtime_startup() == NULL);
	assert(hyper_runtime_initialize(NULL) == HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
	assert(hyper_runtime_startup() == NULL && !heap_calls);
	char argument[] = "/bin/example";
	char option[] = "--help";
	char environment[] = "TERM=hyper";
	hyper_native_startup_handle_t handles[] = {
		{HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_ROOT_VMAR, 0, 11},
		{HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_INITIAL_STACK_VMAR, 0, 22},
		{HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_CONSOLE, 0, 33},
	};
	original_handles = handles;
	uintptr_t words[] = {
		2,
		(uintptr_t)argument,
		(uintptr_t)option,
		0,
		(uintptr_t)environment,
		0,
		HYPER_NATIVE_AUXV_STARTUP_HANDLES,
		(uintptr_t)handles,
		HYPER_NATIVE_AUXV_STARTUP_HANDLE_COUNT,
		3,
		HYPER_NATIVE_AUXV_INITIAL_STACK_BASE,
		0xf1000000,
		HYPER_NATIVE_AUXV_INITIAL_STACK_CAPACITY,
		0x800000,
		HYPER_NATIVE_AUXV_INITIAL_STACK_SIZE,
		0x40000,
		6,
		HYPER_NATIVE_PAGE_SIZE,
		0,
		0,
	};
	assert(hyper_runtime_initialize(words) == HYPER_NATIVE_STATUS_OK);
	const hyper_startup_t *view = hyper_runtime_startup();
	assert(view && view->handle_count == 2 && view->handles != handles);
	assert(view->handles[0].purpose == handles[0].purpose && view->handles[0].handle == 11);
	assert(view->handles[1].purpose == handles[2].purpose && view->handles[1].handle == 33);
	assert(view->argument_count == 2 && view->arguments[0] == argument &&
	       view->arguments[1] == option);
	assert(view->environment_count == 1 && view->environment[0] == environment);
	assert(view->auxiliary_count == 6);
	assert(view->auxiliary[view->auxiliary_count].key == 0);
	assert(view->auxiliary[view->auxiliary_count].value == 0);
	const hyper_auxiliary_entry_t *original_aux = (const hyper_auxiliary_entry_t *)&words[6];
	for (size_t i = 0; i < view->auxiliary_count; ++i) {
		assert(view->auxiliary[i].key == original_aux[i].key);
		if (view->auxiliary[i].key == HYPER_NATIVE_AUXV_STARTUP_HANDLES)
			assert(view->auxiliary[i].value == (uintptr_t)view->handles);
		else if (view->auxiliary[i].key == HYPER_NATIVE_AUXV_STARTUP_HANDLE_COUNT)
			assert(view->auxiliary[i].value == view->handle_count);
		else
			assert(view->auxiliary[i].value == original_aux[i].value);
	}
	assert(handles[1].handle == 22 && original_aux[1].value == 3);
	hyper_native_handle_t output = 0;
	assert(hyper_startup_find_handle(view,
					 HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_INITIAL_STACK_VMAR,
					 &output) != HYPER_NATIVE_STATUS_OK);
	assert(hyper_runtime_initialize(words) == HYPER_NATIVE_STATUS_OK);
	assert(hyper_runtime_startup() == view);
	assert(heap_calls == 1 && stack_calls == 1 && attach_calls == 1 && capability_calls == 1);
	return 0;
}
