#include <stdbool.h>
#include <windows.h>

bool create_window(HWND *window, WNDPROC window_procedure)
{
    HMODULE module = GetModuleHandleA(NULL);
    if (module == NULL)
    {
        return false;
    }

    WNDCLASSEXA class_information = {0};
    class_information.cbSize = sizeof(class_information);
    class_information.lpfnWndProc = window_procedure;
    class_information.hInstance = module;
    class_information.lpszClassName = " ";
    if (RegisterClassExA(&class_information) == 0)
    {
        return false;
    }

    *window = CreateWindowA(
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
    if (*window == NULL)
    {
        return false;
    }

    return true;
}
