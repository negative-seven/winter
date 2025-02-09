use crate::windows::{handle::handle_wrapper, process};
use std::{io, mem::MaybeUninit};
use thiserror::Error;
use winapi::{
    shared::minwindef::FALSE,
    um::{
        processthreadsapi::{
            GetCurrentThread, GetExitCodeThread, GetProcessIdOfThread, GetThreadContext,
            GetThreadId, OpenThread, ResumeThread, SetThreadContext, SuspendThread,
        },
        synchapi::WaitForSingleObject,
        winbase::{Wow64GetThreadContext, Wow64SetThreadContext, INFINITE, WAIT_FAILED},
        winnt::{CONTEXT, CONTEXT_ALL, THREAD_ALL_ACCESS, WOW64_CONTEXT, WOW64_CONTEXT_ALL},
    },
};

handle_wrapper!(Thread);

impl Thread {
    pub fn from_id(id: u32) -> Result<Self, FromIdError> {
        let handle = unsafe { OpenThread(THREAD_ALL_ACCESS, FALSE, id) };
        if handle.is_null() {
            return Err(FromIdError(io::Error::last_os_error()));
        }

        unsafe { Ok(Self::from_raw_handle(handle)) }
    }

    pub fn get_id(&self) -> Result<u32, GetIdError> {
        let id = unsafe { GetThreadId(self.handle.as_raw()) };
        if id == 0 {
            return Err(io::Error::last_os_error().into());
        }
        Ok(id)
    }

    pub fn get_process_id(&self) -> Result<u32, GetProcessIdError> {
        let id = unsafe { GetProcessIdOfThread(self.handle.as_raw()) };
        if id == 0 {
            return Err(io::Error::last_os_error().into());
        }
        Ok(id)
    }

    pub fn increment_suspend_count(&self) -> Result<(), ChangeSuspendCountError> {
        if unsafe { SuspendThread(self.handle.as_raw()) } == 0xffff_ffff {
            return Err(io::Error::last_os_error().into());
        }
        Ok(())
    }

    pub fn decrement_suspend_count(&self) -> Result<(), ChangeSuspendCountError> {
        if unsafe { ResumeThread(self.handle.as_raw()) } == 0xffff_ffff {
            return Err(io::Error::last_os_error().into());
        }
        Ok(())
    }

    pub async fn join(&self) -> Result<u32, JoinError> {
        unsafe {
            if WaitForSingleObject(self.handle.as_raw(), INFINITE) == WAIT_FAILED {
                return Err(io::Error::last_os_error().into());
            }

            let mut exit_code = 0u32;
            if GetExitCodeThread(self.handle.as_raw(), &mut exit_code) == 0 {
                return Err(io::Error::last_os_error().into());
            }

            Ok(exit_code)
        }
    }

    pub fn get_context(&self) -> Result<Context, GetContextError> {
        fn get_normal_context(thread: &Thread) -> Result<CONTEXT, GetContextError> {
            unsafe {
                // https://github.com/retep998/winapi-rs/issues/945
                #[repr(C, align(16))]
                struct AlignedContext(CONTEXT);

                let mut context = MaybeUninit::<AlignedContext>::zeroed().assume_init();
                context.0.ContextFlags = CONTEXT_ALL;
                if GetThreadContext(thread.handle.as_raw(), &mut context.0) == 0 {
                    return Err(io::Error::last_os_error().into());
                }
                Ok(context.0)
            }
        }

        fn get_wow64_context(thread: &Thread) -> Result<WOW64_CONTEXT, GetContextError> {
            unsafe {
                let mut context = MaybeUninit::<WOW64_CONTEXT>::zeroed().assume_init();
                context.ContextFlags = WOW64_CONTEXT_ALL;
                if Wow64GetThreadContext(thread.handle.as_raw(), &mut context) == 0 {
                    return Err(io::Error::last_os_error().into());
                }
                Ok(context)
            }
        }

        let thread_is_64_bit = process::Process::from_id(self.get_process_id()?)?.is_64_bit()?;
        #[cfg(target_pointer_width = "32")]
        if thread_is_64_bit {
            panic!("attempt to get 64-bit thread context from 32-bit process")
        } else {
            Ok(Context::Context32(Box::new(Context32(get_normal_context(
                self,
            )?))))
        }
        #[cfg(target_pointer_width = "64")]
        if thread_is_64_bit {
            Ok(Context::Context64(Box::new(Context64(get_normal_context(
                self,
            )?))))
        } else {
            Ok(Context::Context32(Box::new(Context32(get_wow64_context(
                self,
            )?))))
        }
    }

    pub fn set_context(&self, context: &Context) -> Result<(), SetContextError> {
        fn set_normal_context(thread: &Thread, context: &CONTEXT) -> Result<(), SetContextError> {
            unsafe {
                if SetThreadContext(thread.handle.as_raw(), context) == 0 {
                    return Err(io::Error::last_os_error().into());
                }
                Ok(())
            }
        }

        fn set_wow64_context(
            thread: &Thread,
            context: &WOW64_CONTEXT,
        ) -> Result<(), SetContextError> {
            unsafe {
                if Wow64SetThreadContext(thread.handle.as_raw(), context) == 0 {
                    return Err(io::Error::last_os_error().into());
                }
                Ok(())
            }
        }

        self.increment_suspend_count()?;
        #[cfg(target_pointer_width = "32")]
        match context {
            Context::Context32(context) => set_normal_context(self, &context.as_ref().0),
        }?;
        #[cfg(target_pointer_width = "64")]
        match context {
            Context::Context32(context) => set_wow64_context(self, &context.as_ref().0),
            Context::Context64(context) => set_normal_context(self, &context.as_ref().0),
        }?;
        self.decrement_suspend_count()?;
        Ok(())
    }
}

pub enum Context {
    Context32(Box<Context32>),
    #[cfg(target_pointer_width = "64")]
    Context64(Box<Context64>),
}

pub struct Context32(
    #[cfg(target_pointer_width = "32")] CONTEXT,
    #[cfg(target_pointer_width = "64")] WOW64_CONTEXT,
);

impl Context32 {
    #[must_use]
    pub fn eip(&self) -> u32 {
        self.0.Eip
    }
}

#[cfg(target_pointer_width = "64")]
pub struct Context64(CONTEXT);

#[cfg(target_pointer_width = "64")]
impl Context64 {
    #[must_use]
    pub fn rip(&self) -> u64 {
        self.0.Rip
    }
}

#[derive(Debug, Error)]
#[error("failed to open thread handle from id")]
pub struct FromIdError(#[from] io::Error);

#[derive(Debug, Error)]
#[error("failed to get thread id")]
pub struct GetIdError(#[from] io::Error);

#[derive(Debug, Error)]
#[error("failed to get thread's process id")]
pub struct GetProcessIdError(#[from] io::Error);

#[derive(Debug, Error)]
#[error("failed to change thread's suspend count")]
pub struct ChangeSuspendCountError(#[from] io::Error);

#[derive(Debug, Error)]
#[error("failed to join thread")]
pub struct JoinError(#[from] io::Error);

#[derive(Debug, Error)]
#[error("failed to get thread context")]
pub enum GetContextError {
    GetProcessId(#[from] GetProcessIdError),
    ProcessCheckIs64Bit(#[from] process::CheckIs64BitError),
    Os(#[from] io::Error),
}

#[derive(Debug, Error)]
#[error("failed to set thread context")]
pub enum SetContextError {
    ThreadChangeSuspendCount(#[from] ChangeSuspendCountError),
    Os(#[from] io::Error),
}
