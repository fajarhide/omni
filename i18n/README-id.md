<div align="center">
  <img src="../media/hero.svg" alt="OMNI" width="800" />
  
  **Sistem Operasi Konteks untuk Agen AI. Sedikit noise. Lebih banyak sinyal. Kurangi konsumsi token hingga 90%.**

  [🇺🇸 English](../README.md) | [🇯🇵 日本語](README-ja.md) | [🇨🇳 简体中文](README-zh.md) | [🇸🇦 العربية](README-ar.md) | [🇮🇩 Bahasa Indonesia](README-id.md) | [🇻🇳 Tiếng Việt](README-vi.md) | [🇰🇷 한국어](README-ko.md)

  [![CI](https://github.com/fajarhide/omni/actions/workflows/ci.yml/badge.svg)](https://github.com/fajarhide/omni/actions/workflows/ci.yml)
  [![Release](https://img.shields.io/github/v/release/fajarhide/omni)](https://github.com/fajarhide/omni/releases)
  [![Rust](https://img.shields.io/badge/built_with-Rust-dca282.svg)](https://www.rust-lang.org/)
  [![MCP](https://img.shields.io/badge/MCP-compatible-green.svg?style=flat-square)](https://modelcontextprotocol.io/)
  [![License: MIT](https://img.shields.io/github/license/fajarhide/omni)](https://github.com/fajarhide/omni/blob/main/LICENSE)
  [![Hits](https://hits.sh/github.com/fajarhide/omni.svg)](https://hits.sh/github.com/fajarhide/omni/)
</div>

<br/>

> **OMNI** adalah **Sistem Operasi Konteks (Context OS) untuk Agen AI Otonom**. 
> OMNI bertindak sebagai filter semantik berkinerja tinggi antara terminal Anda dan LLM. Dengan secara cerdas menyaring log yang bising, menyimpan *state*, dan mengelola anggaran token, OMNI memastikan agen Anda tetap fokus, mengurangi halusinasi, dan mengeksekusi *loop* dengan sempurna—semuanya sambil **memangkas biaya API Anda hingga 90%**.
> 
> *Berhenti membayar untuk kebisingan terminal. Mulai membangun dengan sinyal murni.*
---

## Daftar Isi
- [Masalah: Konteks Membengkak, Token Mahal & Output Bising](#masalah-konteks-membengkak-token-mahal--output-bising)
- [Solusi: Omni](#solusi-omni)
- [Filosofi](#filosofi)
- [Kasus Penggunaan Dunia Nyata](#kasus-penggunaan-dunia-nyata)
- [Performa & Tolok Ukur](#performa--tolok-ukur)
- [Penjelasan Fitur](#penjelasan-fitur)
- [Di Balik Layar: Cara Kerja Omni](#di-balik-layar-cara-kerja-omni)
- [Arsitektur](#arsitektur)
- [Mulai Cepat & Instalasi](#mulai-cepat--instalasi)
- [Cara Menggunakan](#cara-menggunakan)
  - [Dukungan Multi-Agen & Integrasi](#dukungan-multi-agen--integrasi)
  - [Indeks Dokumentasi](#indeks-dokumentasi)
- [Bekerja Lebih Baik dengan Heimsense](#bekerja-lebih-baik-dengan-heimsense)
- [Kontribusi & Lisensi](#kontribusi--lisensi)

---

## Masalah: Token Mahal, Halusinasi & Loop Tanpa Akhir

Ketika Anda menggunakan agen AI otonom (seperti Claude Code, Cursor, atau Aider) di terminal, mereka membaca *semuanya*. Perintah sederhana seperti `npm install` atau `cargo test` dapat dengan mudah membuang 10.000 hingga 25.000 token dari kebisingan terminal yang tidak berguna ke dalam jendela konteks AI Anda.

Hal ini menyebabkan kegagalan fatal:
1. **Anggaran Terbakar**: Anda membayar uang sungguhan untuk setiap token dari output sampah tersebut.
2. **"Amnesia" Agen & Halusinasi**: Kesalahan inti terkubur di bawah megabyte *loading bar* dan peringatan dependensi. AI menjadi bingung, kehilangan tujuan awal, dan berhalusinasi memperbaiki masalah yang salah.
3. **Penguncian Model**: Anda dipaksa menggunakan model *flagship* termahal hanya untuk mendapatkan jendela konteks yang cukup besar untuk menangani pembengkakan tersebut.
4. **Loop yang Rapuh**: Loop otonom gagal karena agen tidak menyadari batas token dan tekanan konteks.

## Solusi: OMNI Context OS

OMNI adalah *middleware* transparan pamungkas untuk Agentic AI. 

Ia mencegat perintah terminal secara dinamis, membuang kebisingannya, dan menyuapkan ringkasan semantik yang sangat padat kepada AI Anda. **Hasilnya?** Anda dapat menjalankan agen Anda pada model yang lebih terjangkau, memberinya *nol kebisingan*, dan melihatnya menyelesaikan tugas *coding* kompleks secara instan.

Baik Anda menjalankan pemanggilan alat MCP sederhana atau mengorkestrasi *loop* multi-agen Maker-Checker yang masif, OMNI menyediakan memori persisten, pelacakan anggaran, dan batasan faktual yang dibutuhkan AI Anda untuk berhasil.

Konteks itu mahal dan bising. OMNI memperbaikinya.

---

## Filosofi

OMNI tidak dibangun hanya untuk "memotong konteks" atau "menghemat token"—itu hanyalah efek samping yang membahagiakan. Filosofi sebenarnya di balik OMNI adalah **Kualitas Konteks**.

Agen AI seperti Claude hanya sepintar konteks yang Anda berikan kepada mereka. Ketika Anda membanjiri mereka dengan megabyte log dependensi atau loading bar, Anda memaksa mereka untuk memilah-milah sampah untuk menemukan masalah yang sebenarnya. Hal ini mencairkan penalaran mereka dan mengarah pada respons yang menurun kualitasnya atau tidak membantu.

**Tujuan OMNI adalah untuk memberi AI Anda sinyal murni yang sangat padat.** Ini berarti hanya mengambil konteks yang benar-benar penting dan bermakna bagi Claude. Kami membersihkan kebisingan yang tidak dibutuhkan AI, yang berarti:
1. Secara otomatis, token yang Anda gunakan secara drastis lebih sedikit.
2. Kualitas respons AI menjadi **jauh lebih tinggi** karena jendela konteksnya difokuskan pada masalah yang sebenarnya.

**Cobalah selama seminggu.** Rasakan perbedaan dalam kualitas dan kecepatan penalaran AI Anda saat diberi diet sinyal murni alih-alih kebisingan terminal mentah.

---

## Kasus Penggunaan Dunia Nyata

OMNI dirancang untuk memecahkan frustrasi harian para pengembang Agentic AI. Berikut cara OMNI mengubah alur kerja Anda:

1. **"Infinite Loop of Death" di Monorepo**
   - **Skenario**: Anda meminta Claude menjalankan `npm install` dan `npm run build` di monorepo besar. Terminal mengeluarkan 20.000 baris peringatan dependensi dan kesalahan build kecil di akhir. AI terganggu oleh peringatan dan mencoba memperbaiki masalah dependensi yang tidak relevan, membakar token Anda dan menjebak Anda dalam putaran tanpa akhir.
   - **Solusi OMNI**: OMNI mencegat proses build. Ia sepenuhnya membisukan ratusan peringatan `peer dependency` dan hanya memunculkan `Build Error: Cannot find module 'X'` beserta stack trace-nya. AI melihat output 50 token dan segera memperbaiki kodenya.

2. **"Silent Hallucination" pada File Besar**
   - **Skenario**: AI ingin memahami proyek dan menjalankan `cat src/utils.ts`. File tersebut panjangnya 3.000 baris. AI kesulitan menyimpan semuanya di memori kerja dan mulai berhalusinasi fungsi.
   - **Solusi OMNI**: OMNI memblokir perintah `cat` mentah dan menggantinya dengan **Structured Outline**. OMNI menunjukkan kepada AI impor, API publik (nama fungsi dan tipe), dan penanda risiko, mengurangi output sebesar 80%. OMNI lalu memperingatkan AI: `"File ini memiliki 12 dependensi — gunakan omni_context untuk peta dampak."` AI diarahkan untuk melakukan pengeditan faktual yang lebih aman.

3. **Kolaborasi Multi-Agen**
   - **Skenario**: Anda menggunakan Cursor IDE untuk pengeditan cepat dan Claude Code CLI untuk tugas berat. Keduanya perlu tahu apa yang terjadi tanpa menjalankan perintah berulang dan membuang token.
   - **Solusi OMNI**: OMNI bertindak sebagai lapisan memori bersama. Melalui `omni_agents` dan `Store` SQLite lokal, Cursor dan Claude berbagi aliran memori terfilter, error aktif, dan lingkungan eksekusi yang sama. Mereka berkolaborasi tanpa bentrok.

---

## Performa & Tolok Ukur
<div align="center">
<img src="https://omni.weekndlabs.com/media/performance.png" alt="OMNI" width="600" />
</div>

Angka jujurnya, diukur pada binary release terhadap **1.810 eksekusi perintah nyata** yang diputar ulang dari pemakaian seorang pengembang:

* **58,9% lebih sedikit byte** yang sampai ke model (15,0 MB → 6,2 MB).
* **63,6% dari panggilan itu tidak menghemat apa pun.** OMNI mengembalikan output apa adanya dan **tidak menambah satu byte pun**. Seluruh penghematan datang dari 36,4% sisanya, tempat noise-nya memang nyata.
* **Output terstruktur tidak pernah disentuh.** JSON, YAML, NDJSON, dan CSV lewat byte-for-byte, karena payload yang rusak lebih mahal daripada kompresi yang terlewat.

Butir kedua itulah angka yang jarang dicetak tools sejenis. Alat yang mengklaim menghemat 90% pada setiap perintah sedang memberi tahu Anda bahwa output yang Anda butuhkan ikut diringkas.

Dari mana penghematannya berasal, pada 1.810 eksekusi yang sama:

| Perintah | Panggilan | Input | Output | Hemat |
|----------|-----------|-------|--------|-------|
| `cargo` | 29 | 424 KB | 13 KB | **96,8%** |
| `git` | 256 | 5,9 MB | 509 KB | **91,3%** |
| `ls` | 52 | 71 KB | 29 KB | **59,5%** |
| `kubectl` | 212 | 4,4 MB | 2,3 MB | **48,0%** |
| `find` | 39 | 83 KB | 53 KB | **36,2%** |
| `grep` | 184 | 534 KB | 385 KB | **27,8%** |
| `cat` | 85 | 515 KB | 468 KB | **9,1%** |

`git` dan `cargo` yang menanggung hasilnya; `cat` dan `grep` nyaris tanpa efek. OMNI berguna pada output tooling yang berisik dan berulang, dan menyingkir di tempat lain.

Fixture tunggal dari `tests/fixtures/`, bila ingin direproduksi sendiri:

| Command / Konteks | Input | Output | Hemat |
|-------------------|-------|--------|-------|
| `cargo build` (besar, sukses) | 3.220 B | 9 B | **99,7%** |
| `cargo test` (490 lulus, 10 gagal) | 16,5 KB | 1.100 B | **93,3%** |
| `pytest` (dengan kegagalan) | 730 B | 136 B | **81,4%** |
| `git status` (dirty) | 496 B | 113 B | **77,2%** |
| `git diff` (multi-file) | 397 B | 220 B | **44,6%** |
| `docker build` (noise berat) | 9,2 KB | 5,8 KB | **37,2%** |
| `kubectl get pods` (campuran) | 840 B | 762 B | **9,3%** |

**Latensi adalah biaya nyata, bukan nol.** OMNI berjalan pada setiap perintah yang di-hook, dan harganya tumbuh mengikuti riwayat Anda: `git status` 496 byte butuh ~82 ms terhadap database bersih dan ~308 ms terhadap database 97 MB. `cargo test` 16,5 KB butuh ~276 ms. Perhitungkan itu.

*Untuk melihat penghematan token aktual Anda sendiri, jalankan saja `omni stats` setelah beberapa hari penggunaan.*

---

## Penjelasan Fitur

### Core Distillation Engine (Mesin Distilasi Inti)
- **Tidak Ada Lagi Kebingungan AI**: Omni bertindak seperti saringan pintar. Jika tes gagal, ia hanya menunjukkan baris kesalahan dan stack trace, memblokir log dependensi yang bising.
- **Pengurangan Token 90%**: Dengan menghilangkan noise terminal, Anda memotong tagihan API agen secara drastis.
- **Kompresi Adaptif**: OMNI melacak kapan agen mengambil output yang dihilangkan dan secara otomatis melunakkan kompresi pada waktu berikutnya — menyetel sendiri secara otomatis.
- **Smart High-Speed Bypass**: Untuk menjamin latensi nol pada tugas kecil, OMNI secara otomatis melewati distilasi untuk output di bawah ambang 2000-token.

### Context Safety & Factual Guards (Keamanan Konteks)
- **Nol Kehilangan Informasi**: Output mentah disimpan secara lokal (`RewindStore`). AI dapat memintanya secara otomatis menggunakan `omni_retrieve`.
- **Penjaga Anti-Halusinasi Faktual**: OMNI menyuntikkan peringatan sistem (seperti file dengan dependensi masif) untuk menjaga AI Anda tetap berpijak pada fakta.
- **Visibilitas Penghilangan**: OMNI melabeli konten yang dihapus secara eksplisit (mis. `[OMNI: omitted X lines of noise]`), memberi agen kesadaran situasional.

### Multi-Agent & Workspace Intelligence (Kecerdasan Ruang Kerja)
- **Kolaborasi Multi-Agen**: Jika Anda menjalankan Cursor bersama Claude CLI, mereka berbagi aliran memori terfilter yang sama tanpa bentrok.
- **Kecerdasan Sesi**: OMNI mengingat file yang sedang Anda edit dan berhenti memberikan konteks berulang.
- **Structured ReadFile + Grep**: Alih-alih dump file mentah, OMNI mengembalikan kerangka terstruktur (impor, API) dan ringkasan grep yang dikelompokkan.
- **Grafik Dependensi Ringan**: OMNI membangun grafik relasi file lokal yang cepat. Jika AI membaca file penting, OMNI memperingatkan tentang peta dampaknya.

### Keamanan Konteks & Pemulihan Sesi (Context Fidelity)
- **Tekanan Konteks Proaktif**: OMNI bertindak sebagai "Lampu Lalu Lintas Token." Melalui alat MCP `omni_insight`, OMNI secara proaktif memperingatkan agen ketika jendela konteksnya mencapai ambang "Peringatan" atau "Kritis", memicu agen untuk mengompresi memorinya *sebelum* macet atau berhalusinasi.
- **Engrams (Ringkasan Subtugas Otomatis)**: OMNI secara otomatis mendeteksi saat sebuah subtugas selesai (mis., menyelesaikan kesalahan kompilator, melakukan commit kode, atau memperbaiki tes yang gagal). OMNI membuat cuplikan yang sangat terkompresi ("Engram") tanpa membuang token pada panggilan LLM, sehingga agen Anda tidak akan pernah mengalami "amnesia konteks" selama sesi yang panjang.
- **Pemadatan Konteks Cerdas (Smart Context Compaction)**: Ketika jendela konteks Anda penuh, OMNI tidak memangkas token secara membabi buta. OMNI menggunakan algoritma sadar prioritas untuk mengemas data terpenting terlebih dahulu (File yang Disematkan > Kesalahan Aktif > Engram > Aktivitas Alat > File Panas), menghemat overhead besar-besaran.
- **Serah Terima Sesi (Session Handoffs)**: Beralih dari Claude Code ke Cursor? Gunakan alat `omni_handoff` untuk mengekspor memori sesi saat ini secara instan (file panas, perintah terbaru, kesalahan aktif) ke dalam ringkasan markdown portabel yang dapat langsung diserap oleh agen baru Anda.

### Rekayasa Loop Otonom (Autonomous Loop Engineering)
- **Sistem Operasi Konteks untuk Loop**: OMNI mengelola konteks untuk agen loop otonom yang iteratif. Melalui variabel lingkungan (`OMNI_LOOP_BUDGET`, `OMNI_LOOP_GOAL`), OMNI memberlakukan batas distilasi adaptif dan pelacakan persisten.
- **Pola Verifikasi Maker-Checker**: Skalakan tugas Anda dengan rapi dengan memisahkan eksekusi (Agen Pembuat/Maker) dari validasi (Agen Pemeriksa/Checker), bertukar status konteks secara aman melalui penyimpanan sesi multi-agen OMNI.
- **Batasan Berbasis Tujuan Prediktif**: Distilasi secara otomatis diskalakan berdasarkan tujuan tugas—jika tujuan berisi "debug", OMNI mempertahankan lebih banyak konteks kesalahan. Jika "refactor", OMNI mengompresi jejak kode secara agresif.

### Monitoring & Debugging (Pemantauan)
- **Dasbor Kesehatan Sesi (Session Health Dashboard)**: Jalankan `omni session --health` untuk melihat dasbor visual yang indah tentang tekanan konteks Anda, engram aktif, aktivitas alat bergulir, dan penghematan token.
- **Monitor Distilasi**: Lacak penghematan token menggunakan `omni_budget` di dalam LLM, atau jalankan `omni stats` secara lokal.
- **Dampak Visual (`omni diff`)**: Jalankan `omni diff` untuk membandingkan output mentah dengan versi Omni yang ramping secara berdampingan.
- **Debug Passthrough**: Setel `OMNI_PASSTHROUGH=1` untuk melewati mesin sepenuhnya dan melihat output asli.

---

## Di Balik Layar: Cara Kerja Omni

OMNI lebih dari sekadar skrip regex; ia adalah **Semantic Signal Engine** berkinerja tinggi yang ditulis dengan Rust. Namun bagaimana cara kerjanya memutuskan apa yang boleh dibuang — dan kapan sebaiknya tidak menyentuh apa pun?

Inilah kisah tentang apa yang terjadi di dalam kode OMNI saat Agen AI Anda mengetik perintah seperti `cargo test`:

1. **Intersepsi (`src/hooks` & `src/main.rs`)**: Saat AI menekan "Enter", OMNI mencegat eksekusi secara dinamis. Modul `hooks` membungkus perintah dengan mulus, memungkinkan OMNI menangkap output mentah sebagai aliran data berkecepatan tinggi tanpa memperlambat eksekusi aktual.
2. **Streaming Pipeline (`src/pipeline`)**: Alih-alih menunggu perintah selesai dan membuang megabyte teks ke dalam memori, OMNI memproses baris demi baris melalui pipeline streaming. Jejak memori OMNI tetap datar meskipun menghadapi 10.000 baris log.
3. **Otak Semantik (`src/distillers` & `src/guard`)**: Saat teks masuk, ia melewati Distiller (didukung oleh aturan TOML di `signals/`). 
   - Apakah ini loading spinner? *Buang.* 
   - Apakah ini daftar 500 tes yang lulus? *Buang.* 
   - Apakah ini panic stack trace? **Simpan.** 
   Modul `guard` memastikan fakta dipertahankan dan OMNI tidak pernah mengubah informasi diagnostik penting.
4. **Jaring Pengaman (`src/store`)**: Bagaimana jika AI benar-benar perlu melihat 500 tes yang lulus? OMNI menyimpan output mentah yang belum diedit dengan aman di database SQLite lokal yang sangat cepat (`Store`). OMNI hanya meninggalkan jejak di konteks AI: `[OMNI: omitted 1,200 lines of noise. Use omni_retrieve to view]`.
5. **Antarmuka Multi-Agen (`src/mcp` & `src/session`)**: Output ber-sinyal tinggi akhirnya dikembalikan ke AI. Di belakang layar, server `mcp` bersiap. Jika AI ingin menanyakan riwayat kesalahan atau mengambil log mentah, alat MCP OMNI menyediakan akses terstruktur instan.

**Hasilnya:** Output terminal `25.000` token yang membengkak menjadi laporan kesalahan `400` token yang padat. AI memahami masalahnya secara instan, dan Anda menghemat uang sungguhan.

---

## Arsitektur

<div align="center">
  <img src="../media/architecture.svg" alt="OMNI Architecture Diagram" width="100%" />
</div>

## Mulai Cepat & Instalasi

Omni sangat mudah diatur. Ini terintegrasi secara native ke dalam terminal Anda.

**macOS / Linux:**
```bash
# 1. Instal via Homebrew
brew install fajarhide/tap/omni

# 2. Setup Omni (Menu Interaktif untuk Claude, VS Code, OpenCode, Codex, Antigravity)
omni init

# 3. Verifikasi instalasi berhasil
omni doctor

# 4. Atau perbaiki masalah secara otomatis
omni doctor --fix

# 5. Cek Status Saat Ini
omni init --status
```

**Universal Installer (macOS / Linux / WSL):**
```bash 
curl -fsSL omni.weekndlabs.com/install | bash
```

**Windows (PowerShell):**
```powershell
irm omni.weekndlabs.com/install.ps1 | iex
```

---

## Cara Menggunakan

Setelah diinstal melalui `omni init`, OMNI bekerja tanpa terlihat di latar belakang. Baik Agen AI Anda menjalankan perintah terminal melalui MCP atau Anda mem-pipe output secara manual (`ls | omni`), OMNI secara otomatis melompat masuk sebagai lapisan transparan. Ini secara cerdas menyaring output terminal, menghilangkan log yang bising, dan menyerahkan sinyal bersih kembali ke AI.

Untuk melihat rincian penghematan, perintah, periode, dan rute:
```bash
omni stats
```

Untuk mendiagnosis instalasi OMNI Anda (hook, MCP, filter, database):
```bash
omni doctor
```

Perlu melihat filter beraksi atau menambahkan aturan khusus Anda sendiri?
Anda dapat dengan mudah membuat aturan Anda sendiri menggunakan file TOML sederhana di `~/.omni/signals/`.

### Dukungan Multi-Agen & Integrasi

Secara default, `omni init --claude` secara otomatis masuk ke **Claude Code**. Namun, OMNI bekerja sempurna dengan AI agen apa pun melalui integrasi bawaannya! Jalankan `omni init` untuk melihat menu interaktif.

1. **VS Code & Continue.dev**: Gunakan penyedia konteks MCP kami (`integrations/continue-dev/`).
2. **OpenCode & Codex CLI**: Wrapper bawaan secara otomatis menyalurkan output perintah ke OMNI.
3. **Antigravity IDE**: OMNI mendaftar sebagai server MCP asli dalam konfigurasi Antigravity (`~/.gemini/antigravity/mcp_config.json`). Jalankan `omni init --antigravity` untuk mengatur secara otomatis.
4. **Pi Agent**: Paket OMNI native untuk Pi. Jalankan `omni init --pi` untuk menginstal melalui installer paket Pi.

**Penyetelan Multi-Agen (`~/.omni/config.toml`)**
Agen yang berbeda memiliki titik nyeri yang berbeda. Jaga agar obrolan VS Code tetap bersih, sambil membiarkan OpenCode membaca lebih banyak data. Setel secara individual:
```toml
[global]
aggressiveness = "balanced"

[agents.vscode_continue]
aggressiveness = "aggressive"
enable_readfile_distillation = true

[agents.opencode]
aggressiveness = "conservative"
enable_readfile_distillation = false
```

### Indeks Dokumentasi

**Untuk Pengguna:**
- [Panduan Utama (HOW_TO_USE.md)](../docs/HOW_TO_USE.md) — Segala yang Anda butuhkan: Instalasi, Filter TOML Khusus, dan Perintah CLI.
- [Integrasi OpenClaw](https://clawhub.ai/fajarhide/omni-signal-engine) — Plugin OpenClaw resmi untuk distilasi OMNI native.
- [Integrasi Hermes Agent](https://github.com/wysie/hermes-omni-plugin) — Plugin Hermes Agent komunitas untuk distilasi OMNI native.

**Untuk Developer & System Integrator:**
- [Panduan Rekayasa Loop (LOOP_ENGINEERING.md)](../docs/LOOP_ENGINEERING.md) — Cara mengintegrasikan tekanan konteks OMNI dengan skrip agen otonom (Pola Maker-Checker, loop shell).
- [Panduan Pengembangan](../docs/DEVELOPMENT.md) — Cara membangun dan berkontribusi pada basis kode OMNI.
- [Arsitektur Pengujian](../docs/TESTING.md) — Jaminan kualitas dan keamanan konteks.
- [Keberlanjutan Sesi](../docs/SESSION.md) — Penyelaman mendalam ke dalam memori kerja OMNI.
- [Peta Jalan](../docs/ROADMAP.md) — Status pengembangan saat ini dan fitur yang akan datang.
- [Panduan Migrasi](../docs/MIGRATION.md) — Catatan tentang pemutakhiran dari versi Node/Zig ke versi Rust.

---

## Bekerja Lebih Baik dengan Heimsense

Omni adalah bagian dari sabuk alat AI pribadi saya. Jika Anda menggunakan `claude-code`, saya sangat menyarankan memasangkan Omni dengan proyek saya yang lain: **[Heimsense](https://github.com/fajarhide/heimsense)**.

Heimsense membuka kunci lingkungan terbatas seperti `claude-code` untuk berjalan dengan model gratis atau yang kompatibel dengan OpenAI *apa pun*, alih-alih memaksa Anda untuk menggunakan model Anthropic yang mahal.
**Omni + Heimsense** = Jalankan kerangka kerja agen menggunakan model yang terjangkau dengan nol kebisingan dan akurasi yang tepat sasaran.

---

## Kontribusi & Lisensi

Ini adalah proyek hasrat yang dibangun untuk era Agentic AI. Baik Anda di sini untuk menghemat uang pada token, menguji model gratis, atau membantu membangun sabuk alat agenik pamungkas, kontribusi selalu diterima!

- **Pengembangan**: Ingin membangun dari sumber? Jalankan `make ci` dan `cargo build`. Baca [CONTRIBUTING.md](../CONTRIBUTING.md) kami untuk detailnya.
- **Lisensi**: [MIT License](../LICENSE)

<!-- Star History -->
<p align="center">
  <a href="https://star-history.com/#fajarhide/omni&Date">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/svg?repos=fajarhide/omni&type=Date&theme=dark" />
      <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/svg?repos=fajarhide/omni&type=Date" />
      <img alt="Star History Chart" src="https://api.star-history.com/svg?repos=fajarhide/omni&type=Date" width="600" />
    </picture>
  </a>
</p>

Dibuat dengan ❤️ oleh [Fajar Hidayat](https://github.com/fajarhide)
