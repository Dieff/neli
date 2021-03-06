use std;
use std::error::Error;
use std::fmt::{self,Display};
use std::io;
use std::mem;
use std::str;
use std::string;

use libc;
use buffering::copy::{StreamReadBuffer,StreamWriteBuffer};

use Nl;
use nl::{Nlmsghdr,NlEmpty};
use consts::NlType;

macro_rules! try_err_compat {
    ( $err_name:ident, $( $from_err_name:path ),* ) => {
        $(
            impl From<$from_err_name> for $err_name {
                fn from(v: $from_err_name) -> Self {
                    $err_name::new(v.description())
                }
            }
        )*
    }
}

/// Struct representing netlink packets containing errors
pub struct Nlmsgerr<T> {
    /// Error code
    pub error: libc::c_int,
    /// Packet header for request that failed
    pub nlmsg: Nlmsghdr<T, NlEmpty>,
}

impl<T> Nl for Nlmsgerr<T> where T: NlType {
    type SerIn = ();
    type DeIn = ();

    fn serialize(&self, mem: &mut StreamWriteBuffer) -> Result<(), SerError> {
        self.error.serialize(mem)?;
        self.nlmsg.serialize(mem)?;
        Ok(())
    }

    fn deserialize<B>(mem: &mut StreamReadBuffer<B>) -> Result<Self, DeError> where B: AsRef<[u8]> {
        Ok(Nlmsgerr {
            error: libc::c_int::deserialize(mem)?,
            nlmsg: Nlmsghdr::<T, NlEmpty>::deserialize(mem)?,
        })
    }

    fn size(&self) -> usize {
        mem::size_of::<libc::c_int>() + self.nlmsg.size()
    }
}

/// Netlink protocol error
#[derive(Debug)]
pub enum NlError {
    /// Type indicating a message from a converted error
    Msg(String),
    /// No ack was received when `NlmF::Ack` was specified in the request
    NoAck,
}

try_err_compat!(NlError, io::Error, SerError, DeError);

impl NlError {
    /// Create new error from `&str`
    pub fn new(s: &str) -> Self {
        NlError::Msg(s.to_string())
    }
}

/// Netlink protocol error
impl Display for NlError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let msg = match *self {
            NlError::Msg(ref msg) => msg,
            NlError::NoAck => "No ack received",
        };
        write!(f, "{}", msg)
    }
}

impl Error for NlError {
    fn description(&self) -> &str {
        match *self {
            NlError::Msg(ref msg) => msg.as_str(),
            NlError::NoAck => "No ack received",
        }
    }
}

/// Serialization error
#[derive(Debug)]
pub struct SerError(String);

impl SerError {
    /// Create a new error with the given message as description
    pub fn new<T: ToString>(msg: T) -> Self {
        SerError(msg.to_string())
    }
}

try_err_compat!(SerError, io::Error);

impl Display for SerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Error for SerError {
    fn description(&self) -> &str {
        self.0.as_str()
    }
}

/// Deserialization error
#[derive(Debug)]
pub struct DeError(String);

impl DeError {
    /// Create new error from `&str`
    pub fn new(s: &str) -> Self {
        DeError(s.to_string())
    }
}

try_err_compat!(DeError, io::Error, str::Utf8Error, string::FromUtf8Error,
                std::ffi::FromBytesWithNulError);

impl Display for DeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Error for DeError {
    fn description(&self) -> &str {
        self.0.as_str()
    }
}
