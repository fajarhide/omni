<div align="center">
  <img src="../media/logo.png" alt="OMNI Logo" width="300" />

<h1>OMNI</h1>
<p align="center">
    <em><b>터미널 노이즈 1만 줄을 읽히려고 Claude에 비용을 내는 일은 이제 그만두세요.</b> OMNI는 에이전트가 보기 전에 <code>git</code>을 89%, <code>cargo</code>를 91%, <code>kubectl</code>을 77% 잘라냅니다. 나머지는 그대로 통과합니다. 잃어버리는 것은 없고, 결과를 지어내지도 않습니다.</em>
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
<code>git</code> 89% &middot; <code>cargo</code> 91% &middot; <code>kubectl</code> 77% &middot; 명령당 21 ms &middot; 9,965건 중 출력이 커진 호출 0건 &middot; 잘라낸 모든 것을 바이트 단위로 복원 &middot; 세션 간 메모리 </b>

</br></br>

```bash
brew install fajarhide/tap/omni && omni init
```

Claude Code, Codex CLI, Gemini CLI에서 명령 출력을 증류합니다. 이 호스트들이 OMNI의 재작성을 적용하기 때문입니다. 다른 호스트에서도 MCP 서버, 공유 세션 상태, 그리고 통과시킨 명령을 증류하는 `omni_run`을 사용할 수 있습니다. 각 호스트의 티어는 `omni doctor`로 확인하세요.


### 각 호스트가 OMNI에 허용하는 것

| 티어 | 호스트 | 무엇을 얻는가 |
|---|---|---|
| **Full** | Claude Code, Codex CLI, Gemini CLI, Aider (pipe) | 호스트가 OMNI의 재작성을 적용하므로 모델은 내장 도구의 증류된 출력을 읽습니다. |
| **Handoff-first** | Cursor, Windsurf | 호스트가 내장 도구 출력을 재작성할 수 없습니다. `omni_run`으로 실행한 명령은 증류되며, `omni init --cursor`가 에이전트로 하여금 그것을 선택하게 하는 규칙을 설치합니다. |
| **MCP-only** | Cline, Roo, OpenCode, VS Code, Zed, Copilot, Antigravity, Hermes, Pi | 메모리, 리콜, 세션 상태만. 셸 증류는 없으며 있다고 주장하지도 않습니다. |

`omni doctor`가 설치된 호스트마다 티어를 출력합니다. 절감은 모델이 실제로 더 적게 받았을 때만 집계됩니다.

