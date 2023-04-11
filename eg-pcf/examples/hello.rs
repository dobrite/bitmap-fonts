use eg_pcf::{include_pcf, text::PcfTextStyle, PcfFont, PcfGlyph};
use embedded_graphics::{
    pixelcolor::Rgb888,
    prelude::*,
    text::{Alignment, Text},
};
use embedded_graphics_simulator::{OutputSettingsBuilder, SimulatorDisplay, Window};

const FONT_6X10: PcfFont =
    include_pcf!("examples/6x10.pcf", 'A'..='Z' | 'a'..='z' | '0'..='9' | ' ');
const FONT_10X20: PcfFont = include_pcf!("examples/10x20.pcf");
const FONT_12X12: PcfFont = include_pcf!("examples/OpenSans-Regular-12.pcf");

fn main() -> Result<(), std::convert::Infallible> {
    let mut display = SimulatorDisplay::<Rgb888>::new(Size::new(400, 150));

    let style_small = PcfTextStyle::new(&FONT_6X10, Rgb888::RED);
    let style_large = PcfTextStyle::new(&FONT_10X20, Rgb888::GREEN);
    let style_open = PcfTextStyle::new(&FONT_12X12, Rgb888::WHITE);

    Text::new("Hello PCF! äöü,\"#", Point::new(30, 50), style_large).draw(&mut display)?;

    Text::new("A\nB\nC", Point::new(10, 50), style_large).draw(&mut display)?;

    Text::with_alignment(
        "Hello PCF! äöü,\"#",
        Point::new(150, 100),
        style_large,
        Alignment::Center,
    )
    .draw(&mut display)?;

    Text::with_alignment(
        "Line 1\nLine 2\nLast line",
        Point::new(390, 10),
        style_small,
        Alignment::Right,
    )
    .draw(&mut display)
    .unwrap();

    Text::new("Aa\nBb\nCc\n123\n!%*", Point::new(300, 75), style_open).draw(&mut display)?;

    let output_settings = OutputSettingsBuilder::new().scale(2).build();
    Window::new("PCF Font", &output_settings).show_static(&display);

    Ok(())
}
