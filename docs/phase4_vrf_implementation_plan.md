# Phase 4: VRF + Fairness Plumbing - Design & Implementation Plan

**Date**: October 22, 2025  
**Status**: Planning Phase  
**Target**: Unbiased, auditable RNG (off-circuit for MVP)

## 🎯 **Phase 4 Overview**

Phase 4 introduces **Verifiable Random Functions (VRF)** to replace the current CSPRNG with cryptographically verifiable randomness. This ensures fairness and auditability while maintaining the sub-second bet processing performance achieved in Phase 3.

### **Key Requirements from requirements.txt:**

- **Technology**: ed25519-dalek keypair for VRF operations
- **Message Format**: `H(bet_id||user||nonce)` for deterministic input
- **Outcome Derivation**: `LSB(VRF_sign(msg))` determines coin flip result
- **Storage**: Store `(msg, sig, pubkey)` with each bet for auditability
- **Verification**: Client library can verify signatures independently
- **Fairness**: Distribution ≈ fair over 1M+ simulations
- **Security**: Replay protection via user nonce

## 📋 **Implementation Breakdown: 6 Manageable Chunks**

### **Phase 4a: VRF Keypair Setup (1 day)**

**Goal**: Establish cryptographic foundation for VRF operations

**Key Components**:

- Ed25519 keypair generation and secure storage
- Key rotation mechanism for operational security
- Environment variable configuration
- Integration with existing sequencer architecture

**Files to Create**:

```
sequencer/src/vrf/
├── mod.rs              # VRF module entry point
├── keypair.rs          # Key generation and management
└── tests/
    └── keypair_tests.rs # Unit tests
```

**Acceptance Criteria**:

- ✅ Secure ed25519 keypair generation
- ✅ Key persistence and loading from environment
- ✅ Key validation and error handling
- ✅ Unit tests with 100% coverage

### **Phase 4b: VRF Message Generation (1 day)**

**Goal**: Implement deterministic message creation for VRF input

**Key Components**:

- SHA-256 hash function for message generation
- Standardized format: `bet_id||user_pubkey||nonce`
- Input validation and sanitization
- Performance optimization (target: <1ms per message)

**Files to Create**:

```
sequencer/src/vrf/
├── message.rs          # Message generation logic
└── tests/
    └── message_tests.rs # Unit tests with test vectors
```

**Message Format Specification**:

```rust
// Input: bet_id (String), user_pubkey (Pubkey), nonce (u64)
// Output: SHA-256(bet_id || user_pubkey.to_bytes() || nonce.to_le_bytes())
fn generate_vrf_message(bet_id: &str, user: &Pubkey, nonce: u64) -> [u8; 32]
```

### **Phase 4c: VRF Signature Generation (2 days)**

**Goal**: Core VRF functionality - signature generation and outcome derivation

**Key Components**:

- VRF signature generation using ed25519-dalek
- Outcome derivation: `coin_flip = LSB(VRF_signature) == 0`
- Signature verification functionality
- VRF data structure for storage and transport

**Files to Create**:

```
sequencer/src/vrf/
├── signature.rs        # VRF signature operations
├── outcome.rs          # Outcome derivation logic
└── tests/
    └── signature_tests.rs # Comprehensive test suite
```

**VRF Data Structure**:

```rust
#[derive(Serialize, Deserialize, Clone)]
pub struct VRFProof {
    pub message: [u8; 32],
    pub signature: [u8; 64],
    pub public_key: [u8; 32],
    pub outcome: bool, // true = heads, false = tails
}
```

### **Phase 4d: Sequencer VRF Integration (2 days)**

**Goal**: Replace CSPRNG with VRF in the bet processing pipeline

**Key Components**:

- Modify bet processing to use VRF instead of `rand::random()`
- Update settlement persistence to store VRF proofs
- Enhance API responses to include VRF data
- Maintain backward compatibility and performance

**Files to Modify**:

```
sequencer/src/
├── main.rs                    # Update bet processing
└── settlement_persistence.rs  # Store VRF proofs
```

**Files to Create**:

```
tests/phase4/
├── test-vrf-integration.sh    # Integration test script
└── vrf_integration_tests.rs   # Rust integration tests
```

**Performance Requirements**:

- Maintain sub-second bet processing (<300ms p95)
- VRF generation: <5ms per signature
- No degradation in settlement batch processing

### **Phase 4e: VRF Auditability & Verification (2 days)**

**Goal**: Enable third-party verification and explorer integration

**Key Components**:

- Client library for VRF signature verification
- API endpoints to expose VRF data
- Explorer UI components for VRF display
- JavaScript/TypeScript verification examples

**Files to Create**:

```
client/vrf-verify/
├── package.json
├── src/
│   ├── verify.ts              # Main verification library
│   └── examples/
│       └── verify_bet.js      # Usage examples

explorer/src/components/
└── VRFVerification.tsx        # Explorer UI component

docs/
└── vrf_verification_guide.md  # Third-party integration docs
```

**API Enhancements**:

```rust
// New endpoint: GET /v1/bets/:id/vrf
pub struct VRFResponse {
    pub bet_id: String,
    pub vrf_proof: VRFProof,
    pub verification_status: bool,
}
```

### **Phase 4f: Fairness Testing & Validation (2 days)**

