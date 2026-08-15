# Perkakas MCP

`omni init` mendaftarkan OMNI sebagai server MCP, yang memberi agent perkakas yang bisa
ia panggil sendiri tanpa lewat Anda. Halaman ini menjelaskan seluruh **25** perkakas.
Host Anda hanya diberi tahu sebagiannya.

## Yang diberitahukan ke host Anda

Definisi perkakas berada di awal setiap request, jadi perkakas yang tidak pernah dipanggil
akan dibaca ulang pada setiap request di setiap sesi, bukan dibayar sekali saja. Karena
itu OMNI hanya mengiklankan set yang bisa dipakai oleh tier host Anda:

| tier | yang diiklankan |
|---|---|
| Full, dan host apa pun yang tidak dikenali OMNI | sembilan di bawah ini |
| Handoff-first | sembilan yang sama, `omni_run` selalu termasuk |
| MCP-only | `omni_remember`, `omni_recall`, `omni_retrieve`, `omni_knowledge` |

Yang sembilan itu adalah `omni_retrieve`, `omni_explain_savings`, `omni_remember`,
`omni_recall`, `omni_run`, `omni_find_noise`, `omni_context_breakdown`, `omni_history`
dan `omni_context`. Itulah yang benar-benar pernah dipanggil di korpus yang terekam.

Perkakas lain di halaman ini tetap ada di dalam binary dan hanya berjarak satu setelan.
`OMNI_MCP_TOOLS=all` mengiklankan seluruh 25 perkakas, dan `omni doctor` menyebutkan set
mana yang sedang berlaku serta host mana yang terdeteksi:

```
  MCP tools:      9 of 25 advertised to claude_code (OMNI_MCP_TOOLS=all restores the rest)
```

Pastikan daftarnya terhadap binary Anda sendiri, bukan terhadap halaman ini:

```sh
{ echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"p","version":"1"}}}'
  echo '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  echo '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}'; } \
| omni --mcp | tail -1 | jq -r '.result.tools[].name'
```

## Mengambil isi kembali

| perkakas | apa yang ia lakukan |
|---|---|
| `omni_retrieve` | Ambil isi lengkap yang dihilangkan sebuah penanda, lewat handle-nya |
| `omni_run` | Jalankan perintah shell lalu kembalikan keluaran sulingannya |
| `omni_signal_extract` | Tarik sinyal dari teks mentah, tanpa pipeline hook |

`omni_run` paling penting di host yang tidak bisa menulis ulang tool shell
bawaannya. Di sana, ia satu-satunya jalan menuju keluaran sulingan, dan itu
sebabnya `omni init --cursor` memasang sebuah aturan yang menyuruh agent
mengutamakannya.

## Memahami apa yang dikerjakan OMNI

| perkakas | apa yang ia lakukan |
|---|---|
| `omni_explain_savings` | Rute, penyaring, byte masuk dan keluar, persentase hemat per perintah terkini |
| `omni_history` | Penyulingan terkini beserta penghematan dan rasio per panggilan |
| `omni_context_breakdown` | Rincian token menurut sumbernya untuk giliran saat ini |
| `omni_density` | Seberapa banyak sinyal dibanding kebisingan dalam sepotong teks |
| `omni_budget` | Pemakaian anggaran token dan efisiensi kompresi untuk sesi ini |

Keempat ini jawaban yang tepat untuk "apakah OMNI membantu di sini". Tarik
angkanya, jangan membentuk kesan.

## Ingatan

| perkakas | apa yang ia lakukan |
|---|---|
| `omni_remember` | Simpan sebuah keputusan, jebakan atau batasan |
| `omni_recall` | Pencarian semantik lintas engram, pengetahuan dan riwayat penyulingan |
| `omni_knowledge` | Kueri atau simpan pengetahuan proyek lintas sesi |
| `omni_insight` | Persoalan dan pola galat berulang teratas di seluruh proyek |
| `omni_adaptive_insights` | Pola pengambilan kembali, sebagai penilaian atas keefektifan penyulingan |
| `omni_handoff` | Ekspor keadaan sesi sebagai markdown yang bisa dibawa, tanpa perlu jaringan |

`omni_handoff` hanya ada di MCP. Subperintah CLI dengan nama itu sudah dihapus.

## Sesi dan pencarian

| perkakas | apa yang ia lakukan |
|---|---|
| `omni_session` | Keadaan sesi: status, konteks, bersihkan |
| `omni_search` | Cari di riwayat sesi ini |
| `omni_query` | Kueri riwayat penyulingan dengan bentuk kueri yang tetap |
| `omni_context` | Konteks dependensi ringan untuk sebuah berkas |
| `omni_agents` | Agent lain yang sedang aktif di proyek ini |

## Penyetelan

| perkakas | apa yang ia lakukan |
|---|---|
| `omni_find_noise` | Analisis jejak mentah terkini untuk mencari kebisingan yang berulang |

> Sifatnya saran saja, dan pembelajarnya memperlakukan "berulang" sebagai
> "kebisingan". Ia pernah menyarankan membuang `^metadata:`, `^spec:`, pagar blok
> kode dan `^\[stderr\]`, yang justru struktur dan kanal galat. Jangan pernah
> menempelkan keluarannya ke mana pun tanpa membacanya baris demi baris.

## Loop

| perkakas | apa yang ia lakukan |
|---|---|
| `omni_loop_status` | Cek status sekali panggil untuk orkestrator sebelum tiap iterasi |
| `omni_loop_memory` | Baca dan tulis ingatan loop yang selamat dari restart sesi |
| `omni_set_loop_context` | Perbarui konteks loop secara dinamis |
| `omni_budget_status` | Status anggaran untuk iterasi ini. Panggil sebelum pekerjaan mahal. |
| `omni_verify` | Sebagai subagent pemeriksa, nilai pekerjaan terkini agent pembuat |

Lihat [Loop engineering](../integrations/loops.md).

## Satu perkakas yang bukan perkakas

`omni_auto_noise` muncul sebagai sebuah string di kode sumber server dan
**bukan** sebuah perkakas. Ia nama penyaring yang diteruskan ke pembangkit TOML.
Memanggilnya mengembalikan `-32602 tool not found`.

Ia pernah salah hitung: `grep` atas kode sumber untuk `"omni_*"` mengembalikan
27, jadi 27 salah di mana pun ia muncul. Jalankan panggilan `tools/list` di atas
untuk hitungannya.
