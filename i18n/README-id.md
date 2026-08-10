<div align="center">
  <img src="../media/logo.png" alt="OMNI Logo" width="300" />

<h1>OMNI</h1>
<p align="center">
    <em><b>Berhenti membayar Claude untuk membaca 10.000 baris noise terminal.</b> Selama satu minggu kerja nyata seorang developer, OMNI memangkas 88% output build dan test serta seperempat dari semua yang dibaca ulang agen, 15,7% di seluruh campuran perintah. Sisanya, 97% panggilan, lewat tanpa disentuh. Tidak ada yang hilang, dan ia tidak pernah mengarang hasil.</em>
</p>

[🇺🇸 English](../README.md) | [🇯🇵 日本語](README-ja.md) | [🇨🇳 简体中文](README-zh.md) | [🇸🇦 العربية](README-ar.md) | [🇮🇩 Bahasa Indonesia](README-id.md) | [🇻🇳 Tiếng Việt](README-vi.md) | [🇰🇷 한국어](README-ko.md)

[![CI](https://github.com/fajarhide/omni/actions/workflows/ci.yml/badge.svg)](https://github.com/fajarhide/omni/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/fajarhide/omni)](https://github.com/fajarhide/omni/releases)
  [![Rust](https://img.shields.io/badge/built_with-Rust-dca282.svg)](https://www.rust-lang.org/)
  [![MCP](https://img.shields.io/badge/MCP-compatible-green.svg?style=flat-square)](https://modelcontextprotocol.io/)
  [![License: MIT](https://img.shields.io/github/license/fajarhide/omni)](https://github.com/fajarhide/omni/blob/main/LICENSE)
  [![Hits](https://hits.sh/github.com/fajarhide/omni.svg)](https://hits.sh/github.com/fajarhide/omni/)
</br></br>
<b>
build dan test 88% &middot; baca ulang berkas 25% &middot; 15,7% di seluruh campuran &middot; 21 ms per perintah &middot; 2 dari 7.095 panggilan memperbesar output, dan kami menyebutkannya &middot; setiap potongan bisa dipulihkan byte demi byte &middot; memori lintas sesi </b>

</br></br>

```bash
brew install fajarhide/tap/omni && omni init
```

Mendistilasi output perintah di Claude Code, Codex CLI, dan Gemini CLI, yaitu host yang menerapkan penulisan ulang dari OMNI. Di host lain kamu tetap dapat server MCP, state sesi bersama, dan `omni_run` yang mendistilasi perintah apa pun yang kamu lewatkan melaluinya. Jalankan `omni doctor` untuk melihat tier tiap host.


### Apa yang tiap host izinkan OMNI lakukan

| Tier | Host | Yang kamu dapat |
|---|---|---|
| **Full** | Claude Code, Codex CLI, Gemini CLI, Aider (pipe) | Host menerapkan penulisan ulang OMNI, jadi model membaca output terdistilasi dari tool bawaannya sendiri. |
| **Handoff-first** | Cursor, Windsurf | Host tidak bisa menulis ulang output tool bawaan. `omni_run` mendistilasi apa pun yang kamu lewatkan melaluinya, dan `omni init --cursor` memasang aturan yang membuat agent memilihnya. |
| **MCP-only** | Cline, Roo, OpenCode, VS Code, Zed, Copilot, Antigravity, Hermes, Pi | Memori, recall, dan state sesi. Tidak ada distilasi shell, dan tidak diklaim ada. |

`omni doctor` mencetak tier tiap host yang terpasang. Penghematan hanya dihitung ketika model benar-benar menerima lebih sedikit.

Codex CLI butuh satu langkah tambahan. Codex hanya menjalankan hook yang sudah dipercayainya, dan melewati sisanya tanpa bilang apa-apa. Jadi setelah `omni init --codex`, jalankan `codex` sekali lalu setujui di bagian "Hooks need review". `omni doctor` akan gagal sampai itu dilakukan. Lihat [#359](https://github.com/fajarhide/omni/issues/359).
</br>
<img src="../media/demo.gif" alt="OMNI menyaring cargo test yang bising sampai ke verdict-nya, lalu omni stats" width="820" />
</div>

---

Agen Anda membaca setiap baris yang dicetak terminal. Build log, Docker log, CI log,
progress bar, warna ANSI. Ribuan token untuk menemukan satu baris. Claude tidak
mahal. Terminal Andalah yang mahal.

Dan ia melupakan semuanya semalaman. Restart Cursor, pindah ke Claude Code, dan Anda
menjelaskan ulang proyeknya dari nol.

OMNI memperbaiki keduanya, dan menyingkir di tempat lain.

---

## Perbedaannya

**Masalah 1: terminal Anda menenggelamkan sinyalnya**

`git log` yang sama, berdampingan. Tanpa OMNI, `Author` / `Date` / body satu commit
saja sudah memenuhi layar. Dengan OMNI, **setiap commit tetap ada**, sebagai satu
baris `hash subject`, 94% lebih kecil. Tidak ada yang diringkas hilang; footer-nya
dihitung dari jumlah byte sungguhan, bukan dijanjikan.

<table>
<tr>
<td align="center"><b>Tanpa OMNI</b><br/><sub><code>git log -15</code> mentah</sub></td>
<td align="center"><b>Dengan OMNI</b><br/><sub>setiap commit tetap ada, 94% lebih kecil</sub></td>
</tr>
<tr>
<td valign="top"><img src="../media/demo-git-without.gif" alt="git log -15 mentah yang bertele-tele: Author, Date dan body satu commit memenuhi layar" width="400" /></td>
<td valign="top"><img src="../media/demo-git-with.gif" alt="git log -15 yang sama lewat OMNI: setiap commit jadi satu baris hash dan subject, 94% lebih kecil" width="400" /></td>
</tr>
</table>

Angka nyata, diukur pada `tests/fixtures/` dan trace yang diputar ulang, bukan harapan:

| Perintah | Tanpa OMNI | Dengan OMNI | Hemat |
|---|---|---|---|
| `cargo test` (490 lulus, 10 gagal) | 16,5 KB output per-test | ringkasan lulus/gagal dari runner-nya sendiri | **92,9%** |
| `git status` (kotor) | 496 B porcelain | branch dan path yang berubah | **61,7%** |
| `docker build` (noise cache berat) | 9,2 KB hash layer dan progress bar | hasil build, cache hit dilipat | **35,9%** |
| `git diff` (banyak berkas) | lockfile, spasi, perubahan hasil generate | kode yang benar-benar berubah | **25,2%** |
| `kubectl get pods` (35 pod, 5 crash) | tabel penuh | tabel penuh | **0%**, memang begitu |

Setiap angka di atas adalah payload yang **benar-benar dikirim**, termasuk penanda
pemulihan ~77 byte yang OMNI lampirkan setiap kali ia membuang sesuatu. Rilis
sebelumnya mengutip output distiller sebelum penanda itu, yang membuat payload kecil
terlihat lebih bagus: `git diff` terbaca 25,2% di sini dan 44,6% tanpanya. Penanda
itulah yang membuat potongannya bisa dikembalikan, jadi ia layak ikut dihitung.

Baris `kubectl get pods` yang menarik. Dulu ia melaporkan 9,3%; sekarang tidak
melaporkan apa-apa, karena tabel pod adalah enumerasi di mana setiap baris adalah
data dan tidak ada noise untuk dibuang. Kehilangan 9,3% itu justru perbaikannya.

> **Di mana ia sengaja tidak berbuat apa-apa.** Perintah yang gagal diteruskan apa adanya, karena error yang tersembunyi lebih mahal daripada error yang tidak dipampatkan. Output terstruktur (JSON, YAML, CSV) tidak pernah disentuh, karena langkah berikutnya di pipeline Anda akan mem-parse-nya. OMNI berguna pada celotehan tool yang berulang dan menyingkir di tempat lain, dan itulah yang membuatnya aman dibiarkan aktif untuk setiap perintah yang Anda jalankan.

### Tidak ada yang hilang. Ia tidak pernah mengarang.

Dua janji, dan keduanya ada di kodenya, bukan di paragraf ini.

**Tidak ada yang hilang.** Setiap byte yang OMNI potong diarsipkan secara lokal di RewindStore, dikunci dengan SHA-256. Agen menerima hash bersama output yang sudah disuling dan bisa memanggil `omni_retrieve` untuk menarik aslinya kembali byte demi byte, di tengah percakapan, tanpa menjalankan ulang perintah Anda.

**Ia tidak pernah mengarang.** Distiller yang tidak mengenali apa pun di inputnya mengembalikan input mentah. Itu tipe data, bukan konvensi: `distill` mengembalikan `Option<String>` dan lapisan routing jatuh kembali ke aslinya setiap kali menerima `None`. Tidak ada jalur kode yang menghasilkan baris hijau "no errors" yang tidak OMNI baca.

Kompresor lain meminta Anda *percaya* bahwa yang dipotong tidak penting. OMNI menyerahkan buktinya:

| Jaminan | Caranya | Bukti |
|---|---|---|
| **Aslinya bisa diambil lagi, byte demi byte** | semua yang dipotong diarsipkan di **RewindStore** SQLite lokal (SHA-256 ke konten); agen menerima hash dan memanggil `omni_retrieve` | [`Cara kerjanya`](#cara-kerjanya) |
| **Tidak pernah mengarang hasil** | distiller yang tidak berhasil mem-parse sinyal apa pun mengembalikan output mentah, bukan string hijau `no errors` atau `passed` | [#143](https://github.com/fajarhide/omni/issues/143) |
| **Kegagalan tidak pernah ditutupi** | perintah yang keluar dengan status bukan nol diteruskan apa adanya | [#120](https://github.com/fajarhide/omni/issues/120) |
| **Data terstruktur tidak pernah disentuh** | JSON / YAML / NDJSON / CSV lewat byte demi byte | `pipeline::format` |
| **Angkanya diukur, bukan diharapkan** | 7.095 trace nyata diputar ulang di biner rilis, dan 97,1% panggilan tidak menghemat apa pun, yang juga kami terbitkan | [`Tolok ukur`](#tolok-ukur) |

Itulah satu hal yang tidak bisa dibeli angka kompresi yang lebih besar: **aslinya selalu bisa Anda pulihkan, dan ia tidak akan pernah membohongi agen Anda.**

---

## Tolok Ukur

Diukur pada biner rilis dengan memutar ulang **7.095 eksekusi perintah nyata**
sepanjang **3 sampai 10 Agustus 2026 UTC**, semuanya output yang sampai ke model.
Jendela waktunya bagian dari angkanya: `execution_traces` dipangkas setelah tujuh
hari, jadi sebuah korpus lenyap seminggu setelah diukur.

* **Di tempat yang berisik, filternya mengambil hampir semuanya.** Output build dan
  test 87,9%, dan 92,3% setelah ledger sesi ikut dihitung. Di tempat yang tidak
  berisik mereka tidak mengambil apa pun, dan tabel `kubectl get pods` 0%, karena
  setiap barisnya adalah data.
* **Ledger menjangkau apa yang tidak bisa dijangkau penyaringan.** Baca ulang berkas
  adalah kelas terbesar dengan 1,54 MB, filternya mengambil 0,0% dari situ, dan
  mengembalikan baris yang sudah pernah ditunjukkan ke agen mengambil 24,6%.
* **97,1% panggilan tidak menghemat apa pun** dan menyerahkan outputnya apa adanya.
  Seluruh penghematan datang dari 2,9% sisanya.
* **2 panggilan dari 7.095 justru membesar**, kami laporkan alih-alih dibulatkan
  hilang ([#398](https://github.com/fajarhide/omni/issues/398)).
* **15,7% lebih sedikit byte** di seluruh campuran perintah, 5,2% di antaranya dari
  filter dan sisanya dari ledger. Dihitung dalam token, filternya saja 5,0%.
* **21 ms per perintah** dari ujung ke ujung, tumbuh bersama riwayat Anda dan bukan
  bersama ukuran payload. Pada database 205 MB angkanya 61 ms.

<div align="center">
<img src="https://omni.weekndlabs.com/media/performance.png" alt="OMNI" width="600" />
</div>

Korpus lengkap, rincian per kelas, fixture dan tabel latensi:
**[docs/BENCHMARKS.md](../docs/BENCHMARKS.md)**. Reproduksi dengan
`cargo test --release --test bench_replay -- --ignored`.

### Cara membaca angka penghematan, termasuk angka kami

Setiap tool di kategori ini menerbitkan satu persentase. Ini lima pertanyaan yang
menentukan apakah angka itu berarti, dan jawaban kami:

| Pertanyaan | Kenapa penting | OMNI |
|---|---|---|
| Berapa porsi panggilan yang **tidak** menghemat apa pun? | Tool yang menghemat pada setiap perintah sedang meringkas output yang Anda butuhkan | **97,1%**, kami terbitkan |
| Adakah panggilan yang membuat output **lebih besar**? | Penanda dan header memakan byte, dan tidak ada yang melaporkan yang jadi bumerang | **2 dari 7.095**, dan keduanya punya nomor issue |
| **Populasi** mana yang diukur? | Menghitung byte terminal yang tidak dibaca model menaikkan angka secara gratis | hanya yang sampai ke model, yang menghabiskan 36 poin terakhir kali sebuah korpus memuat baris terminal |
| Bisakah Anda **menjalankannya ulang**? | Angka yang tidak bisa direproduksi itu klaim, bukan pengukuran | satu perintah, pada data Anda sendiri |
| Apakah potongannya **bisa dipulihkan**? | Lossy tidak masalah kalau reversibel, dan fatal kalau tidak | byte demi byte, lewat `omni_retrieve` |

Kami menerbitkan porsi panggilan di mana kami tidak berbuat apa-apa, karena itulah
angka yang memberi tahu Anda seberapa berharga sisanya.

---

## Mulai Cepat & Instalasi

OMNI sangat mudah disiapkan. Ia terintegrasi secara native ke terminal Anda.

**macOS / Linux:**
```bash
# 1. Pasang lewat Homebrew
brew install fajarhide/tap/omni

# 2. Siapkan OMNI (menu interaktif untuk Claude, VS Code, OpenCode, Codex, Antigravity)
omni init

# 3. Pastikan berjalan
omni doctor

# 4. Atau perbaiki otomatis kalau ada masalah
omni doctor --fix

# 5. Cek status saat ini
omni init --status
```

**Installer universal (macOS / Linux / WSL):**
```bash 
curl -fsSL omni.weekndlabs.com/install | bash
```

**Windows (PowerShell):**
```powershell
irm omni.weekndlabs.com/install.ps1 | iex
```

---

---

## FAQ

**Apakah OMNI menghapus log saya secara permanen?**  
Tidak. Log mentahnya dipampatkan dan disimpan lokal di RewindStore SQLite. AI menerima sebuah hash dan bisa mengambil log lengkapnya kalau perlu.

**Apakah ini memperlambat terminal saya?**  
Ya, terukur, dan biayanya tumbuh bersama riwayat Anda. Pipeline distilasinya sendiri berjalan dalam milidetik satu digit, tapi setiap perintah yang dikaitkan juga menulis ke RewindStore lokal: `git status` 496 byte butuh ~21 ms pada database baru dan ~61 ms pada database 205 MB, dan `cargo test` 16,5 KB butuh ~25 ms. Perhitungkan itu. `OMNI_PASSTHROUGH=1` melewati pipeline sepenuhnya kalau Anda butuh output mentahnya kembali.

**Bisakah saya menambahkan filter sendiri?**  
Bisa. Anda bisa mengajari OMNI membuang noise khas tools internal Anda memakai TOML:
```toml
# ~/.omni/signals/custom.toml
[filters.my_tool]
match_command = "^internal-tool\\b"
strip_lines_matching = ["^DEBUG", "syncing..."]
```

**Bagaimana saya melihat penghematan saya sendiri?**
`omni stats` setelah beberapa hari. `omni stats --share` mencetak ringkasan angka yang
sama, siap disalin.

---

## Selengkapnya

* [Cara kerjanya, dan berapa biayanya](../docs/ARCHITECTURE.md): pipeline, RewindStore, Memory OS
* [Tolok ukur lengkap](../docs/BENCHMARKS.md): korpus, per kelas, fixture, latensi
* [Kontribusi](../CONTRIBUTING.md): jalankan `make ci` dan Anda sudah ikut

---

```bash
brew install fajarhide/tap/omni && omni init
```

## Kontribusi & Lisensi

Ini proyek yang lahir dari kesenangan, dibangun untuk era AI Agentik. Entah Anda datang untuk menghemat biaya token, mencoba model gratis, atau ikut membangun toolbelt agentik terbaik, kontribusi selalu diterima!

- **Pengembangan**: ingin membangun dari sumber? Jalankan `make ci` dan `cargo build`. Baca [CONTRIBUTING.md](../CONTRIBUTING.md) untuk detailnya.
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
<center>
Dibuat dengan ❤️ oleh <a href="https://github.com/fajarhide">Fajar Hidayat</a>
</center>
