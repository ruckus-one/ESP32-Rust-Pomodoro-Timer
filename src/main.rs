use anyhow::Result;
use esp_idf_svc::{hal::
    peripherals::Peripherals, timer::EspTimerService}
;
use std::{ops::{Add, Div, Mul, Sub}, sync::{atomic::{ AtomicU16, AtomicU32, Ordering }, Mutex}, thread, time::Duration};

use esp_idf_hal::{adc::{attenuation::DB_11, oneshot::{config::AdcChannelConfig, AdcChannelDriver, AdcDriver}}, gpio::PinDriver, i2c::*};
use esp_idf_hal::prelude::*;
use ssd1306::{prelude::*, I2CDisplayInterface, Ssd1306};

use embedded_graphics::{
    mono_font::{ascii::{FONT_8X13, FONT_10X20}, MonoTextStyleBuilder}, pixelcolor::BinaryColor, prelude::*, primitives::{Arc as Arch, Circle, PrimitiveStyle, PrimitiveStyleBuilder, Rectangle}, text::{Baseline, Text}
};
use std::sync::Arc;

mod rotencoder;
use rotencoder::Rotencoder;

mod push_button;
use push_button::{Button, ButtonState};


fn main() -> Result<()> {
    esp_idf_svc::sys::link_patches();
    let peripherals = Peripherals::take()?;

    let counter = Arc::new(AtomicU32::new(0));
    let counter2 = counter.clone();
    let battery_lvl = Arc::new(AtomicU16::new(2000));
    let battery_lvl2 = battery_lvl.clone();

    let pomodoro = Arc::new(Mutex::new(PomodoroTimer::new([25*60, 5*60])));
    let pomodoro2 = pomodoro.clone();
    let pomodoro3 = pomodoro.clone();

    // let mut guard = pomodoro.try_lock().unwrap();
    // guard.toggle_pause();
    // drop(guard);

    // battery lvl
    let adc = Arc::new(AdcDriver::new(peripherals.adc1)?);
    let adc2 = adc.clone();

    let config = AdcChannelConfig {
        attenuation: DB_11,
        calibration: true,
        ..Default::default()
    };

    let mut adc_pin = AdcChannelDriver::new(adc, peripherals.pins.gpio2, &config)?;
    // end of: battery lvl

    let timer_service = EspTimerService::new().unwrap();
    let callback_timer = {
        timer_service.timer(move || {
            let mut guard = pomodoro.try_lock().unwrap();
            guard.tick();

            counter2.store(guard.counter, Ordering::SeqCst);

            let lvl = adc2.read(&mut adc_pin).unwrap();
            battery_lvl.store(lvl, Ordering::SeqCst);
        })
        .unwrap()
    };
    callback_timer.every(Duration::from_millis(20)).unwrap();

    let _rotencoder_handle = {
        let counter = counter.clone();

        let encoder = Rotencoder::with_callback(
            peripherals.pins.gpio9,
            peripherals.pins.gpio4,
            Arc::new(Mutex::new(move |delta: i8| {

                match delta {
                    1 => counter.fetch_add(1, Ordering::SeqCst),
                    -1 => counter.fetch_sub(1, Ordering::SeqCst),
                    _ => 0_u32,
                };
            }))
        );

        encoder.start_thread()
    };

    let _button_guard = {
        let btn = Button::new(peripherals.pins.gpio10, Arc::new(Mutex::new(move |state: ButtonState| {
            match state {
                ButtonState::Pressed => {
                    println!("Pressed");
                },
                ButtonState::Released => {
                    println!("Released");
                    pomodoro2.lock().unwrap().toggle_pause();
                    // tx.send(AppEvent::ButtonPressed).unwrap();
                },
            }
        })));

        btn.spawn_thread()
    };

    let _oled_handle = thread::Builder::new()
            .stack_size(4000)
            .spawn(move || {
                let i2c = peripherals.i2c0;
                let scl = peripherals.pins.gpio20;
                let sda = peripherals.pins.gpio3;

                let mut reset = PinDriver::output(peripherals.pins.gpio8).unwrap();
                reset.set_low().unwrap();
                thread::sleep(Duration::from_millis(500));
                reset.set_high().unwrap();

                let config = I2cConfig::new().baudrate(400.kHz().into());
                let i2c = I2cDriver::new(i2c, sda, scl, &config).unwrap();

                let interface = I2CDisplayInterface::new(i2c);
                let mut display = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
                    .into_buffered_graphics_mode();
                display.init().unwrap();
                
                let text_style = MonoTextStyleBuilder::new()
                    .font(&FONT_8X13)
                    .text_color(BinaryColor::On)
                    .build();

                let text_larger = MonoTextStyleBuilder::new()
                    .font(&FONT_10X20)
                    .text_color(BinaryColor::On)
                    .build();

                let off = PrimitiveStyleBuilder::new()
                    .stroke_width(1)
                    .stroke_color(BinaryColor::Off)
                    .fill_color(BinaryColor::Off)
                    .build();

                    let on = PrimitiveStyleBuilder::new()
                        .stroke_width(1)
                        .stroke_color(BinaryColor::On)
                        .fill_color(BinaryColor::Off)
                        .build();

                    let fill = PrimitiveStyleBuilder::new()
                        .stroke_width(1)
                        .stroke_color(BinaryColor::On)
                        .fill_color(BinaryColor::On)
                        .build();

                loop {
                    let max = (pomodoro3.lock().unwrap().get_current_phase_time()) as f32;
                    let factor = 360_f32 / max;
                    let position = counter.load(Ordering::SeqCst);
                    let arc_position = (position as f32) * factor;
                    let time_string = format_time(position as u32);
                    let bar = time_string.len() as u32;
                    let mut baz = FONT_10X20.character_size;
                    baz.width = bar * (baz.width + 1);
            
                    Circle::new(Point::new(64-0, 15), 40 + 2*5)
                        .into_styled(PrimitiveStyle::with_fill(BinaryColor::Off))
                        .draw(&mut display)
                        .unwrap();
            
                    Arch::new(Point::new(64+5 + 1, 20 + 1), 38, 0.0.deg(), 360.0.deg())
                        .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
                        .draw(&mut display)
                        .unwrap();

                    Arch::new(Point::new(64+5 -3, 20-3), 46, 0.0.deg(), 360.0.deg())
                        .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
                        .draw(&mut display)
                        .unwrap();

                    Arch::new(Point::new(64+5, 20), 40, 0.0.deg(), (arc_position).deg())
                        .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 5))
                        .draw(&mut display)
                        .unwrap();
            
            
                    // string with cleared background
                    Rectangle::new(Point::new(4, 30), baz)
                        .into_styled(off)
                        .draw(&mut display)
                        .unwrap();
            
                    Text::with_baseline(time_string.as_str(), Point::new(4, 30), text_larger, Baseline::Top)
                        .draw(&mut display)
                        .unwrap();
                    // end of: string with cleared background
            
                    // battery indicator
                    Rectangle::new(Point::new(94,4),  Size::new(32, 8))
                        .into_styled(off)
                        .draw(&mut display)
                        .unwrap();

                    Rectangle::new(Point::new(94,4),  Size::new(32, 8))
                        .into_styled(on)
                        .draw(&mut display)
                        .unwrap();

                    let battery = battery_lvl2.load(Ordering::SeqCst);
                    let battery = clamp(battery, 1300, 1920);
                    let battery = scale(battery, 1300, 1920, 0, 32);

                    Rectangle::new(Point::new(94,4),  Size::new(battery as u32, 8))
                        .into_styled(fill)
                        .draw(&mut display)
                        .unwrap();

                    Rectangle::new(Point::new(126,6),  Size::new(1, 4))
                        .into_styled(fill)
                        .draw(&mut display)
                        .unwrap();
                    // end of: battery indicator

                    // title
                    Text::with_baseline("Pomodoro", Point::new(0, 0), text_style, Baseline::Top)
                        .draw(&mut display)
                        .unwrap();

            
                    match display.flush() {
                        Ok(_) => (),
                        Err(e) => {
                            println!("Error flushing: {:?}", e);
                        }
                    }
            
                    thread::sleep(Duration::from_millis(20));
                }
            })
            .unwrap();

    loop {
        thread::sleep(Duration::from_millis(20));
    }
}

