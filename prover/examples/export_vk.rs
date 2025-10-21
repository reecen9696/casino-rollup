use prover::proof_generator::ProofGenerator;
use std::fs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut generator = ProofGenerator::new(100, 1000); // max_batch_size, max_users

    // Setup to generate verifying key
    generator.setup()?;

    // Serialize the verifying key
    let vk_bytes = generator.serialize_verifying_key()?;

    // Write as hex bytes for embedding in Rust code
    println!("// Verifying key bytes (length: {})", vk_bytes.len());
    print!("const VERIFYING_KEY_BYTES: &[u8] = &[");

    for (i, byte) in vk_bytes.iter().enumerate() {
        if i % 16 == 0 {
            print!("\n    ");
        }
        print!("0x{:02x}, ", byte);
    }

    println!("\n];");

    // Also write to file for reference
    fs::write("verifying_key.bin", &vk_bytes)?;
    println!(
        "\n// Written to verifying_key.bin ({} bytes)",
        vk_bytes.len()
    );

    Ok(())
}
