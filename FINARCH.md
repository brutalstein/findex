# findex FINARCH — Nihai Mimari ve Uygulama Planı

**Hedef:** findex'i kanıtla beraber, en güçlü ve en uygun maliyetli codebase-navigasyon projesine dönüştürmek. Modeli yormadan (sıfır LLM çağrısı) en doğru dosyaları sıralayacak, çeşitli yetenekleri olacak, canlı gerçek testlerle doğrulanacak, GitHub main'e push edilecek.

**Rakipler (güncel kaynaklardan tespit edilen):** Graphify, CodeGraph, GitNexus, code-review-graph, grepai, Serena, Repomix, Harmony MCP.

---

## 1. Tasarım İlkeleri

| İlkke | Anlamı | Findex'te nasıl karşılanır |
|-------|--------|---------------------------|
| Modeli yorma | Dosya sıralaması için hiç LLM çağrısı yok | BM25 + Tree-sitter yapısal sinyal + recency + dosya-türü ağırlığı, pure Rust |
| En ucuz | Yerel, network yok, ücretsiz, açık kaynak | SQLite/FTS5 lokal, Tree-sitter lokal, MIT lisansı, cloud yok |
| En güçlü | Canlı graph, blast radius, çoklu yetenek, çok dili | Canlı semetik graph, artımlı güncelleme, impact analizi, 10 dil |
| Canlı test | Gerçek depolar, gerçek MCP oturumları, gerçek oyun | Built-in benchmark ve live test suite, competitor karşılaştırması |
| Kanıtlı | Her iddiada checksum, dosya, commit | Validation dizinleri, SHA-256, sonuç JSON'ları |

---

## 2. Crate Haritası (mevcut + planlanan)

| Crate | Mevcut sorumluluk | Planlanan ekleme |
|-------|-------------------|------------------|
| `findex-core` | Yollar, izolasyon, hassas dosya, gizli kelime | Cross-platform path utility, rank scoring model |
| `findex-syntax` | Tree-sitter tanımlık çıkarıcı (10 dil) | Çağrı zinciri çıkarıcı (syntactic call edges), dil eklentileri |
| `findex-index` | SQLite/FTS5 lexical dizin, search, symbol, context | `findex-rank` model-free sıralama motoru |
| `findex-relations` | SCIP/LSP import, file-impact graph | Canlı artımlı graph, blast radius, MCP impact araçları |
| `findex-watch` | notify 8.2.0 Windows watcher | Cross-platform watcher, olay kanalı iyileştirme |
| `findex-mcp` | JSON-RPC stdio MCP, 8 araç | 16+ MCP araç (rank, graph, impact, benchmark, compare) |
| `findex-cli` | findex.exe komut satırı | `rank`, `graph-live`, `impact-live`, `benchmark`, `compare` komutları |
| `findex-bench` (yeni) | — | Built-in competitor karşılaştırma ve benchmark araçları |

---

## 3. Yeni Yetenekler (modeli yormaz, sıfır LLM)

### 3.1. `findex rank` — Model-free dosya sıralama
- Girdi: sorgu (terim / sembol / kavram)
- Çıktı: en doğru dosyaların sıralı listesi (yol, satır, puan, neden)
- Sinyaller:
  - BM25 lexical puan (mevcut FTS5)
  - Tanımlık puanı (Tree-sitter: dosyada tanımlı sembol sayısı)
  - Çağrı zinciri puanı (dosya, sorgudaki sembolu çağırıyor mu)
  - Recency (son değiştirilme, git mtime)
  - Dosya türü ağırlığı (kaynak > test > doc > config)
  - Boyut cezası (çok büyük dosyalar biraz aşağı)
- Hiç LLM çağrısı yok → en ucuz, en hızlı, modeli yormaz
- Çıktı: JSON + insan okunabilir tablo

### 3.2. `findex graph-live` — Canlı semetik graph
- SCIP/LSP import (mevcut) + artımlı güncelleme (yeni dosya değiştiğinde sadece ilgili düğüm/kenar)
- Blast radius: bir dosya değiştiğinde etkilenen akışkan ve akıyıcı dosyalar
- Çağrı zinciri görünümü (syntactic + SCIP semantic)
- `findex graph-symbol <id>` — sembolun tüm siteleri ve ilişkileri

