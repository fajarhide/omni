<div align="center">
  <img src="../media/logo.png" alt="OMNI Logo" width="300" />

<h1>OMNI</h1>
<p align="center">
    <em>AI エージェントのためのノイズキャンセリング・コンテキストと長期記憶。<b>ロッシーですが、常に復元でき、結果を決して捏造しません。</b>1万行のターミナルノイズを読ませるために Claude に課金するのは、もう終わりです。</em>
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
モデルに届くバイトを 43.3% 削減、実コマンド 9,965 件で実測 &middot; セッション横断メモリ &middot; フォーマット安全 &middot; 常に復元可能 &middot; フェイルオープン、捏造なし &middot; 再現できる数字 </b>

</br></br>
<img src="../media/demo.gif" alt="ノイズの多い cargo test を判定結果まで蒸留し、続いて omni stats を表示する OMNI" width="820" />
</div>

---

AI コーディングアシスタントには、大きな問題が2つあります。

**1. すべてを読んでしまう。**  
ビルドログ。  
Docker のログ。  
CI のログ。  
プログレスバー。  
ANSI カラー。  
たった1行を見つけるために、何千ものトークン。高いのは Claude ではありません。あなたのターミナルです。

**2. すべてを忘れてしまう。**  
Cursor を再起動するたび、あるいは Claude Code から Windsurf に乗り換えるたびに、エージェントは記憶を失います。プロジェクトの目的を説明し直し、同じフレームワークの落とし穴を何度も伝え直すことになります。

OMNI はその両方を解決します。

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
| `cargo test` (490 成功、10 失敗) | テストごとの出力 16.5 KB | ランナー自身の成否サマリ | **93%** |
| `kubectl get pods` (35 pod、5 crash) | テーブル全体 | `35 pods \| 30 running, 5 error` と失敗した5つの pod 名 | 削らない |
| `git diff` (複数ファイル) | ロックファイル、空白、生成物の差分 | 実際に変わったコード | **45%** |
| `docker build` (キャッシュノイズ多め) | レイヤーハッシュとプログレスバー 9.2 KB | ビルド結果、キャッシュヒットは畳む | **37%** |

> **正直な注意書き:** OMNI が圧縮するのは *成功したがノイズの多い* 出力です。**失敗した**コマンドは**そのまま**通します。隠れたエラーは圧縮されていないエラーより悪いからです。構造化出力 (JSON/YAML/CSV) には一切触れません。繰り返しの多いツールの雑音でこそ働き、それ以外では邪魔をしません。

### ロッシーなツールを信頼できる理由

他の圧縮ツールは、切り捨てた部分が重要でなかったことを *信じてくれ* と言います。OMNI は求めません。保証します。そして各保証は、あなたが読めるコードに裏打ちされています。

