# Hook

Titik masuk yang dipanggil host agent. Anda tidak pernah mengetik ini;
`omni init` menuliskannya ke konfigurasi host.

| titik masuk | kapan host memanggilnya |
|---|---|
| `omni --pre-hook` | Sebelum sebuah tool berjalan |
| `omni --post-hook` | Setelah sebuah tool menghasilkan keluaran |
| `omni --hook` | Dispatcher universal, untuk host dengan satu slot hook |
| `omni --session-start` | Sesi dimulai |
| `omni --session-end` | Sesi berakhir |
| `omni --pre-compact` | Sebelum host memadatkan percakapan |
| `omni --mcp` | Jalan sebagai server MCP lewat stdio |
| `cmd \| omni` | Mode pipa, tanpa host sama sekali |

## Satu panggilan, dua hook

![Satu panggilan tool Bash melewati OMNI dua kali: pre-hook boleh menulis ulang perintahnya sebelum shell menjalankannya, dan post-hook menyuling keluarannya sesudahnya, membaca basis data lokal untuk baris yang sudah ditampilkan dan mengarsipkan apa yang ia buang.](../media/hook-lifecycle.svg)

Shell menjalankan apa pun yang diserahkan pre-hook dan tidak pernah tahu OMNI
ada. Hanya jawabannya yang ditulis ulang, dan itu sebabnya tidak ada apa pun di
sini yang bisa mengubah apa yang dilakukan perintah Anda.

## Apa yang dilakukan masing-masing

**Pre-tool** memutuskan apakah sebuah perintah perlu dilewatkan OMNI sama sekali,
dan bisa menulis ulangnya menjadi `omni exec`. Tulis ulang itu membungkus
**seluruh** string perintahnya, termasuk pengalihan keluaran, dan itu sebabnya
berkas log di disk dari sebuah perintah yang cocok bisa ternyata versi
sulingannya. Patahkan awalannya (`env cargo test`) ketika Anda butuh log
mentahnya.

**Post-tool** acara utamanya: keluaran mentah datang, pipeline berjalan, dan
hasil sulingannya diserahkan kembali untuk disubstitusi host.

**Post-tool-failure** ada karena perintah yang gagal harus lewat apa adanya, dan
para host sangat tidak sepakat soal bagaimana mereka menyatakan sebuah perintah
gagal. Claude Code mengirim string biasa, `Error: Exit code N`. Yang lain membawa
penanda galat terstruktur. Membaca satu bentuk saja adalah bug yang pernah
dimiliki proyek ini.

**Session start** menyuntikkan konteks proyek: berkas yang sedang panas, galat
aktif terakhir, pengetahuan yang tersimpan, tujuan yang dipancang.

**Session end** menulis ringkasannya dan bisa mengekspor CSV.

**Pre-compact** adalah peringatan dari host bahwa percakapannya sebentar lagi
dipendekkan.

## Dua pintu menuju satu pipeline

`post_tool` dan `pipe` adalah titik masuk terpisah yang menjalankan tahapan yang
sama, dan menjaga keduanya seiring sudah berulang kali jadi sumber bug. Tiga
perbaikan terpisah masing-masing membetulkan satu salinan dan meninggalkan yang
lain. Tahap ledger sempat ada di `post_tool` selama satu rilis sebelum `pipe`
memilikinya sama sekali, jadi perintah yang ditulis ulang pre-hook menjadi
`omni exec` mendapat penyaringnya dan tidak lebih.

Kalau Anda mengubah perilaku pipeline, ubah keduanya, atau periksa kenapa tidak.

## Kenapa ia tidak pernah membuat agent Anda mati

Setiap hook berjalan di dalam `catch_unwind`, di titik masuk tertinggi. Panik di
satu tahap berongkos satu penyulingan, bukan satu sesi. Basis data yang tidak mau
dibuka berongkos konteks sesi, bukan pipeline-nya.

Itu aturan **gagal terbuka**, dan ia punya satu sisi tajam yang layak disebut:
gagal terbuka berarti menyerahkan kembali byte mentahnya. Ia tidak berarti
mengeluarkan ringkasan yang riang. Distiller yang tidak mengurai apa pun lalu
mengembalikan `0 tests passed` sedang gagal **tertutup**, dan dengan percaya
diri.

## Apa yang harus dilakukan sebuah host supaya semua ini berarti

Mendaftarkan hook-nya, lalu menghormati apa yang ia kembalikan.

Paruh kedua tidak dijamin. OMNI pernah mengeluarkan hasil sulingannya di bawah
kunci yang diabaikan Claude Code, jadi tidak ada yang diterapkan di jalur itu
selama dua rilis sementara OMNI mencatat sebuah penghematan dan mencetak catatan
kaki untuk masing-masingnya. Perbaikannya membetulkan kuncinya dan meninggalkan
bentuk nilainya tetap salah, dan gejalanya bertahan tanpa disadari.

Dua hal yang diajarkannya, keduanya tidak kentara:

- Tulis ulangnya divalidasi terhadap **skema keluaran milik tool host itu
  sendiri**, satu bentuk per tool. Tidak ada bentuk universal.
- Bidangnya saling bebas. Tulis ulang yang ditolak tetap meloloskan pesan
  konteksnya, jadi catatan kaki penghematan tercetak untuk penyulingan yang
  dibatalkan.

Jadi bukti sebuah hook bekerja bukan catatan kakinya dan bukan `omni stats`.
Buktinya adalah transkrip sesi milik host sendiri:

```sh
grep -c hook_error_during_execution ~/.claude/projects/<proyek>/<sesi>.jsonl
```

Peringatan yang bisa Anda lihat bukan peringatan yang bisa dilihat agent. Lampiran
seperti itu tidak pernah masuk ke konteks model, jadi sebuah agent bisa bilang
hook-nya baik-baik saja sementara terminal Anda penuh penolakan.

## Menguji sebuah hook dengan tangan

Beri ia muatan secara langsung ketimbang menebak jalur mana yang berjalan:

```sh
echo '<json muatan host>' | omni --post-hook
```

`omni exec` dan post-hook memilih rute yang berbeda, jadi hasil dari yang satu
bukan bukti tentang yang lain.
