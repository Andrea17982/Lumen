# Contributing

Feedback is useful even without code. Open an [issue](https://github.com/Andrea17982/Lumen/issues) with app/OS/provider versions, reproducible steps and expected/actual behavior. Distinguish UI preview, native fixture tests and real provider runs. Redact screenshots/logs; never upload credentials, account directories or private transcripts. Follow [Security](SECURITY.md) for vulnerabilities.

## Changes

1. Create a focused branch. Discuss substantial product or protocol changes before implementation.
2. Read [Product](PRODUCT.md), [Design](DESIGN.md) and [Architecture](docs/architecture.md). Preserve explicit provider/model choices, real permissions, native authentication and workspace ownership.
3. Keep UI in components/settings, native ownership in Rust and protocol decisions out of the renderer. Use fixtures for substantive regressions, not live credentials in CI.
4. Run [development checks](docs/development.md), including documentation/version/link checks for doc or release changes.
5. Explain resulting behavior, why it changed, evidence and remaining validation limits. Capture the actual interface with synthetic/private-safe data for UI changes.

Historical release notes stay historical. Current feature/setup guides should describe the shipped product rather than append contradictory version-by-version instructions. Keep one canonical validation record; do not turn lack of live credentials into a fabricated pass.

Contributions are MIT licensed. AI-assisted contributions are welcome; disclose the tools/review performed. The initial product is completely AI Written under human direction; later human work must be credited accurately.
