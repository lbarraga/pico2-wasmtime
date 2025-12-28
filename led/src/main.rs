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
use embedded_alloc::Heap;
use static_cell::StaticCell;
use wasmtime::{Config as WasmtimeConfig, Engine, Instance, Module, Store};
use {defmt_rtt as _, panic_probe as _};
extern crate alloc;
use alloc::format;

static mut TLS_PTR: *mut u8 = core::ptr::null_mut();

#[unsafe(no_mangle)]
pub extern "C" fn wasmtime_tls_get() -> *mut u8 {
    unsafe { TLS_PTR }
}

#[unsafe(no_mangle)]
pub extern "C" fn wasmtime_tls_set(ptr: *mut u8) {
    unsafe {
        TLS_PTR = ptr;
    }
}

#[global_allocator]
static HEAP: Heap = Heap::empty();

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

    // Wait for USB connection
    class.wait_connection().await;

    // Macro to print text to the USB Serial (with Chunking)
    macro_rules! println {
        ($msg:expr) => {{
            let s = $msg;
            let bytes = s.as_bytes();
            for chunk in bytes.chunks(64) {
                let _ = class.write_packet(chunk).await;
            }
            let _ = class.write_packet(b"\r\n").await;
        }};
    }

    {
        use core::mem::MaybeUninit;
        const HEAP_SIZE: usize = 400 * 1024;
        static mut HEAP_MEM: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];

        unsafe {
            let heap_start = core::ptr::addr_of_mut!(HEAP_MEM) as usize;
            HEAP.init(heap_start, HEAP_SIZE);
        }
    }

    println!("DEBUG: Heap initialized (250KB)");
    println!("USB Serial Initialized!");

    // --- WASMTIME INIT ---
    println!("DEBUG: Configuring Wasmtime (FIX APPLIED)...");
    let mut config = WasmtimeConfig::new();

    // 1. Force Pulley Interpreter (32-bit)
    if let Err(_e) = config.target("pulley32") {
        println!("ERROR: Failed to set target to pulley32");
        panic!("Config error");
    }

    // 2. Disable OS features
    config.signals_based_traps(false);
    config.memory_init_cow(false);
    config.max_wasm_stack(32 * 1024);

    println!("DEBUG: Building Engine...");

    match Engine::new(&config) {
        Ok(engine) => {
            println!("DEBUG: Wasmtime Engine initialized successfully!");

            let smoke_bytes = include_bytes!("smoke.pulley");
            let size_msg = format!("DEBUG: Bytecode loaded. Size: {} bytes", smoke_bytes.len());
            println!(size_msg);

            // SAFETY: We trust the bytecode we compiled ourselves.
            match unsafe { Module::deserialize(&engine, smoke_bytes) } {
                Ok(module) => {
                    println!("DEBUG: Module deserialized successfully!");

                    println!("DEBUG: Initializing store...");
                    let mut store = Store::new(&engine, ());
                    println!("DEBUG: Store initialized");
                    match Instance::new(&mut store, &module, &[]) {
                        Ok(_instance) => {
                            println!("SUCCESS: Interpreter is operational!");
                        }
                        Err(e) => {
                            let error_msg = format!("Instantiate Error: {:?}", e);
                            println!(error_msg);
                        }
                    }
                }
                Err(e) => {
                    let error_msg = format!("Deserialize Error: {:?}", e);
                    println!(error_msg);
                }
            }
        }
        Err(e) => {
            let error_msg = format!("Engine Creation Error: {:?}", e);
            println!(error_msg);
        }
    }
    // --- END WASMTIME INIT ---

    println!("Starting the loop");

    loop {
        led.set_high();
        Timer::after_millis(500).await;
        led.set_low();
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
