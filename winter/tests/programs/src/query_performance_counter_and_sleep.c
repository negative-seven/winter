#include <stdio.h>
#include <windows.h>

int main()
{
    for (int i = 0; i < 10; i++)
    {
        LARGE_INTEGER frequency;
        QueryPerformanceFrequency(&frequency);

        LARGE_INTEGER counter;
        QueryPerformanceCounter(&counter);

        printf("%lld/%lld\n", counter.QuadPart, frequency.QuadPart);
        fflush(stdout);

        Sleep(43);
        Sleep(4);
    }
}
