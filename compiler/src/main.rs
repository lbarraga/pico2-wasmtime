use std::fs;
use wasmtime::{Config, Engine};

fn main() -> anyhow::Result<()> {
    println!("🛠️  Initializing Custom Compiler (Wasmtime v40)...");

    let mut config = Config::new();

    config.target("pulley32")?;
    config.wasm_gc(false);
    config.gc_support(false);
    config.wasm_threads(false);
    config.wasm_bulk_memory(false);
    config.wasm_reference_types(false);
    config.wasm_multi_memory(false);
    config.wasm_memory64(false);
    config.memory_init_cow(false);
    config.wasm_component_model(false);

    let engine = Engine::new(&config)?;
    let input_path = "../led/smoke.wat";

    println!("📂 Reading WAT from: {}", input_path);

    let wat_string = fs::read_to_string(input_path)?;
    let wasm_bytes = wat::parse_str(&wat_string)?;
    let serialized_bytes = engine.precompile_module(&wasm_bytes)?;
    let output_path = "../led/src/smoke.pulley";

    fs::write(output_path, &serialized_bytes)?;

    println!(
        "✅ Success! Wrote {} bytes to {}",
        serialized_bytes.len(),
        output_path
    );
    println!("   - Target: pulley32");
    println!("   - GC: Disabled");

    Ok(())
}
