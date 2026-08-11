<div align="center">
  <img src="../media/logo.png" alt="OMNI Logo" width="300" />

<h1>OMNI</h1>
<p align="center">
    <em><b>Agen Anda membaca setiap baris yang dicetak terminal, lalu membaca sebagian besarnya lagi di giliran berikutnya.</b> OMNI membuang noise-nya sebelum model melihat, dan mengembalikan sebuah rujukan untuk baris yang sudah pernah ditunjukkan. Tidak ada yang dihapus, dan ia tidak pernah mengarang hasil.</em>
</p>

[🇺🇸 English](../README.md) | [🇯🇵 日本語](README-ja.md) | [🇨🇳 简体中文](README-zh.md) | [🇸🇦 العربية](README-ar.md) | [🇮🇩 Bahasa Indonesia](README-id.md) | [🇻🇳 Tiếng Việt](README-vi.md) | [🇰🇷 한국어](README-ko.md)

[![CI](https://github.com/fajarhide/omni/actions/workflows/ci.yml/badge.svg)](https://github.com/fajarhide/omni/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/fajarhide/omni)](https://github.com/fajarhide/omni/releases)
  [![Rust](https://img.shields.io/badge/built_with-Rust-dca282.svg)](https://www.rust-lang.org/)
  [![MCP](https://img.shields.io/badge/MCP-compatible-green.svg?style=flat-square)](https://modelcontextprotocol.io/)
  [![License: MIT](https://img.shields.io/github/license/fajarhide/omni)](https://github.com/fajarhide/omni/blob/main/LICENSE)
  [![Hits](https://hits.sh/github.com/fajarhide/omni.svg)](https://hits.sh/github.com/fajarhide/omni/)
</br></br>

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

## Apa yang ia lakukan

**Membuang noise.** Log build, hash layer Docker, progress bar, warna ANSI. Bagian
output yang tidak dibaca siapa pun disingkirkan sebelum sampai ke model.

**Berhenti mengirim ulang apa yang sudah dilihat agen.** Deretan baris yang sudah
ditunjukkan sebelumnya di sesi yang sama kembali sebagai satu penanda dengan handle,
bukan sebagai byte-nya lagi. Ini bagian yang tidak bisa dilakukan filter: ia membuang
byte karena baris itu sudah ada di konteks, bukan karena ada pola yang menyebutnya noise.

**Mengingat lintas sesi.** Restart editor atau pindah agen, konteks proyeknya masih ada.

**Menyingkir.** Perintah yang gagal diteruskan apa adanya. JSON, YAML dan CSV tidak
pernah disentuh. Sebagian besar perintah dikembalikan tanpa diubah, dan itu memang
perilaku yang dituju, bukan kekurangan.

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

Empat jaminan, masing-masing tertaut ke kode atau issue yang membuatnya benar,
bukan kalimat yang meminta Anda percaya.

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

* Output build dan test: **87,8%**. Baca ulang berkas, kelas terbesar: **0,0%** dari
  filter dan **24,7%** dari ledger, dan celah itulah alasan ledger ada.
* **97,1% panggilan tidak menghemat apa pun**, dan kami menerbitkannya karena angka
  itulah yang memberi tahu Anda seberapa berarti sisanya. **Tidak ada panggilan yang
  justru membesar** pada pengukuran ini. Dulu ada 2 sampai ([#398](https://github.com/fajarhide/omni/issues/398)), dan kami
  menerbitkannya selama keduanya masih ada.
* **21 ms per perintah**, tumbuh bersama riwayat Anda dan bukan bersama ukuran
  payload. Pada database 205 MB angkanya 61 ms.

<div align="center">
<img src="https://omni.weekndlabs.com/media/performance.png" alt="OMNI" width="600" />
</div>

Korpus lengkap, rincian per kelas, fixture dan tabel latensi:
**[docs/BENCHMARKS.md](../docs/BENCHMARKS.md)**. Reproduksi dengan
`cargo test --release --test bench_replay -- --ignored`.

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
Filter hanya dibaca dari direktori home Anda. Direktori `.omni/signals/` di dalam sebuah repositori sengaja diabaikan: sebuah filter bisa menyembunyikan baris, jadi filter yang ikut sebuah checkout dapat diam-diam mengubah apa yang dilihat agent seorang pengunjung.

**Bagaimana saya melihat penghematan saya sendiri?**
`omni stats` setelah beberapa hari. `omni stats --share` mencetak ringkasan angka yang
sama, siap disalin.
`omni stats` membuka laporannya dengan umur sesi, yaitu berapa banyak perintah yang dibawa sebuah sesi sebelum host menutupnya, karena itulah yang benar-benar dibayar oleh jendela konteks. Persentase distilasi di bawahnya adalah diagnostik untuk pipeline satu host, bukan klaim produk.

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
