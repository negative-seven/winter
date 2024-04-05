use crate::process;
use std::{
    future::Future,
    io,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
};
use thiserror::Error;
use winapi::{
    ctypes::c_void,
    shared::{minwindef::TRUE, ntdef::NULL, winerror::ERROR_IO_PENDING},
    um::{
        handleapi::{CloseHandle, DuplicateHandle},
        winbase::{RegisterWaitForSingleObject, UnregisterWait, INFINITE},
        winnt::{DUPLICATE_SAME_ACCESS, WT_EXECUTEINWAITTHREAD, WT_EXECUTEONLYONCE},
    },
};

#[derive(Debug)]
pub struct Handle(*mut c_void);

impl Handle {
    pub unsafe fn from_raw(raw_handle: *mut c_void) -> Self {
        Self(raw_handle)
    }

    #[must_use]
    pub unsafe fn as_raw(&self) -> *mut c_void {
        self.0
    }

    #[must_use]
    pub unsafe fn leak(self) -> *mut c_void {
        let raw = self.0;
        std::mem::forget(self);
        raw
    }

    pub fn try_clone(&self) -> Result<Self, CloneError> {
        unsafe {
            let current_process_handle = process::Process::get_current().handle().as_raw();
            let mut duplicated_handle = NULL;
            if DuplicateHandle(
                current_process_handle,
                self.0,
                current_process_handle,
                &mut duplicated_handle,
                0,
                TRUE,
                DUPLICATE_SAME_ACCESS,
            ) == 0
            {
                return Err(io::Error::last_os_error().into());
            }
            Ok(Self::from_raw(duplicated_handle))
        }
    }

    pub async fn wait(&self) -> Result<(), WaitError> {
        struct WaitFutureState {
            wait_handle: Option<WaitHandle>,
            completed: bool,
            waker: Option<Waker>,
        }

        struct WaitFuture {
            handle: Handle,
            state: Arc<Mutex<WaitFutureState>>,
        }

        impl WaitFuture {
            fn new(handle: Handle) -> Self {
                Self {
                    handle,
                    state: Arc::new(Mutex::new(WaitFutureState {
                        wait_handle: None,
                        completed: false,
                        waker: None,
                    })),
                }
            }

            unsafe extern "system" fn callback(this: *mut c_void, _: u8) {
                let state = unsafe { Box::from_raw(this.cast::<Arc<Mutex<WaitFutureState>>>()) };
                let mut state = state.lock().unwrap();
                state.completed = true;
                if let Some(waker) = std::mem::take(&mut state.waker) {
                    waker.wake();
                }
            }
        }

        impl Future for WaitFuture {
            type Output = ();

            fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
                let mut state = self.state.lock().unwrap();
                if state.completed {
                    Poll::Ready(())
                } else {
                    state.waker = Some(cx.waker().clone());
                    if state.wait_handle.is_none() {
                        unsafe {
                            let mut wait_handle = NULL;
                            let state_clone = Box::new(Arc::clone(&self.state));
                            if RegisterWaitForSingleObject(
                                &mut wait_handle,
                                self.handle.as_raw(),
                                Some(WaitFuture::callback),
                                Box::into_raw(state_clone).cast(),
                                INFINITE,
                                WT_EXECUTEONLYONCE | WT_EXECUTEINWAITTHREAD,
                            ) == 0
                            {
                                let last_os_error = io::Error::last_os_error();
                                panic!(
                                    "failed to register wait callback for handle {:p}: {}",
                                    self.handle.as_raw(),
                                    last_os_error,
                                );
                            }
                            state.wait_handle = Some(WaitHandle::from_raw(wait_handle));
                        }
                    }
                    Poll::Pending
                }
            }
        }

        WaitFuture::new(self.try_clone()?).await;

        Ok(())
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            if CloseHandle(self.0) == 0 {
                let last_os_error = io::Error::last_os_error();
                panic!("failed to drop handle {:p}: {}", self.0, last_os_error);
            }
        }
    }
}

unsafe impl Send for Handle {}
unsafe impl Sync for Handle {}

struct WaitHandle(*mut c_void);

impl WaitHandle {
    unsafe fn from_raw(raw_handle: *mut c_void) -> Self {
        Self(raw_handle)
    }

    #[must_use]
    unsafe fn as_raw(&self) -> *mut c_void {
        self.0
    }
}

impl Drop for WaitHandle {
    fn drop(&mut self) {
        unsafe {
            #[allow(clippy::cast_possible_wrap)]
            if UnregisterWait(self.as_raw()) == 0 {
                let last_os_error = io::Error::last_os_error();
                assert!(
                    last_os_error.raw_os_error() == Some(ERROR_IO_PENDING as i32),
                    "failed to unregister wait handle {:p}: {}",
                    self.as_raw(),
                    last_os_error,
                );
            }
        }
    }
}

unsafe impl Send for WaitHandle {}
unsafe impl Sync for WaitHandle {}

#[derive(Debug, Error)]
#[error("failed to clone handle")]
pub struct CloneError(#[from] io::Error);

#[derive(Debug, Error)]
#[error("failed to wait for object")]
pub enum WaitError {
    Clone(#[from] CloneError),
}
