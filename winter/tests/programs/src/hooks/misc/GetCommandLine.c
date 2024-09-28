#include <stdio.h>
#include <tchar.h>
#include <windows.h>

int main()
{
    _tprintf(TEXT("%s"), GetCommandLine());
}