| 保証 | 方法 | 根拠 |
|---|---|---|
| **元をバイト単位で取り戻せる** | 切ったものはすべてローカル SQLite の **RewindStore** に保管 (SHA-256 から内容へ)。エージェントはハッシュを受け取り `omni_retrieve` を呼ぶ | [`仕組み`](#仕組み) |
| **結果を決して捏造しない** | 何のシグナルも解析できなかった蒸留器は、緑色の `no errors` や `passed` ではなく生の出力を返す | [#143](https://github.com/fajarhide/omni/issues/143) |
| **失敗を決して隠さない** | 終了コードが0でないコマンドはそのまま通す | [#120](https://github.com/fajarhide/omni/issues/120) |
| **構造化データに触れない** | JSON / YAML / NDJSON / CSV はバイト単位でそのまま通る | `pipeline::format` |
| **数字は実測であり願望ではない** | リリースバイナリで実トレース9,965件を再生。しかも 90.0% の呼び出しは削減ゼロで、その数字も公開する | [`ベンチマーク`](#ベンチマーク) |

大きな圧縮率では買えないものが、ここにあります。**元は必ず復元でき、エージェントに嘘をつくことは決してありません。**

**問題 2: エージェントは一晩ですべて忘れる**

### 新しいセッションを始めるとき
**OMNI なし:** 「プロジェクト構成をもう一度説明して。auth モジュールが壊れていて、MySQL ではなく Postgres を使ってる」  
**OMNI あり:** エージェントはすでに知っています。あなたが中断したところから続けます。

### 同じバグを二度直すとき
**OMNI なし:** 昨日すでに解決したはずのフレームワークの落とし穴に、記憶がないので再び当たります。  
**OMNI あり:** その修正はすでに保存済み。同じ失敗を繰り返す前に、MCP ツール `omni_recall` 経由で自分から引き出します。

### 複数 IDE をまたぐ作業 (Cursor から Claude Code へ)
**OMNI なし:** 新しい IDE、新しいエージェント、コンテキストはゼロ。振り出しに戻ります。  
**OMNI あり:** セッションの要約が自動で注入され、新しいエージェントはすぐに状況を把握します。

---

## なぜ重要なのか

AI に *送らない* コードは、送るコードと同じくらい重要です。

メガバイト単位のターミナルノイズを与えると、AI はコンテキスト肥大に陥り、見当違いの警告に対する修正を幻視し、API 予算を無関係な出力に費やします。

エージェントを再起動して記憶が空なら、自動的に保たれていたはずの文脈を作り直すのに何時間も失います。

OMNI はその両方を、表に出ずに解決します。

* **ノイズが減る**ことでコストが下がり、モデルがつまずく無関係な出力も減ります。
* **設計からフォーマット安全**: JSON、YAML、NDJSON、CSV はバイト単位でそのまま通り、入力を解析できない蒸留器は要約を捏造せず黙ります。
* **持続する記憶**: プロジェクトを説明し直す必要も、同じ修正を繰り返す必要もありません。
* **一度の導入**: すでに使っているあらゆるエージェントと、静かに連携します。

---

## ベンチマーク

正直な見出しの数字です。ある開発者の実際の利用から再生した **9,965 件の実コマンド
実行** に対し、リリースバイナリで実測しました。

* モデルに届くバイト数が、構成全体で **43.3% 減** (40.1 MB から 22.7 MB へ)。
* **そのうち 90.0% の呼び出しは、一切削減していません。** OMNI は出力をそのまま返し、
  **0** バイトも足していません。削減分はすべて、実際にノイズがあった残り 10.0% から
  来ています。
* **構造化出力には決して触れません。** JSON、YAML、NDJSON、CSV はバイト単位でそのまま
  通ります。壊れたペイロードは、逃した圧縮より高くつくからです。

2つ目の項目こそ、この種のツールがめったに出さない数字です。すべてのコマンドで 90%
削減すると謳うツールは、必要な出力まで要約したと告げているのと同じです。

<div align="center">
<img src="https://omni.weekndlabs.com/media/performance.png" alt="OMNI" width="600" />
</div>

同じ 9,965 件の実行で、削減が実際にどこから来ているか。

| コマンド | 呼び出し | 入力 | 出力 | 削減 |
|---------|-------|-------|--------|-------|
| `cargo` | 124 | 1.5 MB | 127 KB | **91.4%** |
| `git` | 931 | 12.0 MB | 1.3 MB | **89.2%** |
| `ls` | 62 | 264 KB | 176 KB | **33.6%** |
| `kubectl` | 456 | 5.5 MB | 1.3 MB | **76.5%** |
| `find` | 232 | 534 KB | 509 KB | **4.6%** |
| `grep` | 938 | 2.4 MB | 2.0 MB | **18.1%** |
| `cat` | 2,963 | 5.6 MB | 5.5 MB | **2.2%** |

結果を担っているのは `git` と `cargo` で、`cat` と `grep` はほぼ何もしません。OMNI は
ノイズが多く反復的なツール出力でこそ価値を持ち、それ以外では退きます。

手元で1つずつ再現したい場合の、`tests/fixtures/` の単体フィクスチャ。

| コマンド / 文脈 | 入力 | 出力 | 削減 |
|-------------------|-------|--------|-------|
| `cargo build` (大きく、成功) | 3,220 B | 87 B | **97.3%** |
| `cargo test` (490 成功、10 失敗) | 16,515 B | 1,178 B | **92.9%** |
| `pytest` (失敗あり) | 730 B | 136 B | **81.4%** |
| `git status` (変更あり) | 496 B | 190 B | **61.7%** |
| `git diff` (複数ファイル) | 397 B | 297 B | **25.2%** |
| `docker build` (ノイズ多め) | 9,207 B | 5,904 B | **35.9%** |
| `kubectl get pods` (混在) | 840 B | 840 B | **0%** |

**レイテンシは実在するコストで、ゼロではありません。** OMNI はフックされたすべての
コマンドで動き、その代価は履歴とともに増えます。496 バイトの `git status` は新しい
データベースに対して約 21 ms、205 MB のデータベースに対して約 61 ms。16.5 KB の
`cargo test` は約 25 ms かかります。見込んでおいてください。

*自分の実際のトークン削減を見るには、数日使ったあとに `omni stats` を実行してください。*


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

## 連携

OMNI は、あなたがすでに使っているエージェント系ツールとそのまま動きます。ターミナル実行を自動的に横取りします。

* Claude Code
* Cursor
* Windsurf
* Roo Code
* OpenAI Codex
* Antigravity CLI

---

## Adaptive Memory OS

OMNI は単なるターミナルフィルタではなく、AI の健忘症に対する治療です。

AI エージェントと1時間以上作業したことがあるなら、コンテキストが失われる痛みを知っているはずです。エージェントを再起動すると、何をしていたか突然忘れます。プロジェクトの目的を忘れます。リポジトリの文書化されていない癖を忘れて、昨日とまったく同じ間違いを始めます。

OMNI の Memory OS は、これを解くために背後で静かに動きます。

* **目的を説明し直さない (`omni goal`)**: 北極星となる目標を一度だけ設定します。OMNI はその優先事項を毎回のプロンプトで執拗に思い出させ、脱線を防ぎます。
* **思考の流れを失わない (セッション継続)**: Cursor が落ちても、Claude Code に移っても、OMNI は直前のセッションの圧縮された要約を即座に注入します。新しいエージェントは、どのファイルが熱かったか、最後に有効だったエラーが何かを正確に把握し、中断地点から再開します。
* **一度教えれば済む (`omni remember`)**: 同じ幻覚を直し続けるのはやめましょう。エージェントはプロジェクト固有のルール、落とし穴、アーキテクチャ上の判断を OMNI のローカル SQLite バックエンドに直接保存できます。後で行き詰まったとき、意味検索でその答えを自分から引き出します。

エージェントはあなたのコードベースについて日々賢くなり、あなたが同じ説明を繰り返すことは二度とありません。

---

## 仕組み

OMNI は完全にローカルで、決定的な `Read → Guard → Score → Collapse → Distill → Persist` パイプラインとして動作します。

```mermaid
flowchart LR
    Command[生のツール出力] --> Hook[OMNI フック]
    Hook --> Score[スコアラエンジン]
    Score -->|Critical=1.0, Noise=0.1| Distill[コンテンツ蒸留器]
    Distill --> Clean[クリーンなコンテキスト]
    Command --> SQLite[(RewindStore SQLite)]
```

AI が落としたノイズを *本当に* 必要とする場合、OMNI のローカル SQLite **RewindStore** が完全な未圧縮ログをハッシュ付きで安全に保持しており、エージェントはいつでも取り出せます。

---

## アーキテクチャ


<div align="center">
  <img src="../media/architecture.svg" alt="OMNI アーキテクチャ図" width="100%" />
</div>

Rust 製ですが、エンドツーエンドのコストはゼロではありません。

* **蒸留**: スコアリングと畳み込みのパイプライン自体は1桁ミリ秒で動きます。
* **エンドツーエンド**: 実際に待つのはそれに RewindStore への書き込みを足したもので、履歴とともに増えます。新しいデータベースで約 21 ms、205 MB のデータベースで約 61 ms です。無料だと考える前に [ベンチマーク](#ベンチマーク) を見てください。
* **メモリ**: 効率的なストリームで動作し、2万行のログでもメモリ使用量は平坦なままです。
* **フェイルオープン**: OMNI が panic した場合、静かに失敗して生の出力を通します。ホストのエージェントを落とすことは決してありません。

```bash
# 開発
cargo build --release
cargo test --all
make fmt && make clippy
```

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
