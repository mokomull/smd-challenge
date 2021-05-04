#![no_std]
#![no_main]

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

    let mut delay =
        atsamd_hal::delay::Delay::new(core_peripherals.SYST, &mut generic_clock_controller);

    let pins = atsamd_hal::gpio::v2::Pins::new(peripherals.PORT);

    let mut red = pins.pa00.into_push_pull_output();

    loop {
        red.set_low().unwrap();
        delay.delay_ms(500u16);
        red.set_high().unwrap();
        delay.delay_ms(500u16);
    }
}
