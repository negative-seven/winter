#include <stdio.h>
#include <windows.h>

int main()
{
    for (int i = 0; i < 5; i++)
    {
        Sleep(5);
        Sleep(5);
        Sleep(5);
        Sleep(5);

        for (int key = 0; key < 256; key++)
        {
            if (GetKeyState(key) & (1 << 15))
            {
                printf("%d ", key);
            }
        }
        printf("\n");
        fflush(stdout);
    }
}