Codex CLI의 Full 티어는 0.144.6에서 확인되지 않았습니다. OMNI가 설치한 훅이 실행된 정황이 없고 `codex_cli` 행도 기록되지 않았습니다. [#359](https://github.com/fajarhide/omni/issues/359)에서 추적 중입니다.
</br>
<img src="../media/demo.gif" alt="시끄러운 cargo test 실행을 판정 결과까지 정제한 뒤 omni stats를 보여주는 OMNI" width="820" />
</div>

---

에이전트는 터미널이 찍는 모든 줄을 읽습니다. 빌드 로그, Docker 로그, CI 로그,
진행 표시줄, ANSI 색상. 한 줄을 찾으려고 수천 토큰을 씁니다. 비싼 것은 Claude가 아니라
당신의 터미널입니다.

그리고 하룻밤 사이에 그것을 전부 잊습니다. Cursor를 다시 켜고 Claude Code로 옮기면
프로젝트 설명을 처음부터 다시 해야 합니다.

OMNI는 둘 다 고치고, 그 밖에서는 비켜섭니다.

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
| `cargo test` (490 통과, 10 실패) | 테스트별 출력 16.5 KB | 러너 자신의 통과/실패 요약 | **92.9%** |
| `git status` (변경 있음) | porcelain 출력 496 B | 브랜치와 변경된 경로 | **61.7%** |
| `docker build` (캐시 노이즈 많음) | 레이어 해시와 진행 표시줄 9.2 KB | 빌드 결과, 캐시 히트는 접음 | **35.9%** |
| `git diff` (여러 파일) | 락파일, 공백, 생성물 변경 | 실제로 바뀐 코드 | **25.2%** |
| `kubectl get pods` (pod 35개, 5개 크래시) | 전체 테이블 | 전체 테이블 | 의도된 **0%** |

위의 모든 수치는 실제로 **전달된** 페이로드이며, OMNI가 무언가를 버릴 때마다 붙이는
약 77바이트의 복원 마커를 포함합니다. 이전 릴리스는 그 마커를 붙이기 전의 정제기
출력을 인용했고, 그래서 작은 페이로드가 실제보다 좋아 보였습니다. `git diff`는 여기서
25.2%, 마커가 없으면 44.6%입니다. 잘라낸 것을 되돌릴 수 있게 만드는 것이 바로 그
마커이므로 숫자에 포함되는 것이 맞습니다.

흥미로운 줄은 `kubectl get pods`입니다. 예전에는 9.3%를 보고했지만 지금은 아무것도
보고하지 않습니다. pod 테이블은 모든 줄이 데이터인 열거이고, 버릴 노이즈가 없기
때문입니다. 그 9.3%를 잃은 것이 바로 수정이었습니다.

> **의도적으로 아무것도 하지 않는 곳.** 실패한 명령은 그대로 통과시킵니다. 숨겨진 오류가 압축되지 않은 오류보다 비싸기 때문입니다. 구조화된 출력(JSON, YAML, CSV)은 절대 건드리지 않습니다. 파이프라인의 다음 단계가 그것을 파싱할 테니까요. OMNI는 반복적인 툴 잡음에서 제 몫을 하고 그 밖에서는 비켜서며, 그래서 실행하는 모든 명령에 켜둔 채로 두어도 안전합니다.

### 잃어버리는 것은 없습니다. 지어내지도 않습니다.

두 가지 약속이고, 둘 다 이 문단이 아니라 코드 안에 있습니다.

**잃어버리는 것은 없습니다.** OMNI가 잘라낸 모든 바이트는 SHA-256을 키로 로컬 RewindStore에 보관됩니다. 에이전트는 정제된 출력과 함께 해시를 받고, `omni_retrieve`를 호출해 대화 도중에 명령을 다시 실행하지 않고도 원본을 바이트 단위로 되찾을 수 있습니다.

**지어내지도 않습니다.** 입력에서 아무것도 인식하지 못한 정제기는 원본 입력을 그대로 반환합니다. 관례가 아니라 타입입니다. `distill`은 `Option<String>`을 반환하고, 라우팅 계층은 `None`을 받을 때마다 원본으로 되돌아갑니다. OMNI가 읽지 않은 초록색 "no errors"를 만들어내는 코드 경로는 없습니다.

다른 압축 도구는 잘라낸 것이 중요하지 않았다고 *믿어달라*고 합니다. OMNI는 영수증을 건넵니다.

| 보장 | 방법 | 근거 |
|---|---|---|
| **원본을 바이트 단위로 되찾을 수 있음** | 잘라낸 것은 모두 로컬 SQLite **RewindStore**에 보관(SHA-256에서 내용으로). 에이전트는 해시를 받아 `omni_retrieve`를 호출 | [`동작 방식`](#동작-방식) |
| **결과를 결코 지어내지 않음** | 아무 신호도 파싱하지 못한 정제기는 초록색 `no errors`나 `passed`가 아니라 원본 출력을 반환 | [#143](https://github.com/fajarhide/omni/issues/143) |
| **실패를 결코 가리지 않음** | 종료 코드가 0이 아닌 명령은 그대로 통과 | [#120](https://github.com/fajarhide/omni/issues/120) |
| **구조화된 데이터는 건드리지 않음** | JSON / YAML / NDJSON / CSV는 바이트 단위로 그대로 통과 | `pipeline::format` |
| **숫자는 측정된 것이지 희망이 아님** | 릴리스 바이너리에서 실제 트레이스 9,965건 재생. 게다가 호출의 90.0%는 절약이 전혀 없었고 그 숫자도 함께 공개 | [`벤치마크`](#벤치마크) |

더 큰 압축률로는 살 수 없는 것이 바로 이것입니다. **원본은 언제나 복원할 수 있고, 에이전트에게 거짓말하지 않습니다.**

---

## 벤치마크

한 개발자의 실제 사용에서 재생한 **9,965건의 실제 명령 실행**에 대해 릴리스
바이너리에서 측정했습니다.

* **실제로 노이즈를 만드는 명령에서는 76에서 91%.** `cargo` 91.4%, `git` 89.2%,
  `kubectl` 76.5%. 당신의 컨텍스트 예산이 사라지는 곳이 거기이고, OMNI가 일하는 곳도
  거기입니다.
* **OMNI는 10개 명령 중 1개에만 손을 대고, 나머지 9개에는 0바이트를 더합니다.**
  요약기가 아니라 필터입니다. 자를 것이 없으면 완전히 비켜섭니다.
* **9,965건 중 출력을 더 크게 만든 호출은 하나도 없습니다.**
* 시끄러운 명령과 조용한 명령을 합친 전체 구성에서 **바이트 43.3% 감소**.
* **명령당 21 ms** 엔드투엔드. 페이로드 크기가 아니라 기록과 함께 커지며, 205 MB
  데이터베이스에서는 61 ms입니다.

<div align="center">
<img src="https://omni.weekndlabs.com/media/performance.png" alt="OMNI" width="600" />
</div>

전체 코퍼스, 명령별 내역, 픽스처와 지연 표는
**[docs/BENCHMARKS.md](../docs/BENCHMARKS.md)** 에 있습니다. 재현은
`cargo test --release --test bench_replay -- --ignored`.

### 절감 수치를 읽는 법, 우리 수치까지 포함해서

이 부류의 도구는 모두 백분율 하나를 공개합니다. 그 숫자가 의미가 있는지 가르는 다섯
가지 질문과 우리의 답입니다.

| 질문 | 왜 중요한가 | OMNI |
|---|---|---|
| **아무것도 절약하지 못한** 호출의 비율은? | 모든 명령에서 절약하는 도구는 당신에게 필요한 출력을 요약하고 있는 것 | **90.0%**, 공개합니다 |
| 출력을 **더 크게** 만든 호출이 있었나? | 마커와 헤더는 바이트를 먹지만, 역효과가 난 쪽을 아무도 보고하지 않는다 | **9,965건 중 0건** |
| 어떤 **모집단**을 쟀나? | 어떤 모델도 읽지 않는 터미널 바이트를 세면 숫자가 공짜로 부푼다 | 모델에 도달한 것만. 그렇게 말하느라 36포인트를 잃었습니다 |
| **다시 돌려볼** 수 있나? | 재현할 수 없는 숫자는 측정이 아니라 주장 | 명령 하나로, 당신의 데이터에서 |
| 잘라낸 것을 **복원**할 수 있나? | 손실은 되돌릴 수 있으면 괜찮고, 없으면 치명적 | 바이트 단위로, `omni_retrieve`로 |

아무것도 하지 않은 호출의 비율을 공개하는 것은, 그것이 나머지 숫자의 값어치를 알려주는
숫자이기 때문입니다.

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

---

## FAQ

**OMNI가 제 로그를 영구히 삭제하나요?**  
아닙니다. 원본 로그는 압축되어 로컬 SQLite RewindStore에 저장됩니다. AI는 해시를 받고, 필요하면 전체 로그를 가져올 수 있습니다.

**터미널이 느려지나요?**  
네, 측정 가능한 수준으로요. 그리고 비용은 기록과 함께 커집니다. 정제 파이프라인 자체는 한 자릿수 밀리초지만, 후킹된 모든 명령은 로컬 RewindStore에도 씁니다. 496바이트 `git status`는 새 데이터베이스에서 약 21 ms, 205 MB 데이터베이스에서 약 61 ms, 16.5 KB `cargo test`는 약 25 ms입니다. 예산에 넣어두세요. 원본 출력이 필요할 때는 `OMNI_PASSTHROUGH=1`로 파이프라인 전체를 건너뛸 수 있습니다.

**제 필터를 추가할 수 있나요?**  
가능합니다. 사내 도구 특유의 노이즈를 벗기는 법을 TOML로 OMNI에 가르칠 수 있습니다.
```toml
# ~/.omni/signals/custom.toml
[filters.my_tool]
match_command = "^internal-tool\\b"
strip_lines_matching = ["^DEBUG", "syncing..."]
```

**내 절감량은 어떻게 보나요?**
며칠 쓴 뒤 `omni stats`. `omni stats --share`는 같은 수치를 복사하기 좋은 형태로
출력합니다.

---

## 더 알아보기

* [동작 방식과 비용](../docs/ARCHITECTURE.md): 파이프라인, RewindStore, Memory OS
* [벤치마크 전문](../docs/BENCHMARKS.md): 코퍼스, 명령별, 픽스처, 지연
* [기여하기](../CONTRIBUTING.md): `make ci`가 통과하면 준비 끝

---

```bash
brew install fajarhide/tap/omni && omni init
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
