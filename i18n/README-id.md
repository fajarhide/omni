<div align="center">
  <img src="../media/logo.png" alt="OMNI Logo" width="300" />

<h1>OMNI</h1>
<p align="center">
    <em><b>Berhenti membayar Claude untuk membaca 10.000 baris noise terminal.</b> OMNI memangkas <code>git</code> 89%, <code>cargo</code> 91% dan <code>kubectl</code> 77% sebelum agen Anda sempat melihatnya. Selebihnya lewat tanpa disentuh. Tidak ada yang hilang, dan ia tidak pernah mengarang hasil.</em>
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
<code>git</code> 89% &middot; <code>cargo</code> 91% &middot; <code>kubectl</code> 77% &middot; 21 ms per perintah &middot; 0 dari 9.965 panggilan pernah memperbesar output &middot; setiap potongan bisa dipulihkan byte demi byte &middot; memori lintas sesi </b>

</br></br>

```bash
brew install fajarhide/tap/omni && omni init
```

Mendistilasi output perintah di Claude Code, Codex CLI, dan Gemini CLI, yaitu host yang menerapkan penulisan ulang dari OMNI. Di host lain kamu tetap dapat server MCP, state sesi bersama, dan `omni_run` yang mendistilasi perintah apa pun yang kamu lewatkan melaluinya. Jalankan `omni doctor` untuk melihat tier tiap host.

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
| **Angkanya diukur, bukan diharapkan** | 9.965 trace nyata diputar ulang di biner rilis, dan 90,0% panggilan tidak menghemat apa pun, yang juga kami terbitkan | [`Tolok ukur`](#tolok-ukur) |

Itulah satu hal yang tidak bisa dibeli angka kompresi yang lebih besar: **aslinya selalu bisa Anda pulihkan, dan ia tidak akan pernah membohongi agen Anda.**

---

## Tolok Ukur

Diukur pada biner rilis dengan memutar ulang **9.965 eksekusi perintah nyata** dari
penggunaan sehari-hari satu developer:

* **Pada perintah yang memang menghasilkan noise, 76 sampai 91%.** `cargo` 91,4%,
  `git` 89,2%, `kubectl` 76,5%. Di situlah anggaran konteks Anda habis, dan di situ
  pula OMNI bekerja.
* **OMNI bertindak pada 1 dari 10 perintah, dan menambahkan nol byte pada 9 sisanya.**
  Ia filter, bukan peringkas. Kalau tidak ada yang bisa dipotong ia menyingkir
  sepenuhnya.
* **Tidak satu pun dari 9.965 panggilan membuat outputnya lebih besar.**
* **43,3% lebih sedikit byte** di seluruh campuran perintah, yang bising dan yang
  tenang sekaligus.
* **21 ms per perintah** dari ujung ke ujung, tumbuh bersama riwayat Anda dan bukan
  bersama ukuran payload. Pada database 205 MB angkanya 61 ms.

<div align="center">
<img src="https://omni.weekndlabs.com/media/performance.png" alt="OMNI" width="600" />
</div>

Korpus lengkap, rincian per perintah, fixture dan tabel latensi:
**[docs/BENCHMARKS.md](../docs/BENCHMARKS.md)**. Reproduksi dengan
`cargo test --release --test bench_replay -- --ignored`.

### Cara membaca angka penghematan, termasuk angka kami

Setiap tool di kategori ini menerbitkan satu persentase. Ini lima pertanyaan yang
menentukan apakah angka itu berarti, dan jawaban kami:

| Pertanyaan | Kenapa penting | OMNI |
|---|---|---|
| Berapa porsi panggilan yang **tidak** menghemat apa pun? | Tool yang menghemat pada setiap perintah sedang meringkas output yang Anda butuhkan | **90,0%**, kami terbitkan |
| Adakah panggilan yang membuat output **lebih besar**? | Penanda dan header memakan byte, dan tidak ada yang melaporkan yang jadi bumerang | **0 dari 9.965** |
| **Populasi** mana yang diukur? | Menghitung byte terminal yang tidak dibaca model menaikkan angka secara gratis | hanya yang sampai ke model, dan mengakuinya membuat kami kehilangan 36 poin |
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
* [Tolok ukur lengkap](../docs/BENCHMARKS.md): korpus, per perintah, fixture, latensi
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
