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
        printf("%d %s %Id %08Ix\n", GetTickCount(), (message == WM_KEYDOWN ? "KEYDOWN" : "KEYUP"), w_parameter, l_parameter);
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

    int key_message_count = 0;
    MSG message;
    BOOL get_message_result;
    while ((get_message_result = GetMessage(&message, NULL, 0, 0)) != 0)
    {
        if (get_message_result == -1)
        {
            return 1;
        }

        TranslateMessage(&message);
        DispatchMessage(&message);

        if (message.message == WM_KEYDOWN || message.message == WM_KEYUP)
        {
            key_message_count++;
            if (key_message_count >= 16)
            {
                break;
            }
        }
    }

    return 0;
}
