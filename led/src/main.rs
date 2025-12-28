#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_rp as hal;
use embassy_rp::bind_interrupts;
use embassy_rp::block::ImageDef;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::peripherals::USB;
use embassy_rp::usb::{Driver, InterruptHandler};
use embassy_time::Timer;
use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
use embassy_usb::{Builder, Config, UsbDevice};
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

// --- FIX 1: Define the Timestamp (Required by defmt) ---
defmt::timestamp!("{=u64:us}", { embassy_time::Instant::now().as_micros() });

// --- FIX 2: Define the Defmt Panic Handler (Required by defmt) ---
#[defmt::panic_handler]
fn panic() -> ! {
    panic!("defmt panic")
}

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
});

#[unsafe(link_section = ".start_block")]
#[used]
pub static IMAGE_DEF: ImageDef = hal::block::ImageDef::secure_exe();

#[embassy_executor::task]
async fn usb_task(mut usb: UsbDevice<'static, Driver<'static, USB>>) -> ! {
    usb.run().await
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    // --- USB SETUP ---
    let driver = Driver::new(p.USB, Irqs);

    let mut config = Config::new(0xc0de, 0xcafe);
    config.manufacturer = Some("Embassy");
    config.product = Some("Pico 2 Serial");
    config.serial_number = Some("12345");
    config.max_power = 100;
    config.max_packet_size_0 = 64;

    static CONFIG_DESC: StaticCell<[u8; 256]> = StaticCell::new();
    static BOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
    static MSOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
    static CTRL_BUF: StaticCell<[u8; 64]> = StaticCell::new();
    static STATE: StaticCell<State> = StaticCell::new();

    let mut builder = Builder::new(
        driver,
        config,
        CONFIG_DESC.init([0; 256]),
        BOS_DESC.init([0; 256]),
        MSOS_DESC.init([0; 256]),
        CTRL_BUF.init([0; 64]),
    );

    let mut class = CdcAcmClass::new(&mut builder, STATE.init(State::new()), 64);
    let usb = builder.build();
    spawner.spawn(usb_task(usb)).unwrap();
    // --- END USB SETUP ---

    let mut led = Output::new(p.PIN_13, Level::Low);

    // Wait for USB connection so you don't miss the first messages
    class.wait_connection().await;

    // Macro to print text to the USB Serial
    macro_rules! println {
        ($msg:expr) => {{
            let _ = class.write_packet($msg.as_bytes()).await;
            let _ = class.write_packet(b"\r\n").await;
        }};
    }

    println!("USB Serial Initialized!");

    loop {
        led.set_high();
        println!("LED ON");
        Timer::after_millis(500).await;

        led.set_low();
        println!("LED OFF");
        Timer::after_millis(500).await;
    }
}

#[unsafe(link_section = ".bi_entries")]
#[used]
pub static PICOTOOL_ENTRIES: [embassy_rp::binary_info::EntryAddr; 4] = [
    embassy_rp::binary_info::rp_program_name!(c"usb-serial"),
    embassy_rp::binary_info::rp_program_description!(c"USB Serial Example"),
    embassy_rp::binary_info::rp_cargo_version!(),
    embassy_rp::binary_info::rp_program_build_attribute!(),
];
