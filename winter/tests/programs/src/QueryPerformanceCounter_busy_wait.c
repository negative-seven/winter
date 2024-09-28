#include <stdio.h>
#include <windows.h>

#include "assert.h"

int main()
{
    for (int i = 0; i < 200; i++)
    {
        LARGE_INTEGER frequency;
        ASSERT_WINAPI(QueryPerformanceFrequency(&frequency))

        LARGE_INTEGER counter;
        ASSERT_WINAPI(QueryPerformanceCounter(&counter))

        printf("%lli/%lli\n", counter.QuadPart, frequency.QuadPart);
        fflush(stdout);
    }
}
