/// The kernel closes this job on a crash too; never recover children by a bare, reusable PID.
#[cfg(windows)]
pub fn install() -> Result<(), String> {
    use windows::Win32::System::{JobObjects::*, Threading::GetCurrentProcess};
    unsafe {
        let job = CreateJobObjectW(None, None).map_err(|e| e.to_string())?;
        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if let Err(error) = SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            (&info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            std::mem::size_of_val(&info) as u32,
        )
        .and_then(|_| AssignProcessToJobObject(job, GetCurrentProcess()))
        {
            let _ = windows::Win32::Foundation::CloseHandle(job);
            return Err(error.to_string());
        }
        // Intentionally held until process termination, including all nested inference jobs.
    }
    Ok(())
}
#[cfg(unix)]
pub fn install() -> Result<(), String> {
    lumen_core::own_processes()
}
