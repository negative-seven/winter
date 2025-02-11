#include <stdbool.h>
#include <stdio.h>
#include <windows.h>

#include "assert.h"

bool event_get_state(HANDLE event)
{
    switch (WaitForSingleObject(event, 0))
    {
    case WAIT_OBJECT_0:
        return true;

    case WAIT_TIMEOUT:
        return false;

    default:
        ASSERT_WINAPI(false)
    }
}

void event_set_state(HANDLE event, bool state)
{
    BOOL result;
    if (state)
    {
        result = SetEvent(event);
    }
    else
    {
        result = ResetEvent(event);
    }
    ASSERT_WINAPI(result)
}

int main()
{
    HANDLE *i_bits = malloc(sizeof(HANDLE) * 3);
    for (int j = 0; j < 3; j++)
    {
        i_bits[j] = CreateEvent(NULL, j % 2 ? TRUE : FALSE, FALSE, NULL);
        ASSERT_WINAPI(i_bits[j]);
    }

    while (true)
    {
        int i = 0;
        for (int j = 0; j < 3; j++)
        {
            i |= event_get_state(i_bits[j]) << j;
        }

        printf("%d", i);
        fflush(stdout);
        Sleep(1);

        i += 1;
        for (int j = 0; j < 3; j++)
        {
            event_set_state(i_bits[j], (i >> j) & 1);
        }

        if (i >= 5)
        {
            break;
        }
    }
}
