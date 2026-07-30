# Contributing to Gullbúr Enclave

Thank you for considering contributing to Gullbúr Enclave! We welcome contributions from everyone.

## How to Build

```bash
cargo check --workspace
```

For a full build:

```bash
cargo build --workspace
```

## Code Style

We follow standard Rust conventions:

- Run `cargo fmt` to format your code before committing
- Run `cargo clippy` to catch common mistakes and non-idiomatic patterns
- Address all warnings before submitting

## Testing

All code must pass the full test suite before submission:

```bash
cargo test --workspace
```

This runs unit tests, integration tests, and doc tests across the entire workspace. If you're adding new functionality, please include corresponding tests.

## Commit Messages

We follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>: <short description>

[optional body]
```

Types:
- `feat:` — a new feature
- `fix:` — a bug fix
- `docs:` — documentation changes
- `chore:` — maintenance, tooling, dependencies
- `refactor:` — code restructuring without functional change
- `test:` — adding or updating tests
- `ci:` — CI configuration changes

Examples:
```
feat: add key derivation for BIP32 HD wallets
fix: handle edge case in ECDSA signature verification
docs: update architecture overview
```

## Pull Request Process

1. **Open an issue first** — discuss your proposed changes before investing significant time. This avoids duplicated effort and ensures alignment with the project's direction.
2. Fork the repository and create a feature branch from `main`.
3. Make your changes following the guidelines above.
4. Ensure all tests pass and code is formatted.
5. Submit a pull request referencing the related issue.
6. A maintainer will review your PR. Expect constructive feedback — please address all review comments before merge.

## Licensing

By contributing to Gullbúr Enclave, you agree that your contributions will be licensed under the **MIT OR Apache-2.0** dual-license, matching the project's licensing model.