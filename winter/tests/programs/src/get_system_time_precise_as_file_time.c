#include <stdio.h>
#include <windows.h>

int main()
{
    for (int i = 0; i < 200; i++)
    {
        FILETIME file_time = {0xcccccccc, 0xcccccccc};
        GetSystemTimePreciseAsFileTime(&file_time);
        printf("%d %d\n", file_time.dwHighDateTime, file_time.dwLowDateTime);
        fflush(stdout);
    }
}
