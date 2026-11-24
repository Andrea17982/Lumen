# Contributing

Feedback is useful even without code. Open an [issue](https://github.com/Andrea17982/Lumen/issues) with your OS, app and CLI versions, reproducible steps, and the expected and actual behavior. Redact logs, and never upload credentials or account directories. Report vulnerabilities through [Security](SECURITY.md).

## Changes

1. Work on a focused branch, and discuss large product or protocol changes first.
2. Keep execution, permissions and protocol decisions in the Rust daemon. Clients only render state and send commands. See [Architecture](docs/architecture.md).
3. Cover regressions with fixtures, never with live credentials in CI.
4. Run the [development checks](docs/development.md).
5. In the pull request, say what changed, why, the evidence, and what remains unverified.

Docs describe the current product. Keep only the current release notes, and never present missing evidence as a pass.

Contributions are MIT licensed. AI-assisted work is welcome; disclose the tools you used.
