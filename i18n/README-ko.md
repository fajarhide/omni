<div align="center">
  <img src="../media/logo.png" alt="OMNI Logo" width="300" />

<h1>OMNI</h1>
<p align="center">
    <em>AI 에이전트를 위한 노이즈 제거 컨텍스트와 장기 기억. <b>손실이 있지만 언제나 복원 가능하며, 결과를 결코 지어내지 않습니다.</b> 터미널 노이즈 1만 줄을 읽히려고 Claude에 비용을 내는 일은 이제 그만두세요.</em>
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
실제 명령 구성에서 토큰 58.9% 감소 &middot; 세션 간 메모리 &middot; 포맷 안전 &middot; 항상 복원 가능 &middot; 페일 오픈, 지어내지 않음 &middot; 재현 가능한 숫자 </b>

</br></br>
<img src="../media/demo.gif" alt="시끄러운 cargo test 실행을 판정 결과까지 정제한 뒤 omni stats를 보여주는 OMNI" width="820" />
</div>

---

모든 AI 코딩 어시스턴트에는 두 가지 큰 문제가 있습니다.

**1. 전부 다 읽습니다.**  
빌드 로그.  
Docker 로그.  
CI 로그.  
진행 표시줄.  
ANSI 색상.  
한 줄을 찾자고 수천 개의 토큰을 씁니다. 비싼 건 Claude가 아니라 당신의 터미널입니다.

**2. 전부 다 잊습니다.**  
Cursor를 재시작할 때마다, 또는 Claude Code에서 Windsurf로 옮길 때마다 에이전트는 기억을 잃습니다. 프로젝트 목표를 다시 설명해야 하고, 똑같은 프레임워크 함정을 몇 번이고 다시 알려줘야 합니다.

OMNI는 둘 다 해결합니다.

---

## 무엇이 다른가

**문제 1: 터미널이 신호를 덮어버린다**

같은 `git log`를 나란히 놓고 봅니다. OMNI 없이는 커밋 하나의 `Author` / `Date` /
본문만으로 화면이 찹니다. OMNI를 쓰면 **모든 커밋이 남습니다.** `hash subject`
한 줄로, 94% 더 작게. 요약으로 사라진 것은 없고, 푸터의 숫자는 실제 바이트 수에서
측정한 것이지 약속이 아닙니다.

<table>
<tr>
<td align="center"><b>OMNI 없이</b><br/><sub>원본 <code>git log -15</code></sub></td>
<td align="center"><b>OMNI 사용</b><br/><sub>모든 커밋 유지, 94% 감소</sub></td>
</tr>
<tr>
<td valign="top"><img src="../media/demo-git-without.gif" alt="장황한 원본 git log -15. 커밋 하나의 Author, Date, 본문이 화면을 채운다" width="400" /></td>
<td valign="top"><img src="../media/demo-git-with.gif" alt="OMNI를 통과한 같은 git log -15. 각 커밋이 hash와 subject 한 줄로, 94% 더 작다" width="400" /></td>
</tr>
</table>

`tests/fixtures/`와 재생한 트레이스에서 실측한 숫자이며, 희망 사항이 아닙니다.

| 명령 | OMNI 없이 | OMNI 사용 | 절감 |
|---|---|---|---|
| `cargo test` (490 통과, 10 실패) | 테스트별 출력 16.5 KB | 러너 자신의 통과/실패 요약 | **93%** |
| `kubectl get pods` (pod 35개, 5개 크래시) | 전체 테이블 | `35 pods \| 30 running, 5 error` 와 실패한 pod 5개의 이름 | 잘라내지 않음 |
| `git diff` (여러 파일) | 락파일, 공백, 생성물 변경 | 실제로 바뀐 코드 | **45%** |
| `docker build` (캐시 노이즈 많음) | 레이어 해시와 진행 표시줄 9.2 KB | 빌드 결과, 캐시 히트는 접음 | **37%** |

> **솔직한 단서:** OMNI가 압축하는 것은 *성공했지만 시끄러운* 출력입니다. **실패한** 명령은 **그대로** 통과시킵니다. 숨겨진 오류는 압축되지 않은 오류보다 나쁘기 때문입니다. 구조화된 출력(JSON/YAML/CSV)은 절대 건드리지 않습니다. 반복적인 툴 잡음에서 제 몫을 하고, 그 밖에서는 비켜섭니다.

### 손실 있는 도구를 신뢰할 수 있는 이유

다른 압축 도구는 잘라낸 것이 중요하지 않았다고 *믿어달라*고 합니다. OMNI는 부탁하지 않고 보장하며, 각 보장은 당신이 읽을 수 있는 코드가 뒷받침합니다.

