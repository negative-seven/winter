#include <stdbool.h>
#include <stdio.h>
#include <windows.h>

int main()
{
    HANDLE read_pipe;
    HANDLE write_pipe;
    CreatePipe(&read_pipe, &write_pipe, NULL, sizeof(int));

    {
        int i = 0;
        WriteFile(write_pipe, &i, sizeof(i), NULL, NULL);
    }

    while (true)
    {
        int i;
        ReadFile(read_pipe, &i, sizeof(i), NULL, NULL);
        printf("%d", i);
        fflush(stdout);
        Sleep(1);

        CloseHandle(read_pipe);
        CloseHandle(write_pipe);
        CreatePipe(&read_pipe, &write_pipe, NULL, sizeof(int));

        i++;
        if (i >= 5)
        {
            break;
        }

        WriteFile(write_pipe, &i, sizeof(i), NULL, NULL);
    }
}
