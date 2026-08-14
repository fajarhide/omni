# Berapa ongkosnya

Bukan nol. Ini seluruh tagihannya.

## Latensi

Median dari 12 run masing-masing, binary rilis, diukur ujung ke ujung lewat
post-hook:

| | basis data baru | basis data 205 MB |
|---|---|---|
| `git status` (496 B) | **21,1 ms** | **60,7 ms** |
| `cargo test` (16,5 KB) | **24,5 ms** | **64,5 ms** |

Ukuran muatan hampir tidak berpengaruh. Ukuran basis data berpengaruh, dan itu
angka yang perlu diawasi seiring arsip Anda tumbuh.

Penyulingannya sendiri berdurasi milidetik satu digit. Nyaris seluruh sisanya
adalah penulisan arsip. Rilis-rilis sebelumnya terukur 82 ms dan 276 ms di mesin
yang sama, dan bedanya tiga perbaikan, bukan perangkat keras yang lebih cepat:
tokenizer yang dimuat per perintah untuk satu kolom laporan, 249 regex penyaring
baris yang dikompilasi entah penyaringnya cocok atau tidak, dan kolam koneksi
yang membuka empat handle SQLite dalam proses yang selesai setelah satu muatan.

> Ukur latensi dengan cara mencabut, bukan dengan pewaktu di unit test. Sebuah
> microbenchmark di suite melaporkan 66 ms untuk pekerjaan yang oleh A/B pada
> binary rilis dihitung 34,3 ms. Hanya angka jenis kedua yang layak dikutip.

## Memori

Datar. Pipeline-nya bekerja pada aliran, jadi log 20.000 baris tidak memakan
memori residen lebih banyak daripada log pendek.

## Disk

Satu berkas SQLite di `~/.omni/omni.db`.

Isi yang diarsipkan dibatasi 64 KB per blok. Batas itu datang dari pengukuran:
mengarsipkan setiap penyulingan yang kehilangan data berongkos 83,1 MB selama 30
hari, dan batas tersebut menurunkannya ke 13,3 MB sambil tetap mencakup 3.604
dari 3.657 baris.

Jejak benchmark dipangkas pada hari ketujuh (`OMNI_TRACE_RETENTION_DAYS`).
Pemangkasan itu alasan tidak ada angka yang diterbitkan bisa diturunkan ulang
seminggu setelah ia diukur.

## Token

Bagian yang Anda cari, dan versi jujurnya punya dua paruh.

**Yang ia hemat.** Atas 6.656 perintah nyata pada 0.7.3: 14,9% lebih sedikit byte
di seluruh campuran. Per kelas, sebarannya sangat lebar:

| kelas | penyaring | dengan ledger |
|---|---|---|
| build dan test | 76,9% | 78,0% |
| pembacaan berkas | 0,0% | 25,0% |
| `git`, `gh` | 4,4% | 22,1% |
| pencarian | 4,8% | 13,3% |
| infra | 4,4% | 8,2% |
| selebihnya | 0,6% | 6,9% |

**Yang ia biayakan.** Setiap penanda adalah byte yang dibayar agent, dan 97,3%
panggilan tidak menghemat apa pun sambil tetap membayar latensi pipeline. Pada
keluaran pendek, penandanya bisa melampaui penghematannya.

Ada juga ongkos yang tidak bisa dinyatakan hitungan byte mana pun: satu
pengambilan. Ketika agent butuh isi di balik sebuah handle, ia membayar satu kali
bolak-balik yang tidak perlu ia bayar seandainya byte-nya tiba langsung.
Pelipatan bercakupan proyek memikul ambang keuntungan tiga kali lipat persis
karena alasan itu.

## Ongkos yang bukan tanggungan OMNI

Pada paket berlangganan tetap, kompresi sama sekali tidak mengurangi tagihan.
Yang ia beli adalah umur sesi dan lebih sedikit run ulang. Pembacaan prompt cache
ditagih sekitar sepersepuluh masukan baru, jadi byte yang dihemat sekali bukan
uang yang dihemat per giliran.

Itu sebabnya ukuran utama proyek ini sendiri adalah tekanan jendela konteks untuk
pekerjaan yang sama, dan persentase pengurangan adalah alat diagnosis, bukan
judul. Lihat
[Where OMNI is going](https://omni.weekndlabs.com/docs/develop/direction).

## Kalau ia panik

Ia gagal terbuka. Keluaran mentahnya lewat dan agent Anda tidak pernah melihat
galat. Setiap hook berjalan di dalam `catch_unwind`, dan basis data yang tidak
mau dibuka berongkos konteks sesi, bukan seluruh pipeline.
