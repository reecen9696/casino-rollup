use prover::circuits::accounting::{AccountingCircuit, Bet, BetBatch};
use prover::proof_generator::{ProofError, ProofGenerator, SerializableProof};
use prover::witness_generator::{
    create_test_settlement_batch, SettlementBatch, SettlementBet, WitnessError, WitnessGenerator,
};
use std::collections::HashMap;
use std::time::Instant;

#[test]
fn test_phase3c_complete_integration() {
    println!("=== Phase 3c Integration Test: Complete Proof Generation Pipeline ===");

    // Setup proof generator
    let mut generator = ProofGenerator::new(10, 5);
    let start = Instant::now();
    generator.setup().unwrap();
    let setup_time = start.elapsed();
    println!("✓ Setup completed in {:?}", setup_time);

    // Create realistic settlement batch
    let mut initial_balances = HashMap::new();
    initial_balances.insert(0, 25000); // Alice: 25,000
    initial_balances.insert(1, 18000); // Bob: 18,000
    initial_balances.insert(2, 32000); // Charlie: 32,000

    let batch = create_test_settlement_batch(
        12345,
        vec![
            (0, 5000, true, true),   // Alice bets 5k on heads, wins (+5k)
            (1, 3000, false, true),  // Bob bets 3k on tails, loses (-3k)
            (2, 8000, true, false),  // Charlie bets 8k on heads, loses (-8k)
            (0, 2000, false, false), // Alice bets 2k on tails, wins (+2k)
            (1, 1500, true, true),   // Bob bets 1.5k on heads, wins (+1.5k)
        ],
        initial_balances,
        500000, // House starts with 500k
    );

    println!("✓ Created settlement batch with {} bets", batch.bets.len());

    // Generate proof
    let start = Instant::now();
    let proof = generator.generate_proof(&batch).unwrap();
    let proving_time = start.elapsed();
    println!("✓ Proof generated in {:?}", proving_time);

    // Verify proof
    let start = Instant::now();
    let is_valid = generator.verify_proof(&proof).unwrap();
    let verification_time = start.elapsed();
    println!("✓ Proof verified in {:?}: {}", verification_time, is_valid);

    assert!(is_valid);
    assert_eq!(proof.batch_id, 12345);

    // Test serialization round-trip
    let start = Instant::now();
    let serialized = proof.to_bytes().unwrap();
    let deserialized = SerializableProof::from_bytes(&serialized).unwrap();
    let serialization_time = start.elapsed();

    println!("✓ Serialization round-trip in {:?}", serialization_time);
    println!("  Serialized size: {} bytes", serialized.len());

    // Verify deserialized proof
    let is_valid_deserialized = generator.verify_proof(&deserialized).unwrap();
    assert!(is_valid_deserialized);

    // Performance summary
    println!("\n=== Performance Summary ===");
    println!("Setup time:        {:?}", setup_time);
    println!("Proving time:      {:?}", proving_time);
    println!("Verification time: {:?}", verification_time);
    println!("Serialization:     {:?}", serialization_time);
    println!("Proof size:        {} bytes", serialized.len());

    // Verify performance targets
    assert!(
        proving_time.as_millis() < 1000,
        "Proving should be under 1 second"
    );
    assert!(
        verification_time.as_millis() < 200,
        "Verification should be under 200ms"
    );

    println!("✓ All performance targets met!");
}

