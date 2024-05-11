#pragma once

#include <stdbool.h>
#include <windows.h>

#include "assert.h"

void create_window(HWND *window, WNDPROC window_procedure)
{
    HMODULE module = GetModuleHandle(NULL);
    ASSERT_WINAPI(module != NULL)

    WNDCLASSEX class_information = {0};
    class_information.cbSize = sizeof(class_information);
    class_information.lpfnWndProc = window_procedure;
    class_information.hInstance = module;
    class_information.lpszClassName = TEXT(" ");
    ASSERT_WINAPI(RegisterClassEx(&class_information))

    *window = CreateWindow(
        class_information.lpszClassName,
        TEXT(""),
        WS_OVERLAPPED,
        -10000,
        -10000,
        0,
        0,
        NULL,
        NULL,
        module,
        NULL);
    ASSERT_WINAPI(*window != NULL)
}
