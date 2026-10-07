# Deployment Guide

This guide covers deploying AegisMint contracts to Stellar networks.

## 📋 Prerequisites

1. **Rust & Cargo**
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   rustup target add wasm32v1-none
   ```

2. **Soroban CLI**
   ```bash
   cargo install --locked soroban-cli --features opt
   ```

3. **Stellar CLI** (optional)
   ```bash
   cargo install --locked stellar-cli
   ```

4. **Funded Stellar Account**
   - Testnet: Use [Stellar Laboratory](https://laboratory.stellar.org/#account-creator?network=test) or Friendbot
   - Mainnet: Fund account through exchange or wallet

## 🔧 Configuration

### 1. Network Configuration

**Testnet**:
```bash
soroban config network add testnet \
  --rpc-url https://soroban-testnet.stellar.org:443 \
  --network-passphrase "Test SDF Network ; September 2015"
```

**Mainnet** (when ready):
```bash
soroban config network add mainnet \
  --rpc-url https://soroban-mainnet.stellar.org:443 \
  --network-passphrase "Public Global Stellar Network ; September 2015"
```

### 2. Identity Setup

```bash
# Create a new identity
soroban config identity generate deployer

# Get the public key
soroban config identity address deployer

# Fund testnet account
soroban config identity fund deployer --network testnet
```

### 3. Store Secret Key

```bash
# Store your secret key securely
export STELLAR_SECRET_KEY="S..."

# Or use Soroban identity
soroban config identity show deployer
```

## 🏗️ Building Contracts

### Build All Contracts

```bash
# Using make
make build-wasm optimize

# Or manually
cargo build --target wasm32v1-none --release

# Optimize each contract
soroban contract optimize \
  --wasm target/wasm32v1-none/release/asset_factory.wasm

soroban contract optimize \
  --wasm target/wasm32v1-none/release/rwa_token.wasm

soroban contract optimize \
  --wasm target/wasm32v1-none/release/marketplace_escrow.wasm
