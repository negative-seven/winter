#include <stdio.h>
#include <windows.h>

#include "assert.h"

int main()
{
    for (int i = 0; i < 10; i++)
    {
        LARGE_INTEGER frequency;
        ASSERT_WINAPI(QueryPerformanceFrequency(&frequency))

        LARGE_INTEGER counter;
        ASSERT_WINAPI(QueryPerformanceCounter(&counter))

        printf("%lld/%lld\n", counter.QuadPart, frequency.QuadPart);
        fflush(stdout);

        Sleep(43);
        Sleep(4);
    }
}
