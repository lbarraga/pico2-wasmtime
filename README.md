# run wasmtime in pico 2: blinky

```bash
cargo build -p guest --target wasm32-unknown-unknown --release
cargo run -p compiler -- unknown
cd pico2-quick
cargo run --release
```

