#include <stdio.h>
#include <windows.h>

#include "create_window.h"

LRESULT __stdcall window_procedure(HWND window, UINT message, WPARAM w_parameter, LPARAM l_parameter)
{
    if (w_parameter == 1234 && l_parameter == 5678)
    {
        printf("%d\n", message);
        fflush(stdout);
        return 0;
    }
    else
    {
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
    SendMessage(window, WM_SETFOCUS, 1234, 5678);
    SendMessage(window, WM_KILLFOCUS, 1234, 5678);
    SendMessage(window, WM_ACTIVATE, 1234, 5678);
    SendMessage(window, WM_ACTIVATEAPP, 1234, 5678);
    SendMessage(window, WM_TIMER, 1234, 5678);
}
