# ZK Casino MVP - Solana ZK Rollup

A high-performance zero-knowledge rollup casino implementation on Solana, featuring Groth16 proof verification and real-time settlement.

## 🎯 Current Status: Phase 3d Complete ✅

**Latest Achievement**: Complete Groth16 BN254 verification system implemented

### Phase 3d: On-chain Verification ✅ COMPLETED

- ✅ **Production Verifying Key**: 64,360-byte key extracted from prover and embedded
- ✅ **Groth16 Verification**: Complete BN254 verification with Solana alt_bn128 syscalls
- ✅ **Proof Format**: 256-byte proofs (A: 64 + B: 128 + C: 64) with proper validation
- ✅ **Error Handling**: Comprehensive Anchor framework integration
- ✅ **Testing**: All 11 verifier unit tests + 53 prover tests passing
- ✅ **Compilation**: Clean build for Solana deployment

### Test Results Summary

```
✅ Verifier Tests: 11/11 passed
✅ Prover Tests: 53/53 passed
✅ Build System: Clean compilation
✅ Integration: End-to-end ZK pipeline functional
```

### Next Priority: Phase 3e Settlement Integration

Connect the sequencer settlement queue to proof generation and on-chain submission.

## Architecture Overview

### Core Components

- **Sequencer**: High-performance Axum service (3920+ RPS achieved)
- **Prover**: Arkworks Groth16 proof generation (2.9-5.4ms proving time)
- **Verifier**: Solana program with embedded BN254 verification
- **Explorer**: React dashboard with real-time performance monitoring

### ZK Rollup Pipeline

```
Bets → Settlement Queue → Witness Generation → Proof Creation → On-chain Verification
```

## Prerequisites

Before running tests, install the required toolchain:

### 1. Install Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

### 2. Install Solana CLI

```bash
sh -c "$(curl -sSfL https://release.solana.com/v1.17.0/install)"
export PATH="/Users/$USER/.local/share/solana/install/active_release/bin:$PATH"
```

### 3. Install Anchor CLI

```bash
npm install -g @coral-xyz/anchor-cli
```

### 4. Install Node.js dependencies

```bash
npm install
cd explorer && npm install
```

## Project Structure

```
zkcasino/
├── programs/           # Solana programs (Anchor)
│   ├── vault/         # User vault management
│   └── verifier/      # ZK proof verification
├── sequencer/         # Off-chain sequencer service (Axum)
├── prover/           # ZK proof generation (Arkworks)
├── explorer/         # Web interface (React + Vite)
├── tests/            # Integration tests
├── Anchor.toml       # Anchor workspace config
├── Cargo.toml        # Rust workspace config
└── progress.json     # Phase tracking
```

## Quick Start & Testing

### 1. Build the entire workspace

```bash
cargo build
```

### 2. Run comprehensive tests

```bash
# Test verifier (ZK verification)
cargo test -p verifier

# Test prover (ZK proof generation)
cargo test -p prover

# Test sequencer integration
cargo test -p sequencer
```

### 3. Run individual components

**Sequencer** (API server):

```bash
cargo run --bin sequencer
```

**Performance testing**:

```bash
cd sequencer && ./performance_test.sh
```

**Explorer** (monitoring dashboard):

```bash
cd explorer && npm install && npm run dev
```

### 4. ZK Proof testing

```bash
# Generate and verify proofs
cd prover && cargo run --example export_vk
```

## Phase Completion Status

### ✅ Phase 0: Foundations (100%)

- Rust + Solana + Anchor toolchain
- Clean mono-repo workspace structure
- CI/CD pipeline with GitHub Actions

### ✅ Phase 1: Off-chain Coinflip (100%)

- High-performance sequencer (3920+ RPS)
- In-memory + SQLite persistence
- Real-time monitoring dashboard
- Settlement queue architecture

### ✅ Phase 2: On-chain Skeleton (100%)

- Solana program deployment pipeline
- Vault and verifier program structures
- Transaction submission to testnet
- Integration test suite

### ✅ Phase 3a-3c: ZK Foundations (100%)

- Arkworks Groth16 implementation
- Accounting circuit with conservation laws
- Witness generation and proof creation
- Performance: 2.9-5.4ms proving, 1.9-2.1ms verification

### ✅ Phase 3d: On-chain Verification (100%)

- 64,360-byte production verifying key embedded
- BN254 pairing verification on Solana
- 256-byte Groth16 proof format
- All 64 tests passing

### 🔄 Phase 3e: Settlement Integration (Next)

- Connect sequencer to proof generation
- Automated batch processing every 3-5s
- Transaction retry and error handling

### 🔄 Phase 3f: End-to-end Validation (Next)

- Multi-batch testing on testnet
- Database reconciliation
- Performance validation

## Key Files & Documentation

### Core Implementation

- `programs/verifier/src/groth16.rs` - BN254 verification logic
- `programs/verifier/src/verifying_key.rs` - Production VK (64KB)
- `programs/verifier/src/lib.rs` - Main verifier program
- `prover/src/circuits/accounting.rs` - ZK accounting circuit
- `prover/src/proof_generator.rs` - Groth16 proof creation
- `sequencer/src/main.rs` - High-performance API server

### Documentation

- `PHASE_3D_COMPLETION.md` - Implementation summary
- `TESTING_REPORT.md` - Comprehensive test results
- `progress.json` - Detailed phase tracking
- `requirements.txt` - Original specification

### Performance & Architecture

- **Proving Time**: 2.9-5.4ms (Groth16 BN254)
- **Verification Time**: 1.9-2.1ms off-chain
- **Sequencer RPS**: 3920+ (exceeded targets)
- **Proof Size**: 256 bytes (industry standard)
- **Verifying Key**: 64,360 bytes (production grade)

## Development Workflow

For Phase 3e development, the recommended workflow is:

1. **Start with integration tests**: Define the expected behavior
2. **Implement sequencer changes**: Connect to proof generation
3. **Add transaction submission**: Solana program invocation
4. **Test end-to-end**: Full settlement pipeline validation
5. **Performance tuning**: Optimize for 3-5s batch latency targets

The foundation is solid with all 64 tests passing and complete ZK verification implemented.