**Goal**: Comprehensive validation of fairness and security properties

**Key Components**:

- 1M+ bet simulation with statistical analysis
- Chi-square test for randomness validation
- Replay attack prevention testing
- Performance benchmarking under load

**Files to Create**:

```
tests/phase4/
├── test-fairness-simulation.sh   # Main fairness test
├── fairness_analyzer.py          # Statistical analysis
├── test-replay-protection.sh     # Security testing
└── performance_benchmarks.rs     # Performance validation

docs/
└── phase4_fairness_audit_report.md # Compliance report
```

**Fairness Validation Targets**:

- **Distribution**: 49.9% - 50.1% heads/tails over 1M+ trials
- **Chi-square test**: p-value > 0.05 (not significantly biased)
- **Replay protection**: 100% nonce uniqueness enforcement
- **Performance**: <5ms VRF generation, <1ms verification

## 🧪 **Testing Strategy**

### **Unit Testing (Each Phase)**

- Component-specific tests with 100% code coverage
- Test vectors from ed25519 specification
- Error condition and edge case validation
- Performance benchmarks for each component

### **Integration Testing**

- Full pipeline: Bet → VRF → Settlement → Verification
- Multi-batch processing with VRF signatures
- API endpoint validation with VRF data
- Explorer integration with real VRF proofs

### **Fairness & Security Testing**

- **Monte Carlo Simulation**: 1M+ bets with distribution analysis
- **Replay Attack Testing**: Duplicate nonce rejection
- **Performance Load Testing**: High-concurrency VRF generation
- **Third-party Verification**: Independent signature validation

### **Regression Testing**

- Ensure Phase 3 functionality remains intact
- Settlement persistence compatibility
- Performance targets maintained (<300ms p95)
- All existing test suites continue passing

## 🔄 **Integration with Existing Architecture**

### **Settlement Pipeline Enhancement**

```rust
// Enhanced SettlementItem with VRF data
pub struct SettlementItem {
    pub bet_id: String,
    pub user_id: String,
    pub amount: u64,
    pub outcome: bool,
    pub payout: u64,
    pub vrf_proof: Option<VRFProof>, // New field
    pub timestamp: DateTime<Utc>,
}
```

### **API Response Updates**

```rust
// Enhanced BetResponse with VRF verification
pub struct BetResponse {
    pub bet_id: String,
    pub outcome: bool,
    pub payout: u64,
    pub status: String,
    pub vrf_proof: VRFProof, // New field for auditability
}
```

### **Database Schema Evolution**

```json
// Enhanced settlement batch format
{
  "batch_id": 1,
  "status": "Confirmed",
  "transaction_signature": "...",
  "items": [
    {
      "bet_id": "bet-123",
      "user_id": "user-456",
      "amount": 1000000,
      "outcome": true,
      "payout": 1950000,
      "vrf_proof": {
        "message": "0x...",
        "signature": "0x...",
        "public_key": "0x...",
        "outcome": true
      }
    }
  ]
}
```

## 📈 **Success Metrics**

### **Functional Requirements**

- ✅ Ed25519-dalek VRF operational
- ✅ Deterministic message generation working
- ✅ VRF signatures verifiable by third parties
- ✅ Client verification library functional
- ✅ Explorer displays VRF data correctly

### **Performance Requirements**

- ✅ Bet processing remains <300ms p95
- ✅ VRF generation <5ms per signature
- ✅ Verification <1ms per signature
- ✅ No regression in settlement performance

### **Security & Fairness Requirements**

- ✅ Distribution 49.9-50.1% over 1M+ simulations
- ✅ Chi-square p-value > 0.05 (not biased)
- ✅ 100% replay attack prevention
- ✅ Nonce uniqueness enforced

### **Auditability Requirements**

- ✅ All VRF data stored and retrievable
- ✅ Third-party verification possible
- ✅ API exposes complete VRF proofs
- ✅ Explorer provides verification UI

## 🚀 **Deployment Strategy**

### **Development Environment**

1. Implement and test each phase incrementally
2. Maintain backward compatibility with CSPRNG
3. Feature flag for VRF enable/disable
4. Comprehensive unit and integration testing

### **Testnet Deployment**

1. Deploy VRF-enabled sequencer to testnet
2. Run 1M+ simulation for fairness validation
3. Third-party verification testing
4. Performance validation under load

### **Production Readiness**

1. Security audit of VRF implementation
2. Fairness certification from independent auditor
3. Performance benchmarks under production load
4. Monitoring and alerting for VRF operations

## 🔮 **Future Considerations**

### **Circuit Integration (Post-MVP)**

Phase 4 keeps VRF **off-circuit** for MVP simplicity. Future enhancements:

- **Phase 4+**: Move VRF verification into ZK circuit
- **SP1 Integration**: Use SP1 zkVM for in-circuit signature verification
- **Scalability**: Batch VRF verification for multiple bets

### **Advanced VRF Features**

- **Threshold VRF**: Multi-party VRF for decentralization
- **VRF Committees**: Distributed randomness generation
- **On-chain VRF**: Solana VRF instruction integration (when available)

---

**Next Steps**: Begin Phase 4a implementation with VRF keypair setup and establish the cryptographic foundation for verifiable randomness.