#[test]
fn test_witness_generation_error_handling() {
    println!("=== Phase 3c Test: Witness Generation Error Handling ===");

    let generator = WitnessGenerator::new(10, 5);

    // Test 1: Empty batch
    let empty_batch = create_test_settlement_batch(1, vec![], HashMap::new(), 50000);
    let result = generator.generate_witness(&empty_batch);
    assert!(matches!(result, Err(WitnessError::EmptyBatch)));
    println!("✓ Empty batch error handled correctly");

    // Test 2: Insufficient balance
    let mut initial_balances = HashMap::new();
    initial_balances.insert(0, 1000); // Only 1000 balance

    let insufficient_batch = create_test_settlement_batch(
        2,
        vec![(0, 5000, true, true)], // Bet 5000 > 1000 balance
        initial_balances,
        50000,
    );

    let result = generator.generate_witness(&insufficient_batch);
    assert!(matches!(
        result,
        Err(WitnessError::InsufficientBalance { .. })
    ));
    println!("✓ Insufficient balance error handled correctly");

    // Test 3: Unknown user
    let mut initial_balances = HashMap::new();
    initial_balances.insert(0, 10000);

    let unknown_user_batch = create_test_settlement_batch(
        3,
        vec![(99, 1000, true, true)], // User 99 not in initial_balances
        initial_balances,
        50000,
    );

    let result = generator.generate_witness(&unknown_user_batch);
    assert!(matches!(result, Err(WitnessError::UnknownUser { .. })));
    println!("✓ Unknown user error handled correctly");

    // Test 4: Batch too large
    let small_generator = WitnessGenerator::new(2, 5); // Max 2 bets
    let mut initial_balances = HashMap::new();
    initial_balances.insert(0, 50000);

    let large_batch = create_test_settlement_batch(
        4,
        vec![
            (0, 1000, true, true),
            (0, 1000, true, false),
            (0, 1000, false, true), // 3 bets > 2 max
        ],
        initial_balances,
        50000,
    );

    let result = small_generator.generate_witness(&large_batch);
    assert!(matches!(result, Err(WitnessError::BatchTooLarge { .. })));
    println!("✓ Batch too large error handled correctly");

    println!("✓ All error handling tests passed!");
}

#[test]
fn test_deterministic_proof_generation() {
    println!("=== Phase 3c Test: Deterministic Proof Generation ===");

    let mut generator = ProofGenerator::new(5, 3);
    generator.setup().unwrap();

    let mut initial_balances = HashMap::new();
    initial_balances.insert(0, 10000);
    initial_balances.insert(1, 15000);

    let batch = create_test_settlement_batch(
        42,
        vec![
            (0, 1000, true, true),   // Alice wins
            (1, 2000, false, false), // Bob wins
        ],
        initial_balances,
        100000,
    );

    // Generate multiple proofs with same seed
    let seed = 987654321u64;
    let proof1 = generator
        .generate_deterministic_proof(&batch, seed)
        .unwrap();
    let proof2 = generator
        .generate_deterministic_proof(&batch, seed)
        .unwrap();
    let proof3 = generator
        .generate_deterministic_proof(&batch, seed)
        .unwrap();

    // All proofs should be identical
    let bytes1 = proof1.to_bytes().unwrap();
    let bytes2 = proof2.to_bytes().unwrap();
    let bytes3 = proof3.to_bytes().unwrap();

    assert_eq!(bytes1, bytes2);
    assert_eq!(bytes2, bytes3);
    println!("✓ Deterministic proof generation confirmed");

    // Generate proof with different seed
    let proof4 = generator
        .generate_deterministic_proof(&batch, seed + 1)
        .unwrap();
    let bytes4 = proof4.to_bytes().unwrap();

    assert_ne!(bytes1, bytes4);
    println!("✓ Different seeds produce different proofs");

    // All proofs should still verify
    assert!(generator.verify_proof(&proof1).unwrap());
    assert!(generator.verify_proof(&proof2).unwrap());
    assert!(generator.verify_proof(&proof3).unwrap());
    assert!(generator.verify_proof(&proof4).unwrap());
    println!("✓ All deterministic proofs verify correctly");
}

#[test]
fn test_settlement_batch_validation() {
    println!("=== Phase 3c Test: Settlement Batch Validation ===");

    let mut generator = ProofGenerator::new(10, 5);
    generator.setup().unwrap();

    // Valid batch
    let mut initial_balances = HashMap::new();
    initial_balances.insert(0, 20000);
    initial_balances.insert(1, 15000);

    let valid_batch = create_test_settlement_batch(
        1,
        vec![(0, 5000, true, true), (1, 3000, false, false)],
        initial_balances.clone(),
        100000,
    );

    let result = generator.validate_settlement_batch(&valid_batch);
    assert!(result.is_ok());
    println!("✓ Valid batch validation passed");

    // Invalid batch (insufficient balance)
    let invalid_batch = create_test_settlement_batch(
        2,
        vec![(0, 25000, true, true)], // More than 20k balance
        initial_balances,
        100000,
    );

    let result = generator.validate_settlement_batch(&invalid_batch);
    assert!(result.is_err());
    println!("✓ Invalid batch validation failed as expected");
}