fn scale<T>(value: T, in_min: T, in_max: T, out_min: T, out_max: T) -> T
where
    T: Copy + PartialOrd + Sub<Output = T> + Mul<Output = T> + Div<Output = T> + Add<Output = T>,
{
    (value - in_min) * (out_max - out_min) / (in_max - in_min) + out_min
}

fn clamp<T: PartialOrd>(value: T, min: T, max: T) -> T {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

#[derive(Copy, Clone)]
enum PomodoroPhase {
    Work,
    Break,
}

enum PomodoroState {
    Running(PomodoroPhase),
    Paused,
}

struct PomodoroTimer {
    state: PomodoroState,
    phase_times: [u32; 2],
    counter: u32,
    last_phase: PomodoroPhase,
}

impl PomodoroTimer {
    fn new(phase_times: [u32; 2]) -> PomodoroTimer {
        PomodoroTimer {
            state: PomodoroState::Paused,
            last_phase: PomodoroPhase::Work,
            phase_times: phase_times,
            counter: phase_times[0],
        }
    }

    fn tick(&mut self) {
        match self.state {
            PomodoroState::Running(PomodoroPhase::Work) => {
                if self.counter <= 0 {
                    self.state = PomodoroState::Running(PomodoroPhase::Break);
                    self.counter = self.phase_times[PomodoroPhase::Break as usize];
                    println!("Work done, relax...");
                } else {
                    self.counter -= 1;
                    println!("Tick");
                }
            },
            PomodoroState::Running(PomodoroPhase::Break) => {
                if self.counter <= 0 {
                    self.state = PomodoroState::Running(PomodoroPhase::Work);
                    self.counter = self.phase_times[PomodoroPhase::Work as usize];
                    println!("Break done, back to work!");
                } else {
                    self.counter -= 1;
                    println!("Tick");
                }
            },
            PomodoroState::Paused => {
                // no-op
                println!("Paused!");
            },
        }
    }

    fn toggle_pause(&mut self) {
        match self.state {
            PomodoroState::Running(phase) => {
                self.state = PomodoroState::Paused;
                self.last_phase = phase;
            },
            PomodoroState::Paused => {
                self.state = PomodoroState::Running(self.last_phase);
            },
        }
        println!("Toggle pause!");
    }

    fn get_current_phase_time(&self) -> u32 {
        match self.state {
            PomodoroState::Running(phase) => self.phase_times[phase as usize],
            PomodoroState::Paused => self.phase_times[self.last_phase as usize],
        }
    }
}



fn format_time(seconds: u32) -> String {
    let minutes = seconds / 60;
    let remaining_seconds = seconds % 60;
    format!("{:02}:{:02}", minutes, remaining_seconds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_time_zero() {
        assert_eq!(format_time(0), "00:00");
    }

    #[test]
    fn test_format_time_less_than_a_minute() {
        assert_eq!(format_time(45), "00:45");
    }

    #[test]
    fn test_format_time_exactly_one_minute() {
        assert_eq!(format_time(60), "01:00");
    }

    #[test]
    fn test_format_time_more_than_a_minute() {
        assert_eq!(format_time(75), "01:15");
    }

    #[test]
    fn test_format_time_multiple_minutes() {
        assert_eq!(format_time(3605), "60:05");
    }

    #[test]
    fn test_format_time_25_minutes() {
        assert_eq!(format_time(60*25), "25:00");
    }
}