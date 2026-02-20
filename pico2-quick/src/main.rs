#![no_std]
#![no_main]

extern crate alloc;

use defmt::info;
use embassy_executor::Spawner;
use embassy_rp::gpio::{Level, Output};
use embedded_alloc::Heap;
use {defmt_rtt as _, panic_probe as _};

use wasmtime::component::{Component, Linker};
use wasmtime::{Config, Engine, Store};

mod blinky_lib;
use blinky_lib::{BlinkyCtx, BlinkyView};

// 1. Point to the Guest's WIT folder and use the "sos" world
wasmtime::component::bindgen!({
    path: "../guest/wit",
    world: "sos",
});

#[global_allocator]
static ALLOCATOR: Heap = Heap::empty();

pub struct HostState {
    pub blinky_ctx: BlinkyCtx,
}

impl BlinkyView for HostState {
    fn blinky_ctx(&mut self) -> &mut BlinkyCtx {
        &mut self.blinky_ctx
    }
}

// --- Wasmtime TLS Hooks ---
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

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    // Initialize Heap
    {
        use core::mem::MaybeUninit;
        const HEAP_SIZE: usize = 440 * 1024; // 440KB
        static mut HEAP_MEM: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];
        unsafe { ALLOCATOR.init(core::ptr::addr_of_mut!(HEAP_MEM) as usize, HEAP_SIZE) }
    }

    info!("Heap initialized. Setting up Wasmtime...");

    let mut config = Config::new();
    config.target("pulley32").unwrap();
    config.wasm_component_model(true);
    config.gc_support(false);
    config.signals_based_traps(false);
    config.memory_init_cow(false);
    config.memory_guard_size(0);
    config.memory_reservation(0);
    config.max_wasm_stack(32 * 1024);
    config.memory_reservation_for_growth(0);

    let engine = Engine::new(&config).expect("Engine failed");

    let led = Output::new(p.PIN_15, Level::Low);
    let blinky_ctx = BlinkyCtx { led };
    let mut store = Store::new(&engine, HostState { blinky_ctx });

    let mut linker = Linker::new(&engine);

    // 2. Link the library as before
    blinky_lib::add_to_linker(&mut linker).unwrap();

    let guest_bytes = include_bytes!("guest.pulley");
    info!("Deserializing component...");
    let component = unsafe { Component::deserialize(&engine, guest_bytes) }.unwrap();

    info!("Instantiating...");
    // 3. Instantiate the component using the `Sos` struct from your `my:sos` world
    let app = Sos::instantiate(&mut store, &component, &linker).unwrap();

    info!("Starting guest...");
    // 4. Call the exported `run` function on the `Sos` instance
    app.call_run(&mut store).unwrap();
}