#[test]
fn test_conservation_law_enforcement() {
    println!("=== Phase 3c Test: Conservation Law Enforcement ===");

    let generator = WitnessGenerator::new(10, 5);

    // Test balanced scenario (conservation holds)
    let mut initial_balances = HashMap::new();
    initial_balances.insert(0, 20000);
    initial_balances.insert(1, 25000);
    initial_balances.insert(2, 18000);

    let balanced_batch = create_test_settlement_batch(
        1,
        vec![
            (0, 5000, true, true),  // User 0: +5000
            (1, 8000, false, true), // User 1: -8000
            (2, 3000, true, true),  // User 2: +3000
        ],
        initial_balances,
        500000,
    );

    let circuit = generator.generate_witness(&balanced_batch).unwrap();

    // Verify conservation: User gains = +5000 + 3000 = +8000, User losses = -8000
    // House should lose -8000 + 8000 = 0 net
    println!("✓ Balanced conservation scenario validated");

    // Calculate expected values
    let user0_final = 20000 + 5000; // Won 5k
    let user1_final = 25000 - 8000; // Lost 8k
    let user2_final = 18000 + 3000; // Won 3k
    let house_final = 500000; // No net change (8k out, 8k in)

    // Verify final balances in circuit
    use ark_bn254::Fr;
    assert_eq!(circuit.final_balances[0], Fr::from(user0_final));
    assert_eq!(circuit.final_balances[1], Fr::from(user1_final));
    assert_eq!(circuit.final_balances[2], Fr::from(user2_final));
    assert_eq!(circuit.house_final, Fr::from(house_final));

    println!("✓ Conservation law correctly enforced in circuit");
}

#[test]
fn test_verifying_key_extraction() {
    println!("=== Phase 3c Test: Verifying Key Extraction ===");

    let mut generator = ProofGenerator::new(5, 3);

    // Before setup
    assert!(generator.get_verifying_key().is_none());
    let vk_result = generator.serialize_verifying_key();
    assert!(matches!(vk_result, Err(ProofError::InvalidParameters)));

    // After setup
    generator.setup().unwrap();
    assert!(generator.get_verifying_key().is_some());

    let vk_bytes = generator.serialize_verifying_key().unwrap();
    assert!(!vk_bytes.is_empty());
    println!("✓ Verifying key serialized: {} bytes", vk_bytes.len());

    // Verify we can deserialize (basic sanity check)
    use ark_bn254::Bn254;
    use ark_groth16::VerifyingKey;
    use ark_serialize::CanonicalDeserialize;

    let _vk = VerifyingKey::<Bn254>::deserialize_compressed(&vk_bytes[..]).unwrap();
    println!("✓ Verifying key deserialization confirmed");
}

#[test]
fn test_edge_case_scenarios() {
    println!("=== Phase 3c Test: Edge Case Scenarios ===");

    let mut generator = ProofGenerator::new(10, 5);
    generator.setup().unwrap();

    // Test 1: Single user, single bet
    let mut initial_balances = HashMap::new();
    initial_balances.insert(0, 10000);

    let single_bet_batch = create_test_settlement_batch(
        1,
        vec![(0, 1000, true, false)], // Single losing bet
        initial_balances,
        50000,
    );

    let proof = generator.generate_proof(&single_bet_batch).unwrap();
    assert!(generator.verify_proof(&proof).unwrap());
    println!("✓ Single bet scenario");

    // Test 2: All users win
    let mut initial_balances = HashMap::new();
    initial_balances.insert(0, 10000);
    initial_balances.insert(1, 10000);

    let all_win_batch = create_test_settlement_batch(
        2,
        vec![
            (0, 1000, true, true),   // User 0 wins
            (1, 2000, false, false), // User 1 wins
        ],
        initial_balances,
        100000, // House needs enough to cover payouts
    );

    let proof = generator.generate_proof(&all_win_batch).unwrap();
    assert!(generator.verify_proof(&proof).unwrap());
    println!("✓ All users win scenario");

    // Test 3: All users lose
    let mut initial_balances = HashMap::new();
    initial_balances.insert(0, 10000);
    initial_balances.insert(1, 10000);

    let all_lose_batch = create_test_settlement_batch(
        3,
        vec![
            (0, 1000, true, false), // User 0 loses
            (1, 2000, false, true), // User 1 loses
        ],
        initial_balances,
        50000,
    );

    let proof = generator.generate_proof(&all_lose_batch).unwrap();
    assert!(generator.verify_proof(&proof).unwrap());
    println!("✓ All users lose scenario");

    // Test 4: Maximum batch size
    let mut initial_balances = HashMap::new();
    for i in 0..5 {
        initial_balances.insert(i, 20000);
    }

    let mut max_bets = Vec::new();
    for i in 0..10 {
        let user_id = i % 5;
        let outcome = i % 2 == 0;
        max_bets.push((user_id, 1000, true, outcome));
    }

    let max_batch = create_test_settlement_batch(4, max_bets, initial_balances, 200000);

    let proof = generator.generate_proof(&max_batch).unwrap();
    assert!(generator.verify_proof(&proof).unwrap());
    println!("✓ Maximum batch size scenario");

    println!("✓ All edge case scenarios passed!");
}

