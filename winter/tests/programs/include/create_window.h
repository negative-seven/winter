#include <stdbool.h>
#include <windows.h>

bool create_window(HWND *window, WNDPROC window_procedure)
{
    HMODULE module = GetModuleHandle(NULL);
    if (module == NULL)
    {
        return false;
    }

    WNDCLASSEX class_information = {0};
    class_information.cbSize = sizeof(class_information);
    class_information.lpfnWndProc = window_procedure;
    class_information.hInstance = module;
    class_information.lpszClassName = TEXT(" ");
    if (RegisterClassEx(&class_information) == 0)
    {
        return false;
    }

    *window = CreateWindow(
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
