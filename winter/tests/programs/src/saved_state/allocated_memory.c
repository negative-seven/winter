#include <stdbool.h>
#include <stdio.h>
#include <windows.h>

int main()
{
    int *i = VirtualAlloc(NULL, sizeof(int), MEM_COMMIT, PAGE_READWRITE);
    *i = 0;

    while (true)
    {
        printf("%d", *i);
        fflush(stdout);
        Sleep(1);

        {
            int *new_i = VirtualAlloc(NULL, sizeof(int), MEM_COMMIT, PAGE_READWRITE);
            *new_i = *i + 1;
            VirtualFree(i, 0, MEM_RELEASE);
            i = new_i;
        }

        if (*i >= 5)
        {
            break;
        }
    }
}
