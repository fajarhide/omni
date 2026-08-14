# Hermes Agent

OMNI menancap ke Hermes dua kali: sebuah plugin di jalur hook, dan server MCP.

| lapisan | mekanisme | apa yang berubah |
|---|---|---|
| hook | `~/.hermes/plugins/omni-signal-engine/__init__.py` yang memanggil `omni --pre-hook`, `--post-hook`, `--session-start` | keluaran tool terminal disuling sebelum masuk ke konteks Hermes |
| MCP | `mcp_servers.omni` yang menjalankan `omni --mcp` | perkakas MCP OMNI menjadi perkakas kelas satu di Hermes |

## Prasyarat

```sh
brew install fajarhide/tap/omni
omni --version
omni doctor

export HERMES_VENV="${HERMES_HOME:-$HOME/.hermes}/hermes-agent/venv"
export HERMES_PY="$HERMES_VENV/bin/python"
"$HERMES_PY" --version      # 3.11 atau lebih baru
```

Python dari venv-nya dibutuhkan karena `hermes plugins enable` berjalan di
dalamnya.

## Pasang

```sh
omni init --hermes
hermes plugins enable omni-signal-engine
hermes gateway restart
"$HERMES_PY" -m pip install hermes-omni-plugin
```

`omni init --hermes` idempoten. Ia memasang kerangka plugin-nya, mendaftarkan
server MCP di `~/.hermes/config.yaml` kalau belum ada di sana, menyalakan
kompresi Hermes ketika itu aman, dan menulis nilai bawaan berorientasi Hermes ke
`~/.omni/config.toml` **tanpa menimpa konfigurasi OMNI yang sudah ada**.

> Pakai salah satu, `hermes-omni-plugin` atau kerangka `omni init --hermes`,
> jangan keduanya sekaligus, atau Anda akan dapat pendaftaran plugin ganda.

## Konfigurasi

```yaml
# ~/.hermes/config.yaml

plugins:
  enabled:
    - omni-signal-engine

mcp_servers:
  omni:
    command: "/opt/homebrew/bin/omni"
    args: ["--mcp"]
    env:
      OMNI_AGENT_ID: "hermes"

compression:
  enabled: true
  threshold: 0.50     # kompres pada pemakaian konteks 50%
  target_ratio: 0.20  # sisakan 20%
```

Tiga hal harus benar: `plugins.enabled` memuat `omni-signal-engine`,
`mcp_servers.omni` menunjuk binary yang sebenarnya, dan `compression.enabled`
menyala supaya pemadatan Hermes sendiri dan peringatan tekanan OMNI sejalan, bukan
saling berkelahi.

`OMNI_AGENT_ID: "hermes"` lebih penting daripada kelihatannya. Tanpa itu, baris
milik Hermes bercampur dengan milik semua host lain dan tidak ada angka tentang
keduanya yang berarti.

## Verifikasi

```sh
omni doctor

hermes plugins list | grep omni        # harapkan: omni-signal-engine enabled
hermes tools list | grep mcp_omni_     # harapkan 25 perkakas, setelah restart
```

Lalu satu pemeriksaan fungsional pada fixture sungguhan:

```sh
cat tests/fixtures/cargo_test_500.txt | omni --post-hook 2>&1 | head -20
# baris test yang lulus dibuang, kegagalannya dipertahankan
```

Untuk uji langsung, jalankan sesuatu yang berisik lewat tool `terminal` milik
Hermes (`terminal("npm install", timeout=120)`) lalu bandingkan ukuran hasil
tool-nya dengan keluaran npm mentah. Pastikan dengan `omni stats`.

> Hitung perkakasnya, jangan percaya angka yang tertulis. Versi sebelumnya dari
> panduan ini menyebut 27, yang datang dari mem-`grep` kode sumber server; salah
> satu string itu nama penyaring, bukan perkakas. `hermes tools list` di atas
> yang menjadi hitungannya.

## Di mana OMNI membantu dan di mana tidak

| keluaran | efek OMNI |
|---|---|
| `npm install`, `cargo build`, `docker build` | besar, 70% ke atas. Progres, cache hit dan hash layer murni basa-basi. |
| run test | besar. Vonis dan kegagalannya selamat, baris `ok` tidak. |
| pembacaan berkas | nol dari penyaringnya, banyak dari ledger saat dibaca ulang |
| `kubectl -o json`, terraform plan | nol, disengaja. Muatan terstruktur lewat begitu saja. |
| perintah pendek | nol, atau sedikit negatif. Penandanya berongkos lebih mahal daripada penghematannya. |

Pakai perkakas MCP-nya sebagai kendali Hermes atas semua itu:
`omni_explain_savings` untuk melihat berapa sebenarnya ongkos sebuah perintah
terkini, `omni_retrieve` untuk mengambil kembali isi yang terlipat, dan
`omni_budget` untuk melihat token sesinya pergi ke mana.

## Setelah Hermes di-upgrade

```sh
hermes plugins list | grep omni
hermes tools list | grep mcp_omni_
omni doctor
```

Sebuah upgrade bisa menyetel ulang `plugins.enabled` atau memindahkan venv-nya.
Keduanya gagal diam-diam: plugin-nya sekadar berhenti dipanggil, dan tidak ada
yang mengumumkannya.