| 보장 | 방법 | 근거 |
|---|---|---|
| **원본을 바이트 단위로 되찾을 수 있음** | 잘라낸 것은 모두 로컬 SQLite **RewindStore**에 보관(SHA-256에서 내용으로). 에이전트는 해시를 받아 `omni_retrieve`를 호출 | [`동작 방식`](#동작-방식) |
| **결과를 결코 지어내지 않음** | 아무 신호도 파싱하지 못한 정제기는 초록색 `no errors`나 `passed`가 아니라 원본 출력을 반환 | [#143](https://github.com/fajarhide/omni/issues/143) |
| **실패를 결코 가리지 않음** | 종료 코드가 0이 아닌 명령은 그대로 통과 | [#120](https://github.com/fajarhide/omni/issues/120) |
| **구조화된 데이터는 건드리지 않음** | JSON / YAML / NDJSON / CSV는 바이트 단위로 그대로 통과 | `pipeline::format` |
| **숫자는 측정된 것이지 희망이 아님** | 릴리스 바이너리에서 실제 트레이스 1,810건 재생. 게다가 호출의 63.6%는 절약이 전혀 없었고 그 숫자도 함께 공개 | [`벤치마크`](#벤치마크) |

더 큰 압축률로는 살 수 없는 것이 바로 이것입니다. **원본은 언제나 복원할 수 있고, 에이전트에게 거짓말하지 않습니다.**

**문제 2: 에이전트는 하룻밤 사이에 모든 것을 잊는다**

### 새 세션을 시작할 때
**OMNI 없이:** "프로젝트 구조를 다시 설명해 주세요. auth 모듈이 망가졌고, MySQL이 아니라 Postgres를 씁니다."  
**OMNI 사용:** 에이전트는 이미 알고 있습니다. 당신이 멈춘 지점부터 이어갑니다.

### 같은 버그를 두 번 고칠 때
**OMNI 없이:** 어제 이미 풀었던 프레임워크 함정에 기억이 없어 다시 부딪힙니다.  
**OMNI 사용:** 그 해결책은 이미 저장돼 있습니다. 같은 실수를 반복하기 전에 MCP 도구 `omni_recall`로 스스로 꺼내옵니다.

### 여러 IDE를 오가는 작업 (Cursor에서 Claude Code로)
**OMNI 없이:** 새 IDE, 새 에이전트, 컨텍스트는 0. 처음부터 다시 시작합니다.  
**OMNI 사용:** 세션 요약이 자동으로 주입되어 새 에이전트가 곧바로 따라잡습니다.

---

## 왜 중요한가

AI에게 *보내지 않는* 코드는 보내는 코드만큼 중요합니다.

터미널 노이즈를 메가바이트 단위로 먹이면 AI는 컨텍스트 비대에 빠져, 엉뚱한 경고에 대한 수정을 환각하고 API 예산을 무관한 출력에 태웁니다.

에이전트를 재시작했는데 기억이 없으면, 자동으로 보존되었어야 할 맥락을 다시 세우느라 몇 시간을 잃습니다.

OMNI는 둘 다, 눈에 띄지 않게 해결합니다.

* **노이즈가 줄어** 비용이 내려가고, 모델이 걸려 넘어질 무관한 출력도 줄어듭니다.
* **설계부터 포맷 안전**: JSON, YAML, NDJSON, CSV는 바이트 단위로 그대로 통과하고, 입력을 파싱하지 못한 정제기는 요약을 지어내는 대신 조용히 있습니다.
* **지속되는 기억**: 프로젝트를 다시 설명할 필요도, 같은 수정을 반복할 필요도 없습니다.
* **한 번 설치**: 이미 쓰고 있는 모든 에이전트와 조용히 함께 동작합니다.

---

## 벤치마크

솔직한 헤드라인 숫자입니다. 한 개발자의 실제 사용에서 재생한 **1,810건의 실제 명령
실행**에 대해 릴리스 바이너리에서 측정했습니다.

* 모델에 도달하는 바이트가 전체 구성에서 **58.9% 감소** (15.0 MB에서 6.2 MB로).
* **그중 63.6%의 호출은 아무것도 절약하지 못했습니다.** OMNI는 출력을 그대로 돌려주며
  **0** 바이트도 더하지 않았습니다. 절감은 전부 실제로 노이즈가 있었던 나머지 36.4%에서
  나옵니다.
* **구조화된 출력은 결코 건드리지 않습니다.** JSON, YAML, NDJSON, CSV는 바이트 단위로
  그대로 통과합니다. 손상된 페이로드가 놓친 압축보다 비싸기 때문입니다.

두 번째 항목이야말로 이 부류의 도구가 좀처럼 찍지 않는 숫자입니다. 모든 명령에서
90%를 절약한다고 주장하는 도구는, 당신에게 필요한 출력까지 요약해 버렸다고 말하는
셈입니다.

<div align="center">
<img src="https://omni.weekndlabs.com/media/performance.png" alt="OMNI" width="600" />
</div>

같은 1,810건의 실행에서 절감이 실제로 어디서 오는지:

| 명령 | 호출 | 입력 | 출력 | 절감 |
|---------|-------|-------|--------|-------|
| `cargo` | 29 | 424 KB | 13 KB | **96.8%** |
| `git` | 256 | 5.9 MB | 509 KB | **91.3%** |
| `ls` | 52 | 71 KB | 29 KB | **59.5%** |
| `kubectl` | 212 | 4.4 MB | 2.3 MB | **48.0%** |
| `find` | 39 | 83 KB | 53 KB | **36.2%** |
| `grep` | 184 | 534 KB | 385 KB | **27.8%** |
| `cat` | 85 | 515 KB | 468 KB | **9.1%** |

결과를 떠받치는 것은 `git`과 `cargo`이고, `cat`과 `grep`은 거의 아무 일도 하지 않습니다.
OMNI는 시끄럽고 반복적인 툴 출력에서 자리를 얻고, 나머지에서는 비켜섭니다.

직접 하나씩 재현하고 싶다면 `tests/fixtures/`의 단일 픽스처:

| 명령 / 상황 | 입력 | 출력 | 절감 |
|-------------------|-------|--------|-------|
| `cargo build` (대규모, 성공) | 3,220 B | 9 B | **99.7%** |
| `cargo test` (490 통과, 10 실패) | 16.5 KB | 1,100 B | **93.3%** |
| `pytest` (실패 있음) | 730 B | 136 B | **81.4%** |
| `git status` (변경 있음) | 496 B | 113 B | **77.2%** |
| `git diff` (여러 파일) | 397 B | 220 B | **44.6%** |
| `docker build` (노이즈 많음) | 9.2 KB | 5.8 KB | **37.2%** |
| `kubectl get pods` (혼합) | 840 B | 762 B | **9.3%** |

**지연은 실재하는 비용이며 0이 아닙니다.** OMNI는 후킹된 모든 명령에서 동작하고, 그
대가는 기록과 함께 커집니다. 496바이트 `git status`는 새 데이터베이스에서 약 82 ms,
97 MB 데이터베이스에서 약 308 ms가 걸립니다. 16.5 KB `cargo test`는 약 276 ms입니다.
예산에 넣어두세요.

*자신의 실제 토큰 절감을 보려면 며칠 사용한 뒤 `omni stats`를 실행하세요.*


---

## 빠른 시작과 설치

OMNI는 설정이 대단히 쉽고, 터미널에 네이티브로 통합됩니다.

**macOS / Linux:**
```bash
# 1. Homebrew로 설치
brew install fajarhide/tap/omni

# 2. OMNI 설정 (Claude, VS Code, OpenCode, Codex, Antigravity용 대화형 메뉴)
omni init

# 3. 동작 확인
omni doctor

# 4. 문제가 있으면 자동 수정
omni doctor --fix

# 5. 현재 상태 확인
omni init --status
```

**범용 설치 스크립트 (macOS / Linux / WSL):**
```bash 
curl -fsSL omni.weekndlabs.com/install | bash
```

**Windows (PowerShell):**
```powershell
irm omni.weekndlabs.com/install.ps1 | iex
```

---

## 통합

OMNI는 당신이 이미 쓰고 있는 에이전트 도구들과 매끄럽게 동작하며, 그들의 터미널 실행을 자동으로 가로챕니다.

* Claude Code
* Cursor
* Windsurf
* Roo Code
* OpenAI Codex
* Antigravity CLI

---

## Adaptive Memory OS

OMNI는 단순한 터미널 필터가 아니라 AI 건망증의 치료제입니다.

AI 에이전트와 한 시간 넘게 일해 본 적이 있다면 컨텍스트 손실의 고통을 압니다. 에이전트를 재시작하면 무엇을 하고 있었는지 갑자기 잊습니다. 프로젝트 목표를 잊습니다. 저장소의 문서화되지 않은 특이점을 잊었기 때문에, 어제와 똑같은 실수를 다시 시작합니다.

OMNI의 Memory OS는 이를 해결하려고 배경에서 조용히 돌아갑니다.

* **목표를 다시 설명하지 않기 (`omni goal`)**: 북극성 목표를 한 번만 설정하세요. OMNI는 매 프롬프트마다 그 우선순위를 집요하게 상기시켜, 에이전트가 과제에서 벗어나지 않게 합니다.
* **생각의 흐름을 잃지 않기 (세션 연속성)**: Cursor가 죽거나 Claude Code로 옮겨도, OMNI는 직전 세션의 압축 요약을 즉시 주입합니다. 새 에이전트는 어떤 파일이 뜨거웠는지, 마지막 활성 오류가 무엇이었는지 정확히 알고 멈춘 자리에서 이어갑니다.
* **한 번만 가르치기 (`omni remember`)**: 같은 환각을 계속 고치지 마세요. 에이전트는 프로젝트별 규칙, 함정, 아키텍처 결정을 OMNI의 로컬 SQLite 백엔드에 직접 저장할 수 있습니다. 나중에 막히면 의미 검색으로 그 답을 스스로 꺼내옵니다.

에이전트는 매일 당신의 코드베이스에 대해 더 똑똑해지고, 당신은 두 번 다시 같은 말을 반복하지 않아도 됩니다.

---

## 동작 방식

OMNI는 전적으로 로컬에서, 결정적인 `Read → Guard → Score → Collapse → Distill → Persist` 파이프라인으로 동작합니다.

```mermaid
flowchart LR
    Command[원본 툴 출력] --> Hook[OMNI 훅]
    Hook --> Score[스코어러 엔진]
    Score -->|Critical=1.0, Noise=0.1| Distill[콘텐츠 정제기]
    Distill --> Clean[깨끗한 컨텍스트]
    Command --> SQLite[(RewindStore SQLite)]
```

AI가 버려진 노이즈를 *정말로* 필요로 하면, OMNI의 로컬 SQLite **RewindStore**가 압축되지 않은 전체 로그를 해시와 함께 안전하게 보관하고 있어 에이전트가 언제든 가져올 수 있습니다.

---

## 아키텍처


<div align="center">
  <img src="../media/architecture.svg" alt="OMNI 아키텍처 다이어그램" width="100%" />
</div>

Rust로 만들었지만, 엔드투엔드 비용은 0이 아닙니다.

* **정제**: 스코어링과 접기 파이프라인 자체는 한 자릿수 밀리초에 실행됩니다.
* **엔드투엔드**: 실제로 기다리는 것은 거기에 RewindStore 쓰기를 더한 값이며, 기록과 함께 커집니다. 새 데이터베이스에서 약 82 ms, 97 MB 데이터베이스에서 약 308 ms입니다. 공짜라고 가정하기 전에 [벤치마크](#벤치마크)를 보세요.
* **메모리**: 효율적인 스트림으로 동작해, 2만 줄 로그에서도 메모리 사용량이 평평하게 유지됩니다.
* **페일 오픈**: OMNI가 패닉하면 조용히 실패하고 원본 출력을 통과시킵니다. 호스트 에이전트를 죽이는 일은 결코 없습니다.

```bash
# 개발
cargo build --release
cargo test --all
make fmt && make clippy
```

---

## FAQ

**OMNI가 제 로그를 영구히 삭제하나요?**  
아닙니다. 원본 로그는 압축되어 로컬 SQLite RewindStore에 저장됩니다. AI는 해시를 받고, 필요하면 전체 로그를 가져올 수 있습니다.

**터미널이 느려지나요?**  
네, 측정 가능한 수준으로요. 그리고 비용은 기록과 함께 커집니다. 정제 파이프라인 자체는 한 자릿수 밀리초지만, 후킹된 모든 명령은 로컬 RewindStore에도 씁니다. 496바이트 `git status`는 새 데이터베이스에서 약 82 ms, 97 MB 데이터베이스에서 약 308 ms, 16.5 KB `cargo test`는 약 276 ms입니다. 예산에 넣어두세요. 원본 출력이 필요할 때는 `OMNI_PASSTHROUGH=1`로 파이프라인 전체를 건너뛸 수 있습니다.

**제 필터를 추가할 수 있나요?**  
가능합니다. 사내 도구 특유의 노이즈를 벗기는 법을 TOML로 OMNI에 가르칠 수 있습니다.
```toml
# ~/.omni/signals/custom.toml
[filters.my_tool]
match_command = "^internal-tool\\b"
strip_lines_matching = ["^DEBUG", "syncing..."]
```

## 기여와 라이선스

이것은 에이전트 AI 시대를 위해 만들어진, 애정에서 출발한 프로젝트입니다. 토큰 비용을 아끼러 오셨든, 무료 모델을 시험해 보러 오셨든, 최고의 에이전트 도구 벨트를 함께 만들러 오셨든, 기여는 언제나 환영합니다!

- **개발**: 소스에서 빌드하고 싶으신가요? `make ci`와 `cargo build`를 실행하세요. 자세한 내용은 [CONTRIBUTING.md](../CONTRIBUTING.md)를 보세요.
- **라이선스**: [MIT License](../LICENSE)

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
