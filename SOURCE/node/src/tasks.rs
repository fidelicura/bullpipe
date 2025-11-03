use crate::bully::CANDIDATE_WAIT;
use crate::bully::HEARTBEAT_INTERVAL;
use crate::bully::ID;
use crate::bully::INITIALIZATION_TIMEOUT;
use crate::bully::Id;
use crate::bully::LEADER_ALIVE;
use crate::bully::LEADER_TIMEOUT;
use crate::bully::Role;
use crate::bully::STATE;
use crate::led::Mode;
use crate::sync::Lock;
use crate::sync::Mutex;
use crate::wire::Message;
use crate::wire::Request;
use crate::wire::Wire;
use core::sync::atomic::Ordering;
use embassy_futures::select::Either;
use embassy_futures::select::select;
use embassy_sync::channel::Receiver;
use embassy_sync::channel::Sender;
use embassy_time::Duration;
use embassy_time::Instant;
use embassy_time::Timer;
use esp_radio::esp_now::EspNowManager;
use esp_radio::esp_now::EspNowReceiver;
use esp_radio::esp_now::EspNowSender;
use esp_radio::esp_now::EspNowWifiInterface;
use esp_radio::esp_now::PeerInfo;
use heapless::String;

//////////////////////////////////////////////////////////////////////
// Core
//////////////////////////////////////////////////////////////////////

/// # DESCRIPTION
///
/// TODO.
#[embassy_executor::task(pool_size = 1)]
pub async fn hermes(
    receiver: &'static Lock<EspNowReceiver<'static>>,
    manager: &'static EspNowManager<'static>,
    queue: &'static Sender<'static, Mutex, (Message, Id), 16>,
) {
    log::info!("hermes task started");

    loop {
        let packet = {
            let mut receiver = receiver.write().await;
            receiver.receive_async().await
        };
        let destination = packet.info.src_address;

        if !manager.peer_exists(&destination) {
            let peer = PeerInfo {
                interface: EspNowWifiInterface::Sta,
                peer_address: destination.clone(),
                lmk: None,
                channel: None,
                encrypt: false,
            };

            manager
                .add_peer(peer)
                .expect("should not be overfilled or a duplicate peer");
        }

        let payload = packet.data();
        match postcard::from_bytes::<Message>(payload) {
            Ok(message) => {
                let id = Id::from(packet.info.src_address);
                queue.send((message, id)).await;
            }
            Err(error) => {
                log::warn!("skipping invalid packet: {:?}, reason: {}", packet, error);
                continue;
            }
        }
    }
}

/// # DESCRIPTION
///
/// TODO.
#[embassy_executor::task(pool_size = 1)]
pub async fn governor(
    generator: &'static esp_hal::rng::Rng,
    controller: &'static Lock<EspNowSender<'static>>,
    queue: &'static Receiver<'static, Mutex, (Message, Id), 16>,
) {
    log::info!("governor task started");

    let mut last_valid_heartbeat = Instant::now();

    loop {
        let (role, timeout) = {
            let state = STATE.read().await;
            match &state.role {
                // EXPLANATION: just woke up, so listen for a second..
                Role::Initializing => (Role::Initializing, INITIALIZATION_TIMEOUT),

                // EXPLANATION: send heartbeat as soon as possible.
                Role::Leader => (Role::Leader, HEARTBEAT_INTERVAL),

                // EXPLANATION: wait for a leader to die.
                Role::Follower { .. } => (
                    // NOTE: this is dummy state,
                    // used only for matching further.
                    Role::Follower {
                        leader: Id::UNKNOWN,
                    },
                    LEADER_TIMEOUT,
                ),

                // EXPLANATION: wait short random time for veto.
                Role::Candidate => {
                    let jitter = (generator.random() as u64 % 20) * 10;
                    (
                        Role::Candidate,
                        CANDIDATE_WAIT + Duration::from_millis(jitter),
                    )
                }
            }
        };

        if LEADER_ALIVE.load(Ordering::Relaxed) {
            LEADER_ALIVE.store(false, Ordering::Relaxed);

            last_valid_heartbeat = Instant::now();
        }

        let event = select(Timer::after(timeout), queue.receive()).await;
        match event {
            // CASE: message received.
            Either::Second((message, departure)) => {
                message.handle(controller, &departure).await;
            }

            // CASE: timer expired.
            Either::First(_) => {
                match role {
                    // EXPLANATION: no one stopped us, try to run.
                    Role::Initializing => {
                        let mut state = STATE.write().await;
                        state.role = Role::Candidate;
                    }

                    // EXPLANATION: check for non-hardware watchdog.
                    Role::Follower { .. } => {
                        if last_valid_heartbeat.elapsed() > LEADER_TIMEOUT {
                            log::warn!("king is dead, performing revolution");

                            let mut state = STATE.write().await;
                            state.role = Role::Candidate;
                        }
                    }

                    // EXPLNATION: no veto received after waiting, we won.
                    Role::Candidate => {
                        let mut state = STATE.write().await;

                        let still_candidate = state.role == Role::Candidate;
                        if still_candidate {
                            log::info!("complete victory, stepping up as leader");

                            state.role = Role::Leader;
                            drop(state);

                            Request::Victory.throw(controller).await;
                        }
                    }

                    // EXPLANATION: send Heartbeat as a valid leader.
                    Role::Leader => {
                        let state = STATE.read().await;

                        let still_leader = state.role == Role::Leader;
                        if still_leader {
                            log::info!("broadcasting heartbeat as a leader");

                            drop(state);

                            Request::Heartbeat.throw(controller).await;
                        }
                    }
                }
            }
        }
    }
}

