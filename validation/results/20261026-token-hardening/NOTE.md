# Development-only payload prototype

48 deterministic dev queries (12 per corpus), separate from the 115 held-out
queries. `dev-payload-prototype.json` measures an offline matrix transformation
of real native Findex JSON and asserts exact roundtrips. Mean 397.35 tokens
becomes 321.58 (19.1% fewer). This was format selection, not held-out proof or
model-session usage. Production format verification and competitor results are
in `../20261027-payload-cost-v2/`.

A subsequent dev-only zero-sync prototype (`dev-idle-sync-prototype.json`)
encoded the all-zero synchronization object as scalar zero, with exact
roundtrips on all 48 queries: 320.85 → 282.85 mean tokens (38 saved each).
This includes the newer truncation metadata; its matrix count therefore differs
slightly from the earlier prototype. The final release uses both encodings;
see `../20261027-final-comparison/` for results and the reused-set limitation.
