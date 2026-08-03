<div align="center">
  <img src="../media/logo.png" alt="OMNI Logo" width="300" />

<h1>OMNI</h1>
<p align="center">
    <em><b>1万行のターミナルノイズを読ませるために Claude に課金するのは、もう終わりです。</b>OMNI はエージェントが目にする前に <code>git</code> を 89%、<code>cargo</code> を 91%、<code>kubectl</code> を 77% 削ります。それ以外はそのまま通します。失われるものは何もなく、結果を捏造することもありません。</em>
</p>

[🇺🇸 English](../README.md) | [🇯🇵 日本語](README-ja.md) | [🇨🇳 简体中文](README-zh.md) | [🇸🇦 العربية](README-ar.md) | [🇮🇩 Bahasa Indonesia](README-id.md) | [🇻🇳 Tiếng Việt](README-vi.md) | [🇰🇷 한국어](README-ko.md)

[![CI](https://github.com/fajarhide/omni/actions/workflows/ci.yml/badge.svg)](https://github.com/fajarhide/omni/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/fajarhide/omni)](https://github.com/fajarhide/omni/releases)
  [![Rust](https://img.shields.io/badge/built_with-Rust-dca282.svg)](https://www.rust-lang.org/)
  [![MCP](https://img.shields.io/badge/MCP-compatible-green.svg?style=flat-square)](https://modelcontextprotocol.io/)
  [![License: MIT](https://img.shields.io/github/license/fajarhide/omni)](https://github.com/fajarhide/omni/blob/main/LICENSE)
  [![Hits](https://hits.sh/github.com/fajarhide/omni.svg)](https://hits.sh/github.com/fajarhide/omni/)
</br></br>
<b>
<code>git</code> 89% &middot; <code>cargo</code> 91% &middot; <code>kubectl</code> 77% &middot; 1コマンドあたり 21 ms &middot; 9,965 件中、出力が大きくなった呼び出しは 0 件 &middot; 切った分はバイト単位で復元可能 &middot; セッション横断メモリ </b>

</br></br>

```bash
brew install fajarhide/tap/omni && omni init
```

Claude Code、Cursor、Windsurf、Codex、Roo でそのまま動きます。

</br>
<img src="../media/demo.gif" alt="ノイズの多い cargo test を判定結果まで蒸留し、続いて omni stats を表示する OMNI" width="820" />
</div>

---

エージェントはターミナルが吐く行をすべて読みます。ビルドログ、Docker ログ、CI ログ、
プログレスバー、ANSI カラー。1行を見つけるために数千トークン。高いのは Claude では
なく、あなたのターミナルです。

そして一晩でそれを全部忘れます。Cursor を再起動し、Claude Code に切り替えれば、
プロジェクトの説明はまた最初からです。

OMNI は両方を直し、それ以外では退きます。

---

## 何が違うのか

**問題 1: ターミナルがシグナルをかき消す**

同じ `git log` を並べて比べます。OMNI なしでは、1コミットの `Author` / `Date` /
本文だけで画面が埋まります。OMNI ありでは、**すべてのコミットが残ります**。
`hash subject` の1行として、94% 小さくなって。要約で消えたものはなく、フッターの
数字は実際のバイト数から測ったもので、約束ではありません。

<table>
<tr>
<td align="center"><b>OMNI なし</b><br/><sub>生の <code>git log -15</code></sub></td>
<td align="center"><b>OMNI あり</b><br/><sub>全コミット保持、94% 削減</sub></td>
</tr>
<tr>
<td valign="top"><img src="../media/demo-git-without.gif" alt="冗長な生の git log -15。1コミットの Author、Date、本文で画面が埋まる" width="400" /></td>
<td valign="top"><img src="../media/demo-git-with.gif" alt="OMNI を通した同じ git log -15。各コミットが hash と subject の1行になり 94% 小さい" width="400" /></td>
</tr>
</table>

`tests/fixtures/` と再生したトレースで実測した数字であって、願望ではありません。

| コマンド | OMNI なし | OMNI あり | 削減 |
|---|---|---|---|
| `cargo test` (490 成功、10 失敗) | テストごとの出力 16.5 KB | ランナー自身の成否サマリ | **92.9%** |
| `git status` (変更あり) | porcelain 出力 496 B | ブランチと変更されたパス | **61.7%** |
| `docker build` (キャッシュノイズ多め) | レイヤーハッシュとプログレスバー 9.2 KB | ビルド結果、キャッシュヒットは畳む | **35.9%** |
| `git diff` (複数ファイル) | ロックファイル、空白、生成物の差分 | 実際に変わったコード | **25.2%** |
| `kubectl get pods` (35 pod、5 crash) | テーブル全体 | テーブル全体 | 意図して **0%** |

上の数字はすべて、実際に**届けられた**ペイロードで、OMNI が何かを落としたときに付ける
約 77 バイトの復元マーカーを含みます。以前のリリースはこのマーカーを付ける前の蒸留器
出力を引用しており、小さなペイロードほど良く見えていました。`git diff` はここでは
25.2%、マーカーなしなら 44.6% です。マーカーこそが切った分を復元可能にしているので、
数字に含めるのが筋です。

面白いのは `kubectl get pods` の行です。以前は 9.3% と報告していましたが、いまは何も
報告しません。pod のテーブルは1行1行がデータである列挙であり、落とすべきノイズが存在
しないからです。あの 9.3% を失ったことが修正でした。

> **意図して何もしない場所。** 失敗したコマンドはそのまま通します。隠れたエラーは圧縮されていないエラーより高くつくからです。構造化出力 (JSON、YAML、CSV) には一切触れません。あなたのパイプラインの次の工程がそれをパースするからです。OMNI は繰り返しの多いツールの雑音でこそ働き、それ以外では退きます。だからこそ、実行するすべてのコマンドで有効にしたまま安全に使えます。

### 失われるものは何もない。捏造もしない。

2つの約束で、どちらもこの段落ではなくコードの中にあります。

**失われるものは何もない。** OMNI が切ったバイトはすべて、SHA-256 をキーにローカルの RewindStore へ保管されます。エージェントは蒸留された出力とともにハッシュを受け取り、`omni_retrieve` を呼べば会話の途中で、コマンドを再実行することなく、元をバイト単位で取り戻せます。

**捏造もしない。** 入力から何も認識できなかった蒸留器は、生の入力をそのまま返します。これは規約ではなく型です。`distill` は `Option<String>` を返し、ルーティング層は `None` を受け取るたびに元へフォールバックします。OMNI が読んでいない緑色の「no errors」を生み出すコードパスは存在しません。

他の圧縮ツールは、切り捨てた部分が重要でなかったことを *信じてくれ* と言います。OMNI は証拠を手渡します。

| 保証 | 方法 | 根拠 |
|---|---|---|
| **元をバイト単位で取り戻せる** | 切ったものはすべてローカル SQLite の **RewindStore** に保管 (SHA-256 から内容へ)。エージェントはハッシュを受け取り `omni_retrieve` を呼ぶ | [`仕組み`](#仕組み) |
| **結果を決して捏造しない** | 何のシグナルも解析できなかった蒸留器は、緑色の `no errors` や `passed` ではなく生の出力を返す | [#143](https://github.com/fajarhide/omni/issues/143) |
| **失敗を決して隠さない** | 終了コードが0でないコマンドはそのまま通す | [#120](https://github.com/fajarhide/omni/issues/120) |
| **構造化データに触れない** | JSON / YAML / NDJSON / CSV はバイト単位でそのまま通る | `pipeline::format` |
| **数字は実測であり願望ではない** | リリースバイナリで実トレース9,965件を再生。しかも 90.0% の呼び出しは削減ゼロで、その数字も公開する | [`ベンチマーク`](#ベンチマーク) |

大きな圧縮率では買えないものが、ここにあります。**元は必ず復元でき、エージェントに嘘をつくことは決してありません。**

---

## ベンチマーク

ある開発者の実際の利用から再生した **9,965 件の実コマンド実行** に対し、リリース
バイナリで実測しました。

* **実際にノイズを生むコマンドでは 76 から 91%。** `cargo` 91.4%、`git` 89.2%、
  `kubectl` 76.5%。あなたのコンテキスト予算が消えていくのはそこであり、OMNI が働くの
  もそこです。
* **OMNI が手を出すのは 10 コマンドに 1 つ。残り 9 つには 0 バイトも足しません。**
  これは要約器ではなくフィルタです。切るものがなければ完全に退きます。
* **9,965 件のうち、出力が大きくなった呼び出しは 1 件もありません。**
* **構成全体では 43.3% 減。** ノイズの多いコマンドも静かなコマンドも合わせた数字です。
* **1コマンドあたり 21 ms** のエンドツーエンド。ペイロードの大きさではなく履歴とともに
  増え、205 MB のデータベースでは 61 ms です。

<div align="center">
<img src="https://omni.weekndlabs.com/media/performance.png" alt="OMNI" width="600" />
</div>

コーパス全体、コマンド別の内訳、フィクスチャ、レイテンシ表は
**[docs/BENCHMARKS.md](../docs/BENCHMARKS.md)** にあります。再現は
`cargo test --release --test bench_replay -- --ignored` で。

### 削減率の読み方、私たちの数字も含めて

この分野のツールはどれも百分率をひとつ公開します。その数字に意味があるかを決める5つの
問いと、私たちの答えです。

| 問い | なぜ効くか | OMNI |
|---|---|---|
| **何も削減しなかった**呼び出しの割合は | すべてのコマンドで削減するツールは、必要な出力を要約している | **90.0%**、公開しています |
| 出力が**大きくなった**呼び出しはあるか | マーカーやヘッダはバイトを食うが、裏目に出た分を誰も報告しない | **9,965 件中 0 件** |
| どの**母集団**を測ったか | どのモデルも読まないターミナルバイトを数えれば、数字はただで膨らむ | モデルに届いた分だけ。そう言うことで 36 ポイント失っています |
| **再実行**できるか | 再現できない数字は測定ではなく主張 | コマンド1つ、あなた自身のデータで |
| 切った分は**復元できる**か | ロッシーは可逆なら問題なく、不可逆なら致命的 | バイト単位で、`omni_retrieve` から |

何もしなかった呼び出しの割合を公開するのは、それが残りの数字の価値を教える数字だから
です。

---

## クイックスタートとインストール

OMNI の準備は驚くほど簡単で、ターミナルにネイティブに統合されます。

**macOS / Linux:**
```bash
# 1. Homebrew でインストール
brew install fajarhide/tap/omni

# 2. OMNI をセットアップ (Claude、VS Code、OpenCode、Codex、Antigravity 向けの対話メニュー)
omni init

# 3. 動作確認
omni doctor

# 4. 問題があれば自動修復
omni doctor --fix

# 5. 現在の状態を確認
omni init --status
```

**ユニバーサルインストーラ (macOS / Linux / WSL):**
```bash 
curl -fsSL omni.weekndlabs.com/install | bash
```

**Windows (PowerShell):**
```powershell
irm omni.weekndlabs.com/install.ps1 | iex
```

---

---

## FAQ

**OMNI はログを永久に削除しますか?**  
いいえ。生ログは圧縮されローカルの SQLite RewindStore に保存されます。AI はハッシュを受け取り、必要なら完全なログを取得できます。

**ターミナルは遅くなりますか?**  
はい、測定できる程度に。そしてコストは履歴とともに増えます。蒸留パイプライン自体は1桁ミリ秒ですが、フックされたコマンドはすべてローカルの RewindStore にも書き込みます。496 バイトの `git status` は新しいデータベースで約 21 ms、205 MB のデータベースで約 61 ms、16.5 KB の `cargo test` は約 25 ms です。見込んでおいてください。生の出力が必要なときは `OMNI_PASSTHROUGH=1` でパイプライン全体を飛ばせます。

**独自のフィルタを追加できますか?**  
できます。社内ツール固有のノイズを削る方法を、TOML で OMNI に教えられます。
```toml
# ~/.omni/signals/custom.toml
[filters.my_tool]
match_command = "^internal-tool\\b"
strip_lines_matching = ["^DEBUG", "syncing..."]
```

**自分の削減量はどう見ますか。**
数日使ったあとに `omni stats` を。`omni stats --share` は同じ数字をコピーしやすい形で
出力します。

---

## もっと知る

* [仕組みと、そのコスト](../docs/ARCHITECTURE.md): パイプライン、RewindStore、Memory OS
* [ベンチマーク全文](../docs/BENCHMARKS.md): コーパス、コマンド別、フィクスチャ、レイテンシ
* [コントリビュート](../CONTRIBUTING.md): `make ci` が通れば準備完了

---

```bash
brew install fajarhide/tap/omni && omni init
```

## コントリビューションとライセンス

これはエージェント型 AI の時代のために作られた、情熱から生まれたプロジェクトです。トークン代を節約しに来た方も、無料モデルを試しに来た方も、究極のエージェント用ツールベルトを一緒に作りに来た方も、貢献はいつでも歓迎します。

- **開発**: ソースからビルドしたいですか? `make ci` と `cargo build` を実行してください。詳細は [CONTRIBUTING.md](../CONTRIBUTING.md) を参照。
- **ライセンス**: [MIT License](../LICENSE)

<!-- Star History -->
<p align="center">
  <a href="https://star-history.com/#fajarhide/omni&Date">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/svg?repos=fajarhide/omni&type=Date&theme=dark" />
      <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/svg?repos=fajarhide/omni&type=Date" />
      <img alt="Star History Chart" src="https://api.star-history.com/svg?repos=fajarhide/omni&type=Date" width="600" />
    </picture>
  </a>
</p>
<center>
Build with ❤️ by <a href="https://github.com/fajarhide">Fajar Hidayat</a>
</center>
