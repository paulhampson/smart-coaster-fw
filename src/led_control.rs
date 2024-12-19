use embassy_rp::pio::Instance;
use embassy_rp::pio_programs::ws2812::PioWs2812;
use embassy_time::{Duration, Ticker};
use smart_leds::RGB8;


/// Input a value 0 to 255 to get a color value
/// The colours are a transition r - g - b - back to r.
fn colour_wheel(mut wheel_pos: u8) -> RGB8 {
    wheel_pos = 255 - wheel_pos;
    if wheel_pos < 85 {
        return (255 - wheel_pos * 3, 0, wheel_pos * 3).into();
    }
    if wheel_pos < 170 {
        wheel_pos -= 85;
        return (0, wheel_pos * 3, 255 - wheel_pos * 3).into();
    }
    wheel_pos -= 170;
    (wheel_pos * 3, 255 - wheel_pos * 3, 0).into()
}

pub struct LedControl<'a, const LED_COUNT: usize, P: embassy_rp::pio::Instance, const S: usize> {
    ws2812pio: PioWs2812<'a, P, S, LED_COUNT>,
    led_count: usize,
    led_state: [RGB8; LED_COUNT],
}

impl<'a, const LED_COUNT: usize, P, const S: usize> LedControl<'a, LED_COUNT, P, S>
where
    P: Instance,
{
    pub fn new(ws2812pio: PioWs2812<'a, P, S, LED_COUNT>) -> Self {
        Self {
            ws2812pio,
            led_count: LED_COUNT,
            led_state: [RGB8::default(); LED_COUNT],
        }
    }

    pub async fn led_control_update(&mut self) {
        let mut ticker = Ticker::every(Duration::from_millis(50));
        loop {
            for j in 0..(256 * 5) {
                for i in 0..self.led_count {
                    self.led_state[i] = colour_wheel((((i * 256) as u16 / self.led_count as u16 + j as u16) & 255) as u8);
                }
                self.ws2812pio.write(&self.led_state).await;

                ticker.next().await;
            }
        }
    }
}