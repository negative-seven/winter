#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <windows.h>

#include "assert.h"

HANDLE create_timer(bool extended_function, bool manual_reset)
{
    HANDLE timer;
    if (extended_function)
    {
        timer = CreateWaitableTimerEx(
            NULL,
            NULL,
            manual_reset ? CREATE_WAITABLE_TIMER_MANUAL_RESET : 0,
            TIMER_ALL_ACCESS);
    }
    else
    {
        timer = CreateWaitableTimer(
            NULL,
            manual_reset ? TRUE : FALSE,
            NULL);
    }
    ASSERT_WINAPI(timer != NULL)
    return timer;
}

void set_timer(HANDLE timer, int64_t time, LONG period)
{
    LARGE_INTEGER time_as_large_integer;
    time_as_large_integer.QuadPart = time;
    ASSERT_WINAPI(SetWaitableTimer(timer, &time_as_large_integer, period, NULL, NULL, FALSE))
}

void wait_for_timer(HANDLE timer)
{
    ASSERT_WINAPI(WaitForSingleObject(timer, INFINITE) != WAIT_FAILED)
}

bool get_timer_state(HANDLE timer)
{
    switch (WaitForSingleObject(timer, 0))
    {
    case WAIT_OBJECT_0:
        return true;
    case WAIT_TIMEOUT:
        return false;
    case WAIT_FAILED:
    default:
        ASSERT_WINAPI(false)
    }
}

void print_periodic_timer_outcome(HANDLE timer, int time_in_milliseconds, int period_in_milliseconds, int repeat_count)
{
    set_timer(timer, time_in_milliseconds * 1000 * 10, period_in_milliseconds);
    for (int i = 0; i < repeat_count; i++)
    {
        Sleep(1);
        wait_for_timer(timer);
        printf("%d %d\n", GetTickCount(), get_timer_state(timer));
        fflush(stdout);
    }
}

void print_timer_outcome(HANDLE timer, int time_in_milliseconds)
{
    print_periodic_timer_outcome(timer, time_in_milliseconds, 0, 1);
}

int main()
{
    HANDLE timer;

    timer = create_timer(false, false);
    Sleep(5);                                      // 5 ms
    print_timer_outcome(timer, 12);                // 12 ms
    print_timer_outcome(timer, -15);               // 27 ms
    print_periodic_timer_outcome(timer, -9, 3, 3); // 36 ms, 39 ms, 42 ms
    printf("\n");
    fflush(stdout);

    timer = create_timer(false, true);
    print_timer_outcome(timer, 50);                 // 50 ms
    print_timer_outcome(timer, -9);                 // 59 ms
    print_periodic_timer_outcome(timer, -1, 10, 3); // 60 ms, 61 ms, 62 ms (timer stays signalled)
    printf("\n");
    fflush(stdout);

    timer = create_timer(true, false);
    print_timer_outcome(timer, 72);                // 72 ms
    print_timer_outcome(timer, -2);                // 74 ms
    print_periodic_timer_outcome(timer, 82, 7, 4); // 82 ms, 89 ms, 96 ms, 103 ms
    printf("\n");
    fflush(stdout);

    timer = create_timer(true, true);
    print_timer_outcome(timer, 112);               // 112 ms
    print_timer_outcome(timer, -5);                // 117 ms
    print_periodic_timer_outcome(timer, -4, 4, 2); // 121 ms, 122 ms (timer stays signalled)
    printf("\n");
    fflush(stdout);
}
