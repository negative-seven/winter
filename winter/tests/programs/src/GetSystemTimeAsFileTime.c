#include <stdio.h>
#include <windows.h>

int main()
{
    for (int i = 0; i < 10; i++)
    {
        FILETIME file_time = {0xcccccccc, 0xcccccccc};
        GetSystemTimeAsFileTime(&file_time);
        printf("%d %d\n", file_time.dwHighDateTime, file_time.dwLowDateTime);
        fflush(stdout);

        Sleep(49);
        Sleep(91);
        Sleep(17);
        Sleep(36);
    }
}