### 3.3. `findex impact-live` — Gerçek zamanlı etk analizi
- Hangi dosya değişirse hangi dosyalar bozulur (confidence ile)
- Test dosyaları öncelikli (değişen kodu test eden dosyalar)
- PR/commit impact özeti

### 3.4. `findex benchmark` — Built-in rakiper karşılaştırma
- Golden set (642 sorgu) üzerinde findex vs Graphify vs code-review-graph vs grepai vb.
- Token, latency, recall, precision ölçümü
- Sonuç JSON + rapor

### 3.5. `findex compare` — Rakiplerle yan yana karşılaştırma
- İki yapı aynı sorgu setinde çalıştırılır, sonuçlar karşılaştırılır
- "Findex daha iyi / aynı / kötü" etiketi

---

## 4. Cross-Platform (Windows dışına çıkarma)

Mevcut: `crates/findex-cli/src/main.rs:4` `compile_error!` non-Windows.

Plan:
- `compile_error!` kaldırılacak, Rust 2021 edition cross-platform yapılacak
- `notify` zaten cross-platform
- `rusqlite`, `tree-sitter`, `blake3`, `ignore` cross-platform
- Windows-only ACL/test'ler `cfg(windows)` ile ayrıştırılacak
- `findex doctor` platform raporu verecek

---

## 5. MCP Araçları (8 → 16+)

Mevcut 8: search, find_symbol, symbols, context, overview, capabilities, impact, graph_symbol.

Yeni 8: rank, graph-live, impact-live, benchmark, compare, doctor, status, forget (bazıları CLI'den gelen).

Her tool: JSON-RPC, fail-closed, hassas veri sızdırmaz.

---

## 6. Canlı Test ve Kanıt Altyapısı

- `validation/live/` — Canlı test senaryoları (gerçek depolar, gerçek MCP oturumları)
- `validation/bench/` — Rakiplerle karşılaştırma sonuçları
- `validation/results/<run>/` — Mevcut desenle devam eder (manifest, environment, commands, checksums)
- Her test: SHA-256 checksum, çalışma zamanı, bellek, token, sonuç geçerliliği

---

## 7. Uygulama Sırası (Roadmap)

| Adım | İş | Durum |
|------|-----|-------|
| A1 | Mimari belge (FINARCH) | Bu dosya |
| A2 | Cross-platform: compile_error kaldır, cfg(windows) ayır | Planlanıyor |
| A3 | `findex-core`: rank scoring model (BM25 + structural + recency) | Planlanıyor |
| A4 | `findex-index`: `rank` komutu ve MCP `findex_rank` aracı | Planlanıyor |
| A5 | `findex-syntax`: syntactic call edge çıkarıcı | Planlanıyor |
| A6 | `findex-relations`: artımlı graph + blast radius | Planlanıyor |
| A7 | `findex-mcp`: 8 yeni araç | Planlanıyor |
| A8 | `findex-cli`: rank, graph-live, impact-live, benchmark, compare | Planlanıyor |
| A9 | `findex-bench`: built-in karşılaştırma | Planlanıyor |
| A10 | Canlı test senaryoları + kanıt dosyaları | Planlanıyor |
| A11 | Tüm testleri çalıştır, hata düzelt | Planlanıyor |
| A12 | GitHub main'e commit ve push | Planlanıyor |

---

## 8. Başarı Kriterleri (kanıtla beraber)

1. `cargo fmt --all -- --check` PASS
2. `cargo clippy --workspace --all-targets -- -D warnings` PASS
3. `cargo test --workspace --all-targets` PASS
4. `cargo build --release -p findex` PASS (Windows + Linux/macOS)
5. `findex rank` doğru dosyaları sıralar (golden set üzerinde recall@10 ≥ 0.90)
6. `findex graph-live` canlı graph döndürür (SCIP import + artımlı güncelleme)
7. `findex impact-live` blast radius verir (confidence ile)
8. `findex benchmark` rakiplerle karşılaştırma raporu üretir
9. Canlı gerçek MCP oturumu başarılı (Claude Code / Codex)
10. SHA-256 checksumlu sonuç dosyaları mevcut
11. GitHub main branch'e push edildi

---

Bu belge uygulama rehberidir. Her adım tamamlandığında bu dosya güncellenir ve kanıt dosyaları `validation/results/` altında toplanır.