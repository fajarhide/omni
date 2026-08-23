# `omni stats`

Analitik penghematan token, dibaca dari basis data Anda sendiri.

```sh
omni stats
```

Satu layar, dan menjawab satu pertanyaan: berapa byte yang tidak pernah sampai ke
model. Umur sesi, perintah teratas, rute dan agent pindah ke `--view detail`.

## Flag

| flag | efeknya |
|---|---|
| `--since <jendela>` | `hour`, `today`, `week`, `month` (bawaan), `all` |
| `--view <nama>` | `summary` (bawaan), `detail`, `commands`, `projects`, `context`, `rerun`, `share` |
| `--limit <n>` | Jumlah baris di tampilan tabel, bawaan 10, `0` untuk semua |
| `--json` | Bisa dibaca mesin, ikut jendela `--since` |
| `--card` | Tulis ringkasan sebagai gambar, berukuran untuk unggahan media sosial |
| `--help`, `-h` | Bantuan |

Semua ejaan lama tetap jalan: `--detail`, `--today`, `--day`, `-d`, `--week`, `-w`,
`--month`, `-m`, `--hour`, `-H`, `--all-commands`, `--project`, `--context`, `--rerun`
dan `--share`. Semuanya tidak didaftarkan di sini karena sekarang ada satu cara untuk
menyebut tiap hal, dan tidak ada peringatan usang yang dicetak: penggantian namanya
keputusan kami, bukan Anda.

`--json` dan `--card` itu format keluaran, bukan tampilan. `--card` mengalahkan semuanya,
karena menyebutnya hanya bisa berarti menulis berkasnya; `--json` mengalahkan `--view`,
karena laporan yang bisa dibaca mesin cuma satu dan bukan per tampilan. Dulu keduanya
dibaca sebagai tampilan, dan begitulah `--view detail --card` sampai tidak menulis gambar
sama sekali.

## `--rerun` yang wajib diketahui

Persentase pengurangan tidak bisa memberi tahu apakah sebuah distiller membuang
sesuatu yang lalu harus diambil ulang agent. Kalau iya, pengurangannya sebuah
penundaan, bukan penghematan. Flag ini pemeriksaan yang tidak bisa dilakukan
persentase.

## Jebakan

**Baris terminal bukan token.** Keluaran yang ditulis ke TTY dibaca manusia,
bukan model. Pada satu pemasangan, baris seperti itu 73% dari seluruh byte yang
diklaim OMNI sudah dihemat. `stats` sekarang mengecualikannya, begitu juga
harness benchmark-nya, tapi siapa pun yang mengueri `~/.omni/omni.db` langsung
harus menyaring `agent_id` sendiri.

**Angka tinggi pantas dicurigai.** Cacat terburuk di proyek ini melaporkan
pengurangan paling besar, karena menghapus jawabannya terkompresi dengan sangat
baik. Sandingkan angka mana pun dengan `omni diff` pada perintah sungguhan.

**Angka gabungan yang rendah biasanya benar, dan bukan itu ukuran menilai OMNI.**
Sebagian besar panggilan memang dikembalikan utuh, jadi baca baris per kelasnya untuk
melihat di mana kerjanya benar-benar terjadi.

**`--share` dan `--card` tidak mungkin berbeda dari laporannya.** Keduanya
membaca agregasi yang sama dengan `omni stats` sendiri, sebuah pilihan yang
disengaja setelah versi sebelumnya menghitungnya terpisah.
