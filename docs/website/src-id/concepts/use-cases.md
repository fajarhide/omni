# Di mana OMNI membantu

Enam situasi, masing-masing dengan angka terukurnya. Dua di antaranya kasus
ketika OMNI tidak melakukan apa-apa, dan keduanya sengaja dimuat di sini:
perkakas yang mengaku membantu di mana-mana adalah perkakas yang tidak bisa
ditebak siapa pun.

Setiap angka berasal dari pemutaran ulang 6.656 perintah nyata yang sama seperti
yang diuraikan di
[Benchmarks](https://omni.weekndlabs.com/docs/develop/benchmarks), jadi semuanya
rata-rata atas campuran perintah yang nyata, bukan hari bagus yang dipetik dari
sebuah log.

## 1. Agent membaca ulang berkas yang sama terus-menerus

**Situasinya.** Anda minta refactor. Agent membaca `auth.rs`, melipir memeriksa
pemanggilnya, kembali lalu membaca `auth.rs` lagi. Enam giliran kemudian ia
membacanya untuk ketiga kali. Setiap bacaan ditagih penuh, dan tidak satu pun
pengulangan itu memberi tahu sesuatu yang belum diberi tahu bacaan pertama.

**Yang OMNI lakukan.** Bacaan kedua kembali sebagai penanda dengan handle.
Baris-barisnya sudah ada di konteks agent; mengirimnya lagi berarti membayar dua
kali untuk satu fakta.

**Angkanya: 25,0%** lebih hemat pada pembacaan berkas di seluruh korpus, dan
sampai **97,2%** pada satu berkas yang dibaca berulang.

Ini kemenangan tunggal terbesar di seluruh produk dan ia tidak terlihat selagi
bekerja, dan itulah alasan penandanya ada.

## 2. Test gagal dan Anda tidak bisa melihat kenapa

**Situasinya.** 412 test, satu gagal, dan kegagalannya ada di baris 388 keluaran.
Agent Anda membaca semua 412 baris untuk menemukannya, dan kalau run-nya cukup
panjang host memotong ekornya, yang justru tempat vonis itu tinggal.

**Yang OMNI lakukan.** Distiller test menyimpan hitungannya dan setiap kegagalan
lengkap dengan assertion serta posisi berkasnya, lalu membuang baris yang lulus.

**Angkanya: 78,0%** lebih hemat pada keluaran build dan test.

Ini kasus ketika yang bekerja adalah penyaringan, bukan ledger. Keluaran test
sangat berulang di dalam satu run, jadi ada basa-basi sungguhan untuk dibuang
bahkan sebelum ada yang terlihat dua kali.

## 3. `git log` dan `git diff` memenuhi layar

**Situasinya.** Satu commit dengan `Author`, `Date` dan badan yang dilipat itu
lima baris. Lima belas commit jadi satu setengah layar, padahal agent Anda cuma
mau subjeknya.

**Yang OMNI lakukan.** Setiap commit disimpan, sebagai satu baris
`hash subject`. Tidak ada yang diringkas hilang dan tidak ada commit yang lenyap;
yang pergi amplop di sekeliling masing-masing.

**Angkanya: 22,1%** pada `git` dan `gh` di korpus, dan **94%** khusus pada
`git log -15` yang bertele-tele.

## 4. Sesi Anda mati di batas konteks, berulang kali

**Situasinya.** Sesi debugging panjang, dan sekitar dua jam berjalan percakapan
dipadatkan. Agent kehilangan alur, membaca ulang berkas yang sudah ia pahami, dan
Anda menjelaskan ulang tugasnya.

**Yang OMNI lakukan.** Dua hal. Konteks yang terpakai per perintah lebih sedikit
berarti temboknya datang belakangan. Dan [ingatan antar sesi](../use/memory.md)
selamat dari pemadatan: pengetahuan proyek, pola galat yang berulang, dan tujuan
yang Anda pancang dengan `omni goal` ada di SQLite, bukan di jendela konteks.

**Batas jujurnya.** OMNI tidak bisa mencegah pemadatan, dan pada saat pemadatan
terjadi ia sengaja melupakan apa yang sudah ia tunjukkan, karena izin untuk
mengganti baris dengan handle adalah bahwa agent masih memegang baris-baris itu,
dan pemadatan adalah saat hal itu berhenti benar.

## 5. Anda pindah agent, atau pindah mesin, di tengah proyek

**Situasinya.** Anda mulai di Claude Code, pindah ke Codex CLI untuk satu
perubahan, dan keduanya mulai dari nol.

**Yang OMNI lakukan.** Penyimpanannya satu berkas SQLite yang dikunci pada jalur
proyek, bukan pada agent. Agent kedua yang bekerja di direktori yang sama membaca
pengetahuan proyek yang sama, dan cakupan proyek pada ledger akan memberinya
handle untuk keluaran yang sudah dihasilkan sesi sebelumnya. Penanda itu berbunyi
`not shown here`, bukan `already shown`, karena agent ini memang belum
pernah melihat byte tersebut dan kalimatnya harus benar.

**Angka jujurnya.** Pengulangan lintas sesi adalah **3,7%** dari byte setelah
penyaringan, berbanding **19,1%** di dalam satu sesi, jadi nilainya sekitar
seperlima penghematan dalam sesi. Ia nyata, dan ia bukan judulnya.

**Peringatan jujurnya.** Dua agent dalam satu repositori berbagi riwayat itu
sebagai efek samping, bukan karena dirancang begitu. Penandanya dulu berbunyi
`from an earlier session`, yang terbaca sebagai sesi *Anda* padahal itu sesi orang
lain, dan lebih buruk lagi sebagai klaim bahwa isinya sudah sampai; sekarang ia
berbunyi `not shown here`. [Ledger](the-ledger.md#apa-yang-dibagi-dua-agent-dalam-satu-repo) berterus
terang soal apa yang hari ini dikunci pada agent dan apa yang tidak.

## 6. `kubectl get pods -o json | jq`

**Situasinya.** Anda menyalurkan keluaran terstruktur ke sesuatu yang menguraikan
keluaran itu.

**Yang OMNI lakukan: tidak ada.** JSON, YAML, NDJSON, CSV dan TSV lewat persis
byte demi byte. Kompresor yang mengubah format muatan yang sebentar lagi diurai
perintah berikutnya tidak menghemat apa pun untuk Anda, ia merusak pipeline Anda.

**Angkanya: 0%**, memang dirancang begitu. Lihat
[Yang tidak pernah disentuh](format-safety.md).

## Dan satu lagi yang juga tidak terjadi apa-apa

`kubectl get pods` dengan 35 pod mengembalikan tabel yang setiap barisnya sebuah
fakta. Tidak ada basa-basi untuk dibuang dan belum ada yang pernah dilihat, jadi
OMNI mengembalikan seluruh 35 baris dan melaporkan penghematan 0%.

**97,3% dari semua panggilan di korpus seperti ini.** Itu angka yang layak
diresapi: OMNI bukan barang yang mengecilkan segalanya sedikit, ia barang yang
tidak berbuat apa-apa hampir sepanjang waktu lalu berbuat banyak sesekali. Angka
gabungan 14,9% adalah sisa setelah setiap nol tadi ikut dihitung.

## Totalnya jadi apa

| Kelas perintah | Panggilan di korpus | Hemat |
|---|---|---|
| build dan test | 69 | 78,0% |
| pembacaan berkas | 699 | 25,0% |
| `git`, `gh` | 661 | 22,1% |
| pencarian (`grep`, `rg`, `find`) | 828 | 13,3% |
| infra (`kubectl`, `az`, `docker`) | 254 | 8,2% |
| selebihnya | 4.145 | 6,9% |
| **semuanya** | **6.656** | **14,9%** |

Jalankan `omni stats` setelah beberapa hari dan Anda mendapat tabel ini untuk
riwayat Anda sendiri, satu-satunya versi yang menggambarkan pekerjaan Anda.
