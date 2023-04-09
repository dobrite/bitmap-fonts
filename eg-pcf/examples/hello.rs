use eg_pcf::{include_pcf, text::PcfTextStyle, PcfFont};
use embedded_graphics::{pixelcolor::Rgb888, prelude::*, text::Text};
use embedded_graphics_simulator::{OutputSettingsBuilder, SimulatorDisplay, Window};

const FONT_12X12: PcfFont = include_pcf!("examples/OpenSans-Regular-12.pcf");

fn main() -> Result<(), std::convert::Infallible> {
    let mut display = SimulatorDisplay::<Rgb888>::new(Size::new(400, 150));

    let style_large = PcfTextStyle::new(&FONT_12X12, Rgb888::WHITE);

    Text::new("A", Point::new(21, 20), style_large).draw(&mut display)?;

    let output_settings = OutputSettingsBuilder::new().scale(2).build();
    Window::new("PCF Font", &output_settings).show_static(&display);

    Ok(())
}
