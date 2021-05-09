#![no_std]
#![no_main]

use panic_halt as _;

use atsamd_hal::gpio::IntoFunction;
use atsamd_hal::prelude::*;
use usb_device::prelude::*;

#[rustfmt::skip]
static HID_DESCRIPTOR: &[u8] = &[
    // Usage Page - Generic Desktop Controls
    0b0000_01_01, 0x01,
    // Usage - Keyboard
    0b0000_10_01, 0x06,
    // Collection - Application
    0b1010_00_01, 0x01,

    //     Report Count - 5
    0b1001_01_01, 5,
    //     Report Size - 1
    0b0111_01_01, 1,
    //     Usage Page - LEDs
    0b0000_01_01, 0x08,
    //     Usage - Compose
    0b0000_10_01, 0x04,
    //     Usage - Kana
    0b0000_10_01, 0x05,
    //     Usage - Stand-by ("sleep" to Linux)
    0b0000_10_01, 0x27,
    //     Usage - System Suspend ("suspend" to Linux)
    0b0000_10_01, 0x4c,
    //     Usage - Message Waiting ("mail" to Linux)
    0b0000_10_01, 0x19,
    //     Output (Data, Variable, Absolute)
    0b1001_00_01, 0b0000_0010,

    //     Report Count - 3
    0b1001_01_01, 3,
    //     Output (Constant, Variable, Absolute)
    0b1001_00_01, 0b0000_0011,
    // End Collection
    0b1100_00_00,
];

#[cortex_m_rt::entry]
fn main() -> ! {
    let mut peripherals = atsamd_hal::target_device::Peripherals::take().unwrap();
    let core_peripherals = atsamd_hal::target_device::CorePeripherals::take().unwrap();

    // SYSCTRL.OSC8M defaults to a /8 prescaler, but the implementation of this function sets that
    // prescale factor to /1.
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

    // Set up FDPLL96M to output 48MHz
    // That is, 16MHz (XOSC) / 8 => 2MHz (fdpll96m_ref) * 24 => 48MHz
    peripherals.SYSCTRL.dpllratio.write(|w| unsafe {
        w.ldr().bits(24 - 1);
        w.ldrfrac().bits(0);
        w
    });
    peripherals.SYSCTRL.dpllctrlb.write(|w| {
        unsafe {
            w.div().bits(3); // F_fdpll96m_ref = F_xosc * (1 / (2 * (DIV + 1))) according to datasheet.
            w.refclk().ref1();
            w
        }
    });
    peripherals.SYSCTRL.dpllctrla.write(|w| {
        w.ondemand().clear_bit();
        w.enable().set_bit();
        w
    });
    while peripherals
        .SYSCTRL
        .dpllstatus
        .read()
        .clkrdy()
        .bit_is_clear()
    {}

    // Arbitrarily choosing Clock Generator 6 to wire 48MHz to the USB peripheral.  USB requires
    // *exactly* 48MHz.
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
            w.src().dpll96m();
            w.id().bits(6);
            w
        });
        gclk.clkctrl.write(|w| {
            w.clken().set_bit();
            w.gen().gclk6();
            w.id().usb();
            w
        });
    }

    let mut pins = peripherals.PORT.split();
    let mut red = pins.pa0.into_push_pull_output(&mut pins.port);
    red.set_high().unwrap();
    let mut orange = pins.pa1.into_push_pull_output(&mut pins.port);
    orange.set_high().unwrap();
    let mut yellow = pins.pa2.into_push_pull_output(&mut pins.port);
    yellow.set_high().unwrap();
    let mut green = pins.pa3.into_push_pull_output(&mut pins.port);
    green.set_high().unwrap();
    let mut blue = pins.pa4.into_push_pull_output(&mut pins.port);
    blue.set_high().unwrap();

    let usb_bus = atsamd_hal::samd21::usb::UsbBus::new(
        unsafe { &*core::ptr::null() },
        &mut peripherals.PM,
        pins.pa24.into_function(&mut pins.port),
        pins.pa25.into_function(&mut pins.port),
        peripherals.USB,
    );
    let usb_allocator = usb_device::bus::UsbBusAllocator::new(usb_bus);

    let mut usb_hid = usbd_hid::hid_class::HIDClass::new(&usb_allocator, HID_DESCRIPTOR, 10);

    let mut usb_device = UsbDeviceBuilder::new(&usb_allocator, UsbVidPid(0x1337, 0x4209))
        .manufacturer("Matt Mullins")
        .product("smd-challenge")
        .build();

    loop {
        if usb_device.poll(&mut [&mut usb_hid]) {
            let mut buf = [0];
            match usb_hid.pull_raw_output(&mut buf) {
                Ok(1) => {
                    // red
                    if buf[0] & 0x1 == 0 {
                        // remember: we're switching GND for the LEDs, so inverted logic
                        red.set_high().unwrap();
                    } else {
                        red.set_low().unwrap();
                    }

                    // orange
                    if buf[0] & 0x2 == 0 {
                        orange.set_high().unwrap();
                    } else {
                        orange.set_low().unwrap();
                    }

                    // yellow
                    if buf[0] & 0x4 == 0 {
                        yellow.set_high().unwrap();
                    } else {
                        yellow.set_low().unwrap();
                    }

                    // green
                    if buf[0] & 0x8 == 0 {
                        green.set_high().unwrap();
                    } else {
                        green.set_low().unwrap();
                    }

                    // blue
                    if buf[0] & 0x10 == 0 {
                        blue.set_high().unwrap();
                    } else {
                        blue.set_low().unwrap();
                    }
                }
                _ => (),
            }
        }
    }
}
