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

    int key_message_count = 0;
    MSG message;
    BOOL get_message_result;
    while ((get_message_result = GetMessage(&message, NULL, 0, 0)) != 0)
    {
        ASSERT_WINAPI(get_message_result != -1)

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
