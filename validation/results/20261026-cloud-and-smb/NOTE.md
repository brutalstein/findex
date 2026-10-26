# Cloud-only placeholders and SMB paths (2026-10-26)

Native Windows 11, working tree on top of `52d9a8a` with the cloud-only fix.
No model sessions, no paid calls.

## Cloud-only placeholders (`cloud-placeholders.json`)

`validation/harness/cloud_placeholders.py` registers a throwaway Cloud Files
sync root (the API used by OneDrive Files On-Demand), connects a provider that
serves contents and logs every download request, and runs `findex.exe`.

| Check | Before the fix (`52d9a8a` binary) | After |
|---|---|---|
| `findex index` downloads of 3 cloud-only files | **3** (requested by `findex.exe`) | 0 |
| Files still cloud-only after indexing | no | yes |
| CLI search/context, MCP search/context/find-symbol downloads | 0 | 0 |
| File the user opened (made local) is indexed | yes | yes, with no extra download |
| File sent back to the cloud | re-downloaded (1) and kept | 0 downloads, removed from the index |

Before the fix, indexing a OneDrive folder with Files On-Demand downloaded
every cloud-only source file up to the 100,000-file budget. Now `findex index`
prints how many cloud-only files were left out. Limitation: a cloud-only file
is not searchable until it is made available offline (false negative,
reported).

## SMB path (`network-volume.json`)

`validation/harness/network_volume.py` opens a local project through the
loopback administrative share `\\localhost\C$`, so file I/O goes through the
SMB client and server. CLI index/search/context work. The MCP server uses the
native watcher on the SMB path (`examined: 0` on a warm query), and an edit, a
delete and a create are visible to the next query. A remote file server
(latency, oplocks, permissions) is not covered.
