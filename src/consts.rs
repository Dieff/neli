//! # High level notes
//!
//! The items in this module are created by macros, which give them the traits necessary to be
//! serialized into Netlink compatible types. The macros are exported - you can use them too!
//! See `impl_var`, `impl_trait`, and `impl_var_trait`.
//!
//! Note that most of these constants come from the Linux kernel headers, which can be found
//! in `/usr/include/linux` on many distros. You can also see `man 3 netlink`, `man 7 netlink`,
//! and `man 7 rtnetlink` for more information.
//!
//! # Design decisions
//!
//! * Macros are exported so that these conventions are extensible and usable for data types
//!   implemented by the user in the case of new netlink families (which is supported by the
//!   protocol). In this case, there is no way in which I can support every custom netlink family
//!   but my aim is to make this library as flexible as possible so that it is painless to hook
//!   your custom netlink data type into the existing library support.
//! * Enums are used so that:
//!   * Values can be checked based on a finite number of inputs as opposed to the range of
//!     whatever integer data type C defines as the struct member type. This hopefully makes it
//!     easier to catch garbage responses and corruption when an invalid netlink message is sent to
//!     the kernel.
//!   * Only the enum or an enum implementing a marker trait in the case of generics can be used
//!     in the appropriate places when constructing netlink messages. This takes guess work out
//!     of which constants can be used where. Netlink documentation is not always complete
//!     and sometimes takes a bit of trial and error actually sending messages to the kernel
//!     to figure out if you are using the correct constants. This setup should let you know at
//!     compile time if you are doing something you should not be doing.
//! * `UnrecognizedVariant` is included in each enum because completeness cannot be guaranteed for
//!   every constant for every protocol. This allows you to inspect the integer value returned
//!   and if you are sure that it is correct, you can use it. If it is a garbage value, this can
//!   also be useful for error reporting.

use buffering::copy::{StreamReadBuffer, StreamWriteBuffer};
use libc;
use std::mem;

use err::{DeError, SerError};
use Nl;

// This is to facillitate the two different ways to call
// `impl_var`: one with doc comments and one without.
#[macro_export]
#[doc(hidden)]
macro_rules! impl_var_base {
    ($name:ident, $ty:ty, $var_def:ident => $val_def:expr,
      $( $var:ident => $val:expr ),* ) => {

        impl From<$ty> for $name {
            fn from(v: $ty) -> Self {
                match v {
                    i if i == $val_def => $name::$var_def,
                    $( i if i == $val => $name::$var, )*
                    i => $name::UnrecognizedVariant(i)
                }
            }
        }

        impl From<$name> for $ty {
            fn from(v: $name) -> Self {
                match v {
                    $name::$var_def => $val_def,
                    $( $name::$var => $val, )*
                    $name::UnrecognizedVariant(i) => i,
                }
            }
        }

        impl<'a> From<&'a $name> for $ty {
            fn from(v: &'a $name) -> Self {
                match *v {
                    $name::$var_def => $val_def,
                    $( $name::$var => $val, )*
                    $name::UnrecognizedVariant(i) => i,
                }
            }
        }

        impl Nl for $name {
            type SerIn = ();
            type DeIn = ();

            fn serialize(&self, mem: &mut StreamWriteBuffer) -> Result<(), SerError> {
                let v: $ty = self.clone().into();
                v.serialize(mem)
            }

            fn deserialize<T>(mem: &mut StreamReadBuffer<T>) -> Result<Self, DeError>
                    where T: AsRef<[u8]> {
                let v: $ty = Nl::deserialize(mem)?;
                Ok(v.into())
            }

            fn size(&self) -> usize {
                mem::size_of::<$ty>()
            }
        }
    };
}

