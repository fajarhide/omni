# Melihat berapa yang dihemat

```sh
omni stats
```

Semua yang ada di halaman ini membaca agregasi yang sama, jadi angka di kartu
bagikan tidak mungkin berbeda dari angka di laporannya.

## Laporannya

```sh
omni stats                 # 30 hari terakhir, bawaannya
omni stats --today         # atau --hour, --week, --month
omni stats --detail        # perintah, rute, sesi, agent
omni stats --all-commands  # semua perintah, bukan cuma yang teratas
omni stats --project       # dipecah per jalur proyek
omni stats --json          # bisa dibaca mesin
```

Ia memimpin dengan **umur sesi**: berapa perintah yang dibawa sebuah sesi sebelum
host menutupnya. Itu meteran yang benar-benar diperhatikan pengguna. Persentase
penyulingan di bawahnya adalah alat diagnosis untuk pipeline satu host, bukan
klaim produk.

## Membacanya tanpa membodohi diri sendiri

**Pisahkan menurut `agent_id` sebelum mengutip apa pun.** Baris yang tercatat di
bawah `terminal` adalah byte TTY yang tidak pernah dibaca model mana pun. Pada
satu pemasangan, baris seperti itu 73% dari seluruh byte yang diklaim OMNI sudah
dihemat. `omni stats` sekarang mengecualikannya, tapi jebakan yang sama menunggu
siapa pun yang mengueri basis datanya langsung.

**Persentase tinggi tidak otomatis bagus.** Cacat terburuk dalam sejarah proyek
ini melaporkan pengurangan paling besar, karena menghapus jawabannya terkompresi
dengan sangat baik. Sandingkan angka mana pun dengan `omni diff` pada perintah
sungguhan.

**Persentase rendah biasanya benar.** Sekitar 97% panggilan tidak menghemat apa
pun karena memang tidak ada yang bisa dihemat. Muatan terstruktur, perintah yang
gagal dan pendaftaran semuanya lewat begitu saja, memang dirancang begitu.

## Pemeriksaan yang tidak bisa dilakukan persentase

```sh
omni stats --rerun
```

Distiller mana yang berongkos satu run ulang. Kalau sebuah distiller membuang
sesuatu yang lalu harus diambil ulang agent, pengurangannya bukan penghematan,
melainkan penundaan. Tidak ada hitungan byte yang bisa melihat itu.

## Membagikannya

```sh
omni stats --share     # ringkasan siap tempel dari penghematan terukur Anda sendiri
omni stats --card      # ringkasan yang sama, ditulis sebagai gambar
```

Keduanya datang dari basis data Anda sendiri, dan itulah maksudnya. Klaim rasio
di README orang lain tidak bisa diverifikasi sebelum dipasang.

## Di peramban

```sh
omni dashboard             # http://127.0.0.1:7717
omni dashboard --port 8080
```

Hanya baca, basis data yang sama, mengikat loopback dan tidak yang lain.

## Menggali lebih jauh

```sh
omni stats --detail              # rincian per perintah dan per rute
omni query errors in last 5 commands
omni query warnings from cargo
omni query timeline today
omni patterns                    # galat yang terus kembali
omni patterns --tool cargo
```

`omni_history` memberi baris per panggilan yang sama ke klien MCP. Tidak ada
subperintah `omni history`; halaman ini sempat mencantumkannya sampai 0.7.4.

`omni query` berbicara dalam bahasa kueri kecil yang tetap, bukan teks bebas.
Bentuk yang didukung ada di bantuannya sendiri.

## Mengueri basis datanya langsung

`~/.omni/omni.db` adalah SQLite biasa dan tidak ada yang melarang Anda.

> Jangan pernah membaca keluaran `sqlite3` lewat hook Bash saat sedang
> menyelidiki OMNI. Pipeline-nya bisa melipat baris yang sedang Anda hitung, dan
> filter `LIKE` yang menangkap baris keliru sudah pernah memasukkan angka salah
> ke sebuah issue yang terbit. Daftarkan barisnya sebelum mengutip agregat apa
> pun atasnya, dan setel `OMNI_PASSTHROUGH=1`.
