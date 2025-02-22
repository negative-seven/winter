#include <stdbool.h>
#include <stdio.h>
#include <windows.h>

#include "assert.h"

int main()
{
    HMODULE winmm = LoadLibrary("winmm.dll");
    ASSERT_WINAPI(winmm)
    DWORD (*timeGetTime)() = (DWORD(*)())GetProcAddress(winmm, "timeGetTime");
    ASSERT_WINAPI(timeGetTime)
    printf("%d\n", timeGetTime());

    ASSERT_WINAPI(FreeLibrary(winmm))
    printf("%d\n", timeGetTime());

    winmm = LoadLibrary("winmm.dll");
    ASSERT_WINAPI(winmm)
    printf("%d\n", timeGetTime());

    return 0;
}
