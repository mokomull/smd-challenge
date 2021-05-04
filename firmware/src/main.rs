#![no_std]
#![no_main]

use panic_halt as _;

#[cortex_m_rt::entry]
fn main() -> ! {
    loop {
        cortex_m::asm::nop();
    }
}
