# ZK Casino Testing Guide

This project includes a comprehensive test suite covering all aspects of the ZK Casino implementation, from unit tests to full system integration.

## Quick Start

```bash
# Run the default test suite (quick mode)
npm test

# Run all tests including full integration
npm run test:full

# Run only unit tests
npm run test:unit

# Run only static analysis
npm run test:static
```

## Test Categories

### 1. Static Analysis (`--static`)

- **TypeScript Type Checking**: Validates all TypeScript code compiles without errors
- **Rust Format Check**: Ensures code follows standard Rust formatting
- **Rust Clippy Lints**: Catches common mistakes and style issues
- **Cargo Check**: Validates all Rust code compiles

### 2. Unit Tests (`--unit`)

- **Rust Unit Tests**: Tests individual functions and modules in isolation
- **Anchor Program Tests**: Tests Solana programs with local validator
- **Explorer Unit Tests**: React component tests (if dependencies installed)

### 3. Integration Tests (default)

- **Rust Integration Tests**: Cross-module integration testing
- **API Endpoint Tests**: HTTP API validation without full system

### 4. System Integration Tests (`--full`)

- **Complete Solana Integration**: End-to-end validator + sequencer + settlement
- **System Status Validation**: Real-time system health checks

## Available Test Commands

### Main Commands

```bash
npm test                 # Default: static + unit + integration (no system)
npm run test:quick       # Same as npm test
npm run test:full        # All tests including full system integration
npm run test:unit        # Only unit tests
npm run test:static      # Only static analysis
```

### Individual Test Categories

```bash
npm run test:rust        # All Rust tests
npm run test:anchor      # Anchor program tests only
npm run test:explorer    # React/TS tests only
npm run test:api         # API endpoint tests only
npm run test:solana      # Full Solana integration test
npm run test:status      # System status check
```

### Development Tools

```bash
npm run check            # Quick code validation
npm run fmt              # Format all code
npm run clippy           # Run Rust linter
```

## Test Philosophy

The test suite is designed with these principles:

1. **Fast Feedback**: Default `npm test` runs quickly (< 30 seconds) for development workflow
2. **Comprehensive Coverage**: `--full` mode tests the complete system end-to-end
3. **Isolated Testing**: Each test category can be run independently
4. **CI/CD Ready**: All tests are designed to work in automated environments
5. **Clear Results**: Color-coded output with summary statistics

## Integration Test Details

### Complete Solana Integration (`test-solana-complete.sh`)

This test validates the full end-to-end workflow:

1. **Validator Setup**: Starts local Solana validator
2. **Wallet Funding**: Creates and funds test wallets
3. **Program Deployment**: Deploys vault and verifier programs
4. **Sequencer Integration**: Starts sequencer with Solana integration
5. **API Testing**: Validates all endpoints work correctly
6. **Settlement Verification**: Confirms batch processing works
7. **Cleanup**: Proper process termination

### Quick Integration (`test-solana-quick.sh`)

Faster validation assuming services are already running:

- Health checks for running services
- Single bet test with outcome verification
- Settlement queue validation

### System Status (`test-status.sh`)

Real-time system monitoring:

- Service health validation
- Configuration display
- Settlement statistics
- Quick API functionality test

## Common Test Scenarios

### Development Workflow

```bash
# After making changes, run quick tests
npm test

# Before committing, run full suite
npm run test:full

# To check only your Rust changes
npm run test:rust

# To validate only formatting/linting
npm run test:static
```

### CI/CD Pipeline

```bash
# Full validation in CI
npm run test:full
```

### Debugging Failed Tests

```bash
# Run individual components
npm run test:unit          # Check if unit tests pass
npm run test:api           # Test API in isolation
npm run test:status        # Check current system state
./test-solana-complete.sh  # Run full integration with detailed output
```

## Test Output

The test suite provides detailed, color-coded output:

- 🟢 **Green**: Passed tests
- 🔴 **Red**: Failed tests
- 🟡 **Yellow**: Warnings or in-progress
- 🔵 **Blue**: Information and headers

Each test run concludes with a summary showing:

- Total tests executed
- Number passed/failed
- Overall success status

## Adding New Tests

When adding new functionality:

1. **Add Unit Tests**: Test the function/module in isolation
2. **Add Integration Tests**: Test how it works with other components
3. **Update System Tests**: Ensure end-to-end workflow still works
4. **Update This Documentation**: Keep the testing guide current

## Performance Expectations

- **Static Analysis**: ~5-10 seconds
- **Unit Tests**: ~10-20 seconds
- **Integration Tests**: ~15-30 seconds
- **Full System Tests**: ~60-120 seconds

Times may vary based on system performance and whether dependencies need compilation.
