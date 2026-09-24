/* SPDX-FileCopyrightText: 2026 roolrz
 * SPDX-License-Identifier: Apache-2.0
 */

#include <hyper/heap.h>
#include <hyper/startup.h>
#include <hyper/syscall.h>
#include <hyper/thread.h>

static __attribute__((noreturn)) void run(const uintptr_t *initial_stack)
{
	(void)initial_stack;
	int result = hyper_main(hyper_runtime_startup());
	hyper_runtime_thread_detach();
	hyper_process_exit(result);
}

__attribute__((noreturn)) void __hyper_crt_start(const uintptr_t *initial_stack)
{
	/* The dynamic loader already made the one-way handoff before constructors. */
	if (hyper_runtime_startup())
		run(initial_stack);
	hyper_runtime_start(initial_stack, run);
}
