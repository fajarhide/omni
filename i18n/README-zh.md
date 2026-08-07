<div align="center">
  <img src="../media/logo.png" alt="OMNI Logo" width="300" />

<h1>OMNI</h1>
<p align="center">
    <em><b>别再为让 Claude 读一万行终端噪音而付费。</b>OMNI 在你的智能体看到之前，把 <code>git</code> 砍掉 89%、<code>cargo</code> 砍掉 91%、<code>kubectl</code> 砍掉 77%。其余一切原样通过。没有任何东西会丢失，它也绝不编造结果。</em>
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
<code>git</code> 89% &middot; <code>cargo</code> 91% &middot; <code>kubectl</code> 77% &middot; 每条命令 21 ms &middot; 9,965 次调用中 0 次让输出变大 &middot; 每一处裁剪都可按字节还原 &middot; 跨会话记忆 </b>

</br></br>

```bash
brew install fajarhide/tap/omni && omni init
```

在 Claude Code、Codex CLI 与 Gemini CLI 上蒸馏命令输出，这些宿主会应用 OMNI 的重写。其他宿主仍可获得 MCP 服务器、共享会话状态，以及 `omni_run`：凡经它执行的命令都会被蒸馏。运行 `omni doctor` 查看每个宿主所处的层级。


### 每个宿主允许 OMNI 做什么

| 层级 | 宿主 | 你得到什么 |
|---|---|---|
| **Full** | Claude Code, Codex CLI, Gemini CLI, Aider (pipe) | 宿主会应用 OMNI 的重写，因此模型读取的是其内置工具的蒸馏输出。 |
| **Handoff-first** | Cursor, Windsurf | 宿主无法重写内置工具输出。`omni_run` 会蒸馏你经它执行的任何命令，`omni init --cursor` 会安装让代理主动选择它的规则。 |
| **MCP-only** | Cline, Roo, OpenCode, VS Code, Zed, Copilot, Antigravity, Hermes, Pi | 仅记忆、召回与会话状态。没有 shell 蒸馏，也不宣称有。 |

`omni doctor` 会为每个已安装宿主打印层级。只有模型确实收到更少内容时才计入节省。
</br>
<img src="../media/demo.gif" alt="OMNI 把嘈杂的 cargo test 蒸馏到只剩结论，随后展示 omni stats" width="820" />
</div>

---

你的智能体会读终端打印的每一行。构建日志、Docker 日志、CI 日志、进度条、ANSI 颜色。
为了找一行，烧掉几千 Token。贵的不是 Claude，是你的终端。

而且它一夜之间就忘光了。重启 Cursor，换到 Claude Code，你又得从头讲一遍项目。

OMNI 把两件事都解决掉，其余地方它让开。

---

## 差别在哪

**问题 1：终端淹没了信号**

同一条 `git log` 并排对比。没有 OMNI，一条提交的 `Author` / `Date` / 正文就填满
了屏幕。有了 OMNI，**每一条提交都还在**，压成一行 `hash subject`，体积小 94%。
没有任何东西被摘要掉；页脚的数字是按真实字节数量出来的，不是承诺出来的。

<table>
<tr>
<td align="center"><b>没有 OMNI</b><br/><sub>原始 <code>git log -15</code></sub></td>
<td align="center"><b>有 OMNI</b><br/><sub>保留每条提交，体积小 94%</sub></td>
</tr>
<tr>
<td valign="top"><img src="../media/demo-git-without.gif" alt="冗长的原始 git log -15，一条提交的 Author、Date 和正文就占满屏幕" width="400" /></td>
<td valign="top"><img src="../media/demo-git-with.gif" alt="同一条 git log -15 经过 OMNI：每条提交压成 hash 加 subject 一行，小 94%" width="400" /></td>
</tr>
</table>

真实数字，测自 `tests/fixtures/` 与回放的 trace，不是愿景：

