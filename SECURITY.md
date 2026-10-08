# Security Policy

AegisMint Labs takes the security of its smart contracts and decentralized protocols seriously. This document outlines our vulnerability disclosure process, reporting channels, supported versions, and security practices.

## Supported Versions

Security updates and patches are actively maintained for the following versions:

| Contract / Component | Supported Versions | Status |
| :--- | :--- | :--- |
| `asset_factory` | `v0.1.x` | Supported |
| `rwa_token` | `v0.1.x` | Supported |
| `marketplace_escrow` | `v0.1.x` | Supported |

Critical security advisories will be backported to stable release branches when applicable.

## Reporting a Vulnerability

If you discover a security vulnerability or suspect an exploit vector in AegisMint smart contracts, **do not open a public issue or discuss it publicly**. 

Please report vulnerabilities using one of the following secure channels:

1. **GitHub Security Advisory (Preferred)**:
   - Navigate to the [Security Advisories](https://github.com/AegisMint-Labs/aegismint-contract/security/advisories) tab of this repository.
   - Click **Report a vulnerability** to submit a private draft report.

2. **Email**:
   - Send an encrypted email to **security@aegismint.io**.
   - Include a detailed description of the vulnerability, proof of concept (PoC) code or Soroban test case, affected contract functions, and potential business or protocol impact.

### Response SLA & Coordination Timeline

- **Initial Response**: Within 24 hours of receipt.
- **Triage & Severity Assessment**: Within 48 hours.
- **Status Updates**: At least every 72 hours until a fix or mitigation is deployed.
- **Remediation & Coordinated Disclosure**: Once a patch is developed, tested, and deployed to testnet/mainnet, a coordinated public disclosure will be scheduled with attribution to the reporter.

## In Scope

The following components and vectors are within scope:

- **Access Controls & Authorization**: Flaws in `admin.require_auth()`, deployer authorization, or whitelist enforcement.
- **Storage & TTL Lifecycle**: Storage corruption, state deserialization failures, or unhandled TTL expirations on persistent/instance storage.
- **Token Accounting & Escrow Logic**: Reentrancy, balance underflows/overflows, allowance manipulation, or unauthorized escrow withdrawals/cancellations.
- **Marketplace Logic**: Exploits allowing token theft, fee bypass, or unauthorized state transitions in `MarketplaceEscrowContract`.

## Out of Scope

- Vulnerabilities in upstream Soroban SDK, Rust compiler, or Stellar core protocol (please report these directly to the Stellar Development Foundation).
- Social engineering, phishing, or physical attacks against AegisMint team members.
- Denial-of-Service attacks targeting testnet RPC endpoints or public Soroban testnet infrastructure.

## Safe Harbor & Responsible Disclosure

AegisMint Labs supports responsible security research conducted in good faith. If you adhere to the following principles, we will not pursue legal action against you:

- Make a good faith effort to avoid privacy violations, destruction of data, and interruption or degradation of our services.
- Give us reasonable time to remediate the vulnerability before disclosing it publicly.
- Do not exploit a detected security vulnerability beyond the minimum required to demonstrate a proof of concept.

## Security Practices

Our development workflow enforces:
- Strict authorization checks on all mutating entry points using Soroban's native auth framework (`Address::require_auth`).
- Automated continuous integration testing running on pull requests (`cargo test`).
- Explicit storage TTL management for persistent entries.
- Static analysis via `cargo clippy` and deterministic formatting via `rustfmt`.
