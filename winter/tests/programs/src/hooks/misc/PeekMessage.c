#include <stdio.h>
#include <windows.h>

#include "create_window.h"

LRESULT CALLBACK window_procedure(HWND window, UINT message, WPARAM w_parameter, LPARAM l_parameter)
{
    printf("%u %u %Iu %Iu\n", GetTickCount(), message, w_parameter, l_parameter);
    fflush(stdout);
    return DefWindowProc(window, message, w_parameter, l_parameter);
}

int main()
{
    HWND window;
    create_window(&window, window_procedure);
    ShowWindow(window, SW_SHOW);

    for (int i = 0; i < 100; i++)
    {
        Sleep(1);

        MSG message;
        while (PeekMessage(&message, NULL, 0, 0, PM_REMOVE) != 0)
        {
            TranslateMessage(&message);
            DispatchMessage(&message);
        }
    }

    return 0;
}