| 命令 | 没有 OMNI | 有 OMNI | 节省 |
|---|---|---|---|
| `cargo test`（490 通过，10 失败） | 16.5 KB 逐条测试输出 | 运行器自己的通过/失败汇总 | **92.9%** |
| `git status`（有改动） | 496 B 的 porcelain 输出 | 分支与改动过的路径 | **61.7%** |
| `docker build`（缓存噪音很重） | 9.2 KB 的层哈希与进度条 | 构建结果，缓存命中折叠 | **35.9%** |
| `git diff`（多文件） | 锁文件、空白、生成物变动 | 真正改动的代码 | **25.2%** |
| `kubectl get pods`（35 个 pod，5 个崩溃） | 整张表 | 整张表 | **0%**，刻意如此 |

上面每个数字都是**真正交付**的载荷，含 OMNI 每次丢弃内容时附上的约 77 字节还原标记。
早先的版本引用的是加标记之前的蒸馏输出，那会让小载荷显得更好看：`git diff` 在这里读作
25.2%，不含标记则是 44.6%。正是这个标记让裁剪可以还原，所以它该算进数字里。

有意思的是 `kubectl get pods` 这一行。它以前报 9.3%，现在什么也不报，因为 pod 表是一种
每行都是数据的枚举，没有噪音可砍。丢掉那 9.3% 才是修复本身。

> **它刻意什么都不做的地方。** 失败的命令原样放行，因为被藏起来的错误比没压缩的错误代价更高。结构化输出（JSON、YAML、CSV）从不触碰，因为你流水线的下一步要去解析它。OMNI 在重复的工具絮语上赚回自己的位置，在别处让开，这正是它可以对你运行的每一条命令都保持开启的原因。

### 没有任何东西会丢失。它也绝不编造。

两个承诺，两个都在代码里，而不在这段话里。

**没有任何东西会丢失。** OMNI 砍掉的每一个字节都以 SHA-256 为键归档在本地 RewindStore 里。智能体拿到蒸馏输出的同时也拿到一个哈希，随时可以调用 `omni_retrieve`，在对话中途把原文按字节取回，无需重跑你的命令。

**它也绝不编造。** 在输入里什么都没认出来的蒸馏器，返回的是原始输入。这是类型，不是约定：`distill` 返回 `Option<String>`，路由层每次拿到 `None` 就回退到原文。不存在任何一条代码路径，能产出一句 OMNI 没读过的绿色 "no errors"。

别的压缩工具要你*相信*它砍掉的东西不重要。OMNI 把凭据交到你手里：

