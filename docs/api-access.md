# Official API access

Lumen supports separate **OpenAI API**, **Anthropic API** and **Google API** account slots. These call the official HTTPS endpoints directly; subscription CLI credentials are never converted into API credentials. Cloud token charges belong to the user’s provider account.

## Configure an account

1. Add the corresponding API provider in **Settings → Accounts** and expand **API key and budget**.
2. Save your official key. Lumen stores it in the operating-system keychain under the account UUID, clears the password field and returns only a saved/not-saved indicator. There is no plaintext fallback. A locked/unavailable keychain blocks API use.
3. Enter an exact model ID and the **highest applicable input/output USD rates per million tokens**, including any context-size tier. Confirm the official rates using the pricing link. One account slot prices one model; use another named API slot for another priced model.
4. Set a positive cumulative USD budget and an output limit (256–32768 tokens), then save. The official catalog must contain the priced model. No guessed model or silent model substitution is used.
5. Select the API directly in the composer. **Allow Auto to use this API within the budget** is separate and off by default. Merely saving a key does not enable paid Auto requests.

Saving a different model clears its cached catalog. Removing an API slot deletes that UUID’s keychain credential. Old cost records remain in the local budget ledger; changing the budget never resets prior charges. Lumen does not modify provider-side billing caps: configure those separately as an additional limit.

## Budget and displayed cost

`api.sqlite` contains model/pricing settings and a durable request ledger, without API keys. An immediate SQLite transaction reserves the estimated maximum request cost **before each HTTP request**, including every tool-loop step. Concurrent requests share that ledger. Auto opt-in, model identity, current output limit and remaining budget are checked again at reservation time; a changed output limit stops the prepared request.

Text-only request bytes plus protocol headroom conservatively bound input tokens; the configured output cap bounds output tokens. Requests/responses are bounded to 8 MB and a turn to 32 model steps. Reported input/output usage replaces the reservation using the confirmed rates. Cached input is charged at the full input rate; Google reasoning tokens count toward output. Known HTTP request rejections release their reservation. Stop, crashes and uncertain network responses retain the reservation instead of assuming no charge occurred.

**API estimate** in the reply is the sum of the latest estimate per request, not both its reservation and its final charge. This is an estimate, not an invoice or a guarantee against incorrect user-entered rates, provider price changes, hidden fees or service-side accounting differences. The budget covers model token estimates; external MCP/service fees are separate. Unknown usage stays reserved. No automatic refund/retry is inferred from a missing response.

## Tools, permissions and continuity

All three APIs share Lumen’s Moli dispatcher, graph, scoped MCP layer and host file/command tools. Read only denies writes/commands, File edits permits project writes but denies commands, Ask requires an Allow once/Deny decision, and Autopilot runs within the selected host/native service permissions. Command Stop kills the owned process tree. File tools reject traversal, symlink escapes, Git internals and Windows stream/path aliases. These profiles are application controls; Autopilot shell commands are not an OS sandbox.

OpenAI Responses, Anthropic Messages and Google generateContent retain native call IDs/result formats and Google thought signatures. Long or incompatible MCP names receive invocation-local aliases; results preserve the advertised names. Duplicate provider call IDs are rejected before executing the repeated batch. Connector consent and action approvals are never promoted to permanent task instructions.

There is **no native API session resume**. Continuation uses Lumen’s bounded conversation/task context; it does not reconstruct a private provider session or replay tool effects. Responses are buffered JSON, not token streaming. Native image input, native skill/subagent/hook runtimes and arbitrary provider-specific reasoning controls are not integrated. Images produced by tools can still be shown in Lumen. Subscription quota displays and API budgets are separate.

### Experimental Code Mode

Set `"codeMode": "on"` in the daemon data directory's `settings.json` to use `lumen_code` for official API turns. `off` is the default, and `auto` currently uses classic calls pending model validation. The JavaScript environment and its variables last one turn. Each host call follows the same permission and approval path as a classic call. A script can queue up to 200 calls with eight in flight, within 120 seconds and a 256 MiB JavaScript heap. Actions that can prompt or change shared state run in order. Output over 24 KiB has a head/tail preview and **Show full output** in activity.

Production endpoints are fixed HTTPS, without proxies or redirects. A loopback fixture override exists only in debug builds with an isolated test-data directory; release builds ignore it. Windows keychain/IPC/SQLite and all three wire formats are tested locally; fixture success does not certify a real API key, model entitlement, current prices or account quota. macOS Keychain/Linux Secret Service require platform validation.

Primary contracts: [OpenAI function calling](https://developers.openai.com/api/docs/guides/function-calling), [Anthropic tools](https://platform.claude.com/docs/en/agents-and-tools/tool-use/define-tools), [Google generateContent](https://ai.google.dev/api/generate-content), [keyring backends](https://docs.rs/keyring/3.6.3/keyring/).

[Accounts](accounts.md) · [Adapter/integration limits](integrations.md) · [Validation](validation.md)
