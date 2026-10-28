//! IOS socket error translation. Values are from devkitPPC newlib errno.h.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NetError {
    pub domain: u32,
    pub code: i32,
}
impl NetError {
    pub const fn local(code: i32) -> Self {
        Self { domain: 3, code }
    }
    pub fn reply(result: Result<i32, i32>) -> Result<i32, Self> {
        match result {
            Err(code) => Err(Self { domain: 2, code }),
            Ok(code) if code < 0 => Err(Self { domain: 1, code }),
            Ok(value) => Ok(value),
        }
    }
    pub fn encoded(self) -> i64 {
        (0x8000_0000_0000_0000u64 | ((self.domain as u64) << 32) | (self.code as u32 as u64)) as i64
    }
    pub fn decode(value: i64) -> Self {
        Self {
            domain: ((value as u64 >> 32) & 0x7fff_ffff) as u32,
            code: value as i32,
        }
    }
    pub fn errno(self) -> i32 {
        match self.domain {
            3 => self.code,
            // IPC transport failures are distinct from IOS socket replies.
            // These values are the IPC_* constants in ogc-ipc, not indices
            // into the socket service's IOS_ERRNO table.
            2 => match self.code {
                -4 => 22,
                -5 | -22 => 12,
                -6 => 2,
                -8 => 11,
                _ => 5,
            },
            1 => self
                .code
                .checked_neg()
                .and_then(|code| IOS_ERRNO.get(code as usize).copied())
                .unwrap_or(5),
            _ => 5,
        }
    }
    pub fn retryable(self) -> bool {
        self.domain == 1 && matches!(self.code, -6 | -27)
    }
}
const IOS_ERRNO: &[i32] = &[
    0, 7,   // 1 E2BIG
    13,  // 2 EACCES
    112, // 3 EADDRINUSE
    125, // 4 EADDRNOTAVAIL
    106, // 5 EAFNOSUPPORT
    11,  // 6 EAGAIN
    120, // 7 EALREADY
    81,  // 8 EBADFD
    77,  // 9 EBADMSG
    16,  // 10 EBUSY
    140, // 11 ECANCELED
    10,  // 12 ECHILD
    113, // 13 ECONNABORTED
    111, // 14 ECONNREFUSED
    104, // 15 ECONNRESET
    45,  // 16 EDEADLK
    121, // 17 EDESTADDRREQ
    33,  // 18 EDOM
    132, // 19 EDQUOT
    17,  // 20 EEXIST
    14,  // 21 EFAULT
    27,  // 22 EFBIG
    118, // 23 EHOSTUNREACH
    36,  // 24 EIDRM
    138, // 25 EILSEQ
    119, // 26 EINPROGRESS
    4,   // 27 EINTR
    22,  // 28 EINVAL
    5,   // 29 EIO
    127, // 30 EISCONN
    21,  // 31 EISDIR
    92,  // 32 ELOOP
    24,  // 33 EMFILE
    31,  // 34 EMLINK
    122, // 35 EMSGSIZE
    74,  // 36 EMULTIHOP
    91,  // 37 ENAMETOOLONG
    115, // 38 ENETDOWN
    126, // 39 ENETRESET
    114, // 40 ENETUNREACH
    23,  // 41 ENFILE
    105, // 42 ENOBUFS
    61,  // 43 ENODATA
    19,  // 44 ENODEV
    2,   // 45 ENOENT
    8,   // 46 ENOEXEC
    46,  // 47 ENOLCK
    67,  // 48 ENOLINK
    12,  // 49 ENOMEM
    35,  // 50 ENOMSG
    109, // 51 ENOPROTOOPT
    28,  // 52 ENOSPC
    63,  // 53 ENOSR
    60,  // 54 ENOSTR
    88,  // 55 ENOSYS
    128, // 56 ENOTCONN
    20,  // 57 ENOTDIR
    90,  // 58 ENOTEMPTY
    108, // 59 ENOTSOCK
    134, // 60 ENOTSUP
    25,  // 61 ENOTTY
    6,   // 62 ENXIO
    95,  // 63 EOPNOTSUPP
    139, // 64 EOVERFLOW
    1,   // 65 EPERM
    32,  // 66 EPIPE
    71,  // 67 EPROTO
    123, // 68 EPROTONOSUPPORT
    107, // 69 EPROTOTYPE
    34,  // 70 ERANGE
    30,  // 71 EROFS
    29,  // 72 ESPIPE
    3,   // 73 ESRCH
    133, // 74 ESTALE
    62,  // 75 ETIME
    116, // 76 ETIMEDOUT
];