/// # DESCRIPTION
///
/// TODO.
#[embassy_executor::task(pool_size = 1)]
pub async fn publisher(stack: &'static embassy_net::Stack<'static>) {
    log::info!("publisher task started");

    loop {
        if stack.is_link_up() && stack.is_config_up() {
            break;
        } else {
            Timer::after_millis(200).await;
        }
    }

    let mut rx_meta = [embassy_net::udp::PacketMetadata::EMPTY; 16];
    let mut rx_buffer = [0; crate::wire::PACKET_SIZE];
    let mut tx_meta = [embassy_net::udp::PacketMetadata::EMPTY; 16];
    let mut tx_buffer = [0; crate::wire::PACKET_SIZE];

    let mut socket = embassy_net::udp::UdpSocket::new(
        *stack,
        &mut rx_meta,
        &mut rx_buffer,
        &mut tx_meta,
        &mut tx_buffer,
    );
    socket
        .bind(0)
        .expect("state should be valid, address correct, free port allocated");

    let remote_address = embassy_net::Ipv4Address::BROADCAST;
    let remote_port = crate::wire::NETWORK_PORT;
    let endpoint = (remote_address, remote_port);

    let mut message = String::<128>::new();
    loop {
        message.clear();

        {
            let state = STATE.read().await;

            core::fmt::write(
                &mut message,
                format_args!(
                    r#"{{"id": "{}", "role": "{}"}}"#,
                    *ID.read().await,
                    state.role,
                ),
            )
            .expect("should not heap allocate too much");
        }

        log::warn!("sending message: '{message}'");
        let payload = message.as_bytes();
        if let Err(e) = socket.send_to(payload, endpoint).await {
            log::warn!("udp send error: {e:?}");
        }

        Timer::after_millis(330).await;
    }
}

//////////////////////////////////////////////////////////////////////
// Helpers
//////////////////////////////////////////////////////////////////////

/// # DESCRIPTION
///
/// TODO.
#[embassy_executor::task(pool_size = 1)]
pub async fn prometheus(led: &'static mut esp_hal::gpio::Output<'static>) {
    log::info!("prometheus task started");

    loop {
        let mode = {
            let state = STATE.read().await;
            Into::<Mode>::into(&state.role)
        };

        mode.perform(led).await;

        Timer::after_millis(500).await;
    }
}

/// # DESCRIPTION
///
/// TODO.
#[embassy_executor::task(pool_size = 1)]
pub async fn connector(wifi_controller: &'static mut esp_radio::wifi::WifiController<'static>) {
    log::info!("connector task started");

    loop {
        if esp_radio::wifi::sta_state() == esp_radio::wifi::WifiStaState::Connected {
            wifi_controller
                .wait_for_event(esp_radio::wifi::WifiEvent::StaDisconnected)
                .await;
        }

        let started = matches!(wifi_controller.is_started(), Ok(true));
        if !started {
            wifi_controller.start_async().await.unwrap();
        }

        if let Err(e) = wifi_controller.connect_async().await {
            log::warn!("wifi connection failure: {e}");
            wifi_controller.disconnect_async().await.unwrap();
            Timer::after_millis(700).await;
        }
    }
}

/// # DESCRIPTION
///
/// TODO.
#[embassy_executor::task(pool_size = 1)]
pub async fn runner(
    runner: &'static mut embassy_net::Runner<'static, esp_radio::wifi::WifiDevice<'static>>,
) {
    log::info!("runner task started");

    runner.run().await
}
