# AegisMint Architecture

This document provides an overview of the AegisMint contract architecture and design decisions.

## 🏛️ System Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     AegisMint Ecosystem                      │
├─────────────────────────────────────────────────────────────┤
│                                                               │
│  ┌─────────────────┐    ┌──────────────────┐                │
│  │  Asset Factory  │───▶│   RWA Token      │                │
│  │   Contract      │    │   Contract       │                │
│  └─────────────────┘    └──────────────────┘                │
│         │                       │                             │
│         │                       ▼                             │
│         │              ┌──────────────────┐                  │
│         └─────────────▶│ Marketplace      │                  │
│                        │ Escrow Contract  │                  │
│                        └──────────────────┘                  │
│                                                               │
└─────────────────────────────────────────────────────────────┘
```

## 📦 Contract Components

### 1. RWA Token Contract

**Purpose**: Represent fractional ownership of real-world assets as fungible tokens.

**Key Features**:
- Standard token interface (transfer, approve, allowance)
- Pausable transfers for regulatory compliance
- Metadata support for asset information
- Admin controls for emergency situations

**Storage Structure**:
```rust
StorageKey::Admin          → Address
StorageKey::Name           → String
StorageKey::Symbol         → String
StorageKey::Decimals       → u32
StorageKey::TotalSupply    → i128
StorageKey::Balance(addr)  → i128
StorageKey::Allowance(from, to) → i128
StorageKey::Metadata       → TokenMetadata
StorageKey::Paused         → bool
```

**State Transitions**:
```
Initialize → Active ⟷ Paused
                ↓
            Transfers
```

### 2. Asset Factory Contract

**Purpose**: Create and manage RWA token instances.

**Key Features**:
- Deploy new RWA token contracts
- Maintain registry of all created assets
- Query asset information
- Paginated asset listing

**Storage Structure**:
```rust
StorageKey::Admin              → Address
StorageKey::AssetCount         → u64
StorageKey::Asset(id)          → Asset
```

**Asset Lifecycle**:
```
Owner Request → Factory Creates Token → Token Deployed → Asset Registered
```

### 3. Marketplace Escrow Contract

**Purpose**: Facilitate secure peer-to-peer trading of RWA tokens.

**Key Features**:
- Escrow creation between buyer and seller
- Funds held securely during transaction
- Dispute resolution mechanism
- Multiple escrow status states

**Storage Structure**:
```rust
StorageKey::Admin              → Address
StorageKey::EscrowCount        → u64
StorageKey::Escrow(id)         → Escrow
```

**Escrow State Machine**:
```
                    ┌─────────┐
                    │  Active │
                    └────┬────┘
                         │
         ┌───────────────┼───────────────┐
         ▼               ▼               ▼
    ┌─────────┐    ┌──────────┐    ┌──────────┐
    │Completed│    │ Disputed │    │Cancelled │
    └─────────┘    └────┬─────┘    └──────────┘
                        │
                 ┌──────┴──────┐
                 ▼             ▼
          ┌──────────┐   ┌──────────┐
          │Completed │   │Cancelled │
          └──────────┘   └──────────┘
