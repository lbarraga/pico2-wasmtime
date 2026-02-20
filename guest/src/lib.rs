use wit_bindgen::generate;

generate!({
    path: "wit", // Path relative to the guest crate
    world: "sos",
    with: {
        "wasi:blinky/blinky": generate,
    }
});

use wasi::blinky::blinky as host;

struct MyGuest;

impl MyGuest {
    fn dot() {
        host::on();
        host::delay(100); // Short flash (100ms)
        host::off();
        host::delay(100); // Short gap
    }

    fn dash() {
        host::on();
        host::delay(400); // Long flash (400ms)
        host::off();
        host::delay(100); // Short gap
    }

    fn sos() {
        // S: ...
        for _ in 0..3 {
            Self::dot();
        }
        host::delay(200); // Gap between letters

        // O: ---
        for _ in 0..3 {
            Self::dash();
        }
        host::delay(200); // Gap between letters

        // S: ...
        for _ in 0..3 {
            Self::dot();
        }

        // Wait before repeating the whole word
        host::delay(1000);
    }
}

impl Guest for MyGuest {
    fn run() {
        loop {
            Self::sos();
        }
    }
}

export!(MyGuest);
