# Security Policy

Gullbúr Enclave is a non-custodial cryptographic vault handling sensitive key material. We take security seriously.

## Reporting a Vulnerability

**Please do not open public issues for security vulnerabilities.**

Instead, report vulnerabilities privately by emailing the maintainers at a dedicated security contact. If you do not have a direct contact, open a GitHub issue with the `security` label to request the appropriate reporting channel.

When reporting, please include:

- A description of the vulnerability
- Steps to reproduce (proof of concept preferred)
- Affected components and versions
- Potential impact

## Scope

The following areas are in scope for our security policy:

- Cryptographic implementations (key generation, signing, encryption)
- Inter-process communication (IPC) between the Tauri shell and Rust backend
- Key material handling and secure memory management
- Seed phrase generation and storage
- Wallet address derivation
- Network transport (RPC, WebSocket, browser extension messaging)

Out of scope:
- Third-party dependencies (report those upstream)
- Theoretical attacks without a practical exploit path

## Supported Versions

Only the latest `main` branch is supported with security updates. There are no long-term support (LTS) releases at this stage.

## Disclosure Timeline

We aim to:

1. **Acknowledge** receipt of the report within 48 hours
2. **Investigate** and confirm the vulnerability within 7 days
3. **Release a fix** within 90 days of confirmation, depending on severity and complexity

We ask that reporters withhold public disclosure for 90 days after notification to allow time for a fix and coordinated release.

## Bug Bounty

There is no bug bounty program in place at this stage. We still sincerely appreciate and will publicly acknowledge all valid security reports (with the reporter's consent).

## Best Practices

If you are integrating Gullbúr Enclave into your application:

- Always validate inputs before passing them to cryptographic functions
- Do not log or persist key material in plaintext
- Use hardware-backed secure storage where available
- Keep the library and all dependencies updated