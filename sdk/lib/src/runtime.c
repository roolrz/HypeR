/* SPDX-FileCopyrightText: 2026 roolrz
 * SPDX-License-Identifier: Apache-2.0
 */

#include <hyper/heap.h>
#include <hyper/thread.h>
#include "stack-internal.h"
#include <stdatomic.h>
#include <stdlib.h>

static hyper_startup_t process_startup;
static hyper_native_startup_handle_t application_handles[HYPER_NATIVE_STARTUP_MAX_HANDLES];
static atomic_bool initialized;

const hyper_startup_t *hyper_runtime_startup(void)
{
	return atomic_load_explicit(&initialized, memory_order_acquire) ? &process_startup : NULL;
}

/* Invoked through the relocated libhyper image, not the interpreter's static
 * copy of runtime primitives, so constructors and main share one heap. */
hyper_native_status_t hyper_runtime_initialize(const uintptr_t *initial_stack)
{
	if (atomic_load_explicit(&initialized, memory_order_acquire))
		return HYPER_NATIVE_STATUS_OK;
	hyper_startup_t startup;
	hyper_native_status_t status = hyper_startup_parse(initial_stack, &startup);
	if (status != HYPER_NATIVE_STATUS_OK) {
		return status;
	}
	status = hyper_heap_initialize(&startup);
	if (status != HYPER_NATIVE_STATUS_OK)
		return status;
	hyper_stack_t *stack;
	status = hyper_stack_initialize(&startup, &stack);
	if (status != HYPER_NATIVE_STATUS_OK)
		return status;
	status = hyper_runtime_thread_attach_stack(stack);
	if (status != HYPER_NATIVE_STATUS_OK)
		return status;
	/* Initial loader/CRT initialization runs before secondary threads exist. */
	status = hyper_runtime_capabilities_initialize(&startup);
	if (status != HYPER_NATIVE_STATUS_OK)
		return status;
	/* Transfer the initial stack capability to the stack owner. Language
	 * startup owners may close every exposed handle, so never expose this
	 * runtime-owned reservation in their application view. */
	size_t count = 0;
	for (size_t i = 0; i < startup.handle_count; ++i) {
		if (startup.handles[i].purpose !=
		    HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_INITIAL_STACK_VMAR)
			application_handles[count++] = startup.handles[i];
	}
	hyper_auxiliary_entry_t *auxiliary =
		calloc(startup.auxiliary_count + 1, sizeof(*auxiliary));
	if (!auxiliary)
		return HYPER_NATIVE_STATUS_NO_MEMORY;
	for (size_t i = 0; i < startup.auxiliary_count; ++i) {
		auxiliary[i] = startup.auxiliary[i];
		if (auxiliary[i].key == HYPER_NATIVE_AUXV_STARTUP_HANDLES)
			auxiliary[i].value = (uintptr_t)application_handles;
		if (auxiliary[i].key == HYPER_NATIVE_AUXV_STARTUP_HANDLE_COUNT)
			auxiliary[i].value = count;
	}
	process_startup = startup;
	process_startup.auxiliary = auxiliary;
	process_startup.handles = application_handles;
	process_startup.handle_count = count;
	atomic_store_explicit(&initialized, 1, memory_order_release);
	return HYPER_NATIVE_STATUS_OK;
}
