# Membaca penanda

Penanda adalah cara OMNI memberi tahu apa yang ia lakukan. Bentuknya hanya
beberapa, dan mengenalinya adalah beda antara memercayai perkakas ini dan
mencurigainya.

## Bentuk-bentuknya

```
[OMNI: 406 lines omitted, omni retrieve 0000000000000000 for full output]
```

Ada isi yang dipotong dan diarsipkan. Enam belas karakter itu sebuah handle:
`omni retrieve <handle>` mencetak aslinya kembali, persis byte demi byte,
dari shell mana pun di sesi mana pun.

```
[OMNI: 40 lines already shown, omni retrieve 0000000000000000]
```

Ledger. Baris-baris ini sudah dikeluarkan sebelumnya **di sesi ini**, jadi
klaimnya adalah agent masih memegangnya dan handle-nya tidak berongkos kecuali ia
memang mau membaca ulang.

```
[OMNI: 40 lines not shown here, omni retrieve 0000000000000000]
```

Ledger juga, klaim berbeda. Baris-baris ini pergi ke **sesi lain** dari proyek
ini, dan agent ini belum pernah melihatnya. Kalimatnya sengaja bukan "already
shown", karena itu akan salah. Melipatnya adalah taruhan bahwa agent tidak akan
membutuhkannya, dan karena itu ia memikul ambang keuntungan tiga kali lipat.

Sesi lain itu bisa jadi juga agent yang lain. Riwayat proyek dikunci pada
direktorinya, jadi apa pun yang berjalan di repositori ini ikut menyumbang. Lihat
[apa yang dibagi dua agent](../concepts/the-ledger.md#apa-yang-dibagi-dua-agent-dalam-satu-repo).

```
[OMNI: identical to the 40 lines already shown, omni retrieve 0000000000000000]
[OMNI: identical to 40 lines from an earlier session, none shown here, omni retrieve 0000000000000000]
```

Dua klaim yang sama, untuk jawaban yang terulang **seluruhnya**. Ketika
pelipatannya mencakup setiap baris, penandanya adalah seluruh keluaran, bukan
sebuah lubang di dalamnya, jadi ia berbunyi `identical to` dan Anda mendapat satu
baris di tempat run ulang akan mencetak ratusan baris yang sama. Apa pun yang
kurang dari seluruh jawaban memakai kalimat di atasnya.

```
[OMNI: 40 lines already shown from charlie.tf, omni retrieve 0000000000000000]
```

Keempatnya bisa membawa `from <sumber>`, yang menyebut perintah yang keluarannya
pertama kali menampilkan baris-baris itu. Ia hanya muncul kalau perintah tersebut
**bukan** perintah yang baru saja Anda jalankan, yaitu kasus yang tidak bisa Anda
pastikan dari penandanya sendiri: membaca satu berkas lalu sebuah blok dilipat
karena berkas lain sudah menampilkannya lebih dulu. Tanpa klausa itu, membandingkan
dua berkas untuk memeriksa apakah blok bersamanya sama dijawab dengan menghapus
buktinya.

Membaca ulang berkas yang sama tidak membawa klausa apa pun, dan itu disengaja,
bukan kelalaian. Panjang penanda menentukan apa yang layak dilipat sama sekali,
jadi mencantumkan sumber di setiap penanda akan memotong penghematan pada kasus
yang umum demi menamai kasus yang jarang.

```
[N similar lines collapsed]
```

Collapse. Deretan baris yang nyaris identik, diganti sebuah hitungan.

```
[OMNI Active] ⏺ 93.7% reduction (2.3 KB → 147 B) 3ms
```

Catatan kaki, pada `omni exec` dan mode pipa. Ukuran masukan, ukuran keluaran,
dan berapa lama pipeline-nya berjalan.

```
[Partial signal]
```

Pipeline mengenali sebagian keluarannya, tapi tidak semuanya.

## Membaca persentase dengan benar

Bug-bug terburuk dalam sejarah proyek ini melaporkan pengurangan **paling
besar**. Distiller yang menghapus jawabannya terkompresi dengan indah.

Jadi angka besar bukan kabar baik dengan sendirinya. `omni diff` adalah
pemeriksanya:

```sh
omni diff     # perintah terakhir, mentah dibanding hasil sulingan
```

Kalau penghematan 99% ternyata membuang jalur berkas yang justru jawabannya, itu
bug yang layak dilaporkan, dan itu persis kelas yang paling dipedulikan proyek
ini.

## Ketika sama sekali tidak ada penanda

Sering, dan itu tandanya pipeline bekerja, bukan gagal. OMNI mengembalikan
keluarannya langsung setiap kali mengambil sesuatu akan tidak aman atau tidak
sepadan. Itu terjadi ketika:

- Muatannya JSON, YAML, CSV atau TSV. Tidak pernah disentuh, memang disengaja.
- Perintahnya gagal. Keluar dengan status bukan nol lewat apa adanya.
- Tidak ada basa-basi untuk dibuang. Tabel `kubectl get pods` adalah pendaftaran
  yang setiap barisnya sebuah data.
- Keluarannya terlalu pendek untuk pantas diberi penanda.

## Mengambil isinya kembali

```sh
omni retrieve <handle>
```

Bekerja di semua host, dengan atau tanpa MCP. Agent yang server MCP-nya terpasang
bisa memanggil `omni_retrieve` sendiri tanpa bertanya ke Anda.

Satu batas yang tidak bisa dijanjikan sebuah handle: arsipnya jendela bergulir 30
hari, jadi `omni retrieve` atas isi yang lebih tua dari itu tidak akan ketemu.
Jejak apa adanya bahkan lebih pendek, tujuh hari.

## Membedakan penanda asli dari penanda yang sekadar dicetak

Penanda juga muncul di dalam prosa. Halaman ini penuh dengannya, begitu pula source
OMNI sendiri, dan begitu pula laporan bug mana pun yang mengutipnya. Itu penting
ketika Anda mengukur apakah OMNI aktif pada suatu run, karena mencari bentuk penanda
di dalam transkrip akan menemukan contohnya semudah menemukan lipatan aslinya.

Handle-nya yang membedakan. Setiap contoh di manual ini dan di source OMNI memakai
satu nilai cadangan, `0000000000000000`, yang tidak akan pernah diberikan kepada
lipatan sungguhan:

```sh
omni retrieve 0000000000000000   # exit 1, "the documentation example"
omni retrieve <handle-yang-Anda-temukan> # exit 0 jika OMNI benar-benar melipatnya
```

Jadi exit code-nya yang menjawab pertanyaan itu, dan penanda yang disalin dari
dokumentasi tidak bisa disalahartikan sebagai bukti bahwa ada yang dipendekkan.
