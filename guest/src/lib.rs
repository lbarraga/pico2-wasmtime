use wit_bindgen::generate;

generate!({
    world: "blinky",
    path: "wit",
});

// Rename your struct to avoid conflict with the generated 'Guest' trait
struct MyBlinky;

impl Guest for MyBlinky {
    fn run() {
        loop {
            // These calls go out to the Host (RP2350)
            host::on();
            host::delay(500);
            host::off();
            host::delay(500);
        }
    }
}

// Export the implementation struct
export!(MyBlinky);