#[macro_export]
/// For naming a new enum, passing in what type it serializes to and deserializes
/// from, and providing a mapping from variants to expressions (such as libc consts) that
/// will ultimately be used in the serialization/deserialization step when sending the netlink
/// message over the wire.
///
/// # Usage
///  Create an `enum` named "MyNetlinkProtoAttrs" that can be serialized into `u16`s to use with Netlink.
///  Possibly represents the fields on a message you recieved from Netlink.
///  ```ignore
///  impl_var!(MyNetlinkProtoAttrs, u16,
///     Id => 16 as u16,
///     Name => 17 as u16,
///     Size => 18 as u16
///  );
/// ```
/// Or, with doc comments (if you're developing a library)
/// ```ignore
///  impl_var!(
///     /// These are the attributes returned
///     /// by a fake netlink protocol.
///     ( MyNetlinkProtoAttrs, u16,
///     Id => 16 as u16,
///     Name => 17 as u16,
///     Size => 18 as u16 )
///  );
/// ```
///
macro_rules! impl_var {
    (  $(#[$outer:meta])*
      ($name:ident, $ty:ty, $var_def:ident => $val_def:expr,
      $( $var:ident => $val:expr ),* )) => ( // with comments
        $(#[$outer])*
        #[derive(Clone,Debug,Eq,PartialEq)]
        pub enum $name {
            #[allow(missing_docs)]
            $var_def,
            $(
                #[allow(missing_docs)]
                $var,
            )*
            /// Variant that signifies an invalid value while deserializing
            UnrecognizedVariant($ty),
        }

        impl_var_base!($name, $ty, $var_def => $val_def,
            $( $var => $val),*
        );
    );
    ($name:ident, $ty:ty, $var_def:ident => $val_def:expr,
      $( $var:ident => $val:expr ),* ) => ( // without comments
        #[allow(missing_docs)]
        #[derive(Clone,Debug,Eq,PartialEq)]
        pub enum $name {
            #[allow(missing_docs)]
            $var_def,
            $(
                #[allow(missing_docs)]
                $var,
            )*
            /// Variant that signifies an invalid value while deserializing
            UnrecognizedVariant($ty),
        }

        impl_var_base!($name, $ty, $var_def => $val_def,
            $( $var => $val),*
        );
    );
}

#[macro_export]
/// For flagging a new enum as usable in a field that is a generic type.
/// This way, the type can be constrained when the impl is provided to only accept enums that
/// implement the marker trait that corresponds to the given marker trait. The current
/// convention is to use `impl_trait` to create the trait with the name of the field that
/// is the generic type and then use `impl_var_trait` to flag the new enum as usable in
/// this field. See the examples below for more details.
macro_rules! impl_trait {
    ( $(#[$outer:meta])*
    ( $trait_name:ident, $to_from_ty:ty )) => { // with comments
        $(#[$outer])*
        pub trait $trait_name: Nl + From<$to_from_ty> + Into<$to_from_ty> {}
    };
    ( $trait_name:ident, $to_from_ty:ty ) => { // without comments
        #[allow(missing_docs)]
        pub trait $trait_name: Nl + From<$to_from_ty> + Into<$to_from_ty> {}
    };
}

#[macro_export]
/// For implementing a marker trait with the appropriate trait constraints
/// on the newly implemented trait. It accepts a name and a type for serialization and
/// deserialization conversions.
macro_rules! impl_var_trait {
    ( $(#[$outer:meta])*
    ( $name:ident, $ty:ty, $impl_name:ident, $var_def:ident => $val_def:expr,
      $( $var:ident => $val:expr ),* )) => ( // with comments
        impl_var!( $(#[$outer])*
            ($name, $ty, $var_def => $val_def, $( $var => $val ),* )
        );

        impl $impl_name for $name {}
    );
    ( $name:ident, $ty:ty, $impl_name:ident, $var_def:ident => $val_def:expr,
      $( $var:ident => $val:expr ),* ) => ( // without comments
        impl_var!($name, $ty, $var_def => $val_def, $( $var => $val ),* );

        impl $impl_name for $name {}
    );
}

/// Reimplementation of alignto macro in C
pub fn alignto(len: usize) -> usize {
    (len + libc::NLA_ALIGNTO as usize - 1) & !(libc::NLA_ALIGNTO as usize - 1)
}

impl_var!(
    /// Address families 
    ( Af, libc::c_uchar,
    Inet => libc::AF_INET as libc::c_uchar,
    Inet6 => libc::AF_INET6 as libc::c_uchar )
);

impl_var!(
    /// Address families for sockets
    (  AddrFamily, libc::c_int,
    UnixOrLocal => libc::AF_UNIX,
    Inet => libc::AF_INET,
    Inet6 => libc::AF_INET6,
    Ipx => libc::AF_IPX,
    Netlink => libc::AF_NETLINK,
    X25 => libc::AF_X25,
    Ax25 => libc::AF_AX25,
    Atmpvc => libc::AF_ATMPVC,
    Appletalk => libc::AF_APPLETALK,
    Packet => libc::AF_PACKET,
    Alg => libc::AF_ALG )
);

impl_var!(
    /// Interface address flags.
    ( IfaF, u32,
    Secondary => libc::IFA_F_SECONDARY,
    Temporary => libc::IFA_F_TEMPORARY,
    Nodad => libc::IFA_F_NODAD,
    Optimistic => libc::IFA_F_OPTIMISTIC,
    Dadfailed => libc::IFA_F_DADFAILED,
    Homeaddress => libc::IFA_F_HOMEADDRESS,
    Deprecated => libc::IFA_F_DEPRECATED,
    Tentative => libc::IFA_F_TENTATIVE,
    Permanent => libc::IFA_F_PERMANENT,
    Managetempaddr => libc::IFA_F_MANAGETEMPADDR,
    Noprefixroute => libc::IFA_F_NOPREFIXROUTE,
    Mcautojoin => libc::IFA_F_MCAUTOJOIN,
    StablePrivacy => libc::IFA_F_STABLE_PRIVACY )
);

impl_var!(
    /// `rtm_type`
    /// The results of a lookup from a route table
    ( Rtn, libc::c_uchar,
    Unspec => libc::RTN_UNSPEC,
    Unicast => libc::RTN_UNICAST,
    Local => libc::RTN_LOCAL,
    Broadcast => libc::RTN_BROADCAST,
    Anycast => libc::RTN_ANYCAST,
    Multicast => libc::RTN_MULTICAST,
    Blackhole => libc::RTN_BLACKHOLE,
    Unreachable => libc::RTN_UNREACHABLE,
    Prohibit => libc::RTN_PROHIBIT,
    Throw => libc::RTN_THROW,
    Nat => libc::RTN_NAT,
    Xresolve => libc::RTN_XRESOLVE)
);

impl_var!(
    /// `rtm_protocol`
    /// The origins of routes that are defined in the kernel
    ( Rtprot, libc::c_uchar,
    Unspec => libc::RTPROT_UNSPEC,
    Redirect => libc::RTPROT_REDIRECT,
    Kernel => libc::RTPROT_KERNEL,
    Boot => libc::RTPROT_BOOT,
    Static => libc::RTPROT_STATIC )
);

impl_var!(
    /// `rtm_scope`
    /// The distance between destinations
    ( RtScope, libc::c_uchar,
    Universe => libc::RT_SCOPE_UNIVERSE,
    Site => libc::RT_SCOPE_SITE,
    Link => libc::RT_SCOPE_LINK,
    Host => libc::RT_SCOPE_HOST,
    Nowhere => libc::RT_SCOPE_NOWHERE )
);

impl_var!(
    /// `rt_class_t`
    /// Reserved route table identifiers
    ( RtTable, libc::c_uchar,
    Unspec => libc::RT_TABLE_UNSPEC,
    Compat => libc::RT_TABLE_COMPAT,
    Default => libc::RT_TABLE_DEFAULT,
    Main => libc::RT_TABLE_MAIN,
    Local => libc::RT_TABLE_LOCAL )
);

impl_var!(
    /// `rtm_flags`
    /// Flags for rnetlink messages
    ( RtmF, libc::c_uint,
    Notify => libc::RTM_F_NOTIFY,
    Cloned => libc::RTM_F_CLONED,
    Equalize => libc::RTM_F_EQUALIZE,
    Prefix => libc::RTM_F_PREFIX,
    LookupTable => libc::RTM_F_LOOKUP_TABLE,
    FibMatch => libc::RTM_F_FIB_MATCH )
);

impl_var!(
    /// Arp neighbor cache entry states
    ( Nud, u16,
    None => libc::NUD_NONE,
    Incomplete => libc::NUD_INCOMPLETE,
    Reachable => libc::NUD_REACHABLE,
    Stale => libc::NUD_STALE,
    Delay => libc::NUD_DELAY,
    Probe => libc::NUD_PROBE,
    Failed => libc::NUD_FAILED,
    Noarp => libc::NUD_NOARP,
    Permanent => libc::NUD_PERMANENT )
);

impl_var!(
    /// Arg neighbor cache entry flags
    ( Ntf, u8,
    Use => libc::NTF_USE,
    Self_ => libc::NTF_SELF,
    Master => libc::NTF_MASTER,
    Proxy => libc::NTF_PROXY,
    ExtLearned => libc::NTF_EXT_LEARNED,
    Offloaded => libc::NTF_OFFLOADED,
    Router => libc::NTF_ROUTER )
);

impl_trait!(/// Marker trait for `RtAttr.rta_type` field
(RtaType, libc::c_ushort));

impl_var_trait!(
    /// Enum for use with `RtAttr.rta_type`
    ( Ifla, libc::c_ushort, RtaType,
    Unspec => libc::IFLA_UNSPEC,
    Address => libc::IFLA_ADDRESS,
    Broadcast => libc::IFLA_BROADCAST,
    Ifname => libc::IFLA_IFNAME,
    Mtu => libc::IFLA_MTU,
    Link => libc::IFLA_LINK,
    Qdisc => libc::IFLA_QDISC,
    Stats => libc::IFLA_STATS )
);

impl_var_trait!(
    /// Enum for use with `RtAttr.rta_type`
    ( Ifa, libc::c_ushort, RtaType,
    Unspec => libc::IFA_UNSPEC,
    Address => libc::IFA_ADDRESS,
    Local => libc::IFA_LOCAL,
    Label => libc::IFA_LABEL,
    Broadcast => libc::IFA_BROADCAST,
    Anycast => libc::IFA_ANYCAST,
    Cacheinfo => libc::IFA_CACHEINFO,
    Multicast => libc::IFA_MULTICAST,
    Flags => libc::IFA_FLAGS )
);

impl_var_trait!(
    /// Enum for use with `RtAttr.rta_type`. 
    /// Values are routing message attributes
    ( Rta, libc::c_ushort, RtaType,
    Unspec => libc::RTA_UNSPEC,
    Dst => libc::RTA_DST,
    Src => libc::RTA_SRC,
    Iif => libc::RTA_IIF,
    Oif => libc::RTA_OIF,
    Gateway => libc::RTA_GATEWAY,
    Priority => libc::RTA_PRIORITY,
    Prefsrc => libc::RTA_PREFSRC,
    Metrics => libc::RTA_METRICS,
    Multipath => libc::RTA_MULTIPATH,
    Protoinfo => libc::RTA_PROTOINFO, // no longer used in Linux
    Flow => libc::RTA_FLOW,
    Cacheinfo => libc::RTA_CACHEINFO,
    Session => libc::RTA_SESSION, // no longer used in Linux
    MpAlgo => libc::RTA_MP_ALGO, // no longer used in Linux
    Table => libc::RTA_TABLE,
    Mark => libc::RTA_MARK,
    MfcStats => libc::RTA_MFC_STATS,
    Via => libc::RTA_VIA,
    Newdst => libc::RTA_NEWDST,
    Pref => libc::RTA_PREF,
    EncapType => libc::RTA_ENCAP_TYPE,
    Encap => libc::RTA_ENCAP,
    Expires => libc::RTA_EXPIRES,
    Pad => libc::RTA_PAD,
    Uid => libc::RTA_UID,
    TtlPropagate => libc::RTA_TTL_PROPAGATE)
);

impl_var!(
    /// Interface types
    ( Arphrd, libc::c_ushort,
    Netrom => libc::ARPHRD_NETROM,
    Ether => libc::ARPHRD_ETHER,
    Eether => libc::ARPHRD_EETHER,
    AX25 => libc::ARPHRD_AX25,
    Pronet => libc::ARPHRD_PRONET,
    Chaos => libc::ARPHRD_CHAOS,
    Ieee802 => libc::ARPHRD_IEEE802,
    Arcnet => libc::ARPHRD_ARCNET,
    Appletlk => libc::ARPHRD_APPLETLK,
    Dlci => libc::ARPHRD_DLCI,
    Atm => libc::ARPHRD_APPLETLK,
    Metricom => libc::ARPHRD_METRICOM,
    Ieee1394 => libc::ARPHRD_IEEE1394,
    Eui64 => libc::ARPHRD_EUI64,
    Infiniband => libc::ARPHRD_INFINIBAND,

    // Possibly more types here - need to look into ARP more

    Void => libc::ARPHRD_VOID,
    None => libc::ARPHRD_NONE
));

impl_var!(
    /// Values for `ifi_flags` in `rtnl.rs`
    ( Iff, libc::c_uint,
    Up => libc::IFF_UP as libc::c_uint,
    Broadcast => libc::IFF_BROADCAST as libc::c_uint,
    Debug => libc::IFF_DEBUG as libc::c_uint,
    Loopback => libc::IFF_LOOPBACK as libc::c_uint,
    Pointopoint => libc::IFF_POINTOPOINT as libc::c_uint,
    Running => libc::IFF_RUNNING as libc::c_uint,
    Noarp => libc::IFF_NOARP as libc::c_uint,
    Promisc => libc::IFF_PROMISC as libc::c_uint,
    Notrailers => libc::IFF_NOTRAILERS as libc::c_uint,
    Allmulti => libc::IFF_ALLMULTI as libc::c_uint,
    Master => libc::IFF_MASTER as libc::c_uint,
    Slave => libc::IFF_SLAVE as libc::c_uint,
    Multicast => libc::IFF_MULTICAST as libc::c_uint,
    Portsel => libc::IFF_PORTSEL as libc::c_uint,
    Automedia => libc::IFF_AUTOMEDIA as libc::c_uint,
    Dynamic => libc::IFF_DYNAMIC as libc::c_uint,
    LowerUp => libc::IFF_LOWER_UP as libc::c_uint,
    Dormant => libc::IFF_DORMANT as libc::c_uint,
    Echo => libc::IFF_ECHO as libc::c_uint

    // Possibly more types here - need to look into private flags for interfaces
    )
);

impl_var!(
    /// Values for `nl_family` in `NlSocket`
    ( NlFamily, libc::c_int,
    Route => libc::NETLINK_ROUTE,
    Unused => libc::NETLINK_UNUSED,
    Usersock => libc::NETLINK_USERSOCK,
    Firewall => libc::NETLINK_FIREWALL,
    SockOrInetDiag => libc::NETLINK_SOCK_DIAG,
    Nflog => libc::NETLINK_NFLOG,
    Xfrm => libc::NETLINK_XFRM,
    Selinux => libc::NETLINK_SELINUX,
    Iscsi => libc::NETLINK_ISCSI,
    Audit => libc::NETLINK_AUDIT,
    FibLookup => libc::NETLINK_FIB_LOOKUP,
    Connector => libc::NETLINK_CONNECTOR,
    Netfilter => libc::NETLINK_NETFILTER,
    Ip6Fw => libc::NETLINK_IP6_FW,
    Dnrtmsg => libc::NETLINK_DNRTMSG,
    KobjectUevent => libc::NETLINK_KOBJECT_UEVENT,
    Generic => libc::NETLINK_GENERIC,
    Scsitransport => libc::NETLINK_SCSITRANSPORT,
    Ecryptfs => libc::NETLINK_ECRYPTFS,
    Rdma => libc::NETLINK_RDMA,
    Crypto => libc::NETLINK_CRYPTO )
);

impl_trait!(
    /// Trait marking constants valid for use in `Nlmsghdr.nl_type`
    (NlType, u16)
);

impl_var_trait!(
    /// Values for `nl_type` in `Nlmsghdr`
    ( Nlmsg, u16, NlType,
    Noop => libc::NLMSG_NOOP as u16,
    Error => libc::NLMSG_ERROR as u16,
    Done => libc::NLMSG_DONE as u16,
    Overrun => libc::NLMSG_OVERRUN as u16)
);

impl_var_trait!(
    /// Values for `nl_type` in `Nlmsghdr`
    ( GenlId, u16, NlType,
    Ctrl => libc::GENL_ID_CTRL as u16,
    VfsDquot => libc::GENL_ID_VFS_DQUOT as u16,
    Pmcraid => libc::GENL_ID_PMCRAID as u16 )
);

impl_var!(
    /// Values for `nl_flags` in `NlHdr`
    ( NlmF, u16,
    Request => libc::NLM_F_REQUEST as u16,
    Multi => libc::NLM_F_MULTI as u16,
    Ack => libc::NLM_F_ACK as u16,
    Echo => libc::NLM_F_ECHO as u16,
    DumpIntr => libc::NLM_F_DUMP_INTR as u16,
    DumpFiltered => libc::NLM_F_DUMP_FILTERED as u16,
    Root => libc::NLM_F_ROOT as u16,
    Match => libc::NLM_F_MATCH as u16,
    Atomic => libc::NLM_F_ATOMIC as u16,
    Dump => libc::NLM_F_DUMP as u16,
    Replace => libc::NLM_F_REPLACE as u16,
    Excl => libc::NLM_F_EXCL as u16,
    Create => libc::NLM_F_CREATE as u16,
    Append => libc::NLM_F_APPEND as u16 )
);

impl_var!(
    /// Values for `cmd` in `GenlHdr`
    ( CtrlCmd, u8,
    Unspec => libc::CTRL_CMD_UNSPEC as u8,
    Newfamily => libc::CTRL_CMD_NEWFAMILY as u8,
    Delfamily => libc::CTRL_CMD_DELFAMILY as u8,
    Getfamily => libc::CTRL_CMD_GETFAMILY as u8,
    Newops => libc::CTRL_CMD_NEWOPS as u8,
    Delops => libc::CTRL_CMD_DELOPS as u8,
    Getops => libc::CTRL_CMD_GETOPS as u8,
    NewmcastGrp => libc::CTRL_CMD_NEWMCAST_GRP as u8,
    DelmcastGrp => libc::CTRL_CMD_DELMCAST_GRP as u8,
    GetmcastGrp => libc::CTRL_CMD_GETMCAST_GRP as u8 )
);

impl_var!(
    /// Values for `nla_type` in `NlaAttrHdr`
    ( CtrlAttr, u16,
    Unspec => libc::CTRL_ATTR_UNSPEC as u16,
    FamilyId => libc::CTRL_ATTR_FAMILY_ID as u16,
    FamilyName => libc::CTRL_ATTR_FAMILY_NAME as u16,
    Version => libc::CTRL_ATTR_VERSION as u16,
    Hdrsize => libc::CTRL_ATTR_HDRSIZE as u16,
    Maxattr => libc::CTRL_ATTR_MAXATTR as u16,
    Ops => libc::CTRL_ATTR_OPS as u16,
    McastGroups => libc::CTRL_ATTR_MCAST_GROUPS as u16 )
);

impl_var!(
    /// Values for `nla_type` in `NlaAttrHdr`
    ( CtrlAttrMcastGrp, u16,
    Unspec => libc::CTRL_ATTR_MCAST_GRP_UNSPEC as u16,
    Name => libc::CTRL_ATTR_MCAST_GRP_NAME as u16,
    Id => libc::CTRL_ATTR_MCAST_GRP_ID as u16 )
);
