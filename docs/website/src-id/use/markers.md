# Membaca penanda

Penanda adalah cara OMNI memberi tahu apa yang ia lakukan. Bentuknya hanya
beberapa, dan mengenalinya adalah beda antara memercayai perkakas ini dan
mencurigainya.

## Bentuk-bentuknya

```
[OMNI: 406 lines omitted, omni retrieve 3f7bfd89bc5d7cee for full output]
```

Ada isi yang dipotong dan diarsipkan. Enam belas karakter itu sebuah handle:
`omni retrieve 3f7bfd89bc5d7cee` mencetak aslinya kembali, persis byte demi byte,
dari shell mana pun di sesi mana pun.

```
[OMNI: 40 lines already shown, omni retrieve bc7e821a4340073e]
```

Ledger. Baris-baris ini sudah dikeluarkan sebelumnya **di sesi ini**, jadi
klaimnya adalah agent masih memegangnya dan handle-nya tidak berongkos kecuali ia
memang mau membaca ulang.

```
[OMNI: 40 lines not shown here, omni retrieve bc7e821a4340073e]
```

Ledger juga, klaim berbeda. Baris-baris ini pergi ke **sesi lain** dari proyek
ini, dan agent ini belum pernah melihatnya. Kalimatnya sengaja bukan "already
shown", karena itu akan salah. Melipatnya adalah taruhan bahwa agent tidak akan
membutuhkannya, dan karena itu ia memikul ambang keuntungan tiga kali lipat.

Sesi lain itu bisa jadi juga agent yang lain. Riwayat proyek dikunci pada
direktorinya, jadi apa pun yang berjalan di repositori ini ikut menyumbang. Lihat
[apa yang dibagi dua agent](../concepts/the-ledger.md#apa-yang-dibagi-dua-agent-dalam-satu-repo).

```
[OMNI: identical to the 40 lines already shown, omni retrieve bc7e821a4340073e]
[OMNI: identical to 40 lines from an earlier session, none shown here, omni retrieve bc7e821a4340073e]
```

Dua klaim yang sama, untuk jawaban yang terulang **seluruhnya**. Ketika
pelipatannya mencakup setiap baris, penandanya adalah seluruh keluaran, bukan
sebuah lubang di dalamnya, jadi ia berbunyi `identical to` dan Anda mendapat satu
baris di tempat run ulang akan mencetak ratusan baris yang sama. Apa pun yang
kurang dari seluruh jawaban memakai kalimat di atasnya.

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

Sering. Sekitar 97% panggilan tidak menghemat apa pun dan mengembalikan
keluarannya langsung. Itu pipeline yang bekerja, bukan gagal. Itu terjadi ketika:

- Muatannya JSON, YAML, CSV atau TSV. Tidak pernah disentuh, memang disengaja.
- Perintahnya gagal. Keluar dengan status bukan nol lewat apa adanya.
- Tidak ada basa-basi untuk dibuang. Tabel `kubectl get pods` adalah pendaftaran
  yang setiap barisnya sebuah data.
- Keluarannya terlalu pendek untuk pantas diberi penanda.

## Mengambil isinya kembali

```sh
omni retrieve 3f7bfd89bc5d7cee
```

Bekerja di semua host, dengan atau tanpa MCP. Agent yang server MCP-nya terpasang
bisa memanggil `omni_retrieve` sendiri tanpa bertanya ke Anda.

Satu batas yang tidak bisa dijanjikan sebuah handle: arsipnya jendela bergulir 30
hari, jadi `omni retrieve` atas isi yang lebih tua dari itu tidak akan ketemu.
Jejak apa adanya bahkan lebih pendek, tujuh hari.
