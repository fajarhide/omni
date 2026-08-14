# `omni stats`

Analitik penghematan token, dibaca dari basis data Anda sendiri.

```sh
omni stats
```

Memimpin dengan **umur sesi**, berapa perintah yang dibawa sebuah sesi sebelum
host menutupnya. Persentase penyulingan di bawahnya adalah alat diagnosis untuk
pipeline satu host, bukan klaim produk.

## Flag

| flag | efeknya |
|---|---|
| `--detail` | Rincian penuh: perintah, rute, sesi, agent |
| `--hour`, `-H` | Batasi ke 60 menit terakhir |
| `--day`, `--today`, `-d` | Hari ini saja |
| `--week`, `-w` | 7 hari terakhir |
| `--month`, `-m` | 30 hari terakhir, bawaannya |
| `--all-commands` | Semua perintah, bukan cuma yang teratas |
| `--project` | Pecah per jalur proyek |
| `--context` | Sinyal komposisi konteks |
| `--rerun` | Distiller mana yang berongkos satu run ulang |
| `--share` | Ringkasan siap tempel dari penghematan terukur Anda |
| `--card` | Tulis ringkasan itu sebagai gambar, berukuran untuk unggahan media sosial |
| `--json` | Bisa dibaca mesin |
| `--help`, `-h` | Bantuan |

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

**Angka rendah biasanya benar.** Sekitar 97% panggilan tidak menghemat apa pun
karena memang tidak ada yang bisa dihemat.

**`--share` dan `--card` tidak mungkin berbeda dari laporannya.** Keduanya
membaca agregasi yang sama dengan `omni stats` sendiri, sebuah pilihan yang
disengaja setelah versi sebelumnya menghitungnya terpisah.
