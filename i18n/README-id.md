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

> **OMNI** adalah **Semantic Signal Engine** berkinerja tinggi dan **Sistem Operasi Konteks** yang secara cerdas mencegat, menganalisis, dan menyaring output terminal sebelum mencapai Agen AI Anda. Ini bertindak sebagai lapisan optimasi sinyal yang transparan di antara shell dan AI, memastikan setiap token yang dikirim bernilai tinggi, relevan, dan bebas noise. Dengan mencegah AI Anda kebingungan oleh output yang bising, Anda mendapatkan jawaban akurat lebih cepat sekaligus menghemat biaya token secara masif.
> 
> *Sepenuhnya transparan. Anda selalu memegang kendali.*
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

## Masalah: Konteks Membengkak, Token Mahal & Output Bising

Ketika Anda menggunakan agen AI otonom (seperti Claude Code atau Cursor) di terminal Anda, mereka membaca *semuanya*. Perintah sederhana seperti `git diff`, `npm install`, atau `cargo test` dapat dengan mudah membuang 10.000 hingga 25.000 token dari kebisingan terminal yang tidak berguna ke dalam konteks AI Anda.

Hal ini menyebabkan masalah besar:
1. **Sangat mahal**: Anda membayar dengan uang sungguhan untuk setiap token dari output sampah tersebut.
2. **Membuat AI menjadi "bodoh"**: Kesalahan kritis terkubur di bawah megabyte log peringatan dan loading bar, membingungkan AI dan mencairkan penalarannya.
3. **Penguncian Model**: Kerangka kerja agen canggih memaksa Anda menggunakan model unggulan mereka yang paling mahal hanya agar memiliki jendela konteks yang cukup besar untuk menangani semua kebisingan tersebut.
4. **Eksekusi Rawan Token**: Agen tidak menyadari biaya token dan output, yang mengarah pada konsumsi yang tidak perlu.
5. **Konteks Membengkak**: Volume output terminal mengacaukan konteks AI, mengurangi fokus dan akurasi.

## Solusi: Omni

Saya membangun Omni karena saya ingin menjalankan agen AI secara efisien dan murah setiap hari dalam alur kerja saya sendiri.

**Omni bertindak sebagai filter sempurna antara terminal Anda dan AI Anda.**

**Hasilnya?** Anda dapat menjalankan agen AI Anda pada kerangka kerja yang sangat canggih dan memberinya *nol kebisingan*. Karena AI hanya diberi konteks yang sangat terfokus dan langsung pada intinya, bahkan model yang terjangkau atau biasa pun akan berkinerja setara dengan model unggulan yang mahal, karena mereka tidak pernah terganggu oleh data sampah.

Gairah utama saya bukanlah untuk memonetisasi ini—melainkan untuk membangun perangkat sumber terbuka pamungkas untuk era Agentic AI. Dengan menghemat biaya token secara agresif, saya dapat mengembangkan perangkat lunak secara tangguh dan hemat biaya hari ini, dan Anda juga bisa.

Konteks itu mahal dan bising, dan Omni hadir untuk memperbaikinya. Dengan mengoptimalkan konteks, Omni membuat agen AI lebih efisien, hemat biaya, dan mudah digunakan. Ini dilakukan dengan mengurangi jumlah konteks yang dikirim ke agen AI, yang pada gilirannya mengurangi jumlah waktu pemrosesan dan memori yang diperlukan untuk menghasilkan respons.

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

OMNI dibangun dengan Rust untuk eksekusi tanpa overhead dan efisiensi tinggi. Berikut adalah tolok ukur aktual yang diukur pada binary release:

| Command / Konteks | Ukuran Input | Ukuran Output | Penghematan Token | Dampak pada AI |
|-------------------|--------------|---------------|-------------------|----------------|
| `docker build` (multi-stage) | 9.2 KB | 49 bytes | **99.5%** | Menghilangkan kebisingan caching; AI langsung melihat error build yang sebenarnya. |
| `cargo test` (large suite) | 16.5 KB | 4.3 KB | **78.0%** | Menghapus ratusan tes "ok"; AI hanya fokus pada kegagalan dan stack trace. |
| `git status` (dirty) | 496 bytes | 113 bytes | **77.2%** | Menghapus file bersih dan petunjuk; hanya menyimpan file yang dimodifikasi/tidak terlacak. |
| `kubectl get pods` | 840 bytes | 762 bytes | **10.0%** | Secara selektif memunculkan pod CrashLoopBackOff/Error, melewati pod yang sehat. |
| `git diff` (multi-file) | 397 bytes | 220 bytes | **50.0%** | Mempertahankan hunk dengan perubahan, membuang baris konteks yang berlebihan. |