```

### Verify Build

```bash
# Check WASM file sizes (optimized should be smaller)
ls -lh target/wasm32v1-none/release/*.wasm
ls -lh target/wasm32v1-none/release/*.optimized.wasm
```

## 🚀 Deployment

### Deploy to Testnet

#### 1. Deploy Asset Factory

```bash
ASSET_FACTORY_ID=$(soroban contract deploy \
  --wasm target/wasm32v1-none/release/asset_factory.optimized.wasm \
  --source deployer \
  --network testnet)

echo "Asset Factory deployed: $ASSET_FACTORY_ID"
```

#### 2. Deploy RWA Token (Template)

```bash
RWA_TOKEN_ID=$(soroban contract deploy \
  --wasm target/wasm32v1-none/release/rwa_token.optimized.wasm \
  --source deployer \
  --network testnet)

echo "RWA Token deployed: $RWA_TOKEN_ID"
```

#### 3. Deploy Marketplace Escrow

```bash
MARKETPLACE_ID=$(soroban contract deploy \
  --wasm target/wasm32v1-none/release/marketplace_escrow.optimized.wasm \
  --source deployer \
  --network testnet)

echo "Marketplace Escrow deployed: $MARKETPLACE_ID"
```

#### 4. Save Contract IDs

```bash
# Save to a file for later use
cat > deployed-contracts.txt << EOF
ASSET_FACTORY_ID=$ASSET_FACTORY_ID
RWA_TOKEN_ID=$RWA_TOKEN_ID
MARKETPLACE_ID=$MARKETPLACE_ID
DEPLOYER_ADDRESS=$(soroban config identity address deployer)
NETWORK=testnet
DEPLOYED_AT=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
EOF

echo "Contract IDs saved to deployed-contracts.txt"
```

## ⚙️ Initialization

### Initialize Asset Factory

```bash
soroban contract invoke \
  --id $ASSET_FACTORY_ID \
  --source deployer \
  --network testnet \
  -- initialize \
  --admin $(soroban config identity address deployer)
```

### Initialize RWA Token (Example)

```bash
soroban contract invoke \
  --id $RWA_TOKEN_ID \
  --source deployer \
  --network testnet \
  -- initialize \
  --admin $(soroban config identity address deployer) \
  --name "Real Estate Token" \
  --symbol "RET" \
  --decimals 7 \
  --total_supply 1000000 \
  --metadata_uri "ipfs://QmExample..."
```

### Initialize Marketplace Escrow

```bash
soroban contract invoke \
  --id $MARKETPLACE_ID \
  --source deployer \
  --network testnet \
  -- initialize \
  --admin $(soroban config identity address deployer)
```

## ✅ Verification

### Verify Deployments

```bash
# Check Asset Factory admin
soroban contract invoke \
  --id $ASSET_FACTORY_ID \
  --network testnet \
  -- get_admin

# Check Marketplace Escrow admin
soroban contract invoke \
  --id $MARKETPLACE_ID \
  --network testnet \
  -- get_admin

# Check RWA Token details
soroban contract invoke \
  --id $RWA_TOKEN_ID \
  --network testnet \
  -- name

soroban contract invoke \
  --id $RWA_TOKEN_ID \
  --network testnet \
  -- symbol
```

## 📝 Post-Deployment Checklist

- [ ] All contracts deployed successfully
- [ ] Contract IDs saved securely
- [ ] All contracts initialized
- [ ] Admin addresses verified
- [ ] Test basic functionality
- [ ] Document contract addresses
- [ ] Update frontend configuration
- [ ] Set up monitoring/alerts
- [ ] Verify on Stellar Explorer

## 🔍 Testing Deployed Contracts

### Create Test Asset

```bash
# Generate test user
soroban config identity generate test-user
soroban config identity fund test-user --network testnet

# Create asset
ASSET_ID=$(soroban contract invoke \
  --id $ASSET_FACTORY_ID \
  --source test-user \
  --network testnet \
  -- create_asset \
  --owner $(soroban config identity address test-user) \
  --name "Test Property" \
  --symbol "TPROP" \
  --total_supply 100000 \
  --metadata_uri "ipfs://QmTest...")

echo "Created asset ID: $ASSET_ID"
```

### Get Asset Details

```bash
soroban contract invoke \
  --id $ASSET_FACTORY_ID \
  --network testnet \
  -- get_asset \
  --asset_id $ASSET_ID
```

### Create Test Escrow

```bash
# Generate buyer and seller
soroban config identity generate buyer
soroban config identity generate seller
soroban config identity fund buyer --network testnet
soroban config identity fund seller --network testnet

# Create escrow
ESCROW_ID=$(soroban contract invoke \
  --id $MARKETPLACE_ID \
  --source buyer \
  --network testnet \
  -- create_escrow \
  --seller $(soroban config identity address seller) \
  --buyer $(soroban config identity address buyer) \
  --token_address $RWA_TOKEN_ID \
  --amount 1000)

echo "Created escrow ID: $ESCROW_ID"
```

## 🔐 Security Considerations

### Pre-Deployment

- [ ] Audit contract code
- [ ] Run security tests
- [ ] Verify WASM optimization
- [ ] Review access controls
- [ ] Test on testnet first

### During Deployment

- [ ] Use secure key management
- [ ] Verify transaction details
- [ ] Monitor gas costs
- [ ] Confirm contract addresses

### Post-Deployment

- [ ] Secure admin keys
- [ ] Set up monitoring
- [ ] Document emergency procedures
- [ ] Plan upgrade strategy
- [ ] Enable rate limiting (if applicable)

## 🚨 Emergency Procedures

### Pause Token Transfers

```bash
soroban contract invoke \
  --id $RWA_TOKEN_ID \
  --source deployer \
  --network testnet \
  -- pause
```

### Unpause Token Transfers

```bash
soroban contract invoke \
  --id $RWA_TOKEN_ID \
  --source deployer \
  --network testnet \
  -- unpause
```

## 📊 Monitoring

### Key Metrics to Track

1. **Transaction Volume**
   - Asset creations per day
   - Token transfers per day
   - Escrow transactions per day

2. **Gas Usage**
   - Average transaction cost
   - Peak usage times
   - Total gas consumed

3. **Contract Health**
   - Failed transactions
   - Disputed escrows
   - Paused tokens

### Monitoring Tools

- [Stellar Explorer](https://stellar.expert/)
- [StellarChain](https://stellarchain.io/)
- Custom indexer/dashboard

## 🔄 Upgrade Strategy

### Contract Upgrades

Soroban supports contract upgrades. Future versions should include:

1. **Upgrade Authorization**
   - Multi-sig approval
   - Timelock mechanism
   - Emergency upgrade path

2. **State Migration**
   - Plan data migration
   - Maintain backwards compatibility
   - Test migration thoroughly

3. **Rollback Plan**
   - Keep previous versions
   - Document rollback procedure
   - Test rollback process

## 📚 Additional Resources

- [Soroban Documentation](https://soroban.stellar.org/docs)
- [Stellar Developers](https://developers.stellar.org/)
- [Soroban Examples](https://github.com/stellar/soroban-examples)
- [Stellar Discord](https://discord.gg/stellar)

## 💡 Tips

1. **Always test on testnet first**
2. **Keep private keys secure** - Never commit them
3. **Document everything** - Contract IDs, admin addresses, etc.
4. **Monitor deployments** - Set up alerts for issues
5. **Plan for upgrades** - Design with upgradeability in mind
6. **Use version tags** - Tag releases in git
7. **Backup data** - Keep copies of contract IDs and configuration

## 🆘 Troubleshooting

### Common Issues

**"insufficient balance" error**
```bash
# Fund your account
soroban config identity fund deployer --network testnet
```

**"contract already exists" error**
```bash
# Use install + create instead of deploy
soroban contract install --wasm <wasm-file> --network testnet
soroban contract deploy --wasm-hash <hash> --network testnet
```

**"transaction failed" error**
```bash
# Check transaction details
soroban contract invoke --help
# Verify parameters and authorization
```

### Getting Help

- Check [Soroban Discord](https://discord.gg/stellar)
- Review [Stellar Stack Exchange](https://stellar.stackexchange.com/)
- Open an issue on GitHub
