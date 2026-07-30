---
name: Pull Request
about: Submit changes to FOSS Crypto Core
title: ""
labels: ""
assignees: ""
---

## Summary

<!-- Clearly describe what this PR does and why. Keep it concise. -->

## Related Issues

<!-- Link to any related issues with "Closes #N", "Fixes #N", or "Refs #N". -->

- Closes #

## Type of Change

<!-- Mark the relevant option(s) with an "x". Delete options that don't apply. -->

- [ ] **Bug fix** — non-breaking change that fixes an issue
- [ ] **Feature** — non-breaking change that adds functionality
- [ ] **Refactor** — code restructuring without functional change
- [ ] **Documentation** — docs-only changes
- [ ] **CI / chore** — CI configuration, tooling, dependency bumps, or other maintenance
- [ ] **Breaking change** — existing functionality may break

## Checklist

<!-- Confirm each item before requesting review. -->

- [ ] `cargo check --workspace` passes
- [ ] `cargo test --workspace --lib` passes (115 tests)
- [ ] `cargo fmt --check` passes
- [ ] `cargo clippy --workspace` shows **no new warnings**
- [ ] Documentation updated if needed

## Breaking Changes

<!-- If "Breaking change" is checked above, describe what breaks and how to migrate. Otherwise, write "None". -->

None.

## Screenshots

<!-- If this PR changes the UI (Tauri desktop shell, extension popup, etc.), attach before/after screenshots. Otherwise delete this section. -->