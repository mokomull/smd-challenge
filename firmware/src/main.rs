#![no_std]
#![no_main]

use panic_halt as _;

use atsamd_hal::gpio::IntoFunction;
use atsamd_hal::prelude::*;
use usb_device::prelude::*;
use usbd_hid::descriptor::generator_prelude::*;

#[gen_hid_descriptor(
    (collection = APPLICATION, usage_page = 0x08, usage = 0x01) = {
        #[packed_bits 1] led = output;
    }
)]
struct Led {
    led: u8,
}

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
            w.src().dpll96m();
            w.id().bits(6);
            w
        });
        gclk.clkctrl.write(|w| {
            w.clken().set_bit();
            w.gen().gclk6();
            w.id().tcc2_tc3();
            w
        });

        // and route it to the USB device as well.  USB requires *exactly* 48MHz.
        gclk.clkctrl.write(|w| {
            w.clken().set_bit();
            w.gen().gclk6();
            w.id().usb();
            w
        });
    }

    // enable the TCC2 peripheral
    peripherals.PM.apbcmask.modify(|_r, w| w.tcc2_().set_bit());

    // set up TCC2/WO[0] (i.e. PA00) for 50% duty cycle at 1kHz
    peripherals.TCC2.wave.write(|w| {
        w.pol0().set_bit();
        w.wavegen().npwm();
        w
    });
    peripherals.TCC2.per().write(|w| {
        unsafe { w.per().bits(48_000 - 1) }; // 48MHz / 48_000 = 1kHz.
        w
    });
    peripherals.TCC2.cc()[0].write(|w| {
        unsafe { w.cc().bits(0) };
        w
    });
    peripherals.TCC2.ctrla.write(|w| {
        w.prescaler().div1();
        w.resolution().none();
        w.enable().set_bit();
        w
    });

    let mut pins = peripherals.PORT.split();
    let _red = pins.pa0.into_function_e(&mut pins.port);

    let usb_bus = atsamd_hal::samd21::usb::UsbBus::new(
        unsafe { &*core::ptr::null() },
        &mut peripherals.PM,
        pins.pa24.into_function(&mut pins.port),
        pins.pa25.into_function(&mut pins.port),
        peripherals.USB,
    );
    let usb_allocator = usb_device::bus::UsbBusAllocator::new(usb_bus);

    let mut usb_hid = usbd_hid::hid_class::HIDClass::new(&usb_allocator, Led::desc(), 10);

    let mut usb_device = UsbDeviceBuilder::new(&usb_allocator, UsbVidPid(0x1337, 0x4209))
        .manufacturer("Matt Mullins")
        .product("smd-challenge")
        .build();

    loop {
        if usb_device.poll(&mut [&mut usb_hid]) {
            let mut buf = [0];
            match usb_hid.pull_raw_output(&mut buf) {
                Ok(1) => {
                    if buf[0] == 0 {
                        peripherals.TCC2.cc()[0].write(|w| {
                            unsafe { w.cc().bits(0) };
                            w
                        });
                    } else {
                        peripherals.TCC2.cc()[0].write(|w| {
                            unsafe { w.cc().bits(2_000) };
                            w
                        });
                    }
                }
                _ => (),
            }
        }
    }
}
