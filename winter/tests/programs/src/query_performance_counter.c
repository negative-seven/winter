#include <stdio.h>
#include <windows.h>

int main()
{
    for (int i = 0; i < 200; i++)
    {
        LARGE_INTEGER frequency;
        QueryPerformanceFrequency(&frequency);

        LARGE_INTEGER counter;
        QueryPerformanceCounter(&counter);

        printf("%lli/%lli\n", counter.QuadPart, frequency.QuadPart);
        fflush(stdout);
    }
}
