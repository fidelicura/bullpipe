use crate::bully::Id;
use crate::bully::LEADER_ALIVE;
use crate::bully::Role;
use crate::bully::STATE;
use crate::sync::Lock;
use core::cmp::Ordering;
use core::sync::atomic;
use esp_radio::esp_now::EspNowSender;
use serde::Deserialize;
use serde::Serialize;

// TODO.
include!(concat!(env!("OUT_DIR"), "/config.rs"));

/// # DESCRIPTION
///
/// TODO.
pub const PACKET_SIZE: usize = 1024;

//////////////////////////////////////////////////////////////////////
// Wire
//////////////////////////////////////////////////////////////////////

/// # DESCRIPTION
///
/// TODO.
pub trait Wire
where
    Self: Serialize + for<'a> Deserialize<'a>,
    Message: From<Self>,
{
    /// # DESCRIPTION
    ///
    /// TODO.
    fn specification(&self) -> &str;

    /// # DESCRIPTION
    ///
    /// TODO.
    async fn handle(&self, controller: &'static Lock<EspNowSender<'static>>, departure: &Id);

    /// # DESCRIPTION
    ///
    /// TODO.
    // NOTE: peers to which we send must already be known by controller.
    async fn throw(self, controller: &'static Lock<EspNowSender<'static>>) {
        let message = Message::from(self);
        let destination = esp_radio::esp_now::BROADCAST_ADDRESS;

        let mut buffer = [0; esp_radio::esp_now::ESP_NOW_MAX_DATA_LEN];
        let payload = postcard::to_slice(&message, &mut buffer).expect("payload should fit");

        let mut controller = controller.write().await;
        controller.send_async(&destination, &payload).await.expect(
            "esp now should be initialized, all peers known and work on the same wifi channel",
        );
    }
}

//////////////////////////////////////////////////////////////////////
// Message
//////////////////////////////////////////////////////////////////////

/// # DESCRIPTION
///
/// TODO.
#[derive(Serialize, Deserialize)]
pub enum Message {
    Request(Request),
    Response(Response),
}

impl Wire for Message {
    fn specification(&self) -> &str {
        match self {
            Self::Request(request) => request.specification(),
            Self::Response(response) => response.specification(),
        }
    }

    async fn handle(&self, controller: &'static Lock<EspNowSender<'static>>, departure: &Id) {
        log::info!("handling wrapped {} from {departure}", self.specification());

        match self {
            Self::Request(request) => request.handle(controller, departure).await,
            Self::Response(response) => response.handle(controller, departure).await,
        }
    }
}

//////////////////////////////////////////////////////////////////////
// Request
//////////////////////////////////////////////////////////////////////

#[derive(Serialize, Deserialize)]
pub enum Request {
    /// TODO.
    Heartbeat,

    /// TODO.
    Election,

    /// TODO.
    Victory,
}

impl From<Request> for Message {
    fn from(value: Request) -> Self {
        Message::Request(value)
    }
}

impl Wire for Request {
    fn specification(&self) -> &str {
        match self {
            Self::Heartbeat => "heartbeat request",
            Self::Election => "election request",
            Self::Victory => "victory request",
        }
    }

    async fn handle(&self, controller: &'static Lock<EspNowSender<'static>>, departure: &Id) {
        log::info!("handling direct {} from {departure}", self.specification());

        match self {
            Self::Heartbeat => match Id::my().await.cmp(departure) {
                Ordering::Greater => {
                    Request::Election.throw(controller).await;

                    let mut state = STATE.write().await;

                    let is_leader = state.role == Role::Leader;
                    if !is_leader {
                        state.role = Role::Candidate;
                    }
                }
                Ordering::Equal => unreachable!("mac addresses should not be equal"),
                Ordering::Less => {
                    let mut state = STATE.write().await;

                    let leader = departure.clone();
                    state.role = Role::Follower { leader };

                    drop(state);

                    LEADER_ALIVE.store(true, atomic::Ordering::Relaxed);
                }
            },
            Self::Election => match Id::my().await.cmp(departure) {
                Ordering::Greater => Response::Election.throw(controller).await,
                Ordering::Equal => unreachable!("mac addresses should not be equal"),
                Ordering::Less => {}
            },
            Self::Victory => match Id::my().await.cmp(departure) {
                Ordering::Greater => Response::Election.throw(controller).await,
                Ordering::Equal => unreachable!("mac addresses should not be equal"),
                Ordering::Less => {
                    let mut state = STATE.write().await;

                    let leader = departure.clone();
                    state.role = Role::Follower { leader };

                    LEADER_ALIVE.store(true, atomic::Ordering::Relaxed);
                }
            },
        }
    }
}

//////////////////////////////////////////////////////////////////////
// Response
//////////////////////////////////////////////////////////////////////

#[derive(Serialize, Deserialize)]
pub enum Response {
    /// TODO.
    Heartbeat,

    /// TODO.
    Election,

    /// TODO.
    Victory,
}

impl From<Response> for Message {
    fn from(value: Response) -> Self {
        Message::Response(value)
    }
}

impl Wire for Response {
    fn specification(&self) -> &str {
        match self {
            Self::Heartbeat => "heartbeat response",
            Self::Election => "election response",
            Self::Victory => "victory response",
        }
    }

    async fn handle(&self, controller: &'static Lock<EspNowSender<'static>>, departure: &Id) {
        _ = controller; // NOTE: not used as not needed.

        log::info!("handling direct {} from {departure}", self.specification());

        match self {
            Self::Heartbeat => log::info!("ignoring heartbeat response"),
            Self::Election => match Id::my().await.cmp(departure) {
                Ordering::Less => {
                    let mut state = STATE.write().await;

                    let is_candidate = state.role == Role::Candidate;
                    if is_candidate {
                        state.role = Role::Initializing;
                    }
                }
                Ordering::Equal => unreachable!("mac addresses should not be equal"),
                Ordering::Greater => log::warn!("ignoring veto from bulliable node"),
            },
            Self::Victory => log::info!("ignoring victory response"),
        }
    }
}
