#include <stdio.h>
#include <windows.h>

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
    if (ntdll == NULL)
    {
        return 1;
    }
    NtSetInformationThread_type NtSetInformationThread = (NtSetInformationThread_type)GetProcAddress(ntdll, "NtSetInformationThread");
    if (NtSetInformationThread == NULL)
    {
        return 1;
    }
    NTSTATUS result = NtSetInformationThread(NtCurrentThread(), ThreadHideFromDebugger, NULL, 0);
    if (result != 0)
    {
        return 1;
    }

    HANDLE debugger_process;
    {
        char arguments[32];
        sprintf(arguments, "- %d %d", GetCurrentProcessId(), GetCurrentThreadId());
        STARTUPINFOA startup_info = {0};
        startup_info.cb = sizeof(startup_info);
        PROCESS_INFORMATION process_information;
        if (CreateProcessA(
                argv[0],
                arguments,
                NULL,
                NULL,
                FALSE,
                0,
                NULL,
                NULL,
                &startup_info,
                &process_information) == 0)
        {
            return 1;
        }
        if (CloseHandle(process_information.hThread) == 0)
        {
            return 1;
        }
        debugger_process = process_information.hProcess;
    }
    if (SuspendThread(GetCurrentThread()) == -1)
    {
        return 1;
    }

    printf("start\n");
    fflush(stdout);
    send_end_message();

    if (WaitForSingleObject(debugger_process, INFINITE) == WAIT_FAILED)
    {
        return 1;
    }
    int debugger_exit_code;
    if (GetExitCodeProcess(debugger_process, &debugger_exit_code) == 0)
    {
        return 1;
    }
    if (debugger_exit_code != 0)
    {
        return debugger_exit_code;
    }
    if (CloseHandle(debugger_process) == 0)
    {
        return 1;
    }

    return 0;
}

int main_debugger(int argc, char *argv[])
{
    int debuggee_process_id = strtol(argv[1], NULL, 10);
    if (debuggee_process_id == 0)
    {
        return 1;
    }
    int debuggee_thread_id = strtol(argv[2], NULL, 10);
    if (debuggee_thread_id == 0)
    {
        return 1;
    }

    if (DebugActiveProcess(debuggee_process_id) == 0)
    {
        return 1;
    }

    HANDLE debuggee_thread = OpenThread(THREAD_ALL_ACCESS, FALSE, debuggee_thread_id);
    if (debuggee_thread == NULL)
    {
        return 1;
    }

    {
        CONTEXT thread_context = {0};
        thread_context.ContextFlags = CONTEXT_DEBUG_REGISTERS;
        thread_context.Dr0 = (DWORD64)send_end_message;
        thread_context.Dr7 = 0b1;
        if (SetThreadContext(debuggee_thread, &thread_context) == 0)
        {
            return 1;
        }
    }

    if (ResumeThread(debuggee_thread) == -1)
    {
        return 1;
    }

    while (1)
    {
        DEBUG_EVENT debug_event;
        if (WaitForDebugEvent(&debug_event, 3000) == 0)
        {
            return 1;
        }

        if (debug_event.dwDebugEventCode == EXCEPTION_DEBUG_EVENT && debug_event.u.Exception.ExceptionRecord.ExceptionCode == EXCEPTION_SINGLE_STEP)
        {
            printf("breakpoint\n");
            fflush(stdout);

            CONTEXT thread_context;
            thread_context.ContextFlags = CONTEXT_DEBUG_REGISTERS;
            if (GetThreadContext(debuggee_thread, &thread_context) == 0)
            {
                return 1;
            }
            thread_context.Dr7 = 0;
            if (SetThreadContext(debuggee_thread, &thread_context) == 0)
            {
                return 1;
            }
        }
        else if (debug_event.dwDebugEventCode == EXIT_PROCESS_DEBUG_EVENT)
        {
            break;
        }

        if (ContinueDebugEvent(debug_event.dwProcessId, debug_event.dwThreadId, DBG_CONTINUE) == 0)
        {
            return 1;
        }
    }

    CloseHandle(debuggee_thread);

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
