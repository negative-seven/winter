#include <stdio.h>
#include <windows.h>

int main()
{
    for (int i = 0; i < 10; i++)
    {
        printf("%d\n", GetTickCount());
        fflush(stdout);

        Sleep(12);
        Sleep(22);
        Sleep(32);
        Sleep(13);
    }
}
