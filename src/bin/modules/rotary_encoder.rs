use core::{cell::RefCell, cmp::min};

use critical_section::Mutex;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, watch::Watch};
use embassy_time::Timer;
use esp_hal::{
    gpio::{AnyPin, Input, InputConfig, Pull},
    handler,
    interrupt::Priority,
    pcnt::{Pcnt, channel, unit},
    peripherals::PCNT,
};

static UNIT0: Mutex<RefCell<Option<unit::Unit<'static, 1>>>> = Mutex::new(RefCell::new(None));

pub static ROTARY_COUNT: Watch<CriticalSectionRawMutex, i16, 1> = Watch::new();
pub static ROTARY_DELTA: Watch<CriticalSectionRawMutex, i16, 1> = Watch::new();

#[embassy_executor::task]
pub async fn rotary_encoder_task(pcnt: PCNT<'static>, s1: AnyPin<'static>, s2: AnyPin<'static>) {
    // Initialize Pulse Counter (PCNT) unit with limits and filter settings
    let mut pcnt = Pcnt::new(pcnt);
    pcnt.set_interrupt_handler(interrupt_handler);
    let u0 = pcnt.unit1;
    u0.set_low_limit(None).unwrap();
    u0.set_high_limit(None).unwrap();
    u0.set_filter(Some(min(10u16 * 80, 1023u16))).unwrap();
    u0.clear();

    // Pins
    let input_cfg = InputConfig::default().with_pull(Pull::Up);
    let pin_a = Input::new(s1, input_cfg);
    let pin_b = Input::new(s2, input_cfg);
    let input_a = pin_a.peripheral_input();
    let input_b = pin_b.peripheral_input();

    // Set up channels with control and edge signals
    let ch0 = &u0.channel0;
    ch0.set_ctrl_signal(input_a.clone());
    ch0.set_edge_signal(input_b.clone());
    ch0.set_ctrl_mode(channel::CtrlMode::Reverse, channel::CtrlMode::Keep);
    ch0.set_input_mode(channel::EdgeMode::Increment, channel::EdgeMode::Decrement);

    let ch1 = &u0.channel1;
    ch1.set_ctrl_signal(input_b);
    ch1.set_edge_signal(input_a);
    ch1.set_ctrl_mode(channel::CtrlMode::Reverse, channel::CtrlMode::Keep);
    ch1.set_input_mode(channel::EdgeMode::Decrement, channel::EdgeMode::Increment);

    // Enable interrupts and resume pulse counter unit
    u0.listen();
    u0.resume();
    let counter = u0.counter.clone();

    critical_section::with(|cs| UNIT0.borrow_ref_mut(cs).replace(u0));

    // Monitor counter value and print updates
    let total_sender = ROTARY_COUNT.sender();
    let delta_sender = ROTARY_DELTA.sender();

    let mut count: u8 = 0;
    let mut last_value: i16 = 0;

    loop {
        Timer::after_millis(100).await;
        let current_value = counter.get();

        if current_value == last_value {
            continue;
        }

        let delta = current_value.wrapping_sub(last_value);
        delta_sender.send(delta);
        last_value = current_value;

        let new_count = saturating_add_custom_range(count, delta, 0, 100);

        if new_count == count {
            continue;
        }

        count = new_count;
        total_sender.send(count as i16);
    }
}

fn saturating_add_custom_range(value: u8, delta: i16, min: u8, max: u8) -> u8 {
    let new = value.saturating_add_signed(delta as i8);
    new.clamp(min, max)
}

#[handler(priority = Priority::Priority2)]
fn interrupt_handler() {
    critical_section::with(|cs| {
        let mut u0 = UNIT0.borrow_ref_mut(cs);
        let u0 = u0.as_mut().unwrap();
        if u0.interrupt_is_set() {
            u0.reset_interrupt();
        }
    });
}
