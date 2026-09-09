/* SPDX-FileCopyrightText: 2026 roolrz
 * SPDX-License-Identifier: Apache-2.0
 */
#include <hyper/startup.h>
#include <limits.h>

extern int main(int argc, char *const *argv);
int hyper_main(const hyper_startup_t *startup)
{
    if (startup->argument_count > INT_MAX) return 1;
    return main((int)startup->argument_count, startup->arguments);
}
