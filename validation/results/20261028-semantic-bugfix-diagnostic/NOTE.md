# One authorized diagnostic retry

After the six-arm primary study timed out on Findex/Rust, the user explicitly
approved **one** additional diagnostic Codex subscription session. The original
timeout remains in primary scoring. No additional live starts follow this retry.

The command arguments, model profile, executable hashes, source tree and public
reproduction match the timed-out arm. Caches are warm after direct native MCP
probes, and the harness now streams output to files instead of losing partial
output on timeout. These differences prevent treating this as an independent
replacement or a second paired study.

The session completes in **34.104 s**, uses one successful `findex_context` call
and produces a source-only fix passing the independent native host regression.
It uses **210,660 input tokens** (184,448 cached; 26,212 uncached) and **915 output
tokens**, 211,575 total. Model-side compilation/tests are explicitly left to the
host grader. The passing diff and SHA-256 are published in `patches/` and
`patches.json`.

This shows the unchanged product can complete this fixture on a repeat. It does
not determine the first timeout's cause or establish semantic-tool adoption:
the model uses task context, not the semantic position tool. The latter has its
own compiler-generated native fixture tests. No general superiority is inferred.