| 保证 | 怎么做到 | 依据 |
|---|---|---|
| **原文可按字节取回** | 砍掉的一切都归档在本地 SQLite **RewindStore**（SHA-256 到内容）；智能体拿到哈希并调用 `omni_retrieve` | [`工作原理`](#工作原理) |
| **绝不编造结果** | 没能解析出任何信号的蒸馏器返回原始输出，而不是一句绿色的 `no errors` 或 `passed` | [#143](https://github.com/fajarhide/omni/issues/143) |
| **失败绝不被掩盖** | 退出码非零的命令原样放行 | [#120](https://github.com/fajarhide/omni/issues/120) |
| **结构化数据绝不触碰** | JSON / YAML / NDJSON / CSV 按字节原样通过 | `pipeline::format` |
| **数字是测出来的，不是喊出来的** | 在发布版二进制上回放 9,965 条真实 trace，而且 90.0% 的调用一点没省，这个数字我们同样公开 | [`基准测试`](#基准测试) |

这正是更大的压缩率买不到的东西：**你永远能拿回原文，而它永远不会对你的智能体撒谎。**

---

## 基准测试

在发布版二进制上，回放某位开发者真实使用中的 **9,965 次真实命令执行**测得：

* **在真正产生噪音的命令上，76% 到 91%。** `cargo` 91.4%，`git` 89.2%，
  `kubectl` 76.5%。你的上下文预算就是花在那里的，OMNI 也就在那里干活。
* **OMNI 只对十条命令里的一条动手，对另外九条一个字节都不加。** 它是过滤器，不是
  摘要器。没有可砍的东西时它彻底让开。
* **9,965 次调用里，没有一次让输出变大。**
* 把嘈杂和安静的命令算在一起，整个组合上**字节减少 43.3%**。
* **每条命令 21 ms** 端到端，随你的历史增长而不是随载荷大小增长；对着 205 MB 的
  数据库是 61 ms。

<div align="center">
<img src="https://omni.weekndlabs.com/media/performance.png" alt="OMNI" width="600" />
</div>

完整语料、按命令拆分、fixture 和延迟表在
**[docs/BENCHMARKS.md](../docs/BENCHMARKS.md)**。复现命令：
`cargo test --release --test bench_replay -- --ignored`。

### 怎么读一个节省数字，包括我们的

这个类别里每个工具都会公布一个百分比。下面五个问题决定它有没有意义，以及我们的答案：

| 问题 | 为什么要紧 | OMNI |
|---|---|---|
| 有多大比例的调用**一点没省**？ | 一个对每条命令都能省的工具，是在摘要你需要的输出 | **90.0%**，我们公布 |
| 有没有调用让输出**变大**？ | 标记和表头都要占字节，但没人报告适得其反的那些 | **9,965 次里 0 次** |
| 测的是哪个**样本**？ | 把没有模型读过的终端字节算进去，数字白涨 | 只算到达模型的部分，说实话让我们少了 36 个点 |
| 你能**重跑**吗？ | 复现不出来的数字是主张，不是测量 | 一条命令，在你自己的数据上 |
| 砍掉的能**还原**吗？ | 有损没问题，只要可逆；不可逆就致命 | 按字节还原，通过 `omni_retrieve` |

我们之所以公布什么都没做的那部分调用占比，是因为正是这个数字告诉你其余数字值多少。

---

## 快速上手与安装

OMNI 极易配置，原生集成进你的终端。

**macOS / Linux:**
```bash
# 1. 通过 Homebrew 安装
brew install fajarhide/tap/omni

# 2. 配置 OMNI（面向 Claude、VS Code、OpenCode、Codex、Antigravity 的交互菜单）
omni init

# 3. 确认已生效
omni doctor

# 4. 或自动修复问题
omni doctor --fix

# 5. 查看当前状态
omni init --status
```

**通用安装脚本（macOS / Linux / WSL）:**
```bash 
curl -fsSL omni.weekndlabs.com/install | bash
```

**Windows (PowerShell):**
```powershell
irm omni.weekndlabs.com/install.ps1 | iex
```

---

---

## 常见问题

**OMNI 会永久删除我的日志吗？**  
不会。原始日志被压缩后存在本地的 SQLite RewindStore 里。AI 拿到一个哈希，需要时可以取回完整日志。

**这会让我的终端变慢吗？**  
会，而且是可测量的，代价还随历史增长。蒸馏流水线本身是个位数毫秒，但每条被挂钩的命令还要写一次本地 RewindStore：496 字节的 `git status` 对着全新数据库约 21 ms，对着 205 MB 的数据库约 61 ms，16.5 KB 的 `cargo test` 约 25 ms。请算进预算。需要拿回原始输出时，`OMNI_PASSTHROUGH=1` 会完全跳过流水线。

**我能加自己的过滤器吗？**  
能。你可以用 TOML 教 OMNI 剥掉你们内部工具特有的噪音：
```toml
# ~/.omni/signals/custom.toml
[filters.my_tool]
match_command = "^internal-tool\\b"
strip_lines_matching = ["^DEBUG", "syncing..."]
```

**怎么看我自己的节省？**
用几天之后跑 `omni stats`。`omni stats --share` 会把同一批数字打印成方便复制的形式。

---

## 了解更多

* [工作原理与代价](../docs/ARCHITECTURE.md)：流水线、RewindStore、Memory OS
* [完整基准测试](../docs/BENCHMARKS.md)：语料、按命令、fixture、延迟
* [参与贡献](../CONTRIBUTING.md)：跑通 `make ci` 就可以了

---

```bash
brew install fajarhide/tap/omni && omni init
```

## 贡献与许可

这是一个为智能体 AI 时代而生的兴趣项目。无论你是来省 Token 费用、试用免费模型，还是来一起打造终极的智能体工具带，我们都欢迎贡献！

- **开发**：想从源码构建？运行 `make ci` 和 `cargo build`。细节见 [CONTRIBUTING.md](../CONTRIBUTING.md)。
- **许可**：[MIT License](../LICENSE)

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
