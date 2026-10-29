//! Job Object Windows « tuer à la fermeture » : mpv y est rattaché, et Windows le termine
//! dès que le dernier handle du job se ferme, c'est-à-dire quand le processus de l'app
//! disparaît, même sur un plantage (là où `RunEvent::Exit` n'est jamais appelé).

use std::sync::OnceLock;

use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject,
};

/// Handle du job, gardé ouvert jusqu'à la fin du processus (jamais fermé volontairement).
/// Stocké en entier : `HANDLE` contient un pointeur brut, qui n'est pas `Send`.
static JOB: OnceLock<Option<isize>> = OnceLock::new();

fn job() -> Option<HANDLE> {
    let raw = JOB.get_or_init(|| match create() {
        Ok(handle) => Some(handle.0 as isize),
        Err(e) => {
            tracing::warn!("Job Object indisponible, mpv pourrait survivre à un plantage : {e}");
            None
        }
    });
    raw.map(|h| HANDLE(h as *mut _))
}

fn create() -> windows::core::Result<HANDLE> {
    // SAFETY : appels Win32 documentés ; la structure est entièrement initialisée et sa
    // taille passée exactement.
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

/// Rattache le processus `raw_handle` (handle du processus mpv) au job.
pub fn adopt(raw_handle: std::os::windows::io::RawHandle) {
    let Some(job) = job() else {
        return;
    };
    // SAFETY : `raw_handle` est le handle valide d'un processus que l'on vient de lancer.
    if let Err(e) = unsafe { AssignProcessToJobObject(job, HANDLE(raw_handle)) } {
        tracing::warn!("rattachement de mpv au Job Object impossible : {e}");
    }
}
