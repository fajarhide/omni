# OMNI

**Agent AI Anda membayar untuk membaca keluaran yang sama berulang kali. OMNI
menghentikan itu.**

OMNI adalah satu program kecil yang duduk di antara terminal Anda dan agent
Anda. Jalannya lokal, tidak perlu API key, dan setelah terpasang Anda tidak
pernah mengetik namanya lagi.

```bash
brew install fajarhide/tap/omni && omni init
```

Di dalam Claude Code cukup dua baris, sisanya diurus agent:

```
/plugin marketplace add fajarhide/omni
/plugin install omni@omni
```

## Masalahnya, dalam satu layar

Agent Anda menjalankan test. Empat ratus baris kembali, yang penting satu.

```
$ cargo test
    Compiling omni v0.7.4
     Running unittests src/lib.rs
running 412 tests
test pipeline::scorer::tests::scores_errors_critical ... ok
... 409 baris "ok" lagi ...
test result: FAILED. 411 passed; 1 failed
```

Yang dibaca agent Anda adalah ini:

```
cargo test: 411 passed, 1 failed
  FAILED ledger::tests::renders_identical_bytes_for_identical_state
  assertion `left == right` failed at src/ledger/mod.rs:601
[OMNI: 406 lines omitted, omni retrieve 3f7bfd89bc5d7cee for full output]
```

Kegagalannya selamat. 406 baris `ok` tidak. Dan handle di baris terakhir itu
mengembalikan semuanya, persis byte demi byte, kalau suatu saat memang
diperlukan.

## Bagian yang tidak dikerjakan orang lain

Menyaring keluaran yang tidak dibaca siapa pun adalah bagian yang mudah, dan
beberapa perkakas sudah melakukannya. Bagian berikut lebih sulit, dan dari
sanalah sebagian besar penghematan OMNI datang.

Agent Anda membaca sebuah berkas. Tiga giliran kemudian ia membaca berkas yang
sama lagi, karena tidak ada yang mengingat bacaan pertama. Anda membayar penuh
dua-duanya.

OMNI ingat. Bacaan kedua kembali sebagai satu baris:

```
[OMNI: 178 lines already shown, omni retrieve 77a0c474f2e55351]
```

**Berkas 7,6 KB yang dibaca dua kali berharga 7,6 KB lalu 214 byte. Itu 97,2%
lebih murah untuk bacaan kedua.** Tidak ada yang dihapus: baris-baris itu sudah
ada di konteks agent Anda sejak bacaan pertama, jadi mengirimnya lagi tidak
membeli apa pun. Handle-nya disediakan kalau baris-baris itu sampai tergeser
keluar.

Ini namanya ledger, dan pada riwayat perintah sungguhan ia bekerja lebih banyak
daripada semua penyaring digabung.

## Yang Anda dapat

| | |
|---|---|
| **Sesi lebih panjang** | Konteks yang tidak habis untuk basa-basi berarti lebih banyak giliran sebelum mentok, dan lebih sedikit pemadatan yang memutus alur kerja Anda. |
| **Tagihan lebih kecil** | 14,9% lebih sedikit byte pada 6.656 perintah nyata. Pada pembacaan berkas 25,0%. Pada `git` 22,1%. Pada keluaran build dan test 78,0%. |
| **Tidak ada yang hilang** | Semua yang dibuang diarsipkan secara lokal. `omni retrieve <handle>` mencetaknya kembali. |
| **Tidak ada yang dikarang** | Kalau OMNI tidak paham sebuah keluaran, keluaran itu dikembalikan apa adanya, bukan ditebak. |
| **Ingatan antar sesi** | Tutup editor, kembali besok, pindah dari Claude Code ke Codex: konteks proyeknya masih ada. |
| **Tidak ada yang perlu diubah** | Tanpa proxy, tanpa API key, tanpa perintah yang harus diawali apa pun. Pasang, lalu pakai terminal Anda seperti biasa. |

## Di mana ia benar-benar membantu

[Di mana OMNI membantu](concepts/use-cases.md) membahas enam situasi lengkap
dengan angkanya, termasuk dua situasi ketika OMNI tidak melakukan apa-apa dan
kenapa itu memang benar.

## Mulai dari mana

**Cuma ingin ia jalan.** [Pasang](use/install.md) makan waktu sekitar lima
menit. Setelah itu baca [Membaca penanda](use/markers.md), satu-satunya halaman
yang sepadan dengan waktu Anda, karena penanda adalah cara OMNI memberi tahu apa
yang sudah ia lakukan.

**Ingin paham dulu.** [Apa itu OMNI](concepts/what-it-is.md), lalu
[Ledger](concepts/the-ledger.md).

**Ingin ikut mengerjakannya.**
[Architecture](https://omni.weekndlabs.com/docs/develop/architecture) dan
[The pipeline, stage by stage](https://omni.weekndlabs.com/docs/develop/pipeline)
adalah petanya. Keduanya berbahasa Inggris.

## Tiga hal yang tidak akan ia lakukan

**Tidak mengirim apa pun ke mana pun.** Setiap tahap berjalan di mesin Anda dan
arsipnya sebuah berkas SQLite di direktori home Anda.

**Tidak berdiri di antara Anda dan model Anda.** Tidak ada proxy dan tidak ada
API key yang diserahkan ke proses lokal. Itu
[diputuskan untuk tidak dilakukan](https://omni.weekndlabs.com/docs/develop/direction#non-goals),
dan alasannya ditulis.

**Tidak menebak diam-diam.** Tahap yang gagal memahami masukannya mengembalikan
masukan itu tanpa diubah. Data terstruktur seperti JSON dan YAML sama sekali
tidak disentuh. Apa pun yang dibuang meninggalkan penanda. Ketiga aturan itu
mengalahkan kompresi, dalam urutan itu, setiap kali bertabrakan.

## Versi jujur dari angka-angkanya

Dari 6.656 perintah nyata, **97,3% panggilan tidak menghemat apa-apa**, karena
memang tidak ada yang bisa dihemat. `git status` dua baris tidak punya basa-basi
untuk dibuang dan tidak punya pengulangan untuk dilipat, jadi OMNI
mengembalikannya langsung alih-alih mengarang penghematan untuk dilaporkan.

Angka 14,9% itu adalah sisa setelah semua nol tadi ikut dihitung. Itu rata-rata
nyata atas campuran perintah yang nyata, bukan kasus terbaik yang dipetik dari
hari yang bagus.

Kami juga menerbitkan perbandingan yang kami kalah: pada penyaringan saja, rtk
dapat 6,2% di korpus itu dan OMNI dapat 2,7%. Yang membuat OMNI unggul secara
keseluruhan adalah ledger, dan menjalankan penyaring rtk dengan ledger OMNI akan
mengalahkan keduanya.
[Benchmarks](https://omni.weekndlabs.com/docs/develop/benchmarks) memuat
metodenya dan perintah untuk mereproduksi setiap baris di riwayat Anda sendiri.

Kalau Anda ingin angka yang menggambarkan mesin Anda, bukan mesin orang lain,
jalankan `omni stats` setelah beberapa hari.

## Bertanya di mana

[Discord](https://discord.gg/zHTuvZhF2M) untuk pertanyaan, terutama untuk kasus
yang paling dipedulikan proyek ini: OMNI menyatakan hasil yang tidak didukung
masukannya. [Issue tracker](https://github.com/fajarhide/omni/issues) juga bisa.
Laporan yang memuat keluaran mentah dan keluaran hasil olahan berdampingan akan
diperbaiki lewat jalur mana pun.
