# Phase 0 - Foundations Setup

This directory contains the foundation setup for the ZK Casino MVP.

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

## Testing Phase 0 Completion

Run the following commands to verify the setup:

### 1. Test Rust workspace builds

```bash
cargo build
```

### 2. Test sequencer runs

```bash
cargo run --bin sequencer
```

### 3. Test prover compiles

```bash
cargo test -p prover
```

### 4. Test explorer builds

```bash
cd explorer
npm run build
```

### 5. Test Anchor programs (requires Anchor CLI)

```bash
anchor test
```

### 6. Start local Solana validator

```bash
solana-test-validator
```

## Exit Criteria for Phase 0

- ✅ Clean mono-repo structure created
- ✅ Rust workspace with proper dependencies
- ✅ Anchor workspace with vault & verifier programs
- ✅ Sequencer scaffold with Axum
- ✅ Prover scaffold with Arkworks
- ✅ Explorer scaffold with React/Vite
- ✅ CI configuration
- ✅ Basic hello-world tests
- 🔄 All builds pass locally
- 🔄 Tests run green
- 🔄 Local validator boots successfully

## Next Steps (Phase 1)

After Phase 0 is complete, we'll move to Phase 1: "Fast off-chain Coinflip" to implement:

- Sequencer API with REST endpoints
- In-memory ledger + SQLite persistence
- CSPRNG-based coin flip outcomes
- Sub-second UX (<150ms p50, <300ms p95)
- Explorer stub for bet visualization

## Troubleshooting

If anchor commands fail, ensure:

1. Solana CLI is in PATH
2. Anchor CLI version matches workspace (0.29.0)
3. Local validator is running on correct port

If Rust builds fail, check:

1. Rust toolchain is stable and recent
2. All workspace members have correct dependencies
3. No conflicting versions in Cargo.lock
# casino-rollup
