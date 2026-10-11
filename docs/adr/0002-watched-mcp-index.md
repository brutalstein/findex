# Native watched MCP index (ADR-0002)

**Status:** experimental / Windows CI-backed, not globally benchmarked.

## Operational model

- Keep the official Claude Code process and MCP stdio transport unchanged.
- Lazy watcher initialization avoids a full scan just for MCP handshake.
- Native `notify 8.2.0` Windows backend observes the canonical root recursively.
- Watch callback is nonblocking and sends into a bounded 2048-event channel.
- Content-only changes to existing indexed files use transactional targeted refresh.
- New, removed, renamed or unknown paths / gitignore changes cause full reconciliation.
- Dirty/overflow, failed watcher events, bounded batch overflow and a 120-second
  timer trigger full reconciliation. On watcher start failure, per-query scanning
  takes over without stopping Claude Code.
- Filesystem events are hints. Matching snippets still require content-hash
  validation; newly changed matches can be undiscoverable until refresh.
- No secondary daemon, remote socket, source upload or model API is introduced.

## Limitations and validation

- On network filesystems, event delivery may be incomplete. Periodic reconciliation
  bounds ordinary stale-index duration but does not guarantee instantaneous discovery.
- The watcher processes events before queries, not continuously in a background worker.
- A full reconciliation still reads source files in the current lexical prototype.
- Deterministic classification and mutation tests run in Windows CI, without sleeps.
- Next steps: quantify cold/warm latency and peak memory; centralize filesystem
  security and add AST/LSP capability reporting before a production release.
