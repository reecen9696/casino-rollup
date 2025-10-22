#[cfg(test)]
mod integration_tests {
    use sequencer::vrf::{generate_vrf_message, generate_vrf_message_from_string};

    #[test]
    fn test_deterministic_message_generation() {
        let bet_id = 12345u64;
        let user = [0x42u8; 32];
        let nonce = 67890u64;

        // Generate multiple messages with same inputs
        let msg1 = generate_vrf_message(bet_id, &user, nonce);
        let msg2 = generate_vrf_message(bet_id, &user, nonce);
        let msg3 = generate_vrf_message(bet_id, &user, nonce);

        assert_eq!(msg1, msg2);
        assert_eq!(msg2, msg3);

        // Test string version
        let str_msg1 = generate_vrf_message_from_string("bet_12345", &user, nonce);
        let str_msg2 = generate_vrf_message_from_string("bet_12345", &user, nonce);
        
        assert_eq!(str_msg1, str_msg2);
    }

    #[test]
    fn test_message_uniqueness() {
        let user = [0x42u8; 32];
        let nonce = 67890u64;

        // Different bet IDs should produce different messages
        let msg1 = generate_vrf_message(1, &user, nonce);
        let msg2 = generate_vrf_message(2, &user, nonce);
        assert_ne!(msg1, msg2);

        // Different users should produce different messages
        let user2 = [0x43u8; 32];
        let msg3 = generate_vrf_message(1, &user2, nonce);
        assert_ne!(msg1, msg3);

        // Different nonces should produce different messages
        let msg4 = generate_vrf_message(1, &user, nonce + 1);
        assert_ne!(msg1, msg4);
    }
}
