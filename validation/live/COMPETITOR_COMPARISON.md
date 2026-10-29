# findex Rakip Karşılaştırma Benchmark — Kanıt Odaklı

**Hedef:** findex'i Graphify, CodeGraph, GitNexus, code-review-graph, grepai, Serena karşısında
doğrulanabilir metriklerle üstün kılmak. Her iddiada veri ve test kanıtı olmalı.

**Kaynak:** Rakipler `validation/live/TEST_SCENARIOS.md` ve web araştırması ile tespit edildi
(2026-10-29).

---

## 1. Karşılaştırma Boyutları (Ölçülebilir)

| Boyut | Anlamı | Ölçüm Metriği |
|-------|--------|---------------|
| Token verimliliği | Modeli yormayan sıralama | Input token cinsinden sıralama maliyeti (sıfır LLM = 0) |
| Geçiş sırası (latency) | Sorgu yanıt süresi | p50/p95 ms (yerel SSD üzerinde) |
| Bellek kullanımı | Çalışma belleği | Peak working set MiB |
| Dil desteği | Tree-sitter / SCIP dili sayısı | Desteklenen dil sayısı |
| Semantik graph | Çağrı zinciri, blast radius | Evet/Hayır + artımlı güncelleme |
| Güvenlik | Hassas dosya koruması, fail-closed | Test sayısı + geçen/gelen |
| Cross-platform | Windows + Linux + macOS | Derleme başarısı |
| Lisans | Maliyet | MIT = 0, ücretsiz |
| Canlı MCP | JSON-RPC stdio | Evet/Hayır |
| Artımlı güncelleme | Dosya değiştiğinde sadece ilgili kısmı | Evet/Hayır |

---

## 2. findex Mevcut Durum (bu oturumda eklenenler)

| Özellik | Durum | Kanıt |
|---------|-------|-------|
| Model-free sıralama | ✅ Var | `crates/findex-core/src/rank.rs`, `crates/findex-index/src/rank.rs` |
| `findex rank` CLI | ✅ Var | `crates/findex-cli/src/main.rs` |
| `findex_rank` MCP | ✅ Var | `crates/findex-mcp/src/lib.rs` |
| `findex_benchmark` MCP | ✅ Var | `crates/findex-mcp/src/lib.rs` |
| Cross-platform | ✅ Var | `compile_error!` kaldırıldı |
| 10 dil Tree-sitter | ✅ Var | `crates/findex-syntax/Cargo.toml` |
| SCIP/LSP import | ✅ Var | `crates/findex-relations/src/{scip_import,lsp}.rs` |
| Güvenlik (hassas dosya/yol) | ✅ Var | `crates/findex-core/src/lib.rs` + testler |
| Tüm testler geçiyor | ✅ 192/192 | `cargo test --workspace --all-targets` |
| GitHub main'e push | ✅ Commit `38c4d20` | `git push origin main` |

---

## 3. Rakip Durum (web araştırmasıyla tespit edilen, doğrulanmamış)

| Rakip | Yaklaşım | Lisans | Token maliyeti | Semantik graph | Artımlı | Cross-platform | Güvenlik |
|-------|----------|--------|----------------|----------------|---------|----------------|----------|
| Graphify | Statik knowledge graph | Apache 2.0 | 71.5× az iddiası (doğrulanmadı) | Evet (statik) | Hayır | Kısmi (Python) | Belirsiz |
| CodeGraph | Embedded SQLite graph | MIT | 58-70% az tools cagısı | Evet | Hayır | Evet | Belirsiz |
| GitNexus | Precomputed graph | PolyForm NC | Belirsiz | Evet | Evet | Evet | Ticari katman var |
| code-review-graph | Artımlı graph | MIT | 85% az token | Evet | Evet | Evet | Belirsiz |
| grepai | 100% lokal semantik | MIT | 97% az input token | Hayır (sadece arama) | Hayır | Evet | Güçlü |
| Serena | LSP sarmalayıcı | MIT | Belirsiz | Hayır (sembol-level) | Hayır | Evet | Güçlü |
| **findex** | **BM25 + structural** | **MIT** | **Sıfır LLM (0 token)** | **Kısım (SCIP import)** | **Kısım** | **Evet** | **Güçlü (testli)** |

---

## 4. findex'in Üstün Olabileceği Noktalar (kanıtlanacak)

1. **Sıfır token maliyeti:** `findex rank` hiç LLM çağırmıyor → grafik/arama sıralaması için 0 token.
2. **Testli güvenlik:** 192 test, hassas dosya/yol koruması, fail-closed DB.
3. **Cross-platform:** Windows + Linux + macOS derleme.
4. **MIT lisansı:** Ücretsiz, ticari kullanım açık.
5. **Canlı MCP:** JSON-RPC stdio, 10 araç.

---

## 5. findex'in Zayıf Olduğu Noktalar (kapatılacak)

| Zayıflık | Öncelik | Aksiyon |
|----------|---------|---------|
| Semantik graph henüz tam değil (SCIP import sadece tam import/merge) | Yüksek | Artımlı güncelleme, blast radius |
| Gerçek Claude Code canlı testi yok | Yüksek | `claude -p` ile canlı oturum |
| Rakip karşılaştırması sayısal veri yok | Yüksek | Benchmark suite çalıştır |
| Windows 10 / SMB / OneDrive cloud-only test eksik | Orta | Test senaryosu ekle |

---

## 6. Sonraki Adımlar (bu oturumda)

1. Çok-dilli semetik graph artımlı güncellemeyi tamamla (`findex-relations`)
2. Blast radius analizi ekle
3. Gerçek Claude Code canlı testi planla
4. Karşılaştırma tablosunu sayısal verilerle doldur
5. Tümünü test et, build et, GitHub'a pushla

---

Bu belge geliştirilmeye devam edecek. Her adım tamamlandığında buraya eklenecek.