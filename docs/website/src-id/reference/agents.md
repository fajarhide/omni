# Agent yang didukung

Host mana yang Anda jalankan menentukan apa yang bisa dilakukan OMNI, dan
langit-langitnya milik host, bukan milik pipeline-nya. Halaman ini layak dibaca
sebelum menilai apakah OMNI layak berada di sana.

## Tingkatannya

| tingkat | host | yang Anda dapat |
|---|---|---|
| **Penuh** | Claude Code, Codex CLI, Gemini CLI, OpenClaw, Hermes, Pi, Aider (pipa) | Host menerapkan tulis ulang OMNI, jadi model membaca keluaran sulingan dari tool bawaannya sendiri. |
| **Handoff dulu** | Cursor, Windsurf | Host tidak bisa menulis ulang keluaran tool bawaannya. `omni_run` menyuling apa pun yang dilewatkan melaluinya, dan `omni init --cursor` memasang aturan yang membuat agent meraihnya. |
| **MCP saja** | Cline, Roo, OpenCode, VS Code, Zed, Copilot, Antigravity | Ingatan, pemanggilan kembali dan keadaan sesi. Tanpa penyulingan shell, dan tanpa klaim soal itu. |

```sh
omni doctor     # mencetak tingkat untuk setiap host yang terpasang
```

Penghematan hanya pernah dihitung di tempat model benar-benar menerima lebih
sedikit. Host yang tidak bisa menerapkan tulis ulangnya tidak akan menggerakkan
angka penyulingan sebaik apa pun penyaringnya, dan mengaku sebaliknya akan
menjadi cacat yang sama dengan distiller yang melaporkan penghematan yang tidak
ia lakukan.

## Memasang untuk masing-masing

```sh
omni init --claude       omni init --cursor      omni init --zed
omni init --cline        omni init --roo         omni init --roo-code
omni init --copilot      omni init --gemini      omni init --opencode
omni init --codex        omni init --openclaw    omni init --antigravity
omni init --hermes       omni init --vscode      omni init --pi
omni init --all
```

## Catatan khusus per host

**Codex CLI** hanya menjalankan hook yang sudah dinyatakan tepercaya, dan
melewati sisanya tanpa sepatah kata. Setelah `omni init --codex`, jalankan
`codex` sekali lalu setujui di bagian "Hooks need review". `omni doctor` gagal
sampai Anda melakukannya. Ini pernah menggigit: Codex menjalankan nol hook
selama satu rilis penuh sementara semuanya tampak terpasang dengan benar.

**Cursor** tidak bisa menulis ulang keluaran tool shell bawaannya. Mencegat
shell-nya dengan menolak eksekusi lalu mengembalikan keluarannya sebagai pesan
hook secara teknis mungkin dan sudah ditolak: itu memberi tahu agent bahwa
perintahnya diblokir, menghilangkan kode keluarnya, memindahkan semantik eksekusi
ke dalam OMNI, dan melewati alur persetujuan host.

**Claude Code** mencocokkan lebih dari sekadar `Bash`. Pencocok post-tool-nya
`Bash|Read|Grep|WebFetch`, dan itulah yang akhirnya membuat distiller pembacaan
berkas, pencarian dan pengambilan web berjalan sama sekali. Tiga di antaranya
sudah ditulis dan diuji sepenuhnya dan tidak pernah sekali pun berjalan di sesi
sungguhan.

**OpenClaw** Penuh di giliran berikutnya, bukan giliran saat ini. Hook
`tool_result_persist`-nya menulis ulang hasil tool yang disimpan OpenClaw, jadi model
membaca byte sulingan setiap kali transkrip dibaca ulang, sementara giliran yang
menjalankan perintahnya masih melihat keluaran mentah. Di situlah biaya sebuah hasil
tool sebenarnya berada, karena ia dibaca ulang berkali-kali, tetapi ini Penuh yang lebih
sempit daripada milik Claude Code.

**Hermes** menyerahkan setiap hasil tool ke OMNI, bukan hanya terminal, dan itu
jangkauan terluas di antara host di sini: ia yang menjalankan distiller baca berkas,
pencarian dan fetch di host yang punya ketiganya. Ia juga punya halaman integrasinya
sendiri: [Hermes Agent](../integrations/hermes.md).

**Windows** didukung. Jalur, akhiran baris dan imbuhan `.exe` sudah ditangani,
dan matriks CI-nya menyertakan `windows-latest`.

## Beberapa agent sekaligus

Beri masing-masing identitasnya sendiri supaya angkanya tetap bisa dipisah:

```sh
OMNI_AGENT_ID=claude ...
OMNI_AGENT_ID=cursor ...
```

`omni_agents` melaporkan agent mana yang sedang aktif di proyeknya. Setiap baris
penyulingan membawa id-nya, dan angka apa pun yang mencampurnya sedang
menggambarkan sebuah campuran, bukan sebuah produk.

## Menambahkan sebuah host

Modul agent-nya ada di `src/agents/`, satu berkas per host, dan masing-masing
menulis format konfigurasi milik host itu di lokasi milik host itu. Polanya kecil
dan sebagian besar mekanis.

Bagian yang tidak mekanis adalah verifikasinya. "Penyedia tidak bisa dihubungi"
bukan alasan membiarkan sebuah jalur hook tak terverifikasi: sajikan API-nya, dan
palsukan modelnya saja. Hook yang tidak pernah berjalan di produksi sudah
ditemukan di tiga host justru dengan cara itu.
