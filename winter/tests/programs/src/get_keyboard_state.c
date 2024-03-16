#include <stdio.h>
#include <windows.h>

int main()
{
    for (int i = 0; i < 5; i++)
    {
        Sleep(20);

        unsigned char keyboard_state[256];
        GetKeyboardState(&keyboard_state);
        for (int key = 0; key < 256; key++)
        {
            if (keyboard_state[key] & (1 << 7))
            {
                printf("%d ", key);
            }
        }
        printf("\n");
        fflush(stdout);
    }
}
