#pragma once

#define ASSERT(expression)                                           \
    {                                                                \
        if (!(expression))                                           \
        {                                                            \
            printf("Assertion failed at %s:%d", __FILE__, __LINE__); \
            fflush(stdout);                                          \
            ExitProcess(1);                                          \
        }                                                            \
    }

#define ASSERT_WINAPI(expression)                                                                   \
    {                                                                                               \
        if (!(expression))                                                                          \
        {                                                                                           \
            printf("Error in Windows API call at %s:%d: 0x%x", __FILE__, __LINE__, GetLastError()); \
            fflush(stdout);                                                                         \
            ExitProcess(1);                                                                         \
        }                                                                                           \
    }

#define ASSERT_NTAPI(expression)                                                                   \
    {                                                                                              \
        NTSTATUS status = (expression);                                                            \
        if (status != 0)                                                                           \
        {                                                                                          \
            printf("Error in Windows Native API call at %s:%d: 0x%x", __FILE__, __LINE__, status); \
            fflush(stdout);                                                                        \
            ExitProcess(1);                                                                        \
        }                                                                                          \
    }
