# AegisMint Contract

Soroban smart contracts for tokenizing and trading Real World Assets (RWAs) on the Stellar blockchain.

## 🏗️ Architecture

This workspace contains three core smart contracts:

### 1. **RWA Token** (`contracts/rwa_token`)
A compliant token contract representing fractional ownership of real-world assets.

**Features:**
- Standard token operations (transfer, approve, allowance)
- Pausable transfers for compliance
- Metadata support (name, symbol, decimals, URI)
- Admin controls

### 2. **Asset Factory** (`contracts/asset_factory`)
Factory contract for creating and managing RWA token instances.

**Features:**
- Create new RWA tokens
- Track all created assets
- Query asset details
- Paginated asset listing

### 3. **Marketplace Escrow** (`contracts/marketplace_escrow`)
Secure escrow system for peer-to-peer RWA token trading.

**Features:**
- Create escrow agreements between buyer and seller
- Complete or cancel escrow transactions
- Dispute resolution mechanism (admin-mediated)
- Escrow status tracking

## 🚀 Getting Started

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (latest stable)
- [Soroban CLI](https://soroban.stellar.org/docs/getting-started/setup)
- [Stellar CLI](https://developers.stellar.org/docs/tools/developer-tools)

### Installation

```bash
# Clone the repository
git clone https://github.com/AegisMint-Labs/aegismint-contract.git
cd aegismint-contract

# Build all contracts
cargo build --release

# Run tests
cargo test
```

### Building Individual Contracts

```bash
# Build asset factory
cargo build --package asset_factory --target wasm32-unknown-unknown --release

# Build RWA token
cargo build --package rwa_token --target wasm32-unknown-unknown --release

# Build marketplace escrow
cargo build --package marketplace_escrow --target wasm32-unknown-unknown --release
```

## 🧪 Testing

```bash
# Run all tests
cargo test

# Run tests for a specific contract
cargo test --package rwa_token
cargo test --package asset_factory
cargo test --package marketplace_escrow

# Run tests with output
cargo test -- --nocapture
```

## 📦 Deployment

### Deploy to Testnet

```bash
# Configure Soroban for testnet
soroban config network add --global testnet \
  --rpc-url https://soroban-testnet.stellar.org:443 \
  --network-passphrase "Test SDF Network ; September 2015"

# Deploy asset factory
soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/asset_factory.wasm \
  --source <YOUR_SECRET_KEY> \
  --network testnet

# Deploy RWA token
soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/rwa_token.wasm \
  --source <YOUR_SECRET_KEY> \
  --network testnet

# Deploy marketplace escrow
soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/marketplace_escrow.wasm \
  --source <YOUR_SECRET_KEY> \
  --network testnet
```

## 🔧 Contract Interactions

### Initialize Asset Factory

```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  --source <YOUR_SECRET_KEY> \
  --network testnet \
  -- initialize \
  --admin <ADMIN_ADDRESS>
```

### Create a New RWA Asset

```bash
soroban contract invoke \
  --id <FACTORY_CONTRACT_ID> \
  --source <YOUR_SECRET_KEY> \
  --network testnet \
  -- create_asset \
  --owner <OWNER_ADDRESS> \
  --name "Real Estate Token" \
  --symbol "RET" \
  --total_supply 1000000 \
  --metadata_uri "ipfs://QmExample..."
```

### Create an Escrow

```bash
soroban contract invoke \
  --id <ESCROW_CONTRACT_ID> \
  --source <BUYER_SECRET_KEY> \
  --network testnet \
  -- create_escrow \
  --seller <SELLER_ADDRESS> \
  --buyer <BUYER_ADDRESS> \
  --token_address <TOKEN_ADDRESS> \
  --amount 100000
```

## 📁 Project Structure

```
aegismint-contract/
├── contracts/
│   ├── asset_factory/
│   │   ├── src/
│   │   │   └── lib.rs
│   │   └── Cargo.toml
│   ├── rwa_token/
│   │   ├── src/
│   │   │   └── lib.rs
│   │   └── Cargo.toml
│   └── marketplace_escrow/
│       ├── src/
│       │   └── lib.rs
│       └── Cargo.toml
├── Cargo.toml
├── README.md
└── .gitignore
```

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'feat: add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## 📄 License

This project is licensed under the MIT License - see the LICENSE file for details.

## 🔗 Links

- [Soroban Documentation](https://soroban.stellar.org/docs)
- [Stellar Developers](https://developers.stellar.org/)
- [AegisMint Labs](https://github.com/AegisMint-Labs)

## 📧 Contact

For questions and support, please open an issue in the GitHub repository.
