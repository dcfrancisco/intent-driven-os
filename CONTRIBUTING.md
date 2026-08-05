# Contributing to Open Intelligence Desktop

Contributions are welcome. Please start by opening an issue for substantial changes so the design and scope can be discussed before implementation.

## Development expectations

- Keep the terminal and user visibility central to every feature.
- Keep crates focused and depend on abstractions rather than concrete adapters.
- Do not add AI or hidden system actions without an accepted design decision.
- Add unit tests, integration scaffolding, and documentation for public APIs.
- Use `cargo fmt`, `cargo test --workspace`, and Clippy before submitting a change.
- Add or update an ADR when a change affects architecture, security, persistence, or public extension points.

## Pull requests

Describe the problem, the design, the user-visible behavior, the evidence/rollback implications, and the verification performed. Small, focused pull requests are easier to review.

## Code of conduct

Participation is governed by [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md). Security issues should be reported according to [SECURITY.md](SECURITY.md), not in a public issue.

