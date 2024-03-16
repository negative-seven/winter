#include <stdio.h>
#include <windows.h>

int main()
{
    for (int i = 0; i < 5; i++)
    {
        Sleep(1);
        Sleep(18);
        Sleep(1);

        for (int key = 0; key < 256; key++)
        {
            if (GetAsyncKeyState(key) & (1 << 15))
            {
                printf("%d ", key);
            }
        }
        printf("\n");
        fflush(stdout);
    }
}
