# Local service and recovery

The 0.10 desktop consists of `lumen.exe` (the Tauri interface) and `lumen-daemon.exe` (the local execution service). The corresponding Unix executables have no `.exe` suffix. Keep the service beside the interface, including in portable installations.

## Closing, stopping and login

Closing the window leaves accepted work, native questions, permission requests, Orchestra workers and scheduled quota continuations in the service. With the notification-area preference enabled, the interface stays available in the tray. Otherwise the interface exits; opening it again connects to the same service. **Stop Lumen and quit** in the tray or Settings > General cancels active work, waits for cleanup and stops the service.

The existing **Open Lumen at login** preference remains opt-in. It starts the service quietly, without a window. Existing enabled login entries are rewritten to the service on first launch. Isolated test roots never change the installed application's login item.

## Data and migration

The application identifier and data directory remain `io.github.andrea17982.lumen`. Account UUIDs, native profiles and the API-keychain service are unchanged. Only the daemon opens `lumen.db`; one OS file lock owns each data directory. The GUI never marks live work interrupted by reopening storage.

Schema 4 adds an ordered event journal, message checkpoint sequences and durable quota intentions to the existing SQLite database. Migration and message/checkpoint writes are transactional. Older 0.8/0.9 schemas upgrade in place; a newer schema is refused. Back up the complete data directory with Lumen stopped before upgrading. Returning to 0.9 requires restoring that backup; do not open a schema-4 database with an older binary.

Run events reach SQLite before they reach the interface. A message snapshot includes uncheckpointed text/activity and sequence numbers. The interface deduplicates streamed events against snapshots and preserves events received during a snapshot request. Running-worker text is compacted into bounded replacement events independently of orchestrator checkpoints, and is copied into interrupted worker state during crash recovery. Other checkpointed event rows are pruned; deleting or rewinding a message removes its remaining event rows and associated quota intention.

## Crash and quota recovery

A GUI crash does not stop the service. A service crash or machine restart marks unfinished work interrupted. Partial output remains available from the journal. Uncertain work is **not automatically resubmitted**: inspect its filesystem changes and use Resume. This prevents pretending that a lost connection proves a tool did nothing.

An explicitly scheduled quota continuation is different: its saved intention can be restored after an orderly service restart or a crash. The service revalidates the native account/session, workspace, access profile and latest reply. It consumes the intention before submitting the continuation; a crash after that claim requires manual Resume. Waiting holds no inference/work permit. Stop cancels the intention; stopping the whole service preserves a still-waiting intention for the next start.

Windows owns execution descendants through a process-wide kill-on-close Job Object, in addition to existing invocation jobs. Unix commands use private process groups and a pipe tied to the daemon's lifetime. A small supervisor anchors each group; its guard kills that group when the supervisor or daemon exits, including an exited root's remaining descendants. Cleanup uses owned kernel objects/groups rather than searching for processes by name or trusting a stale PID. Legacy interrupted PIDs are never terminated from a timestamp approximation: an ambiguous live process blocks Resume or Discard until it is reviewed/stopped manually. Programs that deliberately escape their process group or OS containment are outside this guarantee. Desktop editors, login terminals and user-opened links are launched by the GUI and are not service children.

## Update procedure

1. Stop active work and close every Lumen interface. A hidden tray window is still an interface; use **Stop Lumen and quit** when convenient.
2. From the **currently installed** service directory, inspect or stop the service:

   ```powershell
   .\lumen-daemon.exe --status
   .\lumen-daemon.exe --prepare-update
   ```

   `--prepare-update` refuses connected interfaces or active/scheduled work. `--shutdown` explicitly cancels active work and drains the service; waiting quota intentions survive that service shutdown. On Unix use `./lumen-daemon`.

3. Wait for a successful stopped result and process exit, then install/extract the new complete package. Reopen Lumen. Never overwrite the service binary while it is executing.
4. After an abnormal interruption, review partial work before Resume. Do not manually remove the lock to bypass a live owner.

The CLI also accepts `--data-dir <directory>` for a separate service root and `--resources <directory>` for an explicit bundle resource root. GUI test isolation remains debug-only. Protocol/build mismatches fail with an update instruction; mutating commands are never retried after transmission. A transient busy Windows pipe is retried only before sending the request.

## Local security boundary

Windows uses a local named pipe with remote clients rejected and first-instance protection. Before parsing a bounded request, the server compares the impersonated client token's SID with its own; the client checks the server process token. Unix uses a private user-owned socket directory, socket permissions and peer UID checks. The handshake also checks protocol and the service executable's SHA-256, preventing accidental mixed-build clients. This hash is a compatibility check, not protection from malicious code already running as the same OS user.

There is no new TCP listener, remote pairing or network exposure for the desktop/service connection. Existing invocation-scoped loopback MCP brokers retain their capability checks. RPC bodies containing API keys or private connector answers are not journalled. Signed connector URLs are opened in Rust and do not return to the webview. Keep app-data backups private: message content and event journals contain the same sensitive task material as the transcript.

Packaging follows Tauri's [sidecar contract](https://v2.tauri.app/develop/sidecar/) and [build-hook target variables](https://v2.tauri.app/reference/config/). LAN/VPN control, remote pairing and the full updater belong to later [roadmap milestones](roadmap-1.0.md).
