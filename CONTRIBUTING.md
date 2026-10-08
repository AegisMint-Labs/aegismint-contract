# Contributing to AegisMint Contract

Thank you for your interest in contributing to AegisMint! This document provides guidelines and instructions for contributing.

## 🤝 Code of Conduct

We are committed to providing a welcoming and inclusive environment for all contributors. Please be respectful and constructive in all interactions.

## 🌿 Branching Strategy

AegisMint follows a dual-branch development model (`main` and `dev`):

- **`main`**: The production-ready branch. Contains stable, reviewed, and audited smart contract releases. Direct commits to `main` are strictly prohibited.
- **`dev`**: The primary integration branch. All new features, performance improvements, and bug fixes must be branched off and targeted toward `dev`.
- **Feature Branches**: Branch names must follow standard conventions prefixed with their purpose:
  - `feature/feature-name` (e.g., `feature/ttl-extension-safety`)
  - `fix/bug-description` (e.g., `fix/escrow-refund-timeout`)
  - `docs/doc-update` (e.g., `docs/api-reference`)

All pull requests must target the **`dev`** branch unless delivering a hotfix directly tagged for a production release into **`main`**.

## 🚀 Getting Started

1. **Fork the repository**
   ```bash
   gh repo fork AegisMint-Labs/aegismint-contract
   ```

2. **Clone your fork**
   ```bash
   git clone https://github.com/AegisMint-Labs/aegismint-contract.git
   cd aegismint-contract
   ```

3. **Check out the `dev` branch**
   ```bash
   git checkout dev
   ```

4. **Create a new feature branch**
   ```bash
   git checkout -b feature/your-feature-name
   ```

## 💻 Development Workflow

### Building

```bash
# Check your code compiles
make check

# Build all contracts
make build

# Build optimized WASM binaries
make build-wasm optimize
```

### Testing

```bash
# Run all tests
make test

# Run tests for a specific contract
make test-asset-factory
make test-rwa-token
make test-marketplace

# Run tests with verbose output
make test-verbose
```

### Code Quality

Before submitting a PR, ensure your code passes all checks:

```bash
# Format code
make format

# Run linter
make lint

# Run all checks
make all
```

## 📝 Commit Convention

We follow [Conventional Commits](https://www.conventionalcommits.org/) specification:

### Types
- `feat:` New feature
- `fix:` Bug fix
- `docs:` Documentation changes
- `style:` Code style changes (formatting, etc.)
- `refactor:` Code refactoring
- `test:` Adding or updating tests
- `chore:` Maintenance tasks

### Scopes
- `asset_factory` - Asset Factory contract
- `rwa_token` - RWA Token contract
- `marketplace_escrow` - Marketplace Escrow contract
- `workspace` - Workspace-level changes
- `ci` - CI/CD changes
- `docs` - Documentation

### Examples
```bash
feat(rwa_token): add burn functionality
fix(marketplace_escrow): resolve dispute resolution edge case
docs(readme): update deployment instructions
test(asset_factory): add pagination tests
chore(workspace): update soroban-sdk to v28.0.1
```

## 🔍 Pull Request Process

1. **Update your branch**
   ```bash
   git fetch upstream
   git rebase upstream/dev
   ```

2. **Run all checks**
   ```bash
   make all
   ```

3. **Commit your changes**
   ```bash
   git add .
   git commit -m "feat(scope): description"
   ```

4. **Push to your fork**
   ```bash
   git push origin feature/your-feature-name
   ```

5. **Create a Pull Request**
   - Use a clear, descriptive title following conventional commits
   - Provide a detailed description of your changes
   - Reference any related issues
   - Ensure CI checks pass

### PR Requirements

- [ ] Code follows project style guidelines
- [ ] All tests pass
- [ ] New tests added for new features
- [ ] Documentation updated if needed
- [ ] Commit messages follow conventional commits
- [ ] No merge conflicts with main branch

## 🐛 Reporting Bugs

When reporting bugs, please include:

1. **Description**: Clear description of the bug
2. **Steps to Reproduce**: Step-by-step instructions
3. **Expected Behavior**: What should happen
4. **Actual Behavior**: What actually happens
5. **Environment**: 
   - OS version
   - Rust version (`rustc --version`)
   - Soroban CLI version (`soroban version`)
6. **Additional Context**: Logs, screenshots, etc.

## 💡 Suggesting Enhancements

Enhancement suggestions are welcome! Please provide:

1. **Clear description** of the enhancement
2. **Use case** - why it would be useful
3. **Proposed solution** - how it might work
4. **Alternatives considered** - other approaches

## 📚 Documentation

Documentation improvements are always appreciated:

- Code comments for complex logic
- README updates for new features
- API documentation in contract code
- Tutorial content

## 🔒 Security

If you discover a security vulnerability, please email security@aegismint.io instead of opening a public issue.

## ✅ Code Review

All submissions require code review. We use GitHub pull requests for this:

- Reviewers will provide constructive feedback
- Address all review comments
- Keep discussions professional and focused
- Once approved, a maintainer will merge your PR

## 📜 License

By contributing, you agree that your contributions will be licensed under the MIT License.

## 🙏 Thank You!

Your contributions help make AegisMint better for everyone. Thank you for taking the time to contribute!
