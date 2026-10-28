//! Typed LWP errors translated at the std ABI boundary.
use ogc_lwp_driver::TlsError;

pub(crate) fn tls_error(error: TlsError) -> i32 {
    match error {
        TlsError::InvalidKey => ogc_sysbase::EINVAL,
        TlsError::KeyCapacity => ogc_sysbase::ENOMEM,
        TlsError::DestructorLimit => ogc_sysbase::EIO,
        TlsError::ReentrantCleanup => ogc_sysbase::EBUSY,
        TlsError::ExceptionContext => 1,
    }
}

pub(crate) fn thread_error(error: ogc_lwp_driver::ThreadError) -> i32 {
    use ogc_lwp_driver::ThreadError;
    match error {
        ThreadError::ExceptionContext => ogc_sysbase::EACCES,
        ThreadError::InterruptsDisabled
        | ThreadError::CleanupComplete
        | ThreadError::CleanupCallbackConflict => ogc_sysbase::EBUSY,
        ThreadError::InvalidThread
        | ThreadError::InvalidStack
        | ThreadError::InvalidTimeout
        | ThreadError::MissingEntry
        | ThreadError::InvalidName
        | ThreadError::BufferTooSmall => ogc_sysbase::EINVAL,
        ThreadError::ResourceExhausted => ogc_sysbase::ENOMEM,
        ThreadError::UnknownStack => ogc_sysbase::ENOTSUP,
        ThreadError::TlsCleanup => ogc_sysbase::EIO,
        ThreadError::Cancelled => ogc_sysbase::ECANCELED,
    }
}
