<div align="center">
  <img src="../media/hero.svg" alt="OMNI" width="800" />
  
  **AI 에이전트를 위한 컨텍스트 운영 체제. 노이즈는 줄이고 신호는 늘립니다. 토큰 소비를 최대 90%까지 줄이세요.**

  [🇺🇸 English](../README.md) | [🇯🇵 日本語](README-ja.md) | [🇨🇳 简体中文](README-zh.md) | [🇸🇦 العربية](README-ar.md) | [🇮🇩 Bahasa Indonesia](README-id.md) | [🇻🇳 Tiếng Việt](README-vi.md) | [🇰🇷 한국어](README-ko.md)

  [![CI](https://github.com/fajarhide/omni/actions/workflows/ci.yml/badge.svg)](https://github.com/fajarhide/omni/actions/workflows/ci.yml)
  [![Release](https://img.shields.io/github/v/release/fajarhide/omni)](https://github.com/fajarhide/omni/releases)
  [![Rust](https://img.shields.io/badge/built_with-Rust-dca282.svg)](https://www.rust-lang.org/)
  [![MCP](https://img.shields.io/badge/MCP-compatible-green.svg?style=flat-square)](https://modelcontextprotocol.io/)
  [![License: MIT](https://img.shields.io/github/license/fajarhide/omni)](https://github.com/fajarhide/omni/blob/main/LICENSE)
  [![Hits](https://hits.sh/github.com/fajarhide/omni.svg)](https://hits.sh/github.com/fajarhide/omni/)
</div>

<br/>

> **OMNI**는 터미널 출력이 AI 에이전트에 도달하기 전에 지능적으로 가로채고, 분석하고, 추출하는 고성능 **시맨틱 신호 엔진** 및 **컨텍스트 운영 체제**입니다. 셸과 AI 사이의 투명한 신호 최적화 계층 역할을 하여 모델로 전송되는 모든 토큰이 가치가 높고 관련성이 있으며 노이즈가 없는지 확인합니다. 시끄러운 출력으로 인해 AI가 혼란스러워하는 것을 방지함으로써 정확한 답변을 더 빨리 얻는 동시에 막대한 토큰 비용을 절약할 수 있습니다.
> 
> *완벽하게 투명합니다. 여러분이 항상 통제권을 쥐고 있습니다.*
---

## 목차
- [문제: 부풀려진 컨텍스트, 비싼 토큰 및 노이즈 출력](#문제-부풀려진-컨텍스트-비싼-토큰-및-노이즈-출력)
- [해결책: Omni](#해결책-omni)
- [철학](#철학)
- [실제 사용 사례](#실제-사용-사례)
- [성능 및 벤치마크](#성능-및-벤치마크)
- [기능 설명](#기능-설명)
- [내부 구조: Omni 작동 방식](#내부-구조-omni-작동-방식)
- [아키텍처](#아키텍처)
- [빠른 시작 및 설치](#빠른-시작-및-설치)
- [사용 방법](#사용-방법)
  - [다중 에이전트 지원 및 통합](#다중-에이전트-지원-및-통합)
  - [문서 색인](#문서-색인)
- [Heimsense와 함께 사용하면 더 좋습니다](#heimsense와-함께-사용하면-더-좋습니다)
- [기여 및 라이선스](#기여-및-라이선스)

---

## 문제: 부풀려진 컨텍스트, 비싼 토큰 및 노이즈 출력

터미널에서 자율 AI 에이전트(예: Claude Code 또는 Cursor)를 사용하면 *모든 것*을 읽습니다. `git diff`, `npm install` 또는 `cargo test`와 같은 간단한 명령은 쓸모없는 터미널 노이즈의 10,000~25,000 토큰을 AI 컨텍스트에 쉽게 쏟아낼 수 있습니다.

이는 세 가지 큰 문제를 일으킵니다.
1. **매우 비쌉니다**: 그 정크 출력의 모든 단일 토큰에 대해 실제 돈을 지불합니다.
2. **AI를 "바보"로 만듭니다**: 치명적인 오류가 메가바이트 단위의 경고 로그와 로딩 표시줄 아래에 묻혀 AI를 혼란스럽게 하고 추론 능력을 떨어뜨립니다.
3. **모델 종속성**: 고급 에이전트 프레임워크는 모든 노이즈를 처리할 수 있을 만큼 큰 컨텍스트 창을 확보하기 위해 가장 비싼 플래그십 모델을 사용하도록 강요합니다.
4. **토큰을 인식하지 못하는 실행**: 에이전트는 토큰 비용과 출력을 인식하지 못하여 불필요한 소비로 이어집니다.
5. **컨텍스트 팽창**: 터미널 출력의 양이 AI의 컨텍스트를 어지럽혀 집중력과 정확성을 떨어뜨립니다.

## 해결책: Omni

저는 제 워크플로에서 매일 AI 에이전트를 효율적이고 저렴하게 실행하고 싶었기 때문에 Omni를 구축했습니다.

**Omni는 터미널과 AI 사이의 완벽한 필터 역할을 합니다.**

**결과는요?** 최고급 프레임워크에서 AI 에이전트를 실행하고 *노이즈 제로*를 제공할 수 있습니다. AI에는 고도로 집중되고 핵심을 찌르는 컨텍스트만 제공되기 때문에, 합리적인 가격의 모델이나 일반 모델도 쓰레기 데이터로 인해 주의가 산만해지지 않으므로 비싼 플래그십 모델과 동등한 성능을 발휘합니다.

제 궁극적인 열정은 이것으로 수익을 창출하는 것이 아니라 Agentic AI 시대를 위한 최고의 오픈 소스 도구 벨트를 구축하는 것입니다. 토큰 비용을 적극적으로 절감함으로써 저는 오늘 강력하고 비용 효율적인 소프트웨어를 개발할 수 있으며 여러분도 할 수 있습니다.

컨텍스트는 비싸고 시끄러우며 Omni는 이를 수정하기 위해 존재합니다. 컨텍스트를 최적화함으로써 Omni는 AI 에이전트를 더 효율적이고 비용 효율적이며 사용하기 쉽게 만듭니다. 이는 AI 에이전트로 전송되는 컨텍스트의 양을 줄임으로써 수행되며, 이는 결과적으로 응답을 생성하는 데 필요한 처리 시간과 메모리 양을 줄입니다.

---

## 철학

OMNI는 단순히 "컨텍스트를 줄이거나" "토큰을 절약"하기 위해 구축된 것이 아닙니다. 이는 그저 행복한 부작용일 뿐입니다. OMNI 이면의 진정한 철학은 **컨텍스트 품질**입니다.

Claude와 같은 AI 에이전트는 제공하는 컨텍스트만큼만 똑똑합니다. 메가바이트에 달하는 종속성 로그나 로딩 바를 쏟아부으면 실제 문제를 찾기 위해 쓰레기 더미를 뒤져야 합니다. 이는 추론 능력을 떨어뜨리고 품질이 저하되거나 도움이 되지 않는 응답으로 이어집니다.

**OMNI의 목표는 AI에 순수하고 밀도가 높은 신호를 제공하는 것입니다.** 즉, Claude에게 실제로 중요하고 의미 있는 컨텍스트만 가져옵니다. AI에 필요하지 않은 노이즈를 정리합니다. 즉,
1. 사용하는 토큰이 자동으로 대폭 줄어듭니다.
2. 컨텍스트 창이 실제 문제에 집중되어 있으므로 AI 응답의 **품질이 훨씬 높아집니다**.

**일주일 동안 사용해 보세요.** 원시 터미널 노이즈 대신 순수한 신호를 제공할 때 AI의 추론 품질과 속도의 차이를 느껴보세요.

---

## 실제 사용 사례

OMNI는 Agentic AI 개발자의 일상적인 불만을 해결하도록 설계되었습니다. OMNI가 워크플로를 어떻게 변화시키는지 확인해 보세요.

1. **모노레포의 "무한 죽음의 루프"**
   - **시나리오**: Claude에게 대규모 모노레포에서 `npm install` 및 `npm run build`를 실행하도록 요청합니다. 터미널은 20,000줄의 종속성 경고를 출력하고 끝에 작은 빌드 오류를 출력합니다. AI는 경고에 주의를 빼앗겨 관련 없는 종속성 문제를 해결하려고 시도하여 토큰을 소모하고 무한 루프에 빠뜨립니다.
   - **OMNI의 수정 사항**: OMNI가 빌드를 가로챕니다. 수백 개의 `peer dependency` 경고를 완전히 음소거하고 스택 추적과 함께 정확한 `Build Error: Cannot find module 'X'`만 표시합니다. AI는 50토큰 출력을 보고 코드를 즉시 수정합니다.

2. **대용량 파일의 "조용한 환각"**
   - **시나리오**: AI가 프로젝트를 이해하기 위해 `cat src/utils.ts`를 실행합니다. 이 파일은 3,000줄입니다. AI는 이를 모두 작업 메모리에 보관하는 데 어려움을 겪고 함수 서명에 대해 환각을 느끼기 시작합니다.
   - **OMNI의 수정 사항**: OMNI는 원시 `cat`을 차단하고 **구조화된 개요(Structured Outline)** 로 바꿉니다. AI에게 가져오기, 공개 API(함수 이름 및 유형), 위험 마커를 표시하여 출력을 80% 줄입니다. 그런 다음 OMNI는 AI에게 경고합니다. `"이 파일에는 12개의 종속성이 있습니다. 전체 영향 맵을 보려면 omni_context를 사용하세요."` AI는 더 안전하고 사실적인 편집을 하도록 안내됩니다.

3. **다중 에이전트 협업**
   - **시나리오**: 빠른 편집에는 Cursor IDE를 사용하고 무거운 작업에는 Claude Code CLI를 사용합니다. 두 사람 모두 중복 명령을 실행하고 토큰을 낭비하지 않고 무슨 일이 일어나고 있는지 알아야 합니다.
   - **OMNI의 수정 사항**: OMNI는 공유 메모리 계층 역할을 합니다. `omni_agents`와 로컬 SQLite `Store`를 사용하여 Cursor와 Claude는 동일하게 필터링된 메모리 스트림, 활성 오류 및 실행 환경을 공유합니다. 충돌 없이 협업합니다.

---

## 성능 및 벤치마크
<div align="center">
<img src="https://omni.weekndlabs.com/media/performance.png" alt="OMNI" width="600" />
</div>

OMNI는 제로 오버헤드 실행과 무자비한 효율성을 위해 Rust로 구축되었습니다. 다음은 릴리스 바이너리에서 측정한 실제 벤치마크입니다.

| 명령 / 컨텍스트 | 입력 크기 | 출력 크기 | 토큰 절약 | AI에 미치는 영향 |
|-------------------|------------|-------------|---------------|--------------|
| `docker build` (다단계) | 9.2 KB | 49 bytes | **99.5%** | 캐싱 노이즈를 제거합니다. AI가 실제 빌드 오류를 즉시 확인합니다. |
| `cargo test` (대규모 제품군) | 16.5 KB | 4.3 KB | **78.0%** | 수백 개의 "ok" 테스트를 제거합니다. AI는 실패와 스택 추적에만 집중합니다. |
| `git status` (더티) | 496 bytes | 113 bytes | **77.2%** | 깨끗한 파일과 힌트를 제거합니다. 수정되거나 추적되지 않은 파일만 유지합니다. |
| `kubectl get pods` | 840 bytes | 762 bytes | **10.0%** | CrashLoopBackOff/Error 포드를 선택적으로 표시하고 정상 포드는 건너뜁니다. |
| `git diff` (다중 파일) | 397 bytes | 220 bytes | **50.0%** | 변경 사항이 있는 헝크를 유지하고 과도한 컨텍스트 줄을 삭제합니다. |

- **파이프라인 지연 시간**: **< 100ms** (바이너리 시작을 포함한 종단 간)
- **역대 절약액**: 평균 개발 세션 전체에서 **97.3%** 토큰 감소.
- **ROI**: 개발자/월별 **$35+ USD** 절약 (플래그십 모델 기준 측정).

*실제 토큰 절약액을 확인하려면 며칠 사용 후 `omni stats`를 실행하세요.*

---

## 기능 설명

### 핵심 증류 엔진 (Core Distillation Engine)
- **AI의 혼란 방지**: Omni는 스마트 필터 역할을 합니다. 테스트가 실패하면 불필요한 의존성 로그를 차단하고 특정 오류 줄과 스택 트레이스*만* AI에 표시합니다.
- **90% 토큰 감소**: 쓸모없는 터미널 노이즈를 제거하여 API 비용을 즉시 대폭 절감합니다.
- **적응형 압축 (Adaptive Compression)**: OMNI는 에이전트가 생략된 출력을 언제 검색하는지 추적합니다. 특정 명령이 자주 검색되면 OMNI는 다음에 자동으로 압축을 완화합니다.
- **스마트 고속 우회**: 작은 작업에서 지연 시간을 0으로 유지하기 위해 OMNI는 2000토큰 임계값 미만의 출력에 대해 증류를 자동으로 우회합니다.

### 컨텍스트 안전 및 사실 보호 (Context Safety)
- **정보 손실 제로**: 중요한 정보가 필터링될까 걱정되시나요? Omni는 원본 출력을 로컬(`RewindStore`)에 저장합니다. AI는 `omni_retrieve`를 사용하여 자동으로 요청할 수 있습니다.
- **환각 방지 팩트 가드**: OMNI는 확실한 사실이 있을 때만 경고를 보냅니다. 출력이 크게 압축되었거나 파일에 의존성이 많은 경우, 시스템 경고를 주입하여 AI가 현실을 파악하도록 돕습니다.
- **생략 가시성**: OMNI는 출력에서 제거된 콘텐츠(예: `[OMNI: omitted X lines of noise]`)를 명시적으로 표시하여 AI 에이전트에게 완벽한 상황 인식을 제공합니다.

### 다중 에이전트 및 작업 공간 인텔리전스
- **다중 에이전트 협업**: Cursor와 Claude CLI를 함께 실행하는 경우, 충돌 없이 동일하게 필터링된 메모리 스트림과 활성 오류를 원활하게 공유합니다.
- **세션 인텔리전스**: 작업 중인 파일을 기억하고 중복된 컨텍스트를 AI에 공급하는 것을 중지합니다.
- **구조화된 ReadFile + Grep**: 원시 파일 덤프 대신 구조화된 개요(가져오기, 공용 API)와 그룹화된 grep 요약(우선순위 줄 먼저)을 반환합니다.
- **경량 의존성 그래프**: OMNI는 후크 시간에 빠른 로컬 파일 관계 그래프를 구축합니다. AI가 많이 가져온 파일을 읽으면 OMNI가 영향을 경고합니다.

### 컨텍스트 충실도 및 세션 복구 (Context Fidelity & Session Recovery)
- **엔그램 (자동 하위 작업 요약)**: OMNI는 하위 작업(예: 컴파일러 오류 해결, 코드 커밋 또는 실패한 테스트 수정)이 완료되는 시점을 자동으로 감지합니다. LLM 호출에서 토큰을 낭비하지 않고 고도로 압축된 스냅샷("엔그램")을 생성하므로 에이전트가 긴 세션 동안 "컨텍스트 건망증"을 겪지 않습니다.
- **스마트 컨텍스트 압축 (Smart Context Compaction)**: 컨텍스트 창이 꽉 차면 OMNI는 맹목적으로 토큰을 다듬지 않습니다. 우선 순위를 인식하는 알고리즘을 사용하여 가장 중요한 데이터(고정된 파일 > 활성 오류 > 엔그램 > 도구 활동 > 핫 파일)를 먼저 압축하여 엄청난 오버헤드를 절약합니다.
- **세션 핸드오프 (Session Handoffs)**: Claude Code에서 Cursor로 전환하시나요? `omni_handoff` 도구를 사용하여 현재 세션의 메모리(핫 파일, 최근 명령, 활성 오류)를 새 에이전트가 즉시 흡수할 수 있는 이식 가능한 마크다운 요약으로 즉시 내보냅니다.

### 자율 루프 엔지니어링 (Autonomous Loop Engineering)
- **루프를 위한 컨텍스트 OS**: OMNI는 반복적인 자율 루프 에이전트의 컨텍스트를 관리합니다. 환경 변수(`OMNI_LOOP_BUDGET`, `OMNI_LOOP_GOAL`)를 통해 OMNI는 적응형 증류 제한과 영구 추적을 강제합니다.
- **메이커-체커 검증 패턴**: 작업의 실행(메이커/Maker 에이전트)과 검증(체커/Checker 에이전트)을 분리하고 OMNI의 멀티 에이전트 세션 저장소를 통해 안전하게 컨텍스트 상태를 교환함으로써 작업을 효율적으로 확장합니다.
- **예측 가능한 목표 기반 제한**: 증류는 작업 목표에 따라 자동으로 확장됩니다. 목표에 "debug"가 포함된 경우 OMNI는 더 많은 오류 컨텍스트를 유지합니다. "refactor"인 경우 OMNI는 코드 트레이스를 공격적으로 압축합니다.

### 모니터링 및 디버깅
- **세션 상태 대시보드 (Session Health Dashboard)**: `omni session --health`를 실행하면 컨텍스트 압력, 활성 엔그램, 롤링 도구 활동 및 토큰 절감액을 보여주는 아름다운 시각적 대시보드가 ​​표시됩니다.
- **증류 모니터**: LLM 내부에서 `omni_budget` 및 `omni_history`를 사용하거나 로컬에서 `omni stats`를 실행하여 비용 절감액을 시각화합니다.
- **시각적 효과 (`omni diff`)**: `omni diff`를 실행하여 방대한 원시 출력과 Omni의 세련된 필터링 버전을 나란히 비교해 보세요.
- **디버그 통과**: 원시 출력이 필요한 경우 환경에서 `OMNI_PASSTHROUGH=1`을 설정하면 엔진을 완전히 우회하고 원본을 볼 수 있습니다.

---

## 내부 구조: Omni 작동 방식

OMNI는 단순한 정규식 스크립트가 아닙니다. Rust로 작성된 고성능 **시맨틱 신호 엔진**입니다. 하지만 어떻게 100ms 미만으로 토큰 소비의 90%를 줄일 수 있을까요?

AI 에이전트가 `cargo test`와 같은 명령을 입력할 때 OMNI 코드베이스 내부에서 일어나는 일은 다음과 같습니다.

1. **가로채기 (`src/hooks` & `src/main.rs`)**: AI가 "Enter"를 누르는 순간 OMNI는 실행을 가로챕니다. `main.rs`는 컨텍스트(파이프, 후크 또는 MCP 호출인지 여부)를 동적으로 감지합니다. `hooks` 모듈은 명령을 매끄럽게 감싸 OMNI가 실제 실행을 늦추지 않고 원시 터미널 출력을 고속 데이터 스트림으로 캡처할 수 있도록 합니다.
2. **스트리밍 파이프라인 (`src/pipeline`)**: 명령이 완료될 때까지 기다렸다가 메가바이트의 텍스트를 메모리에 덤프하는 대신 OMNI는 메모리 효율적인 스트리밍 파이프라인을 사용하여 출력을 한 줄씩 처리합니다. 이를 통해 명령이 10,000줄의 로그를 내뱉더라도 OMNI의 메모리 공간은 거의 평평하게 유지됩니다.
3. **시맨틱 두뇌 (`src/distillers` & `src/guard`)**: 텍스트가 스트리밍되면 Distillers를 통과합니다. 선언적 TOML 규칙(`signals/`)을 기반으로 하는 증류기는 출력의 의미론적 의미를 분석합니다.
   - 이것은 로딩 스피너입니까? *버리세요.*
   - 합격한 500개 테스트 목록입니까? *버리세요.*
   - 패닉 스택 추적입니까? **보관하세요.**
   한편, `guard` 모듈은 사실이 보존되도록 하여 OMNI가 중요한 진단 정보를 조용히 변경하지 않도록 보장합니다.
4. **안전망 (`src/store`)**: AI가 실제로 통과한 500개의 테스트를 봐야 한다면 어떻게 될까요? OMNI는 엄격한 "정보 손실 제로" 정책을 따릅니다. 노이즈가 폐기되기 전에 편집되지 않은 원시 출력이 로컬의 매우 빠른 SQLite 데이터베이스(`Store`)에 안전하게 저장됩니다. OMNI는 AI 컨텍스트에 작은 빵 부스러기를 남깁니다. `[OMNI: omitted 1,200 lines of noise. Use omni_retrieve to view]`
5. **다중 에이전트 인터페이스 (`src/mcp` & `src/session`)**: 마지막으로, 추출된 높은 신호 출력이 AI에 반환됩니다. 무대 뒤에서 `session` 관리자는 현재 토큰 예산을 추적하고 `mcp`(모델 컨텍스트 프로토콜) 서버가 준비되어 있습니다. AI가 역사적 오류를 쿼리하거나, 생략된 원시 로그를 가져오거나, 종속성 그래프(`src/graph`)를 확인하려는 경우 MCP 도구는 즉각적이고 구조화된 액세스를 제공합니다.

**결과:** 부풀려진 `25,000` 토큰 터미널 덤프가 간결한 `400` 토큰 오류 보고서가 됩니다. AI는 문제를 즉시 이해하고 여러분은 실제 돈을 절약할 수 있습니다.

---

## 아키텍처

<div align="center">
  <img src="../media/architecture.svg" alt="OMNI Architecture Diagram" width="100%" />
</div>

## 빠른 시작 및 설치

Omni는 설정하기가 매우 쉽습니다. 터미널에 기본적으로 통합됩니다.

**macOS / Linux:**
```bash
# 1. Homebrew를 통해 설치
brew install fajarhide/tap/omni

# 2. Omni 설정 (Claude, VS Code, OpenCode, Codex, Antigravity용 대화형 메뉴)
omni init

# 3. 작동하는지 확인
omni doctor

# 4. 또는 문제 자동 수정
omni doctor --fix

# 5. 현재 상태 확인
omni init --status
```

**범용 설치 프로그램(macOS / Linux / WSL):**
```bash 
curl -fsSL omni.weekndlabs.com/install | bash
```

**Windows (PowerShell):**
```powershell
irm omni.weekndlabs.com/install.ps1 | iex
```

---

## 사용 방법

`omni init`을 통해 설치되면 OMNI는 백그라운드에서 보이지 않게 작동합니다. AI 에이전트가 MCP를 통해 터미널 명령을 실행하든 출력을 수동으로 파이프하든(`ls | omni`) OMNI는 투명한 계층으로 자동으로 뛰어듭니다. 터미널 출력을 지능적으로 필터링하고 시끄러운 로그를 제거하며 깨끗한 신호를 AI에 넘겨줍니다.

절감액, 명령, 기간 및 경로별 자세한 분류:
```bash
omni stats
```

OMNI 설치 진단(후크, MCP, 필터, 데이터베이스):
```bash
omni doctor
```

필터가 작동하는 것을 보거나 고유한 사용자 지정 규칙을 추가해야 합니까?
`~/.omni/signals/`의 간단한 TOML 파일을 사용하여 고유한 규칙을 쉽게 만들 수 있습니다.

### 다중 에이전트 지원 및 통합

기본적으로 `omni init --claude`는 자동으로 **Claude Code**에 연결됩니다. 그러나 OMNI는 내장된 통합 기능을 통해 모든 Agentic AI와 완벽하게 작동합니다! `omni init`을 실행하여 대화형 메뉴를 확인하세요.

1. **VS Code & Continue.dev**: MCP 컨텍스트 공급자(`integrations/continue-dev/`)를 사용합니다.
2. **OpenCode & Codex CLI**: 내장 래퍼는 명령 출력을 자동으로 OMNI로 파이프합니다.
3. **Antigravity IDE**: OMNI는 Antigravity 구성(`~/.gemini/antigravity/mcp_config.json`)에 기본 MCP 서버로 등록됩니다. `omni init --antigravity`를 실행하여 자동으로 설정합니다.
4. **Pi Agent**: Pi용 기본 OMNI 패키지입니다. Pi의 패키지 설치 프로그램을 통해 설치하려면 `omni init --pi`를 실행하세요.

**다중 에이전트 미세 조정 (`~/.omni/config.toml`)**
에이전트마다 문제점이 다릅니다. VS Code 채팅을 깨끗하게 유지하면서 OpenCode가 더 많은 데이터를 읽을 수 있도록 합니다. 개별적으로 미세 조정하세요.
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

### 문서 색인

**사용자용:**
- [최고의 가이드 (HOW_TO_USE.md)](../docs/HOW_TO_USE.md) — 설치, 사용자 지정 TOML 필터 및 CLI 명령 등 필요한 모든 것.
- [OpenClaw 통합](https://clawhub.ai/fajarhide/omni-signal-engine) — 기본 OMNI 증류를 위한 공식 OpenClaw 플러그인.
- [Hermes Agent 통합](https://github.com/wysie/hermes-omni-plugin) — 기본 OMNI 증류를 위한 커뮤니티 Hermes Agent 플러그인.

**개발자 및 시스템 통합자용:**
- [루프 엔지니어링 가이드](../docs/LOOP_ENGINEERING.md) — OMNI를 자율 에이전트와 통합하는 방법.
- [개발 가이드](../docs/DEVELOPMENT.md) — OMNI 코드베이스를 구축하고 기여하는 방법.
- [테스트 아키텍처](../docs/TESTING.md) — 품질 보증 및 컨텍스트 안전.
- [세션 연속성](../docs/SESSION.md) — OMNI 작업 메모리에 대한 심층 분석.
- [로드맵](../docs/ROADMAP.md) — 현재 개발 상태 및 향후 기능.
- [마이그레이션 가이드](../docs/MIGRATION.md) — Node/Zig에서 Rust 버전으로 업그레이드하는 방법에 대한 참고 사항.

---

## Heimsense와 함께 사용하면 더 좋습니다

Omni는 제 개인 AI 도구 벨트의 일부입니다. `claude-code`를 사용하는 경우 Omni를 저의 다른 프로젝트인 **[Heimsense](https://github.com/fajarhide/heimsense)**와 페어링하는 것이 좋습니다.

Heimsense는 값비싼 Anthropic 모델을 사용하도록 강요하는 대신 `claude-code`와 같은 제한된 환경의 잠금을 해제하여 *어떤* 무료 모델이나 OpenAI 호환 모델과도 함께 실행할 수 있도록 합니다.
**Omni + Heimsense** = 합리적인 가격의 모델을 사용하여 노이즈 제로와 정확한 정확도로 세계적 수준의 에이전트 프레임워크를 실행합니다.

---

## 기여 및 라이선스

이것은 Agentic AI 시대를 위해 구축된 열정 프로젝트입니다. 토큰 비용을 절약하거나 무료 모델을 테스트하거나 최고의 에이전트 도구 벨트를 구축하는 데 도움을 주고자 하는 모든 기여를 환영합니다!

- **개발**: 소스에서 빌드하고 싶으신가요? `make ci` 및 `cargo build`를 실행합니다. 자세한 내용은 [CONTRIBUTING.md](../CONTRIBUTING.md)를 읽어보세요.
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

Dibuat dengan ❤️ oleh [Fajar Hidayat](https://github.com/fajarhide)
