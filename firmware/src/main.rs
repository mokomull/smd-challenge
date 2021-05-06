#![no_std]
#![no_main]

use core::convert::Infallible;

use panic_halt as _;

use atsamd_hal::prelude::*;

#[cortex_m_rt::entry]
fn main() -> ! {
    let mut peripherals = atsamd_hal::target_device::Peripherals::take().unwrap();
    let core_peripherals = atsamd_hal::target_device::CorePeripherals::take().unwrap();

    let mut generic_clock_controller =
        atsamd_hal::clock::GenericClockController::with_internal_8mhz(
            peripherals.GCLK,
            &mut peripherals.PM,
            &mut peripherals.SYSCTRL,
            &mut peripherals.NVMCTRL,
        );

    peripherals.SYSCTRL.xosc.modify(|_r, w| {
        w.xtalen().set_bit();
        w.gain()._3();
        w.ondemand().clear_bit();
        w.enable().set_bit();
        w
    });
    while peripherals.SYSCTRL.pclksr.read().xoscrdy().bit_is_clear() {}

    let mut delay =
        atsamd_hal::delay::Delay::new(core_peripherals.SYST, &mut generic_clock_controller);

    let pins = atsamd_hal::gpio::v2::Pins::new(peripherals.PORT);

    let mut red = pins.pa00.into_push_pull_output();
    let mut orange = pins.pa01.into_push_pull_output();
    let mut yellow = pins.pa02.into_push_pull_output();
    let mut green = pins.pa03.into_push_pull_output();
    let mut blue = pins.pa04.into_push_pull_output();

    let pins: &mut [&mut dyn embedded_hal::digital::v2::OutputPin<Error = Infallible>] =
        &mut [&mut red, &mut orange, &mut yellow, &mut green, &mut blue];

    for pin in pins.iter_mut() {
        pin.set_high().unwrap();
    }

    loop {
        for pin in pins.iter_mut() {
            pin.set_low().unwrap();
            delay.delay_ms(100u16);
            pin.set_high().unwrap();
            delay.delay_ms(100u16);
        }
    }
}
