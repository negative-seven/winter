#include <stdbool.h>
#include <stdio.h>
#include <windows.h>

#include "assert.h"

bool mutex_get_state(HANDLE mutex)
{
    switch (WaitForSingleObject(mutex, 0))
    {
    case WAIT_OBJECT_0:
        return true;

    case WAIT_TIMEOUT:
        return false;

    default:
        ASSERT_WINAPI(false)
    }
}

void mutex_set_state(HANDLE mutex, bool state)
{
    bool previous_state = mutex_get_state(mutex);

    if (state && !previous_state)
    {
        ASSERT_WINAPI(SetEvent(mutex))
    }
    else if (!state && previous_state)
    {
        ASSERT_WINAPI(ResetEvent(mutex))
    }
}

int main()
{
    HANDLE *i_bits = malloc(sizeof(HANDLE) * 3);
    for (int j = 0; j < 3; j++)
    {
        i_bits[j] = CreateMutex(NULL, FALSE, NULL);
        ASSERT_WINAPI(i_bits[j]);
    }

    while (true)
    {
        int i = 0;
        for (int j = 0; j < 3; j++)
        {
            i |= mutex_get_state(i_bits[j]) << j;
        }

        printf("%d", i);
        fflush(stdout);
        Sleep(1);

        i += 1;
        for (int j = 0; j < 3; j++)
        {
            mutex_set_state(i_bits[j], (i >> j) & 1);
        }

        if (i >= 5)
        {
            break;
        }
    }
}
