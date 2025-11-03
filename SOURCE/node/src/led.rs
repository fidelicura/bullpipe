use embassy_time::Duration;
use embassy_time::Timer;
use esp_hal::gpio::Output;

/// # DESCRIPTION
///
/// TODO.
#[derive(Debug, Clone, Copy)]
pub enum Mode {
    QuickBlink,
    SlowBlink,
    SteadyOn,
    SteadyOff,
}

impl Mode {
    /// # DESCRIPTION
    ///
    /// TODO.
    pub async fn perform(&self, led: &mut Output<'_>) {
        log::debug!("performing led light indication");

        self.respective_action(led);
        self.respective_delay().await;

        log::debug!("led light indicated successfully");
    }

    /// # DESCRIPTION
    ///
    /// TODO.
    async fn respective_delay(&self) {
        let duration = self.into();
        Timer::after(duration).await;
    }

    /// # DESCRIPTION
    ///
    /// TODO.
    fn respective_action(&self, led: &mut Output<'_>) {
        match self {
            Self::QuickBlink | Self::SlowBlink => led.toggle(),
            Self::SteadyOn => led.set_high(),
            Self::SteadyOff => led.set_low(),
        }
    }
}

impl<'a> From<&'a Mode> for Duration {
    fn from(value: &'a Mode) -> Self {
        Duration::from_millis(match value {
            Mode::QuickBlink => 45,
            Mode::SlowBlink => 200,
            Mode::SteadyOn | Mode::SteadyOff => 0,
        })
    }
}
