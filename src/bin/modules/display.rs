use alloc::format;
use defmt::info;
use embassy_time::Timer;
use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_6X10},
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{Arc, Circle, Line, PrimitiveStyleBuilder, Rectangle, StrokeAlignment, Triangle},
    text::{Alignment, Text},
};

use esp_hal::peripherals::I2C0;
use esp_hal::{
    i2c::master::{Config, I2c},
    peripherals::{GPIO4, GPIO5},
};
use esp_println as _;
use ssd1306::mode::DisplayConfig;
use ssd1306::prelude::DisplayRotation;
use ssd1306::size::DisplaySize128x64;
use ssd1306::{I2CDisplayInterface, Ssd1306};

use crate::modules::state::STATE;

#[embassy_executor::task]
pub async fn display_task(sda: GPIO4<'static>, scl: GPIO5<'static>, i2c0: I2C0<'static>) {
    let i2c = I2c::new(i2c0, Config::default())
        .unwrap()
        .with_scl(scl)
        .with_sda(sda);

    let interface = I2CDisplayInterface::new(i2c);

    let mut display = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate90)
        .into_buffered_graphics_mode();

    // Log error
    display
        .init()
        .map_err(|e| {
            defmt::error!("Display init error: {:?}", e);
        })
        .unwrap();

    let text_default = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);

    let fill = PrimitiveStyleBuilder::new()
        .fill_color(BinaryColor::On)
        .build();

    let thin_stroke = PrimitiveStyleBuilder::new()
        .stroke_color(BinaryColor::On)
        .stroke_width(1)
        .stroke_alignment(StrokeAlignment::Inside)
        .build();

    let thick_stroke = PrimitiveStyleBuilder::new()
        .stroke_color(BinaryColor::On)
        .stroke_width(2)
        .stroke_alignment(StrokeAlignment::Inside)
        .build();

    info!("Display task started");

    let mut old_value = 0;
    loop {
        Timer::after_millis(50).await;

        let (attributes, selected) = {
            let state = STATE.lock().await;
            (state.attributes(), state.selected_option())
        };

        let current_attribute = &attributes[selected];
        if current_attribute.value == old_value {
            continue;
        }
        old_value = current_attribute.value;

        // clear display
        display.clear(BinaryColor::Off).unwrap();

        match selected {
            0 => {
                let size = map_range((0, 127), (5, 60), current_attribute.value);
                Rectangle::with_center(Point::new(32, 32), Size::new(size, size))
                    .into_styled(thin_stroke)
                    .draw(&mut display)
                    .unwrap();
            }
            1 => {
                let triangle_y_middle = 32;
                let triangle_height = 16;
                let triangle_x_middle = 20;
                let triangle_width = 10;
                Triangle::new(
                    Point::new(triangle_x_middle - triangle_width, triangle_y_middle),
                    Point::new(
                        triangle_x_middle + triangle_width,
                        triangle_y_middle + triangle_height,
                    ),
                    Point::new(
                        triangle_x_middle + triangle_width,
                        triangle_y_middle - triangle_height,
                    ),
                )
                .into_styled(fill)
                .draw(&mut display)
                .unwrap();

                let center = Point::new(triangle_x_middle - triangle_width, triangle_y_middle);
                Circle::with_center(center, 10)
                    .into_styled(fill)
                    .draw(&mut display)
                    .unwrap();

                for (_, r) in [10, 22, 34, 46]
                    .iter()
                    .enumerate()
                    .take(level_to_arc_count(current_attribute.value))
                {
                    Arc::with_center(
                        Point::new(32, triangle_y_middle),
                        *r,
                        (-60.0).deg(),
                        (120.0).deg(),
                    )
                    .into_styled(thick_stroke)
                    .draw(&mut display)
                    .unwrap();
                }
            }
            _ => {}
        }

        let line_y = 70;
        Line::new(Point::new(0, line_y), Point::new(64, line_y))
            .into_styled(thin_stroke)
            .draw(&mut display)
            .unwrap();

        // Draw centered text.
        let text_y = 82;
        Text::with_alignment(
            &format!("{}:\n{}", current_attribute.name, current_attribute.value),
            Point::new(32, text_y),
            text_default,
            Alignment::Center,
        )
        .draw(&mut display)
        .unwrap();

        display.flush().ok();
    }
}

pub fn map_range(old: (u32, u32), new: (u32, u32), x: u8) -> u32 {
    (new.0 + (x as u32 * (new.1 - new.0) / (old.1 - old.0))) as u32
}

fn level_to_arc_count(level: u8) -> usize {
    if level == 0 {
        0
    } else {
        1 + ((level as u16 * 3) / 127) as usize
    }
}
