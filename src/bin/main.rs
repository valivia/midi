#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

use defmt::info;
use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use embassy_time::Timer;
use esp_hal::clock::CpuClock;
use esp_hal::gpio::{Input, InputConfig, Pull};
use esp_hal::timer::timg::TimerGroup;
use esp_println as _;

use crate::modules::rotary_encoder::rotary_encoder_task;

pub mod modules;

#[panic_handler]
fn panic(error: &core::panic::PanicInfo) -> ! {
    info!("Panic: {}", error);
    loop {}
}

extern crate alloc;

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

static PRESSED: Signal<CriticalSectionRawMutex, ()> = Signal::new();

#[allow(
    clippy::large_stack_frames,
    reason = "it's not unusual to allocate larger buffers etc. in main"
)]
#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    // generator version: 1.1.0

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 73744);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timg0.timer0);

    info!("Embassy initialized!");

    let input_cfg = InputConfig::default().with_pull(Pull::Up);
    let mut re_key = Input::new(peripherals.GPIO0, input_cfg);

    spawner
        .spawn(rotary_encoder_task(
            peripherals.PCNT,
            peripherals.GPIO34.into(),
            peripherals.GPIO33.into(),
        ))
        .unwrap();

    spawner
        .spawn(modules::display::display_task(
            peripherals.GPIO4,
            peripherals.GPIO5,
            peripherals.I2C0,
        ))
        .unwrap();

    loop {
        re_key.wait_for_falling_edge().await;
        info!("Pressed");
        PRESSED.signal(());
        Timer::after_millis(200).await;
    }

    // for inspiration have a look at the examples at https://github.com/esp-rs/esp-hal/tree/esp-hal-v~1.0/examples
}
