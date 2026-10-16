# Security hardening — sensitive project path policy

Native Windows / Rust change.

The same conservative admission rule is enforced in full index scans, targeted
refresh, syntax and lexical lookups, and semantic graph imports. It also applies
to hash-matching **old index rows**, so outdated caches cannot re-expose
sources newly excluded by this version.

Excluded credential stores / directories: .ssh, .aws, .azure, .kube,
.docker, .gnupg, .gcloud and .terraform (in addition to existing ignored
build/dependency folders).

Examples of excluded files: .npmrc, .pypirc, .netrc, .git-credentials,
.envrc, *.tfvars, *.tfstate, *.tfstate.backup, *.jks, *.keystore,
*.kdbx, service-account.json and firebase-adminsdk-*.json, as well as
existing environment/key material patterns.

**Deliberate trade-off:** Some nonsecret development configuration in these
locations will not be indexed. Claude's built-in tools remain available,
subject to the user's permissions. Findex must not silently bypass the
restriction for additional recall.

Tests: core path classification, Rust regression injecting a hash-matching
legacy FTS5 secret cache entry, native Windows black-box with 17 confidential
files, unchanged source retrieval, plus all prior Rust/MSRV/RustSec and
1.2k/10k durability/Claude plugin gates.

No path-name filter can identify all secrets (e.g. arbitrary credentials
inside normal source files). Repository-level file permissions and cloud
provider handling remain the user's responsibility. This is risk reduction,
not an absolute non-exfiltration guarantee.
