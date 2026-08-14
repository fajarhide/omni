# Sisanya

Perintah yang cukup butuh satu paragraf ketimbang satu halaman.

## `update`

```sh
omni update
```

Mengambil rilis terbaru dari GitHub lalu memperbarui. Untuk saat ini hanya
pemasangan lewat Homebrew; cara pemasangan lain diperbarui lewat kanalnya
sendiri.

Jalankan ulang `omni init` sesudahnya kalau catatan rilisnya menyebut kontrak
hook berubah.

## `reset`

```sh
omni reset            # menu interaktif
omni reset --all      # semua integrasi, dan menawarkan menghapus omni.db
omni reset --claude   # satu host
```

Flag per host mencerminkan [`init`](init.md): `--claude`, `--cursor`, `--zed`,
`--cline`, `--roo` / `--roo-code`, `--copilot`, `--gemini`, `--opencode`,
`--codex`, `--antigravity`, `--hermes`, `--pi`.

`--all` satu-satunya yang menawarkan menghapus basis data Anda, dan ia bertanya
lebih dulu. Ia menyimpan cadangan konfigurasi yang ia cabut.

## `dashboard`

```sh
omni dashboard
omni dashboard --port 8080     # bawaannya 7717
```

Angka yang sama dengan yang dicetak `omni stats`, di peramban. Hanya baca,
membaca basis data yang sama, dan mengikat `127.0.0.1` dan tidak yang lain.
Ctrl-C menghentikannya.

## `diff`

```sh
omni diff
```

Keluaran perintah terakhir, mentah dibanding hasil sulingan. Cara tercepat
menumbuhkan kepercayaan pada apa yang dikerjakan OMNI, dan hal pertama yang
dijalankan ketika sebuah hasil terlihat salah.

## `query`

```sh
omni query errors in last 5 commands
omni query warnings from cargo
omni query context for src/main.rs
omni query timeline today
omni query timeline today --json
```

Bahasa kueri kecil yang tetap atas riwayat penyulingan, bukan teks bebas. Empat
bentuk didukung dan itulah keempatnya di atas. `--json` untuk keluaran yang bisa
dibaca mesin.

## `patterns`

```sh
omni patterns
omni patterns --tool cargo
```

Galat yang terus kembali lintas sesi. `--tool <nama>` membatasinya ke satu tool.

Berguna untuk pertanyaan "apakah saya pernah kena ini", pertanyaan yang tidak
bisa dijawab sendiri oleh sesi yang baru.

## `remember`

```sh
omni remember 'The staging database ignores migrations run outside the deploy job'
```

Menyimpan sebuah fakta di ingatan permanen, bisa diambil kembali lewat
`omni_recall` atau lewat suntikan konteks sesi.

Layak disimpan: sebuah keputusan dan alasannya, sebuah jebakan, sebuah batasan
yang tidak disebut berkas mana pun. Tidak layak disimpan: apa pun yang sudah
dicatat repositorinya.

## `engram`

```sh
omni engram
omni engram --json
```

Ringkasan subtugas yang selesai, ditulis seiring pekerjaan rampung.

## `goal`

```sh
omni goal set 'Migrate the billing service off the legacy queue'
omni goal show
omni goal clear
```

Memancang sebuah tujuan utama. Penilainya mengutamakan keluaran yang berkaitan
dengannya, dan agent diingatkan padanya alih-alih melantur sepanjang sesi yang
panjang. Ingatan tujuan menghormati `ttl_days` miliknya sendiri, bukan tingkat
retensi standar.

`set` adalah subperintah bawaannya, jadi `omni goal 'sebuah teks'` juga bekerja.

## `version`

```sh
omni version
omni version --json
```

Rincian versi dan lingkungan: tanggal build, hash git, dan jalur yang diputuskan
OMNI untuk konfigurasi dan basis datanya. Layak disertakan di laporan bug mana
pun.
