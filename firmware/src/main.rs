#![no_std]
#![no_main]

use panic_halt as _;

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

    // Arbitrarily choosing Clock Generator 6 to wire 48MHz to TCC2
    unsafe {
        let gclk = &*atsamd21e::GCLK::ptr();
        gclk.gendiv.write(|w| {
            w.div().bits(1);
            w.id().bits(6);
            w
        });
        gclk.genctrl.write(|w| {
            w.runstdby().clear_bit();
            w.divsel().clear_bit();
            w.oe().clear_bit();
            w.idc().clear_bit();
            w.genen().set_bit();
            w.src().dfll48m();
            w.id().bits(6);
            w
        });
        gclk.clkctrl.write(|w| {
            w.clken().set_bit();
            w.gen().gclk6();
            w.id().tcc2_tc3();
            w
        });
    }

    // enable the TCC2 peripheral
    peripherals.PM.apbcmask.modify(|_r, w| w.tcc2_().set_bit());

    // set up TCC2/WO[0] (i.e. PA00) for 50% duty cycle at 1kHz
    peripherals.TCC2.wave.write(|w| {
        w.pol0().clear_bit();
        w.wavegen().npwm();
        w
    });
    peripherals.TCC2.per().write(|w| {
        unsafe { w.per().bits(48_000 - 1) }; // 48MHz / 48_000 = 1kHz.
        w
    });
    peripherals.TCC2.cc()[0].write(|w| {
        unsafe { w.cc().bits(24_000) }; // half duty cycle
        w
    });
    peripherals.TCC2.ctrla.write(|w| {
        w.prescaler().div1();
        w.resolution().none();
        w.enable().set_bit();
        w
    });

    let pins = atsamd_hal::gpio::v2::Pins::new(peripherals.PORT);
    let _red = pins.pa00.into_alternate::<atsamd_hal::gpio::v2::pin::E>();

    loop {
        cortex_m::asm::wfi();
    }
}
