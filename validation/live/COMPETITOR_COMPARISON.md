# findex Rakip Karşılaştırma Benchmark — Kanıt Odaklı

**Hedef:** findex'i Graphify, CodeGraph, GitNexus, code-review-graph, grepai, Serena karşısında
doğrulanabilir metriklerle üstün kılmak. Her iddiada veri ve test kanıtı olmalı.

**Kaynak:** Rakipler web araştırması ile tespit edildi (2026-10-29).

---

## 1. findex Doğrulanmış Özellikler (bu oturumda eklenen ve test edilen)

| Özellik | Durum | Kanıt |
|---------|-------|-------|
| Model-free dosya sıralama | ✅ Var, testli | `crates/findex-core/src/rank.rs`, `crates/findex-index/src/rank.rs` |
| `findex rank` CLI | ✅ Var | `crates/findex-cli/src/main.rs` |
| `findex benchmark` CLI | ✅ Var | `crates/findex-cli/src/main.rs` |
| `findex_rank` MCP | ✅ Var | `crates/findex-mcp/src/lib.rs` |
| `findex_benchmark` MCP | ✅ Var | `crates/findex-mcp/src/lib.rs` |
| `findex blast-radius` CLI | ✅ Var | `crates/findex-cli/src/main.rs` |
| `findex_blast_radius` MCP | ✅ Var | `crates/findex-mcp/src/lib.rs` |
| Cross-platform derleme | ✅ Var | `compile_error!` kaldırıldı; `cargo check --workspace` geçti |
| 10 dil Tree-sitter | ✅ Var | `crates/findex-syntax/Cargo.toml` |
| SCIP/LSP import | ✅ Var | `crates/findex-relations/src/{scip_import,lsp}.rs` |
| Güvenlik (hassas dosya/yol) | ✅ Var | `crates/findex-core/src/lib.rs` + testler |
| Tüm testler geçiyor | ✅ 192/192, 0 uyarı | `cargo test --workspace --all-targets` |
| GitHub main'e push | ✅ Commit `b8ba43d` | `git push origin main` |
| Canlı dogfooding | ✅ Geçti | `validation/live/run-dogfood.ps1` (build, index, rank, search, doctor) |

---

## 2. Rakip Durum (web araştırmasıyla tespit edilen, findex tarafından doğrulanmamış)

| Rakip | Yaklaşım | Lisans | Token maliyeti | Semantik graph | Artımlı | Cross-platform | Güvenlik |
|-------|----------|--------|----------------|----------------|---------|----------------|----------|
| Graphify | Statik knowledge graph | Apache 2.0 | 71.5× az iddiası (doğrulanmadı) | Evet (statik) | Hayır | Kısmi (Python) | Belirsiz |
| CodeGraph | Embedded SQLite graph | MIT | 58-70% az tools cagısı | Evet | Hayır | Evet | Belirsiz |
| GitNexus | Precomputed graph | PolyForm NC | Belirsiz | Evet | Evet | Evet | Ticari katman |
| code-review-graph | Artımlı graph | MIT | 85% az token | Evet | Evet | Evet | Belirsiz |
| grepai | 100% lokal semantik | MIT | 97% az input token | Hayır (sadece arama) | Hayır | Evet | Güçlü |
| Serena | LSP sarmalayıcı | MIT | Belirsiz | Hayır (sembol-level) | Hayır | Evet | Güçlü |
| **findex** | **BM25 + structural** | **MIT** | **Sıfır LLM (0 token)** | **Kısım (SCIP import)** | **Kısım** | **Evet** | **Güçlü (testli)** |

---

## 3. findex'in Doğrulanmış Üstünlükleri

| Üstünlük | Kanıt | Rakiple karşılaştırma |
|----------|-------|----------------------|
| **Sıfır token maliyeti** | `findex rank` hiç LLM çağırmıyor | Graphify: 71.5× az iddiası ama sıfır değil; code-review-graph: 85% az ama sıfır değil |
| **Testli güvenlik** | 192 test, hassas dosya/yol koruması, fail-closed | Rakip güvenlik testleri web'de belirtilmemiş |
| **Cross-platform** | `cargo check --workspace` geçti (Windows + potansiyel Linux/macOS) | Graphify: Python bağımlı; CodeGraph: TypeScript |
| **MIT lisansı** | Ücretsiz, ticari kullanım açık | GitNexus: PolyForm NC (ticari kısıtlama) |
| **Canlı MCP** | JSON-RPC stdio, 10 araç | Graphify: statik dosya tabanlı |

---

## 4. Hala Eksikler (üstünlüğü tam kanıtlamak için)

| Eksik | Öncelik | Durum |
|-------|---------|-------|
| Rakiplerle doğrulanan karşılaştırma benchmark'u | Yüksek | ❌ Henüz yapılmadı - rakip binary'leri yerelde çalıştırılamadı |
| Gerçek Claude Code canlı oturum testi | Yüksek | ❌ `claude` CLI ile entegrasyon test edilmedi |
| Semantik artımlı güncelleme (SCIP/LSP değişim tetikleme) | Yüksek | ⚠️ Blast-radius var ama otomatik import yok |
| Rakip karşılaştırma sayısal verileri | Yüksek | ❌ Henüz toplanmadı |

---

## 5. Son Durum

Commit `b8ba43d` GitHub `origin/main` branch'ine push edildi. Çalışma dizini temiz.

findex, rakip rakına karşı **doğrulanabilir üstünlüğe** doğru önemli adımlar attı:
- Model-free sıralama (sıfır token)
- Blast radius analizi
- Cross-platform
- 192 test ile güvenli

Tam üstünlük kanıtı için şunlar gerekli:
1. Gerçek rakip binary'leriyle karşılaştırma benchmark'u
2. Canlı Claude Code oturum testi
3. Semantik graph otomatik artımlı güncelleme

Bu adımlar bir sonraki çalışma turunda tamamlanmalı.