#[test]
fn test_performance_benchmarks() {
    println!("=== Phase 3c Test: Performance Benchmarks ===");

    let mut generator = ProofGenerator::new(20, 10); // Larger for performance test

    let start = Instant::now();
    generator.setup().unwrap();
    let setup_time = start.elapsed();
    println!("Setup time (larger circuit): {:?}", setup_time);

    // Create larger batch for performance testing
    let mut initial_balances = HashMap::new();
    for i in 0..10 {
        initial_balances.insert(i, 50000);
    }

    let mut perf_bets = Vec::new();
    for i in 0..15 {
        let user_id = i % 10;
        let amount = 1000 + (i * 100) as u64;
        let guess = i % 2 == 0;
        let outcome = i % 3 == 0; // Mix of wins/losses
        perf_bets.push((user_id, amount, guess, outcome));
    }

    let perf_batch = create_test_settlement_batch(999, perf_bets, initial_balances, 1000000);

    // Benchmark proof generation
    let start = Instant::now();
    let proof = generator.generate_proof(&perf_batch).unwrap();
    let proving_time = start.elapsed();
    println!("Proving time (15 bets): {:?}", proving_time);

    // Benchmark verification
    let start = Instant::now();
    let is_valid = generator.verify_proof(&proof).unwrap();
    let verification_time = start.elapsed();
    println!("Verification time: {:?}", verification_time);

    assert!(is_valid);

    // Benchmark serialization
    let start = Instant::now();
    let serialized = proof.to_bytes().unwrap();
    let serialization_time = start.elapsed();
    println!(
        "Serialization time: {:?}, size: {} bytes",
        serialization_time,
        serialized.len()
    );

    // Performance assertions
    assert!(
        proving_time.as_millis() < 2000,
        "Proving should be under 2 seconds for large batch"
    );
    assert!(
        verification_time.as_millis() < 500,
        "Verification should be under 500ms"
    );

    println!("✓ Performance benchmarks passed!");
}

#[test]
fn test_malformed_settlement_data_handling() {
    println!("=== Phase 3c Test: Malformed Settlement Data Handling ===");

    let generator = WitnessGenerator::new(10, 5);

    // Test: User ID exceeds max_users
    let mut initial_balances = HashMap::new();
    initial_balances.insert(0, 10000);
    initial_balances.insert(10, 10000); // User ID 10 >= max_users (5)

    let invalid_user_batch = create_test_settlement_batch(
        1,
        vec![(10, 1000, true, true)], // User 10 is invalid
        initial_balances,
        50000,
    );

    let result = generator.generate_witness(&invalid_user_batch);
    assert!(result.is_err());
    println!("✓ Invalid user ID handling");

    // Test: Zero amount bet (edge case)
    let mut initial_balances = HashMap::new();
    initial_balances.insert(0, 10000);

    let zero_bet_batch = create_test_settlement_batch(
        2,
        vec![(0, 0, true, true)], // Zero amount bet
        initial_balances,
        50000,
    );

    // This should work (zero bet is technically valid)
    let result = generator.generate_witness(&zero_bet_batch);
    assert!(result.is_ok());
    println!("✓ Zero amount bet handling");

    // Test: Extremely large bet amount
    let mut initial_balances = HashMap::new();
    initial_balances.insert(0, u64::MAX); // Maximum balance

    let large_bet_batch = create_test_settlement_batch(
        3,
        vec![(0, u64::MAX, true, true)], // Maximum bet
        initial_balances,
        u64::MAX,
    );

    let result = generator.generate_witness(&large_bet_batch);
    // This should work as long as balances are sufficient
    assert!(result.is_ok());
    println!("✓ Large bet amount handling");

    println!("✓ Malformed data handling tests completed!");
}
