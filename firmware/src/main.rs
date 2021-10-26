#![no_std]
#![no_main]

use panic_halt as _;

use atsamd_hal::gpio::v2::Pins;
use atsamd_hal::hal::digital::v2::OutputPin;

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

    // Run the CPU on the most default-est clock available.  The implementation uses DPLL0 to
    // generate the 120MHz, it seems.
    let mut generic_clock_controller =
        atsamd_hal::clock::GenericClockController::with_internal_32kosc(
            peripherals.GCLK,
            &mut peripherals.MCLK,
            &mut peripherals.OSC32KCTRL,
            &mut peripherals.OSCCTRL,
            &mut peripherals.NVMCTRL,
        );

    peripherals.OSCCTRL.xoscctrl[1].modify(|_r, w| {
        w.xtalen().set_bit();
        w.startup().bits(0x6);
        unsafe {
            // from table 28-7: External Multipurpose Crystal Oscillator Current Settings
            w.imult().bits(3);
            w.iptat().bits(2);
        }
        w.ondemand().clear_bit();
        w.xtalen().set_bit();
        w.enable().set_bit();
        w
    });
    while peripherals.OSCCTRL.status.read().xoscrdy1().bit_is_clear() {}

    // Set up DPLL1 to output 96MHz.  We really want 48MHz, but the datasheet says the DPLLs can
    // only do 96MHz to 200MHz.
    //
    // That is, 8MHz (XOSC) / 4 => 2MHz ("CKR") * 48 => 96MHz.
    peripherals.OSCCTRL.dpll[1].dpllratio.write(|w| unsafe {
        w.ldr().bits(48 - 1);
        w.ldrfrac().bits(0);
        w
    });
    peripherals.OSCCTRL.dpll[1].dpllctrlb.write(|w| {
        unsafe {
            w.div().bits(1); // F_div = F_xosc / (2 * (DIV + 1)) according to datasheet.
            w.refclk().xosc1();
            w
        }
    });
    peripherals.OSCCTRL.dpll[1].dpllctrla.write(|w| {
        w.ondemand().clear_bit();
        w.enable().set_bit();
        w
    });
    while peripherals.OSCCTRL.dpll[1]
        .dpllstatus
        .read()
        .clkrdy()
        .bit_is_clear()
    {}

    // Arbitrarily choosing Clock Generator 6 to wire 48MHz to the USB peripheral.  USB requires
    // *exactly* 48MHz.
    unsafe {
        let gclk = &*atsamd_hal::target_device::GCLK::ptr();
        gclk.genctrl[6].write(|w| {
            w.div().bits(2);
            w.runstdby().clear_bit();
            w.divsel().clear_bit();
            w.oe().clear_bit();
            w.idc().clear_bit();
            w.genen().set_bit();
            w.src().dpll1();
            w
        });
        gclk.pchctrl[10 /* GCLK_USB */].write(|w| {
            w.chen().set_bit();
            w.gen().gclk6();
            w
        });
    }

    let pins = Pins::new(peripherals.PORT);
    let mut red = pins.pb12.into_push_pull_output();
    red.set_low().unwrap();
    let mut orange = pins.pb13.into_push_pull_output();
    orange.set_low().unwrap();
    let mut yellow = pins.pb14.into_push_pull_output();
    yellow.set_low().unwrap();
    let mut green = pins.pb15.into_push_pull_output();
    green.set_low().unwrap();
    let mut blue = pins.pa04.into_push_pull_output();
    blue.set_high().unwrap();

    let usb_bus = atsamd_hal::usb::UsbBus::new(
        unsafe { &*core::ptr::null() },
        &mut peripherals.MCLK,
        pins.pa24,
        pins.pa25,
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
                    if buf[0] & 0x1 > 0 {
                        red.set_high().unwrap();
                    } else {
                        red.set_low().unwrap();
                    }

                    // orange
                    if buf[0] & 0x2 > 0 {
                        orange.set_high().unwrap();
                    } else {
                        orange.set_low().unwrap();
                    }

                    // yellow
                    if buf[0] & 0x4 > 0 {
                        yellow.set_high().unwrap();
                    } else {
                        yellow.set_low().unwrap();
                    }

                    // green
                    if buf[0] & 0x8 > 0 {
                        green.set_high().unwrap();
                    } else {
                        green.set_low().unwrap();
                    }

                    // blue
                    if buf[0] & 0x10 > 0 {
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
