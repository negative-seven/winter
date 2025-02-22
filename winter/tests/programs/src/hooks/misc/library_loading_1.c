#include <stdbool.h>
#include <stdio.h>
#include <windows.h>

#include "assert.h"

int __stdcall free_library_thread_main(void *module)
{
    FreeLibraryAndExitThread((HMODULE)module, 0);
}

int main()
{
    HMODULE winmm = LoadLibraryEx("winmm.dll", NULL, 0);
    ASSERT_WINAPI(winmm)
    DWORD (*timeGetTime)() = (DWORD(*)())GetProcAddress(winmm, "timeGetTime");
    ASSERT_WINAPI(timeGetTime)
    printf("%d\n", timeGetTime());

    HANDLE free_library_thread = CreateThread(NULL, 0, free_library_thread_main, winmm, 0, NULL);
    ASSERT_WINAPI(free_library_thread)
    ASSERT_WINAPI(WaitForSingleObject(free_library_thread, INFINITE) == WAIT_OBJECT_0)
    printf("%d\n", timeGetTime());

    winmm = LoadLibraryEx("winmm.dll", NULL, 0);
    ASSERT_WINAPI(winmm)
    printf("%d\n", timeGetTime());

    return 0;
}
