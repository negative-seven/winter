#include <stdio.h>
#include <windows.h>

int main()
{
    for (int i = 0; i < 10; i++)
    {
        printf("%d\n", timeGetTime());
        fflush(stdout);

        Sleep(2);
        Sleep(3);
        Sleep(5);
        Sleep(7);
        Sleep(11);
        Sleep(13);
    }
}
