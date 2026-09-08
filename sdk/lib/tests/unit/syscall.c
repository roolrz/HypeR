/* SPDX-FileCopyrightText: 2026 roolrz
 * SPDX-License-Identifier: Apache-2.0
 */

#include <hyper/syscall.h>

#include <assert.h>

static uint64_t captured_number;
static uint64_t captured_arguments[6];

hyper_call_result_t hyper_native_call6(
    uint64_t number,
    uint64_t argument0,
    uint64_t argument1,
    uint64_t argument2,
    uint64_t argument3,
    uint64_t argument4,
    uint64_t argument5)
{
    captured_number = number;
    captured_arguments[0] = argument0;
    captured_arguments[1] = argument1;
    captured_arguments[2] = argument2;
    captured_arguments[3] = argument3;
    captured_arguments[4] = argument4;
    captured_arguments[5] = argument5;
    return (hyper_call_result_t){
        .status = HYPER_NATIVE_STATUS_OK,
        .value0 = UINT64_C(123456789),
        .value1 = 0,
    };
}

int main(void)
{
    hyper_call_result_t result = hyper_clock_get_monotonic();

    assert(captured_number == HYPER_NATIVE_SYS_CLOCK_GET_MONOTONIC);
    for (size_t index = 0; index < 6; ++index) {
        assert(captured_arguments[index] == 0);
    }
    assert(result.status == HYPER_NATIVE_STATUS_OK);
    assert(result.value0 == UINT64_C(123456789));
    assert(result.value1 == 0);
    return 0;
}
