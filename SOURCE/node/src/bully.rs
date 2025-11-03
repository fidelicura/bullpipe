use crate::led::Mode;
use crate::sync::Lock;
use core::sync::atomic::AtomicBool;
use embassy_time::Duration;

pub const INITIALIZATION_TIMEOUT: Duration = Duration::from_millis(2000);
pub const HEARTBEAT_INTERVAL: Duration = Duration::from_millis(500);
pub const LEADER_TIMEOUT: Duration = Duration::from_millis(2000);
pub const CANDIDATE_WAIT: Duration = Duration::from_millis(500);

/// # DESCRIPTION
///
/// TODO.
pub static LEADER_ALIVE: AtomicBool = AtomicBool::new(false);

//////////////////////////////////////////////////////////////////////
// Id
//////////////////////////////////////////////////////////////////////

/// # DESCRIPTION
///
/// TODO.
pub static ID: Lock<Id> = Lock::new(Id::UNKNOWN);

/// # DESCRIPTION
///
/// TODO.
#[repr(transparent)]
#[derive(Clone, PartialEq, PartialOrd, Eq, Ord)]
pub struct Id([u8; 6]);

impl Id {
    /// # DESCRIPTION
    ///
    /// TODO.
    pub const BROADCAST: Self = Self(esp_radio::esp_now::BROADCAST_ADDRESS);

    /// # DESCRIPTION
    ///
    /// TODO.
    pub const UNKNOWN: Self = Self([0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);

    /// # DESCRIPTION
    ///
    /// TODO.
    pub async fn my<'a>() -> Self {
        ID.read().await.clone()
    }
}

impl From<[u8; 6]> for Id {
    fn from(value: [u8; 6]) -> Self {
        Self(value)
    }
}

impl From<Id> for [u8; 6] {
    fn from(value: Id) -> Self {
        value.0
    }
}

impl core::fmt::Display for Id {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let array = &self.0;
        write!(
            f,
            "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            array[0], array[1], array[2], array[3], array[4], array[5]
        )
    }
}

//////////////////////////////////////////////////////////////////////
// State
//////////////////////////////////////////////////////////////////////

/// # DESCRIPTION
///
/// TODO.
pub static STATE: Lock<State> = Lock::new(State::DEFAULT);

/// # DESCRIPTION
///
/// TODO.
pub struct State {
    pub role: Role,
}

impl State {
    pub const DEFAULT: Self = Self {
        role: Role::Initializing,
    };
}

//////////////////////////////////////////////////////////////////////
// Role
//////////////////////////////////////////////////////////////////////

#[derive(Eq, PartialEq)]
pub enum Role {
    /// TODO.
    Initializing,

    /// TODO.
    Follower { leader: Id },

    /// TODO.
    Candidate,

    /// TODO.
    Leader,
}

impl<'a> From<&'a Role> for Mode {
    fn from(value: &'a Role) -> Self {
        match value {
            Role::Initializing => Mode::SteadyOff,
            Role::Follower { .. } => Mode::SlowBlink,
            Role::Candidate => Mode::QuickBlink,
            Role::Leader => Mode::SteadyOn,
        }
    }
}

impl core::fmt::Display for Role {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Initializing => write!(f, "initializing"),
            Self::Follower { leader } => write!(f, "follower({leader})"),
            Self::Candidate => write!(f, "candidate"),
            Self::Leader => write!(f, "leader"),
        }
    }
}
