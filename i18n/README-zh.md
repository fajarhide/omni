<div align="center">
  <img src="../media/hero.svg" alt="OMNI" width="800" />
  
  **AI智能体的上下文操作系统。减少噪音，增加信号。将Token消耗降低高达90%。**

  [🇺🇸 English](../README.md) | [🇯🇵 日本語](README-ja.md) | [🇨🇳 简体中文](README-zh.md) | [🇸🇦 العربية](README-ar.md) | [🇮🇩 Bahasa Indonesia](README-id.md) | [🇻🇳 Tiếng Việt](README-vi.md) | [🇰🇷 한국어](README-ko.md)

  [![CI](https://github.com/fajarhide/omni/actions/workflows/ci.yml/badge.svg)](https://github.com/fajarhide/omni/actions/workflows/ci.yml)
  [![Release](https://img.shields.io/github/v/release/fajarhide/omni)](https://github.com/fajarhide/omni/releases)
  [![Rust](https://img.shields.io/badge/built_with-Rust-dca282.svg)](https://www.rust-lang.org/)
  [![MCP](https://img.shields.io/badge/MCP-compatible-green.svg?style=flat-square)](https://modelcontextprotocol.io/)
  [![License: MIT](https://img.shields.io/github/license/fajarhide/omni)](https://github.com/fajarhide/omni/blob/main/LICENSE)
  [![Hits](https://hits.sh/github.com/fajarhide/omni.svg)](https://hits.sh/github.com/fajarhide/omni/)
</div>

<br/>

> **OMNI** 是**专为自治 AI 智能体设计的上下文操作系统 (Context OS)**。
> 它作为终端和 LLM 之间的高性能语义过滤器运行。通过智能提取嘈杂的日志、缓存状态并管理 Token 预算，OMNI 确保您的智能体保持专注、减少幻觉并完美执行循环——同时**将您的 API 成本降低高达 90%**。
> 
> *停止为终端噪音付费。开始用纯净的信号进行构建。*
---

## 目录
- [问题：上下文膨胀、昂贵的Token与嘈杂的输出](#问题上下文膨胀昂贵的token与嘈杂的输出)
- [解决方案：Omni](#解决方案omni)
- [理念](#理念)
- [真实场景应用](#真实场景应用)
- [性能与基准测试](#性能与基准测试)
- [功能解析](#功能解析)
- [幕后：Omni 是如何工作的](#幕后omni-是如何工作的)
- [架构](#架构)
- [快速入门与安装](#快速入门与安装)
- [如何使用](#如何使用)
  - [多智能体支持与集成](#多智能体支持与集成)
  - [文档索引](#文档索引)
- [与 Heimsense 结合使用效果更好](#与-heimsense-结合使用效果更好)
- [贡献与许可证](#贡献与许可证)

---

## 问题：昂贵的 Token、幻觉和无限循环

当您在终端中运行自治 AI 智能体（如 Claude Code、Cursor 或 Aider）时，它们会读取*所有内容*。一个简单的 `npm install` 或 `cargo test` 命令就能轻易地将 10,000 到 25,000 个无用的终端噪音 Token 倾倒进您 AI 的上下文窗口中。

这会导致致命的失败：
1. **烧钱的预算**：您要为垃圾输出的每一个 Token 支付真金白银。
2. **智能体“失忆”与幻觉**：核心错误被淹没在数兆字节的加载条和依赖警告下。AI 变得困惑，丢失了最初的目标，并针对错误的问题产生修复幻觉。
3. **模型锁定**：您被迫使用最昂贵的旗舰模型，仅仅是为了获得足够大的上下文窗口来处理这些臃肿的数据。
4. **脆弱的循环**：由于智能体缺乏对 Token 限制和上下文压力的感知，自治循环很容易崩溃。

## 解决方案：OMNI Context OS

OMNI 是 Agentic AI 的终极透明中间件。

它动态拦截终端命令，消除噪音，并为您的 AI 提供高度浓缩的语义摘要。**结果如何？** 您可以在价格合理的模型上运行您的智能体，为其提供*零噪音*，并看着它立即解决复杂的编码任务。

无论您是运行快速的 MCP 工具调用，还是协调庞大的多智能体 Maker-Checker 循环，OMNI 都能提供您的 AI 成功所需的持久内存、预算跟踪和事实护栏。

上下文既昂贵又嘈杂。OMNI 解决这个问题。

---

## 理念

OMNI 的构建不仅仅是为了“削减上下文”或“节省 token”——这些只是令人高兴的副作用。OMNI 背后的真正理念是 **上下文质量**。

像 Claude 这样的 AI 智能体只有在您提供给它们的上下文时才聪明。当您用几兆字节的依赖项日志或加载条淹没它们时，您迫使它们在垃圾中筛选以寻找实际问题。这削弱了它们的推理能力，并导致退化或无用的响应。

**OMNI 的目标是为您的 AI 提供纯净、高密度的信号。** 这意味着只抓取对 Claude 真正重要和有意义的上下文。我们清理 AI 不需要噪音，这意味着：
1. 您使用的 token 自动大幅减少。
2. AI 响应的**质量显著提高**，因为它的上下文窗口激光般聚焦于实际问题。

**试用一周。** 体验 AI 推理的质量和速度的差异，当它被喂食纯净的信号而不是原始的终端噪音时。

---

## 真实场景应用

OMNI 旨在解决 Agentic AI 开发者的日常挫折。以下是 OMNI 如何改变您的工作流程：

1. **单一代码库中的“无限死亡循环”**
   - **场景**: 您要求 Claude 在大型 monorepo 中运行 `npm install` 和 `npm run build`。它输出 20,000 行依赖警告，末尾带有一个小构建错误。AI 被警告分散了注意力，试图修复不相关的依赖问题，烧光了您的 token，让您陷入无限循环。
   - **OMNI的修复**: OMNI 拦截构建。它完全使数百个 `peer dependency` 警告静音，只在堆栈跟踪旁边显示确切的 `Build Error: Cannot find module 'X'`。AI 看到一个 50 token 的输出，并立即修复了代码。

2. **大文件上的“静默幻觉”**
   - **场景**: AI 想了解项目并运行 `cat src/utils.ts`。文件有 3,000 行长。AI 努力将其全部保留在工作记忆中，并开始对函数签名产生幻觉。
   - **OMNI的修复**: OMNI 拦截原始的 `cat`，并用 **结构化大纲 (Structured Outline)** 替换它。它向 AI 显示导入、公共 API（函数名和类型）和风险标记，将输出减少 80%。OMNI 然后警告 AI：`"该文件有 12 个依赖项 — 使用 omni_context 获取完整的影响图。"` 引导 AI 进行更安全、基于事实的编辑。

3. **多智能体协作**
   - **场景**: 您使用 Cursor IDE 进行快速编辑，使用 Claude Code CLI 进行繁重的工作。它们都需要知道发生了什么，而无需运行多余的命令并浪费 token。
   - **OMNI的修复**: OMNI 充当共享内存层。使用 `omni_agents` 及其本地 SQLite `Store`，Cursor 和 Claude 共享相同的过滤内存流、活动错误和执行环境。它们协作而不会发生冲突。

---

## 性能与基准测试
<div align="center">
<img src="https://omni.weekndlabs.com/media/performance.png" alt="OMNI" width="600" />
</div>

诚实的数字：在发布二进制文件上，针对从一位开发者真实使用中重放的 **1,810 次真实命令执行**测量所得。

* 抵达模型的字节**减少 58.9%**（15.0 MB → 6.2 MB）。
* **其中 63.6% 的调用完全没有节省。** OMNI 原样交还输出，**不增加一个字节**。全部节省都来自另外 36.4%——那里确实有噪音可去。
* **结构化输出从不触碰。** JSON、YAML、NDJSON 和 CSV 逐字节通过，因为损坏的载荷比错过一次压缩代价更大。

第二条才是同类工具很少印出来的数字。一个宣称对每条命令都节省 90% 的工具，等于在告诉你：你需要的输出也被摘要掉了。

节省究竟来自哪里，基于同样的 1,810 次执行：

| 命令 | 调用次数 | 输入 | 输出 | 节省 |
|------|----------|------|------|------|
| `cargo` | 29 | 424 KB | 13 KB | **96.8%** |
| `git` | 256 | 5.9 MB | 509 KB | **91.3%** |
| `ls` | 52 | 71 KB | 29 KB | **59.5%** |
| `kubectl` | 212 | 4.4 MB | 2.3 MB | **48.0%** |
| `find` | 39 | 83 KB | 53 KB | **36.2%** |
| `grep` | 184 | 534 KB | 385 KB | **27.8%** |
| `cat` | 85 | 515 KB | 468 KB | **9.1%** |

结果由 `git` 和 `cargo` 撑起；`cat` 和 `grep` 近乎无效。OMNI 在嘈杂、重复的工具输出上体现价值，其余场合则让路。

来自 `tests/fixtures/` 的单个固定样本，可自行复现：

| 命令 / 上下文 | 输入 | 输出 | 节省 |
|-------------------|------|------|------|
| `cargo build` (大型，成功) | 3,220 B | 9 B | **99.7%** |
| `cargo test` (490 通过，10 失败) | 16.5 KB | 1,100 B | **93.3%** |
| `pytest` (含失败) | 730 B | 136 B | **81.4%** |
| `git status` (脏) | 496 B | 113 B | **77.2%** |
| `git diff` (多文件) | 397 B | 220 B | **44.6%** |
| `docker build` (重噪音) | 9.2 KB | 5.8 KB | **37.2%** |
| `kubectl get pods` (混合) | 840 B | 762 B | **9.3%** |

**延迟是真实成本，不是零。** OMNI 在每条被挂钩的命令上运行，代价随你的历史增长：496 字节的 `git status` 对空数据库约 82 ms，对 97 MB 的数据库约 308 ms；16.5 KB 的 `cargo test` 约 276 ms。请把它算进预算。

*要查看您自己的实际 token 节省，只需在使用几天后运行 `omni stats`。*

---

## 功能解析

### 核心提炼引擎 (Core Distillation Engine)
- **告别AI困惑**: Omni像一个智能过滤器。如果测试失败，它*仅*向AI显示特定的错误行和堆栈跟踪，屏蔽嘈杂的依赖日志。
- **90% Token 减少**: 通过消除无用的终端噪音，您可瞬间大幅削减Agent API账单。
- **自适应压缩 (Adaptive Compression)**: OMNI会跟踪Agent何时检索被省略的输出。如果某个命令频繁被检索，OMNI会在下次自动放宽压缩（无需配置的自动调整）。
- **智能高速旁路**: 为确保小任务的零延迟，OMNI自动对低于2000 token阈值的输出绕过提炼。

### 上下文安全与事实防护 (Context Safety)
- **零信息丢失**: 担心Omni过滤了重要信息？别担心。Omni将原始输出保存在本地(`RewindStore`)。AI可以使用`omni_retrieve`自动请求它。
- **反幻觉事实守卫**: OMNI仅在有确凿事实时发出警告。如果输出被严重压缩，或者文件有大量依赖，OMNI会注入系统警告，让AI立足于现实。
- **省略可见性**: OMNI明确标记输出中被删除的内容（例如 `[OMNI: omitted X lines of noise]`），为AI提供完美的态势感知。

### 多智能体与工作区智能
- **多智能体协作**: 通过`omni_agents`完全感知其环境。如果您同时运行Cursor和Claude CLI，它们可以无缝共享相同的过滤记忆流和错误，而不会发生冲突。
- **会话智能**: OMNI会记住您正在做的事情。它知道您正在编辑哪些文件，并停止向AI提供多余的上下文。
- **结构化的 ReadFile + Grep**: OMNI不再提供原始文件转储，而是返回结构化大纲（导入、公共API）和分组的grep摘要。
- **轻量级依赖图**: OMNI在执行时构建快速的本地文件关系图。如果您的AI读取了一个被大量导入的文件，OMNI会向其警告影响范围图。

### 上下文保真度与会话恢复 (Context Fidelity & Session Recovery)
- **记忆痕迹 (自动子任务摘要)**：OMNI 会自动检测子任务何时完成（例如，解决编译器错误、提交代码或修复损坏的测试）。它创建一个高度压缩的快照（“Engram”），而不会在 LLM 调用上浪费 token，因此您的智能体在长时间会话期间永远不会遭受“上下文健忘症”。
- **智能上下文压缩 (Smart Context Compaction)**：当您的上下文窗口变满时，OMNI 不会盲目修剪 token。它使用优先级感知算法首先打包最重要的数据（固定文件 > 活动错误 > 记忆痕迹 > 工具活动 > 热点文件），节省大量开销。
- **会话交接 (Session Handoffs)**：从 Claude Code 切换到 Cursor？使用 `omni_handoff` 工具立即将当前会话的内存（热点文件、最近的命令、活动错误）导出到您的新智能体可以立即吸收的便携式 Markdown 摘要中。

### 自动化循环工程 (Autonomous Loop Engineering)
- **循环操作系统的上下文管理**：OMNI 为迭代的自主循环智能体管理上下文。通过环境变量 (`OMNI_LOOP_BUDGET`, `OMNI_LOOP_GOAL`)，OMNI 强制执行自适应蒸馏限制和持久跟踪。
- **检查者-执行者验证模式 (Maker-Checker Pattern)**：通过将执行（执行者/Maker 智能体）与验证（检查者/Checker 智能体）分离，将任务扩展得井井有条，并通过 OMNI 的多智能体会议存储安全地交换上下文状态。
- **基于目标的预测限制**：蒸馏根据任务目标自动缩放——如果目标包含“debug”，OMNI 会保留更多错误上下文。如果目标是“refactor”，OMNI 会积极压缩代码足迹。

### 监控与调试
- **会话健康仪表板**: 运行 `omni session --health` 获得一个精美的可视化仪表板，显示您的上下文压力、活动记忆痕迹、滚动工具活动和 token 节省情况。
- **提炼监视器**: 在LLM内部使用`omni_budget`和`omni_history`跟踪token节省情况，或在本地运行`omni stats`。
- **视觉对比 (`omni diff`)**: 运行`omni diff`以并排比较笨重的原始输出和Omni精简过滤后的版本。
- **调试直通**: 需要原始输出？设置`OMNI_PASSTHROUGH=1`可完全绕过引擎并查看原始输出的每个字符。

---

## 幕后：Omni 是如何工作的

OMNI 不仅仅是一个正则表达式脚本；它是一个用 Rust 编写的高性能 **语义信号引擎**。但是，它是如何在大约 100 毫秒内将 token 消耗降低 90% 的呢？

当您的 AI 智能体输入像 `cargo test` 这样的命令时，在 OMNI 代码库内部发生的事情是这样的：

1. **拦截 (`src/hooks` & `src/main.rs`)**: 当 AI 击中“Enter”的那一刻，OMNI 拦截了执行。`main.rs` 动态检测上下文（无论是管道、钩子还是 MCP 调用）。`hooks` 模块无缝包装了命令，允许 OMNI 捕获原始终端输出作为高速数据流，而不会降低实际执行的速度。
2. **流式管道 (`src/pipeline`)**: OMNI 不是等待命令完成并将兆字节的文本转储到内存中，而是使用内存高效的流式管道逐行处理输出。这确保了即使命令吐出 10,000 行日志，OMNI 的内存占用也几乎保持平坦。
3. **语义大脑 (`src/distillers` & `src/guard`)**: 随着文本的流入，它通过 Distillers。在声明性 TOML 规则（`signals/`）的驱动下，蒸馏器分析输出的语义含义。
   - 这是一个加载转轮吗？*丢弃它。*
   - 这是一个包含 500 个通过测试的列表吗？*丢弃它。*
   - 这是 panic 堆栈跟踪吗？**保留它。**
   同时，`guard` 模块确保保留事实，保证 OMNI 永远不会静默更改关键的诊断信息。
4. **安全网 (`src/store`)**: 如果 AI 实际上需要查看那 500 个通过的测试怎么办？OMNI 遵循严格的“零信息丢失”策略。在丢弃任何噪音之前，原始未编辑的输出都安全地隐藏在一个本地、快速的 SQLite 数据库（`Store`）中。OMNI 在 AI 的上下文中留下了一个小面包屑：`[OMNI: omitted 1,200 lines of noise. Use omni_retrieve to view]`。
5. **多智能体接口 (`src/mcp` & `src/session`)**: 最后，提取的、高信号输出返回给 AI。在幕后，`session` 管理器跟踪当前的 token 预算，而 `mcp`（模型上下文协议）服务器随时待命。如果 AI 想要查询历史错误、获取省略的原始日志或检查依赖关系图（`src/graph`），MCP 工具提供即时的结构化访问。

**结果:** 臃肿的 `25,000` token 终端转储变成了一份简明的 `400` token 错误报告。AI 瞬间明白问题所在，而您节省了真金白银。

---

## 架构

<div align="center">
  <img src="../media/architecture.svg" alt="OMNI Architecture Diagram" width="100%" />
</div>

## 快速入门与安装

Omni 的设置非常简单。它原生集成到您的终端中。

**macOS / Linux:**
```bash
# 1. 通过 Homebrew 安装
brew install fajarhide/tap/omni

# 2. 设置 Omni（针对 Claude, VS Code, OpenCode, Codex, Antigravity 的交互式菜单）
omni init

# 3. 验证它是否正常工作
omni doctor

# 4. 或自动修复任何问题
omni doctor --fix

# 5. 检查当前状态
omni init --status
```

**通用安装程序 (macOS / Linux / WSL):**
```bash 
curl -fsSL omni.weekndlabs.com/install | bash
```

**Windows (PowerShell):**
```powershell
irm omni.weekndlabs.com/install.ps1 | iex
```

---

## 如何使用

通过 `omni init` 安装后，OMNI 在后台隐形工作。无论是您的 AI 智能体通过 MCP 运行终端命令，还是您手动通过管道输出（`ls | omni`），OMNI 都会自动作为透明层跳入。它智能过滤终端输出，删除嘈杂日志，并将干净的信号交还给 AI。

有关按节省、命令、时段和路由的详细细分：
```bash
omni stats
```

诊断您的 OMNI 安装（钩子、MCP、过滤器、数据库）：
```bash
omni doctor
```

需要查看过滤器的实际操作或添加您自己的自定义规则吗？
您可以使用 `~/.omni/signals/` 中的简单 TOML 文件轻松创建自己的规则。

### 多智能体支持与集成

默认情况下，`omni init --claude` 自动挂钩到 **Claude Code**。然而，OMNI 通过其内置集成与任何 Agentic AI 完美配合！运行 `omni init` 查看交互式菜单。

1. **VS Code & Continue.dev**: 使用我们的 MCP 上下文提供程序（`integrations/continue-dev/`）。
2. **OpenCode & Codex CLI**: 内置包装器自动将命令输出通过管道传送到 OMNI。
3. **Antigravity IDE**: OMNI 在 Antigravity 的配置中注册为原生 MCP 服务器（`~/.gemini/antigravity/mcp_config.json`）。运行 `omni init --antigravity` 自动设置。
4. **Pi Agent**: Pi 的原生 OMNI 包。运行 `omni init --pi` 通过 Pi 的包安装程序安装 OMNI Pi 包。

**多智能体微调 (`~/.omni/config.toml`)**
不同的智能体有不同的痛点。保持 VS Code 聊天干净，同时让 OpenCode 读取更多数据。单独微调它们：
```toml
[global]
aggressiveness = "balanced"

[agents.vscode_continue]
aggressiveness = "aggressive"
enable_readfile_distillation = true

[agents.opencode]
aggressiveness = "conservative"
enable_readfile_distillation = false
```

### 文档索引

**对于用户:**
- [终极指南 (HOW_TO_USE.md)](../docs/HOW_TO_USE.md) — 您需要的一切：安装、自定义 TOML 过滤器和 CLI 命令。
- [OpenClaw 集成](https://clawhub.ai/fajarhide/omni-signal-engine) — 用于原生 OMNI 蒸馏的官方 OpenClaw 插件。
- [Hermes Agent 集成](https://github.com/wysie/hermes-omni-plugin) — 用于原生 OMNI 蒸馏的社区 Hermes Agent 插件。

**对于开发者和系统集成商:**
- [循环工程指南](../docs/LOOP_ENGINEERING.md) — 如何将 OMNI 与自主智能体集成。
- [开发指南](../docs/DEVELOPMENT.md) — 如何构建和为 OMNI 代码库做出贡献。
- [测试架构](../docs/TESTING.md) — 质量保证和上下文安全。
- [会话连续性](../docs/SESSION.md) — 深入了解 OMNI 的工作记忆。
- [路线图](../docs/ROADMAP.md) — 当前开发状态和即将推出的功能。
- [迁移指南](../docs/MIGRATION.md) — 关于从 Node/Zig 升级到 Rust 版本的说明。

---

## 与 Heimsense 结合使用效果更好

Omni 是我个人 AI 工具带的一部分。如果您使用 `claude-code`，我强烈建议将 Omni 与我的另一个项目 **[Heimsense](https://github.com/fajarhide/heimsense)** 配对。

Heimsense 解锁了像 `claude-code` 这样受限的环境，可以使用 *任何* 免费或与 OpenAI 兼容的模型运行，而不是强迫您使用昂贵的 Anthropic 模型。
**Omni + Heimsense** = 使用负担得起的模型运行世界级智能体框架，实现零噪音和精确定位准确性。

---

## 贡献与许可证

这是一个为 Agentic AI 时代构建的热情项目。无论您是来这里节省 token 资金、测试免费模型，还是帮助构建终极 agentic 工具带，都随时欢迎您的贡献！

- **开发**: 想要从源代码构建？运行 `make ci` 和 `cargo build`。阅读我们的 [CONTRIBUTING.md](../CONTRIBUTING.md) 了解详细信息。
- **许可证**: [MIT License](../LICENSE)

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

Dibuat dengan ❤️ oleh [Fajar Hidayat](https://github.com/fajarhide)
