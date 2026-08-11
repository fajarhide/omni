<div align="center">
  <img src="../media/logo.png" alt="OMNI Logo" width="300" />

<h1>OMNI</h1>
<p align="center">
    <em><b>エージェントはターミナルが吐く全行を読み、その大半を次のターンでもう一度読みます。</b>OMNI はモデルが見る前にノイズを落とし、すでに見せた行については参照を返します。何も削除せず、結果を捏造することもありません。</em>
</p>

[🇺🇸 English](../README.md) | [🇯🇵 日本語](README-ja.md) | [🇨🇳 简体中文](README-zh.md) | [🇸🇦 العربية](README-ar.md) | [🇮🇩 Bahasa Indonesia](README-id.md) | [🇻🇳 Tiếng Việt](README-vi.md) | [🇰🇷 한국어](README-ko.md)

[![CI](https://github.com/fajarhide/omni/actions/workflows/ci.yml/badge.svg)](https://github.com/fajarhide/omni/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/fajarhide/omni)](https://github.com/fajarhide/omni/releases)
  [![Rust](https://img.shields.io/badge/built_with-Rust-dca282.svg)](https://www.rust-lang.org/)
  [![MCP](https://img.shields.io/badge/MCP-compatible-green.svg?style=flat-square)](https://modelcontextprotocol.io/)
  [![License: MIT](https://img.shields.io/github/license/fajarhide/omni)](https://github.com/fajarhide/omni/blob/main/LICENSE)
  [![Hits](https://hits.sh/github.com/fajarhide/omni.svg)](https://hits.sh/github.com/fajarhide/omni/)
</br></br>

</br></br>

```bash
brew install fajarhide/tap/omni && omni init
```

Claude Code、Codex CLI、Gemini CLI ではコマンド出力を蒸留します。これらはホストが OMNI の書き換えを適用するためです。それ以外のホストでも MCP サーバー、共有セッション状態、そして通したコマンドを蒸留する `omni_run` が使えます。各ホストのティアは `omni doctor` で確認できます。


### 各ホストが OMNI に許可すること

| ティア | ホスト | 得られるもの |
|---|---|---|
| **Full** | Claude Code, Codex CLI, Gemini CLI, Aider (pipe) | ホストが OMNI の書き換えを適用するため、モデルは組み込みツールの蒸留済み出力を読みます。 |
| **Handoff-first** | Cursor, Windsurf | ホストは組み込みツールの出力を書き換えられません。`omni_run` を通したコマンドは蒸留され、`omni init --cursor` がエージェントにそれを選ばせるルールを導入します。 |
| **MCP-only** | Cline, Roo, OpenCode, VS Code, Zed, Copilot, Antigravity, Hermes, Pi | メモリ、リコール、セッション状態のみ。シェルの蒸留はなく、あるとも主張しません。 |

`omni doctor` が導入済みホストごとにティアを表示します。削減量はモデルが実際に受け取る量が減った場合にのみ計上されます。

Codex CLI にはもう一手間必要です。信頼済みとして登録されたフックしか実行せず、それ以外は何も告げずに無視します。`omni init --codex` の後に `codex` を一度起動し、"Hooks need review" で承認してください。それまで `omni doctor` は失敗します。[#359](https://github.com/fajarhide/omni/issues/359) を参照。
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

## 何をするのか

**ノイズを落とす。** ビルドログ、Docker のレイヤハッシュ、プログレスバー、ANSI カラー。
誰も読まない部分を、モデルに届く前に取り除きます。

**すでに見せたものを送り直さない。** 同じセッションで先に見せた行のまとまりは、バイト列
ではなくハンドル付きのマーカー一つとして返ります。これはフィルタにできない側の仕事です。
パターンがノイズと呼ぶからではなく、すでに文脈にあるから落とすのです。

**セッションをまたいで覚える。** エディタを再起動してもエージェントを変えても、プロジェクトの
文脈は残っています。

**邪魔をしない。** 失敗したコマンドはそのまま通します。JSON、YAML、CSV には触れません。
ほとんどのコマンドは手を加えずに返され、それは不足ではなく意図した挙動です。


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

四つの保証。どれも信じてくださいという一文ではなく、それを本当にしたコードか issue への
リンクです。

| 保証 | 方法 | 根拠 |
|---|---|---|
| **元をバイト単位で取り戻せる** | 切ったものはすべてローカル SQLite の **RewindStore** に保管 (SHA-256 から内容へ)。エージェントはハッシュを受け取り `omni_retrieve` を呼ぶ | [`仕組み`](#仕組み) |
| **結果を決して捏造しない** | 何のシグナルも解析できなかった蒸留器は、緑色の `no errors` や `passed` ではなく生の出力を返す | [#143](https://github.com/fajarhide/omni/issues/143) |
| **失敗を決して隠さない** | 終了コードが0でないコマンドはそのまま通す | [#120](https://github.com/fajarhide/omni/issues/120) |
| **構造化データに触れない** | JSON / YAML / NDJSON / CSV はバイト単位でそのまま通る | `pipeline::format` |
| **数字は実測であり願望ではない** | リリースバイナリで実トレース7,095件を再生。しかも 97.1% の呼び出しは削減ゼロで、その数字も公開する | [`ベンチマーク`](#ベンチマーク) |

大きな圧縮率では買えないものが、ここにあります。**元は必ず復元でき、エージェントに嘘をつくことは決してありません。**

---

## ベンチマーク

**2026-08-03 から 08-10 UTC** をカバーする **7,095 件の実コマンド実行** を再生し、
リリースバイナリで実測しました。すべてモデルに届いた出力です。期間は数字の一部です。
`execution_traces` は7日で刈られるため、コーパスは測定の1週間後には消えています。

* ビルドとテストの出力は **87.8%**。最大のクラスであるファイル再読み込みはフィルタが
  **0.0%**、台帳が **24.7%** で、その差こそ台帳が存在する理由です。
* **呼び出しの 97.1% は何も削減しませんでした。** 残りがどれだけの価値かを示す数字なので
  公開しています。**この計測では、出力が大きくなった呼び出しは 1 件もありません。**
  以前は 2 件あり、([#398](https://github.com/fajarhide/omni/issues/398)) で修正しました。あった間はその数字も公開していました。
* **1コマンドあたり 21 ms**。ペイロードの大きさではなく履歴とともに増え、205 MB の
  データベースでは 61 ms です。
<div align="center">
<img src="https://omni.weekndlabs.com/media/performance.png" alt="OMNI" width="600" />
</div>

コーパス全体、クラス別の内訳、フィクスチャ、レイテンシ表は
**[docs/BENCHMARKS.md](../docs/BENCHMARKS.md)** にあります。再現は
`cargo test --release --test bench_replay -- --ignored` で。

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
フィルタはホームディレクトリからのみ読み込まれます。リポジトリ内の `.omni/signals/` は意図的に無視されます。フィルタは行を隠せるため、チェックアウトに同梱されたフィルタは訪問者のエージェントが見る内容を密かに書き換えられるからです。

**自分の削減量はどう見ますか。**
数日使ったあとに `omni stats` を。`omni stats --share` は同じ数字をコピーしやすい形で
出力します。
`omni stats` はセッション寿命、つまりホストが閉じるまでに 1 セッションが処理したコマンド数から表示します。コンテキストウィンドウが実際に消費するのはそこだからです。その下の蒸留率は 1 つのホストのパイプラインに対する診断値であり、製品としての主張ではありません。

---

## もっと知る

* [仕組みと、そのコスト](../docs/ARCHITECTURE.md): パイプライン、RewindStore、Memory OS
* [ベンチマーク全文](../docs/BENCHMARKS.md): コーパス、クラス別、フィクスチャ、レイテンシ
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
