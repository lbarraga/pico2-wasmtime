#![no_std]
#![no_main]

extern crate alloc;

use defmt::info;
use embassy_executor::Spawner;
use embassy_rp::gpio::{Level, Output};
use embedded_alloc::Heap;
use {defmt_rtt as _, panic_probe as _};

use wasmtime::component::{Component, HasSelf, Linker};
use wasmtime::{Config, Engine, Store};

#[global_allocator]
static ALLOCATOR: Heap = Heap::empty();

wasmtime::component::bindgen!({
    path: "../guest/wit/pico.wit",
    world: "blinky",
});

pub struct HostState {
    pub led: Output<'static>,
}

impl host::Host for HostState {
    fn on(&mut self) {
        self.led.set_high();
        info!("Guest: ON");
    }
    fn off(&mut self) {
        self.led.set_low();
        info!("Guest: OFF");
    }
    fn delay(&mut self, ms: u32) {
        embassy_time::block_for(embassy_time::Duration::from_millis(ms as u64));
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
    let mut store = Store::new(&engine, HostState { led });
    let mut linker = Linker::new(&engine);

    Blinky::add_to_linker::<HostState, HasSelf<HostState>>(&mut linker, |state: &mut HostState| {
        state
    })
    .unwrap();

    let guest_bytes = include_bytes!("guest.pulley");
    info!("Step 6: Deserializing component...");
    let component = unsafe { Component::deserialize(&engine, guest_bytes) }.unwrap();

    info!("Step 7: Instantiating...");
    let blinky = Blinky::instantiate(&mut store, &component, &linker).unwrap();

    info!("Starting guest...");
    blinky.call_run(&mut store).unwrap();
}
