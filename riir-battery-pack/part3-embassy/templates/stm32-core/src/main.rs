#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_time::Timer;
use {defmt_rtt as _, panic_probe as _};

/// Sanity-check task: confirms the executor, the chip HAL, and RTT logging
/// are wired up end to end before writing real peripheral code against it.
#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let _p = embassy_stm32::init(Default::default());

    loop {
        defmt::info!("tick");
        Timer::after_millis(1000).await;
    }
}
