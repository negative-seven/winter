#include <stdio.h>
#include <windows.h>

#include "create_window.h"

LRESULT CALLBACK window_procedure(HWND window, UINT message, WPARAM w_parameter, LPARAM l_parameter)
{
    switch (message)
    {
    case WM_DESTROY:
        PostQuitMessage(0);
        return 0;
    case WM_KEYDOWN:
    case WM_KEYUP:
        printf("%s %d %08x\n", (message == WM_KEYDOWN ? "KEYDOWN" : "KEYUP"), w_parameter, l_parameter);
        fflush(stdout);
        return 0;
    default:
        return DefWindowProc(window, message, w_parameter, l_parameter);
    }
}

int main()
{
    HWND window;
    if (!create_window(&window, window_procedure))
    {
        return 1;
    }
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
