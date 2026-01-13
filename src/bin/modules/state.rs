use defmt::info;
use embassy_futures::{
    select::{Either, select},
    yield_now,
};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex, signal::Signal};
use heapless::{String, format};
use midi_convert::midi_types::{Channel, Control, MidiMessage, Value7};

use crate::modules::{display::map_range, midi::MIDI_QUEUE, rotary_encoder::ROTARY_DELTA};

pub type SharedState = Mutex<CriticalSectionRawMutex, State>;

pub static STATE: SharedState = Mutex::new(State {
    attributes: [
        Attribute {
            name: "Delay",
            channel: Channel::C1,
            control: Control::new(20),
            min: 0,
            max: 100,
            value: 15,
            to_human_readable: |v| format!("{} ms", map_range((0, 100), (0, 1000), v)).unwrap(),
        },
        Attribute {
            name: "Feedback",
            channel: Channel::C1,
            control: Control::new(21),
            min: 0,
            max: 100,
            value: 50,
            to_human_readable: |v| format!("{} %", map_range((0, 100), (0, 100), v)).unwrap(),
        },
    ],
    selected_option: 0,
});

pub static BUTTON_PRESSED: Signal<CriticalSectionRawMutex, ()> = Signal::new();

#[embassy_executor::task]
pub async fn state_task() {
    let mut delta_receiver = ROTARY_DELTA.receiver().unwrap();

    loop {
        match select(delta_receiver.changed(), BUTTON_PRESSED.wait()).await {
            Either::First(delta) => {
                STATE.lock().await.adjust_selected(delta).await;
            }
            Either::Second(_) => {
                STATE.lock().await.next_option();
            }
        };

        // Do some work...
        yield_now().await;
    }
}

#[derive(Copy, Clone)]
pub struct Attribute {
    pub name: &'static str,
    pub channel: Channel,
    pub control: Control,
    pub min: u8,
    pub max: u8,
    pub value: u8,
    pub to_human_readable: fn(u8) -> String<32>,
}

pub type Attributes = [Attribute; 2];

pub struct State {
    attributes: Attributes,
    selected_option: usize,
}

impl State {
    pub fn attributes(&self) -> Attributes {
        self.attributes.clone()
    }

    pub fn selected_option(&self) -> usize {
        self.selected_option
    }

    pub async fn adjust_selected(&mut self, delta: i16) {
        if let Some(attr) = self.attributes.get_mut(self.selected_option) {
            let new_value =
                (attr.value as i16 + delta).clamp(attr.min as i16, attr.max as i16) as u8;
            attr.value = new_value;

            info!("{} adjusted to {} ({})", attr.name, attr.value, delta);

            let packet =
                MidiMessage::ControlChange(attr.channel, attr.control, Value7::from(attr.value));

            MIDI_QUEUE.try_send(packet).ok();
        }
    }

    pub fn next_option(&mut self) {
        self.selected_option = (self.selected_option + 1) % self.attributes.len();
        if let Some(attr) = self.attributes.get(self.selected_option) {
            info!("Selected option: {}", attr.name);
        }
    }
}
