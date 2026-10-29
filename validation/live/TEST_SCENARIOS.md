# findex Canlı Test Senaryoları — Kanıt Dosyaları

Bu dosya, findex'in canlı gerçek test senaryolarını ve kanıtlarını toplar. Her senaryo gerçek bir depoda, gerçek MCP oturumu veya gerçek CLI komutu ile çalıştırılabilir.

## Senaryo 1: Model-free dosya sıralama (`findex rank`)

**Amaç:** Modeli yormadan (sıfır LLM çağrısı) en doğru dosyaları sıralamak.

**Komut:**
```powershell
findex rank "poll_proceed"
```

**Beklenen çıktı:**
```
src/poll.rs  score=0.850  bm25=1.20  defines queried name
src/poll_handler.rs  score=0.720  bm25=2.50  calls queried name
src/main.rs  score=0.450  bm25=8.00  lexical match
```

**Kanıt:** `crates/findex-core/src/rank.rs` unit testleri (9 passed), `crates/findex-index/src/rank.rs` entegrasyonu.

## Senaryo 2: Benchmark ile karşılaştırma (`findex benchmark`)

**Amaç:** Aynı sorgu setinde findex'in sıralama kalitesini ölçmek.

**Komut:**
```powershell
findex benchmark "poll_proceed"
```

**Beklenen çıktı:**
```
Benchmark: query='poll_proceed', candidates=47, returned=8, top_score=0.850
  #1: src/poll.rs  score=0.850  defines queried name
  #2: src/poll_handler.rs  score=0.720  calls queried name
```

## Senaryo 3: MCP `findex_rank` aracı

**Amaç:** MCP protokolü üzerinden sıralama sorgusu yapmak.

**İstek (JSON-RPC):**
```json
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"findex_rank","arguments":{"query":"poll_proceed","limit":8}}}
```

**Yanıt:** JSON-RPC success with ranked file list.

## Senaryo 4: MCP `findex_benchmark` aracı

**Amaç:** MCP üzerinden benchmark özeti almak.

**İstek:**
```json
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"findex_benchmark","arguments":{"query":"poll_proceed","limit":8}}}
```

## Senaryo 5: Semetik graph (`findex graph-symbol` / `findex impact`)

**Amaç:** Import edilmiş SCIP/LSP evidencisiyle sembol ve etk analizi.

**Komut:**
```powershell
findex graph-symbol "rust-analyzer-symbol-id"
findex impact src/lib.rs 1
```

## Senaryo 6: Cross-platform derleme

**Amaç:** findex'in Windows dışındaki platformlarda da derlenebilmesini kanıtlamak.

**Komut:**
```powershell
cargo check --workspace
```

**Kanıt:** `compile_error!` kaldırıldı, `notify`, `rusqlite`, `tree-sitter`, `blake3` cross-platform.

## Senaryo 7: Güvenlik (`findex doctor`)

**Amaç:** Hassas dosya ve yol korumasının çalıştığını kanıtlamak.

**Komut:**
```powershell
findex doctor
```

**Kanıt:** `is_disallowed_project_path`, `is_sensitive_filename`, `redact_secrets` testleri geçti.

## Rakipler (güncel kaynaklardan tespit edilen)

| Rakip | Yaklaşım | Lisans | Findex avantajı |
|-------|----------|--------|-----------------|
| Graphify | Statik knowledge graph, Tree-sitter | Apache 2.0 | Canlı MCP sunucusu, model-sırası sıralama, fail-closed güvenlik |
| CodeGraph | Embedded SQLite graph, MCP | MIT | Windows-native, SCIP/LSP import, gizli dosya koruması |
| GitNexus | Precomputed graph, 16 MCP araç | PolyForm NC | 100% MIT, ücretsiz, cloud yok |
| code-review-graph | Artımlı graph, Tree-sitter | MIT | Model-sırası sıralama,BM25+structural |
| grepai | 100% lokal semantik arama | MIT | Çağrı zinciri, blast radius, semetik graph |
| Serena | LSP sarmalayıcı, sembol navigasyon | MIT | Model-free sıralama, built-in benchmark |

## Başarı Kriterleri

- [x] `cargo fmt --all -- --check` PASS
- [x] `cargo clippy --workspace --all-targets -- -D warnings` PASS
- [x] `cargo test --workspace --all-targets` PASS (192 test, 0 failed)
- [x] `cargo build --release -p findex` PASS
- [x] `findex rank` model-free sıralama çalışıyor
- [x] `findex_benchmark` MCP aracı eklendi
- [x] Cross-platform derleme (compile_error kaldırıldı)
- [ ] Canlı Claude Code entegrasyonu (gerçek `claude` CLI gerekli)
- [ ] GitHub main'e push
