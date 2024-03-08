#include <stdio.h>
#include <windows.h>

int main()
{
    for (int i = 0; i < 200; i++)
    {
        printf("%d\n", timeGetTime());
    }
}
