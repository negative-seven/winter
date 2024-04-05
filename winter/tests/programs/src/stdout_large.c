#include <stdio.h>
#include <windows.h>

#define LENGTH (1024 * 1024 - 1)

int main()
{
    char *string = malloc(LENGTH + 1);
    memset(string, 's', LENGTH);
    string[LENGTH] = '\0';

    printf("%s", string);
}
