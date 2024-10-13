#include <stdio.h>
#include <windows.h>

int main()
{
    for (int i = 0; i < 5; i++)
    {
        printf("%d", i);
        fflush(stdout);
        Sleep(1);
    }
}
