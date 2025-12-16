use alloc::format;
use defmt::info;
use embassy_time::Timer;
use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_6X10},
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{PrimitiveStyleBuilder, StrokeAlignment},
    text::{Alignment, Text},
};

use esp_hal::peripherals::I2C0;
use esp_hal::{
    i2c::master::{Config, I2c},
    peripherals::{GPIO4, GPIO5},
};
use esp_println as _;
// use modules::encoder::{rotary_encoder_task, ROTARY_COUNT};
use ssd1306::mode::DisplayConfig;
use ssd1306::prelude::DisplayRotation;
use ssd1306::size::DisplaySize128x64;
use ssd1306::{I2CDisplayInterface, Ssd1306};

#[embassy_executor::task]
pub async fn display_task(sda: GPIO4<'static>, scl: GPIO5<'static>, i2c0: I2C0<'static>) {
    let i2c = I2c::new(i2c0, Config::default())
        .unwrap()
        .with_scl(scl)
        .with_sda(sda);

    let interface = I2CDisplayInterface::new(i2c);

    let mut display = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
        .into_buffered_graphics_mode();

    // Log error
    display
        .init()
        .map_err(|e| {
            defmt::error!("Display init error: {:?}", e);
        })
        .unwrap();

    let character_style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);
    let border_stroke = PrimitiveStyleBuilder::new()
        .stroke_color(BinaryColor::On)
        .stroke_width(3)
        .stroke_alignment(StrokeAlignment::Inside)
        .build();

    // Outline
    display
        .bounding_box()
        .into_styled(border_stroke)
        .draw(&mut display)
        .unwrap();

    display.flush().unwrap();

    // let mut rotary_count = ROTARY_COUNT.receiver().unwrap();
    let mut count = 0;
    let mut active = true;

    info!("Display task started");

    loop {
        // match select(rotary_count.changed(), PRESSED.wait()).await {
        //     select::Either::First(value) => {
        //         count = value;
        //     }
        //     select::Either::Second(()) => {
        //         active = !active;
        //     }
        // }

        // clear display
        display.clear(BinaryColor::Off).unwrap();

        // Draw border
        if active {
            display
                .bounding_box()
                .into_styled(border_stroke)
                .draw(&mut display)
                .unwrap();
        }

        // Draw centered text.
        Text::with_alignment(
            &format!("Count: {}", count),
            display.bounding_box().center() + Point::new(0, 15),
            character_style,
            Alignment::Center,
        )
        .draw(&mut display)
        .unwrap();

        display.flush().unwrap();

        count = (count + 1) % 100;
        Timer::after_secs(1).await;
    }
}
