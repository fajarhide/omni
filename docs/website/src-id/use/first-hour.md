# Satu jam pertama

Diasumsikan `omni init` dan `omni doctor` sudah dijalankan. Tidak ada di sini
yang mengubah konfigurasi; semuanya soal belajar membaca apa yang sedang
dikerjakan OMNI supaya Anda bisa menilainya.

## Lihat satu penyulingan terjadi

Minta agent Anda menjalankan sesuatu yang berisik. Test atau build paling pas.

Lalu, di shell Anda sendiri:

```sh
omni diff
```

Mentah di satu sisi, hasil sulingan di sisi lain, untuk perintah terakhir. Ini
cara tercepat menumbuhkan kepercayaan atau kecurigaan, dan keduanya berguna.

## Coba satu dengan tangan

```sh
omni exec cargo test
```

`omni exec` menjalankan sebuah perintah lewat seluruh pipeline lalu mencetak
hasilnya dengan catatan kaki. Ia harness yang diminta dipakai setiap laporan bug
di proyek ini, karena ia mengeluarkan host dari gambar.

Bentuk argumennya persis: `omni exec cargo test`, **bukan**
`omni exec -- cargo test` dan bukan string yang dikutip. Keduanya gagal dengan
"No such file or directory".

## Lihat angkanya

```sh
omni stats
```

Ia memimpin dengan umur sesi, berapa perintah yang dibawa sebuah sesi sebelum
host menutupnya, karena itulah yang sebenarnya dibebankan jendela konteks kepada
Anda. Persentase penyulingan di bawahnya adalah alat diagnosis untuk pipeline
satu host.

```sh
omni stats --detail        # per perintah, per rute, per sesi, per agent
omni stats --rerun         # distiller mana yang berongkos satu run ulang
omni dashboard             # angka yang sama di peramban, hanya di 127.0.0.1
```

`--rerun` yang menarik. Persentase pengurangan tidak bisa memberi tahu apakah
sebuah distiller membuang sesuatu yang lalu harus diambil ulang agent; yang ini
bisa.

## Pancang apa yang sedang Anda kerjakan

```sh
omni goal set 'Migrate the billing service off the legacy queue'
```

Penilainya mengutamakan keluaran yang berkaitan dengan tujuan itu, dan agent
diingatkan padanya alih-alih melantur. `omni goal show` untuk memeriksa,
`omni goal clear` untuk melepasnya.

## Matikan untuk satu perintah

```sh
OMNI_PASSTHROUGH=1 kubectl get pods -o yaml
```

Hal pertama yang harus diraih ketika Anda curiga OMNI mengubah sesuatu yang
seharusnya tidak ia ubah. Kalau keluarannya identik dengan dan tanpa variabel
itu, OMNI tidak terlibat.

## Hal yang layak diketahui sebelum ia menggigit

**Membaca berkas lewat shell Anda bisa tiba dalam bentuk sulingan.** Karena
hook-nya memang benar-benar menulis ulang keluaran Bash, `cat` atau `sed` atas
sebuah berkas sumber bisa kembali terlipat. Pakai perkakas pembaca berkas milik
agent Anda, atau `OMNI_PASSTHROUGH=1`, ketika Anda butuh byte yang persis.

**Perintah yang cocok bisa ditulis ulang sebelum ia berjalan.** Pre-hook mengubah
sebagian perintah menjadi `omni exec`, termasuk pengalihan keluarannya, sehingga
berkas log yang kemudian Anda baca adalah yang sudah disuling. Patahkan awalannya
(`env cargo test`, atau `true && cargo test`) ketika Anda butuh log mentah di
disk.

**Jangan menilai OMNI dari keluaran yang Anda baca lewat OMNI.** Sebuah
`cargo test` yang dibaca lewat hook pernah melaporkan "1 failed" untuk suite yang
hijau dengan 398 lulus. Alihkan ke sebuah berkas dengan passthrough menyala
sebelum membuat klaim apa pun tentang sebuah hasil.

## Kapan minta bantuan

Kalau keluaran terlihat lebih pendek daripada seharusnya, kalau ada baris yang
hilang, atau kalau OMNI melaporkan sukses untuk sesuatu yang gagal, itu layak
dilaporkan. Reproduksi dulu dengan `omni exec`, dan baca **seluruh** keluaran
sulingannya, bukan hasil `grep` atasnya: mem-`grep` menyembunyikan judul yang
sering justru membuat keluaran itu ternyata tidak kehilangan apa pun.
