#include <stdio.h>
#include <windows.h>

int main()
{
    for (int i = 0; i < 10; i++)
    {
        printf("%lld\n", GetTickCount64());
        fflush(stdout);

        Sleep(62);
        Sleep(1);
        Sleep(99);
        Sleep(45);
    }
}
