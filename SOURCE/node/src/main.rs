#![no_std] // NOTE: does not work in embedded world.
#![no_main] // NOTE: we use our own main entrypoint.

extern crate alloc;
use alloc::string::ToString;

// NOTE: useful in monitoring purposes.
use esp_backtrace as _;

// NOTE: populates image application description.
esp_bootloader_esp_idf::esp_app_desc!();

mod bully;
mod led;
mod sync;
mod tasks;
mod wire;

// NOTE: as we cannot safely share exclusive references with static lifetimes in
// asynchronous contexts, we must initialize everything here and use dependency
// injection to forward values.
#[esp_rtos::main]
async fn main(spawner: embassy_executor::Spawner) {
    esp_println::logger::init_logger_from_env();
    log::info!("logging facility initialized successfully");

    let clock_frequency = esp_hal::clock::CpuClock::_80MHz;
    let general_config = esp_hal::Config::default().with_cpu_clock(clock_frequency);
    let peripherals = make_leaked!(
        esp_hal::peripherals::Peripherals,
        esp_hal::init(general_config)
    );
    log::info!("basic peripherals initialized successfully");

    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 66320);
    log::info!("allocation facility initialized successfully");

    let random_generator = make_leaked!(esp_hal::rng::Rng, esp_hal::rng::Rng::new());
    log::info!("randomness facility initialized successfully");

    let peripheral_timer = peripherals.TIMG0.reborrow();
    let timer_group = esp_hal::timer::timg::TimerGroup::new(peripheral_timer);
    log::info!("timer group initialized successfully");

    let software_interrupt = peripherals.SW_INTERRUPT.reborrow();
    let interrupt_controller =
        esp_hal::interrupt::software::SoftwareInterruptControl::new(software_interrupt);
    log::info!("software interrupts initialized successfully");

    esp_rtos::start(timer_group.timer0, interrupt_controller.software_interrupt0);
    log::info!("operating system initialized successfully");

    let led_gpio = peripherals.GPIO4.reborrow();
    let gpio_level = esp_hal::gpio::Level::High;
    let gpio_config = Default::default();
    let led_controller = make_leaked!(
        esp_hal::gpio::Output<'static>,
        esp_hal::gpio::Output::new(led_gpio, gpio_level, gpio_config)
    );
    led_controller.set_low();
    log::info!("indication facility initialized successfully");

    let general_controller = make_leaked!(
        esp_radio::Controller<'static>,
        esp_radio::init().expect(
            "scheduler should be initialized, interrupts enabled, minimum clock rate is met"
        )
    );
    log::info!("radio controller initialized successfully");

    let wifi_peripheral = peripherals.WIFI.reborrow();
    // NOTE: broadcast peer is automatically added to known peers here.
    let (wifi_controller, wifi_interfaces) =
        esp_radio::wifi::new(general_controller, wifi_peripheral, Default::default())
            .expect("interrupts should be enabled, IEEE 802.15.4 not be used");
    let wifi_controller = make_leaked!(esp_radio::wifi::WifiController<'static>, wifi_controller);
    log::info!("wifi peripheral initialized successfully");

    let auth_method = esp_radio::wifi::AuthMethod::Wpa2Personal;
    let scan_method = esp_radio::wifi::ScanMethod::AllChannels;
    let network_ssid = crate::wire::NETWORK_NAME.to_string();
    let network_pass = crate::wire::NETWORK_PASS.to_string();
    let network_config = esp_radio::wifi::ModeConfig::Client(
        esp_radio::wifi::ClientConfig::default()
            .with_auth_method(auth_method)
            .with_scan_method(scan_method)
            .with_ssid(network_ssid)
            .with_password(network_pass),
    );
    wifi_controller
        .set_config(&network_config)
        .expect("should apply successfully");
    log::info!("wifi config initialized successfully: {network_config:#?}");

    wifi_controller
        .set_mode(esp_radio::wifi::WifiMode::Sta)
        .expect("arguments should be valid, mode valid, Wi-Fi initialized");
    wifi_controller
        .set_power_saving(esp_radio::wifi::PowerSaveMode::None)
        .expect("arguments should be valid, mode valid, Wi-Fi initialized");
    log::info!("wifi miscellaneous initialized successfully");

    wifi_controller
        .start()
        .expect("should have enough memory, valid mode, valid arguments, Wi-Fi initialized");
    while !wifi_controller
        .is_started()
        .expect("should have enough memory, valid mode, valid arguments, Wi-Fi initialized")
    {}
    log::info!("wifi controller started successfully");

    let (now_manager, now_sender, now_receiver) = wifi_interfaces.esp_now.split();
    let now_manager = make_leaked!(esp_radio::esp_now::EspNowManager<'static>, now_manager);
    let now_sender = make_shared!(esp_radio::esp_now::EspNowSender<'static>, now_sender);
    let now_receiver = make_shared!(esp_radio::esp_now::EspNowReceiver<'static>, now_receiver);
    log::info!("esp now initialized successfully");

    let raw_address = wifi_interfaces.sta.mac_address();
    let mac_address = crate::bully::Id::from(raw_address);
    *crate::bully::ID.write().await = mac_address;
    log::info!("mac address acquired successfully");

    let stack_resources = make_leaked!(
        embassy_net::StackResources::<3>,
        embassy_net::StackResources::<3>::new()
    );
    let stack_config = embassy_net::Config::dhcpv4(Default::default());
    log::info!("network resources initialized successfully");

    let interface_seed = u64::from(esp_hal::rng::Rng::new().random());
    let (network_stack, network_runner) = embassy_net::new(
        wifi_interfaces.sta,
        stack_config,
        stack_resources,
        interface_seed,
    );
    let network_stack = make_leaked!(embassy_net::Stack<'static>, network_stack);
    let network_runner = make_leaked!(
        embassy_net::Runner<'static, esp_radio::wifi::WifiDevice<'static>>,
        network_runner
    );
    log::info!("network stack initialized successfully");

    let message_queue = make_leaked!(crate::sync::Channel<(crate::wire::Message, crate::bully::Id), 16>, crate::sync::Channel::new());
    let queue_receiver = make_leaked!(
        embassy_sync::channel::Receiver<
            'static,
            crate::sync::Mutex,
            (crate::wire::Message, crate::bully::Id),
            16,
        >,
        message_queue.receiver()
    );
    let queue_sender = make_leaked!(
        embassy_sync::channel::Sender<
            'static,
            crate::sync::Mutex,
            (crate::wire::Message, crate::bully::Id),
            16,
        >,
        message_queue.sender()
    );
    log::info!("message queue intiialized successfully");

    spawner
        .spawn(tasks::runner(network_runner))
        .expect("only one instance of runner task should be spawned");
    spawner
        .spawn(tasks::connector(wifi_controller))
        .expect("only one instance of connector task should be spawned");
    spawner
        .spawn(tasks::prometheus(led_controller))
        .expect("only one instance of prometheus task should be spawned");
    log::info!("miscellaneous tasks spawned successfully");

    log::info!("waiting for network connection");
    while esp_radio::wifi::sta_state() != esp_radio::wifi::WifiStaState::Connected {
        log::info!("retrying network connection establishment");
        embassy_time::Timer::after_millis(700).await;
    }
    log::info!("network connection success");

    // spawner
    //     .spawn(tasks::scout(now_sender))
    //     .expect("only one instance of scout task should be spawned");
    spawner
        .spawn(tasks::hermes(now_receiver, now_manager, queue_sender))
        .expect("only one instance of sentinel task should be spawned");
    spawner
        .spawn(tasks::governor(
            random_generator,
            now_sender,
            queue_receiver,
        ))
        .expect("only one instance of governor task should be spawned");
    spawner
        .spawn(tasks::publisher(network_stack))
        .expect("only one instance of publisher task should be spawned");
    log::info!("core tasks spawned successfully");
}
