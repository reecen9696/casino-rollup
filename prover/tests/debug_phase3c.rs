use prover::circuits::accounting::{AccountingCircuit, Bet};
use prover::proof_generator::ProofGenerator;
use prover::witness_generator::{create_test_settlement_batch, WitnessGenerator};
use std::collections::HashMap;

#[test]
fn test_simple_single_bet_proof() {
    println!("=== Debug: Simple Single Bet Proof ===");

    // Create a minimal case
    let mut generator = ProofGenerator::new(5, 3);
    generator.setup().unwrap();
    println!("✓ Setup completed");

    // Single user, single bet
    let mut initial_balances = HashMap::new();
    initial_balances.insert(0, 10000u64);

    let batch = create_test_settlement_batch(
        1,
        vec![(0, 1000, true, true)], // User 0 bets 1000 on heads, wins
        initial_balances,
        50000,
    );

    println!("✓ Created settlement batch");

    // Debug: Generate witness manually
    let witness_generator = WitnessGenerator::new(5, 3);
    let circuit = witness_generator.generate_witness(&batch).unwrap();

    println!("Generated circuit:");
    println!("  batch_id: {:?}", circuit.batch_id);
    println!("  initial_balances: {:?}", circuit.initial_balances);
    println!("  final_balances: {:?}", circuit.final_balances);
    println!("  house_initial: {:?}", circuit.house_initial);
    println!("  house_final: {:?}", circuit.house_final);
    println!("  bets: {:?}", circuit.bets);

    // Try direct circuit proof
    use ark_bn254::{Bn254, Fr};
    use ark_groth16::Groth16;
    use ark_snark::{CircuitSpecificSetupSNARK, SNARK};
    use ark_std::rand::thread_rng;

    println!("✓ Testing direct circuit proof generation");

    // Setup with the actual circuit
    let mut rng = thread_rng();
    let (pk, vk) = Groth16::<Bn254>::setup(circuit.clone(), &mut rng).unwrap();

    // Extract public inputs in correct order
    let mut public_inputs = vec![circuit.batch_id];
    public_inputs.extend(circuit.initial_balances.clone());
    public_inputs.extend(circuit.final_balances.clone());
    public_inputs.push(circuit.house_initial);
    public_inputs.push(circuit.house_final);

    println!("Public inputs count: {}", public_inputs.len());
    println!("Public inputs: {:?}", public_inputs);

    // Generate proof
    let proof = Groth16::<Bn254>::prove(&pk, circuit, &mut rng).unwrap();
    println!("✓ Proof generated successfully");

    // Verify proof
    let is_valid = Groth16::<Bn254>::verify(&vk, &public_inputs, &proof).unwrap();
    println!("✓ Direct verification result: {}", is_valid);

    assert!(is_valid, "Direct circuit proof should verify");
}

#[test]
fn test_manual_circuit_construction() {
    println!("=== Debug: Manual Circuit Construction ===");

    // Create circuit manually with known values
    let bet = Bet::new(0, 1000, true, true); // User 0, 1000 amount, guess heads, outcome heads (win)

    let initial_balances = [10000u64, 0, 0]; // User 0 has 10k, others have 0
    let final_balances = [11000u64, 0, 0]; // User 0 gains 1k from win

    let circuit = AccountingCircuit::new(
        vec![bet],
        1, // batch_id
        &initial_balances,
        &final_balances,
        50000, // house initial
        49000, // house final (loses 1k to user)
    );

    println!("Manual circuit:");
    println!("  batch_id: {:?}", circuit.batch_id);
    println!("  initial_balances: {:?}", circuit.initial_balances);
    println!("  final_balances: {:?}", circuit.final_balances);
    println!("  house_initial: {:?}", circuit.house_initial);
    println!("  house_final: {:?}", circuit.house_final);

    // Test this circuit
    use ark_bn254::{Bn254, Fr};
    use ark_groth16::Groth16;
    use ark_snark::{CircuitSpecificSetupSNARK, SNARK};
    use ark_std::rand::thread_rng;

    let mut rng = thread_rng();
    let (pk, vk) = Groth16::<Bn254>::setup(circuit.clone(), &mut rng).unwrap();

    let mut public_inputs = vec![circuit.batch_id];
    public_inputs.extend(circuit.initial_balances.clone());
    public_inputs.extend(circuit.final_balances.clone());
    public_inputs.push(circuit.house_initial);
    public_inputs.push(circuit.house_final);

    println!("Manual public inputs: {:?}", public_inputs);

    let proof = Groth16::<Bn254>::prove(&pk, circuit, &mut rng).unwrap();
    let is_valid = Groth16::<Bn254>::verify(&vk, &public_inputs, &proof).unwrap();

    println!("✓ Manual circuit verification: {}", is_valid);
    assert!(is_valid, "Manual circuit should verify");
}

#[test]
fn test_conservation_validation() {
    println!("=== Debug: Conservation Validation ===");

    // Test case: User wins 1000, house loses 1000
    let bet = Bet::new(0, 1000, true, true);
    println!(
        "Bet: user_id={}, amount={}, guess={}, outcome={}, won={}",
        bet.user_id,
        bet.amount,
        bet.guess,
        bet.outcome,
        bet.won()
    );

    let user_delta = if bet.won() {
        bet.amount as i64
    } else {
        -(bet.amount as i64)
    };
    let house_delta = -user_delta;

    println!("User delta: {}", user_delta);
    println!("House delta: {}", house_delta);
    println!(
        "Conservation check: {} + {} = {}",
        user_delta,
        house_delta,
        user_delta + house_delta
    );

    assert_eq!(user_delta + house_delta, 0, "Conservation should hold");
    println!("✓ Conservation validated");
}
