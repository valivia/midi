use core::ptr::addr_of_mut;

use defmt::info;
use esp_hal::otg_fs;
use esp_hal::peripherals::{GPIO19, GPIO20, USB0};
use esp_println::println;
use heapless::Vec;
use midi_convert::midi_types::{Channel, Control, MidiMessage, Note, Value7};
use midi_convert::{parse::MidiTryParseSlice, render_slice::MidiRenderSlice};
use usb_device::prelude::*;
use usbd_midi::{CableNumber, UsbMidiClass, UsbMidiEventPacket, UsbMidiPacketReader};

static mut EP_MEMORY: [u32; 1024] = [0; 1024];

// Size of the used SysEx buffers in bytes.
const SYSEX_BUFFER_SIZE: usize = 64;

#[embassy_executor::task]
pub async fn midi_task(usb0: USB0<'static>, usb_dp: GPIO20<'static>, usb_dm: GPIO19<'static>) {
    let usb_bus_allocator = otg_fs::UsbBus::new(otg_fs::Usb::new(usb0, usb_dp, usb_dm), unsafe {
        &mut *addr_of_mut!(EP_MEMORY)
    });

    // Create a MIDI class with 1 input and 1 output jack.
    let mut midi_class = UsbMidiClass::new(&usb_bus_allocator, 1, 1).unwrap();

    // Build the device. It's important to use `0` for the class and subclass fields because
    // otherwise the device will not enumerate correctly on certain hosts.
    let mut usb_dev = UsbDeviceBuilder::new(&usb_bus_allocator, UsbVidPid(0x16c0, 0x5e4))
        .device_class(0)
        .device_sub_class(0)
        .strings(&[StringDescriptors::default()
            .manufacturer("Music Company")
            .product("MIDI Device")
            .serial_number("12345678")])
        .unwrap()
        .build();

    // Buffer for received SysEx messages from the host.
    let mut sysex_receive_buffer = Vec::<u8, SYSEX_BUFFER_SIZE>::new();

    loop {
        if usb_dev.poll(&mut [&mut midi_class]) {
            // Receive messages.
            let mut buffer = [0; 64];

            if let Ok(size) = midi_class.read(&mut buffer) {
                let packet_reader = UsbMidiPacketReader::new(&buffer, size);
                for packet in packet_reader.into_iter().flatten() {
                    if !packet.is_sysex() {
                        // Just a regular 3-byte message that can be processed directly.
                        let message = MidiMessage::try_parse_slice(packet.payload_bytes());
                        println!(
                            "Regular Message, cable: {:?}, message: {:?}",
                            packet.cable_number(),
                            message
                        );
                    } else {
                        // If a packet containing a SysEx payload is detected, the data is saved
                        // into a buffer and processed after the message is complete.
                        if packet.is_sysex_start() {
                            info!("SysEx message start");
                            sysex_receive_buffer.clear();
                        }

                        match sysex_receive_buffer.extend_from_slice(packet.payload_bytes()) {
                            Ok(_) => {
                                if packet.is_sysex_end() {
                                    info!("SysEx message end");
                                    println!("Buffered SysEx message: {:?}", sysex_receive_buffer);

                                    // Process the SysEx message as request in a separate function
                                    // and send an optional response back to the host.
                                    if let Some(response) =
                                        process_sysex(sysex_receive_buffer.as_ref())
                                    {
                                        for chunk in response.chunks(3) {
                                            let packet = UsbMidiEventPacket::try_from_payload_bytes(
                                                CableNumber::Cable0,
                                                chunk,
                                            );
                                            match packet {
                                                Ok(packet) => loop {
                                                    // Make sure to add some timeout in case the host
                                                    // does not read the data.
                                                    let result =
                                                        midi_class.send_packet(packet.clone());
                                                    match result {
                                                        Ok(_) => break,
                                                        Err(err) => {
                                                            if err != UsbError::WouldBlock {
                                                                break;
                                                            }
                                                        }
                                                    }
                                                },
                                                Err(err) => {
                                                    println!(
                                                        "SysEx response packet error: {:?}",
                                                        err
                                                    )
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            Err(_) => {
                                info!("SysEx buffer overflow.");
                                break;
                            }
                        }
                    }
                }
            }
        }

        // Send a message when the button state changes.
        let mut bytes = [0; 3];

        let button_level = false;
        let message = if button_level {
            MidiMessage::ControlChange(Channel::C1, Control::new(0), Value7::from(100))
        } else {
            MidiMessage::NoteOff(Channel::C1, Note::C3, Value7::from(0))
        };

        message.render_slice(&mut bytes);

        let packet =
            UsbMidiEventPacket::try_from_payload_bytes(CableNumber::Cable0, &bytes).unwrap();

        // Try to send the packet.
        // An `UsbError::WouldBlock` is returned if the host has not read previous data.
        let result = midi_class.send_packet(packet);
        info!("Send result {:?}", result);
    }
}

pub fn process_sysex(request: &[u8]) -> Option<Vec<u8, SYSEX_BUFFER_SIZE>> {
    /// Identity request message.
    ///
    /// See section *DEVICE INQUIRY* of the *MIDI 1.0 Detailed Specification* for further details.
    const IDENTITY_REQUEST: [u8; 6] = [0xF0, 0x7E, 0x7F, 0x06, 0x01, 0xF7];

    if request == IDENTITY_REQUEST {
        let mut response = Vec::<u8, SYSEX_BUFFER_SIZE>::new();
        response
            .extend_from_slice(&[
                0xF0, 0x7E, 0x7F, 0x06, 0x02, // Header
                0x01, // Manufacturer ID
                0x02, // Family code
                0x03, // Family code
                0x04, // Family member code
                0x05, // Family member code
                0x00, // Software revision level
                0x00, // Software revision level
                0x00, // Software revision level
                0x00, // Software revision level
                0xF7,
            ])
            .ok();

        return Some(response);
    }

    None
}