- **Latensi Pipeline**: **< 100ms** (end-to-end, termasuk startup binary)
- **Penghematan Sepanjang Waktu**: **97.3%** pengurangan token di seluruh sesi pengembangan rata-rata.
- **ROI**: **$35+ USD** dihemat per pengembang/bulan (diukur terhadap model unggulan).

*Untuk melihat penghematan token aktual Anda sendiri, jalankan saja `omni stats` setelah beberapa hari penggunaan.*

---

## Penjelasan Fitur

### 🧠 Core Distillation Engine (Mesin Distilasi Inti)
- **Tidak Ada Lagi Kebingungan AI**: Omni bertindak seperti saringan pintar. Jika tes gagal, ia hanya menunjukkan baris kesalahan dan stack trace, memblokir log dependensi yang bising.
- **Pengurangan Token 90%**: Dengan menghilangkan noise terminal, Anda memotong tagihan API agen secara drastis.
- **Kompresi Adaptif**: OMNI melacak kapan agen mengambil output yang dihilangkan dan secara otomatis melunakkan kompresi pada waktu berikutnya — menyetel sendiri secara otomatis.
- **Smart High-Speed Bypass**: Untuk menjamin latensi nol pada tugas kecil, OMNI secara otomatis melewati distilasi untuk output di bawah ambang 2000-token.

### 🛡️ Context Safety & Factual Guards (Keamanan Konteks)
- **Nol Kehilangan Informasi**: Output mentah disimpan secara lokal (`RewindStore`). AI dapat memintanya secara otomatis menggunakan `omni_retrieve`.
- **Penjaga Anti-Halusinasi Faktual**: OMNI menyuntikkan peringatan sistem (seperti file dengan dependensi masif) untuk menjaga AI Anda tetap berpijak pada fakta.
- **Visibilitas Penghilangan**: OMNI melabeli konten yang dihapus secara eksplisit (mis. `[OMNI: omitted X lines of noise]`), memberi agen kesadaran situasional.

### 🤝 Multi-Agent & Workspace Intelligence (Kecerdasan Ruang Kerja)
- **Kolaborasi Multi-Agen**: Jika Anda menjalankan Cursor bersama Claude CLI, mereka berbagi aliran memori terfilter yang sama tanpa bentrok.
- **Kecerdasan Sesi**: OMNI mengingat file yang sedang Anda edit dan berhenti memberikan konteks berulang.
- **Structured ReadFile + Grep**: Alih-alih dump file mentah, OMNI mengembalikan kerangka terstruktur (impor, API) dan ringkasan grep yang dikelompokkan.
- **Grafik Dependensi Ringan**: OMNI membangun grafik relasi file lokal yang cepat. Jika AI membaca file penting, OMNI memperingatkan tentang peta dampaknya.

### 📊 Monitoring & Debugging (Pemantauan)
- **Monitor Distilasi**: Lacak penghematan token menggunakan `omni_budget` di dalam LLM, atau jalankan `omni stats` secara lokal.
- **Dampak Visual (`omni diff`)**: Jalankan `omni diff` untuk membandingkan output mentah dengan versi Omni yang ramping secara berdampingan.
- **Debug Passthrough**: Setel `OMNI_PASSTHROUGH=1` untuk melewati mesin sepenuhnya dan melihat output asli.

---

## Di Balik Layar: Cara Kerja Omni

OMNI lebih dari sekadar skrip regex; ia adalah **Semantic Signal Engine** berkinerja tinggi yang ditulis dengan Rust. Namun bagaimana cara kerjanya memotong 90% konsumsi token dalam waktu kurang dari 100ms?

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