```

## 🔐 Security Model

### Authentication

All sensitive operations require caller authentication:
```rust
caller.require_auth();
```

### Authorization

- **Admin-only operations**: Pause tokens, resolve disputes
- **Owner operations**: Create assets, transfer tokens
- **Participant operations**: Complete/cancel escrow

### Access Control Matrix

| Operation | Admin | Owner | Buyer | Seller | Anyone |
|-----------|-------|-------|-------|--------|--------|
| Create Asset | ✓ | ✓ | ✗ | ✗ | ✗ |
| Transfer Token | - | ✓ | ✓ | ✓ | ✓* |
| Pause Token | ✓ | ✗ | ✗ | ✗ | ✗ |
| Create Escrow | - | - | ✓ | - | ✗ |
| Complete Escrow | ✓ | - | ✓ | - | ✗ |
| Cancel Escrow | ✓ | - | - | ✓ | ✗ |
| Dispute Escrow | ✓ | - | ✓ | ✓ | ✗ |
| Resolve Dispute | ✓ | ✗ | ✗ | ✗ | ✗ |

*With proper authorization

## 💾 Storage Patterns

### Instance Storage

Used for contract-level state that persists across invocations:
```rust
env.storage().instance().set(&key, &value);
env.storage().instance().get(&key);
```

### Temporary Storage

Not used in current implementation, but available for ephemeral data within a transaction.

### Persistent Storage

Can be used for long-lived data with explicit TTL management (future enhancement).

## 🔄 Data Flow

### Asset Creation Flow

```
1. Owner calls AssetFactory.create_asset()
2. Factory validates parameters
3. Factory creates Asset record
4. Asset ID returned to owner
5. (Future) RWA Token contract deployed
```

### Token Transfer Flow

```
1. Holder calls RWAToken.transfer()
2. Contract checks if paused
3. Contract validates balance
4. Balances updated atomically
5. Transfer event emitted (future)
```

### Escrow Transaction Flow

```
1. Buyer calls MarketplaceEscrow.create_escrow()
2. Escrow record created (Active status)
3. (Future) Tokens locked in escrow
4. Either:
   a. Buyer completes → tokens to seller
   b. Seller cancels → tokens to buyer
   c. Dispute raised → admin resolves
```

## 🎯 Design Decisions

### Why Instance Storage?

Instance storage is appropriate for:
- Contract configuration (admin, settings)
- Moderate-sized datasets (asset registry)
- Data accessed frequently

Trade-offs:
- ✅ Simple API
- ✅ Automatic persistence
- ⚠️ Fixed cost structure
- ⚠️ Limited TTL control

### Why Separate Contracts?

Each contract has a single responsibility:
- **RWA Token**: Token mechanics
- **Asset Factory**: Asset creation and registry
- **Marketplace Escrow**: Trading and settlement

Benefits:
- Independent upgrades
- Clearer code organization
- Reusable components
- Isolated failure domains

### Why Pausable Tokens?

Real-world assets often require regulatory compliance:
- Emergency stops
- Regulatory holds
- Investigation periods
- Migration windows

### Future Enhancements

1. **Events**: Emit events for off-chain indexing
2. **Token Deployment**: Automated RWA token contract deployment
3. **Actual Token Transfers**: Integrate with token transfer in escrow
4. **Multi-sig Admin**: Decentralized administration
5. **Upgradability**: Contract upgrade mechanism
6. **Access Control Lists**: Fine-grained permissions
7. **KYC Integration**: Identity verification hooks
8. **Fee Structure**: Platform fee mechanism

## 📊 Performance Considerations

### Gas Optimization

- Batch operations where possible
- Minimize storage reads/writes
- Use appropriate data structures
- Optimize WASM binary size

### Scalability

- Pagination for large lists
- Consider off-chain indexing for analytics
- Limit on-chain state growth

## 🧪 Testing Strategy

### Unit Tests

Each contract includes tests for:
- Initialization
- Core functionality
- Edge cases
- Authorization checks

### Integration Tests

Future additions:
- Cross-contract interactions
- End-to-end workflows
- Gas usage benchmarks

### Security Tests

- Access control verification
- Reentrancy protection
- Integer overflow/underflow
- State consistency

## 🔮 Roadmap

### Phase 1: Core Functionality ✅
- Basic token contract
- Asset factory
- Escrow mechanism

### Phase 2: Production Readiness (In Progress)
- Event emission
- Actual token deployment
- Token locking in escrow
- Comprehensive testing

### Phase 3: Advanced Features
- Multi-signature admin
- Advanced access control
- Fee mechanism
- Upgrade system

### Phase 4: Ecosystem
- Off-chain indexer
- Frontend interface
- Oracle integration
- Cross-chain bridges
