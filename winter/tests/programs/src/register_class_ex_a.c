#include <stdio.h>
#include <windows.h>

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
        return DefWindowProcA(window, message, w_parameter, l_parameter);
    }
}

int main()
{
    HMODULE module = GetModuleHandleA(NULL);
    if (module == NULL)
    {
        return 1;
    }

    WNDCLASSEXA class_information = {0};
    class_information.cbSize = sizeof(class_information);
    class_information.lpfnWndProc = window_procedure;
    class_information.hInstance = module;
    class_information.lpszClassName = " ";
    if (RegisterClassExA(&class_information) == 0)
    {
        return 1;
    }

    HWND window = CreateWindowA(
        class_information.lpszClassName,
        "",
        WS_OVERLAPPED,
        -10000,
        -10000,
        0,
        0,
        NULL,
        NULL,
        module,
        NULL);
    if (window == NULL)
    {
        return 1;
    }

    SendMessageA(window, WM_SETFOCUS, 1234, 5678);
    SendMessageA(window, WM_KILLFOCUS, 1234, 5678);
    SendMessageA(window, WM_ACTIVATE, 1234, 5678);
    SendMessageA(window, WM_ACTIVATEAPP, 1234, 5678);
    SendMessageA(window, WM_TIMER, 1234, 5678);
}
