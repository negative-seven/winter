#include <stdio.h>
#include <windows.h>

#include "assert.h"

typedef enum
{
    ThreadHideFromDebugger = 17,
} THREADINFOCLASS;

typedef NTSTATUS(NTAPI *NtSetInformationThread_type)(HANDLE, THREADINFOCLASS, PVOID, ULONG);

HANDLE NtCurrentThread()
{
    return (HANDLE)-2;
}

void __declspec(noinline) send_end_message()
{
    printf("end\n");
    fflush(stdout);
}

int main_debuggee(int argc, char *argv[])
{
    HMODULE ntdll = GetModuleHandleA("ntdll.dll");
    ASSERT_WINAPI(ntdll != NULL)
    NtSetInformationThread_type NtSetInformationThread = (NtSetInformationThread_type)GetProcAddress(ntdll, "NtSetInformationThread");
    ASSERT_WINAPI(NtSetInformationThread != NULL)
    ASSERT_NTAPI(NtSetInformationThread(NtCurrentThread(), ThreadHideFromDebugger, NULL, 0))

    HANDLE debugger_process;
    {
        char arguments[32];
        sprintf_s(arguments, sizeof(arguments), "- %d %d", GetCurrentProcessId(), GetCurrentThreadId());
        STARTUPINFOA startup_info = {0};
        startup_info.cb = sizeof(startup_info);
        PROCESS_INFORMATION process_information;
        ASSERT_WINAPI(CreateProcessA(
            argv[0],
            arguments,
            NULL,
            NULL,
            FALSE,
            0,
            NULL,
            NULL,
            &startup_info,
            &process_information))
        ASSERT_WINAPI(CloseHandle(process_information.hThread))
        debugger_process = process_information.hProcess;
    }
    ASSERT_WINAPI(SuspendThread(GetCurrentThread()) != -1)

    printf("start\n");
    fflush(stdout);
    send_end_message();

    ASSERT_WINAPI(WaitForSingleObject(debugger_process, INFINITE) != WAIT_FAILED)
    int debugger_exit_code;
    ASSERT_WINAPI(GetExitCodeProcess(debugger_process, &debugger_exit_code))
    if (debugger_exit_code != 0)
    {
        return debugger_exit_code;
    }
    ASSERT_WINAPI(CloseHandle(debugger_process))

    return 0;
}

int main_debugger(int argc, char *argv[])
{
    int debuggee_process_id = strtol(argv[1], NULL, 10);
    ASSERT(debuggee_process_id != 0)
    int debuggee_thread_id = strtol(argv[2], NULL, 10);
    ASSERT(debuggee_thread_id != 0)

    ASSERT_WINAPI(DebugActiveProcess(debuggee_process_id))

    HANDLE debuggee_thread = OpenThread(THREAD_ALL_ACCESS, FALSE, debuggee_thread_id);
    ASSERT_WINAPI(debuggee_thread != NULL)

    {
        CONTEXT thread_context = {0};
        thread_context.ContextFlags = CONTEXT_DEBUG_REGISTERS;
        thread_context.Dr0 = (DWORD64)send_end_message;
        thread_context.Dr7 = 0b1;
        ASSERT_WINAPI(SetThreadContext(debuggee_thread, &thread_context))
    }

    ASSERT_WINAPI(ResumeThread(debuggee_thread) != -1)

    while (1)
    {
        DEBUG_EVENT debug_event;
        ASSERT_WINAPI(WaitForDebugEvent(&debug_event, 3000))

        if (debug_event.dwDebugEventCode == EXCEPTION_DEBUG_EVENT && debug_event.u.Exception.ExceptionRecord.ExceptionCode == EXCEPTION_SINGLE_STEP)
        {
            printf("breakpoint\n");
            fflush(stdout);

            CONTEXT thread_context;
            thread_context.ContextFlags = CONTEXT_DEBUG_REGISTERS;
            ASSERT_WINAPI(GetThreadContext(debuggee_thread, &thread_context))
            thread_context.Dr7 = 0;
            ASSERT_WINAPI(SetThreadContext(debuggee_thread, &thread_context))

            ASSERT_WINAPI(ContinueDebugEvent(debug_event.dwProcessId, debug_event.dwThreadId, DBG_CONTINUE))
            break;
        }

        ASSERT_WINAPI(ContinueDebugEvent(debug_event.dwProcessId, debug_event.dwThreadId, DBG_CONTINUE))
    }

    ASSERT_WINAPI(DebugActiveProcessStop(debuggee_process_id))
    ASSERT_WINAPI(CloseHandle(debuggee_thread))
    ASSERT_WINAPI(DebugSetProcessKillOnExit(FALSE))
    return 0;
}

int main(int argc, char *argv[])
{
    if (argc == 1)
    {
        return main_debuggee(argc, argv);
    }
    else if (argc == 3)
    {
        return main_debugger(argc, argv);
    }
    else
    {
        return 1;
    }
}
