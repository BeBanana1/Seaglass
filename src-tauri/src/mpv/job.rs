//! Windows "kill on close" Job Object: mpv is assigned to it, and Windows terminates it as
//! soon as the job's last handle closes, that is when the app's process goes away, even on
//! a crash (where `RunEvent::Exit` is never called).

use std::sync::OnceLock;

use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject,
};

/// Handle of the job, kept open until the process ends (never closed on purpose).
/// Stored as an integer: `HANDLE` holds a raw pointer, which is not `Send`.
static JOB: OnceLock<Option<isize>> = OnceLock::new();

fn job() -> Option<HANDLE> {
    let raw = JOB.get_or_init(|| match create() {
        Ok(handle) => Some(handle.0 as isize),
        Err(e) => {
            tracing::warn!("Job Object unavailable, mpv could outlive a crash: {e}");
            None
        }
    });
    raw.map(|h| HANDLE(h as *mut _))
}

fn create() -> windows::core::Result<HANDLE> {
    // SAFETY: documented Win32 calls; the structure is fully initialized and its exact size
    // is passed.
    unsafe {
        let handle = CreateJobObjectW(None, None)?;
        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        SetInformationJobObject(
            handle,
            JobObjectExtendedLimitInformation,
            std::ptr::from_ref(&info).cast(),
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )?;
        Ok(handle)
    }
}

/// Assigns the process `raw_handle` (handle of the mpv process) to the job.
pub fn adopt(raw_handle: std::os::windows::io::RawHandle) {
    let Some(job) = job() else {
        return;
    };
    // SAFETY: `raw_handle` is the valid handle of a process that was just started.
    if let Err(e) = unsafe { AssignProcessToJobObject(job, HANDLE(raw_handle)) } {
        tracing::warn!("cannot attach mpv to the Job Object: {e}");
    }
}
