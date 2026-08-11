<div align="center">
  <img src="../media/logo.png" alt="OMNI Logo" width="300" />

<h1>OMNI</h1>
<p align="center">
    <em><b>Agent của bạn đọc mọi dòng terminal in ra, rồi đọc lại phần lớn trong số đó ở lượt sau.</b> OMNI bỏ phần nhiễu trước khi mô hình nhìn thấy, và trả về một tham chiếu cho những dòng đã từng cho xem. Không xóa gì cả, và không bao giờ bịa ra kết quả.</em>
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

Chưng cất đầu ra lệnh trên Claude Code, Codex CLI và Gemini CLI, những host áp dụng bản ghi đè của OMNI. Ở các host khác bạn vẫn có máy chủ MCP, trạng thái phiên dùng chung, và `omni_run` chưng cất mọi lệnh bạn chạy qua nó. Chạy `omni doctor` để xem tier của từng host.


### Mỗi host cho phép OMNI làm gì

| Tier | Host | Bạn nhận được gì |
|---|---|---|
| **Full** | Claude Code, Codex CLI, Gemini CLI, Aider (pipe) | Host áp dụng bản ghi đè của OMNI, nên mô hình đọc đầu ra đã chưng cất từ công cụ tích hợp của chính nó. |
| **Handoff-first** | Cursor, Windsurf | Host không thể ghi đè đầu ra công cụ tích hợp. `omni_run` chưng cất mọi lệnh bạn chạy qua nó, và `omni init --cursor` cài quy tắc khiến agent chọn nó. |
| **MCP-only** | Cline, Roo, OpenCode, VS Code, Zed, Copilot, Antigravity, Hermes, Pi | Chỉ bộ nhớ, recall và trạng thái phiên. Không chưng cất shell, và không tuyên bố là có. |

`omni doctor` in ra tier của từng host đã cài. Tiết kiệm chỉ được tính khi mô hình thực sự nhận ít hơn.

Codex CLI cần thêm một bước. Nó chỉ chạy những hook đã được tin cậy và bỏ qua phần còn lại mà không báo gì, nên sau `omni init --codex` hãy chạy `codex` một lần rồi phê duyệt ở mục "Hooks need review". `omni doctor` sẽ báo lỗi cho tới khi bạn làm việc đó. Xem [#359](https://github.com/fajarhide/omni/issues/359).
</br>
<img src="../media/demo.gif" alt="OMNI chưng cất một lần chạy cargo test ồn ào xuống còn kết luận, rồi hiển thị omni stats" width="820" />
</div>

---

Agent của bạn đọc từng dòng terminal in ra. Build log, Docker log, CI log, thanh tiến
trình, màu ANSI. Hàng nghìn token chỉ để tìm một dòng. Claude không đắt. Terminal của
bạn mới đắt.

Và nó quên sạch sau một đêm. Khởi động lại Cursor, chuyển sang Claude Code, và bạn
giải thích lại dự án từ đầu.

OMNI sửa cả hai, và tránh đường ở mọi chỗ khác.

---

## Nó làm gì

**Bỏ phần nhiễu.** Log build, hash lớp Docker, thanh tiến trình, màu ANSI. Phần đầu ra
không ai đọc bị loại bỏ trước khi tới mô hình.

**Ngừng gửi lại thứ agent đã thấy.** Một loạt dòng đã cho xem trước đó trong cùng phiên
quay lại dưới dạng một dấu kèm handle, chứ không phải các byte đó lần nữa. Đây là nửa mà
bộ lọc không làm được: nó bỏ đi vì chúng đã nằm trong ngữ cảnh, không phải vì một mẫu nào
gọi chúng là nhiễu.

**Nhớ xuyên phiên.** Khởi động lại trình soạn thảo hay đổi agent, ngữ cảnh dự án vẫn còn.

**Biết tránh đường.** Lệnh thất bại đi qua nguyên vẹn. JSON, YAML và CSV không bao giờ bị
đụng tới. Phần lớn lệnh được trả lại y nguyên, và đó là hành vi có chủ đích chứ không phải
thiếu sót.


---

## Khác biệt ở đâu

**Vấn đề 1: terminal nhấn chìm tín hiệu**

Cùng một `git log`, đặt cạnh nhau. Không có OMNI, riêng `Author` / `Date` / phần thân
của một commit đã đầy màn hình. Có OMNI, **mọi commit đều còn nguyên**, dưới dạng một
dòng `hash subject`, nhỏ hơn 94%. Không có gì bị tóm tắt mất đi; con số ở chân trang
được đo từ số byte thật, không phải lời hứa.

<table>
<tr>
<td align="center"><b>Không có OMNI</b><br/><sub><code>git log -15</code> thô</sub></td>
<td align="center"><b>Có OMNI</b><br/><sub>giữ mọi commit, nhỏ hơn 94%</sub></td>
</tr>
<tr>
<td valign="top"><img src="../media/demo-git-without.gif" alt="git log -15 thô dài dòng: Author, Date và phần thân của một commit lấp đầy màn hình" width="400" /></td>
<td valign="top"><img src="../media/demo-git-with.gif" alt="cùng git log -15 qua OMNI: mỗi commit thành một dòng hash và subject, nhỏ hơn 94%" width="400" /></td>
</tr>
</table>

Con số thật, đo trên `tests/fixtures/` và các trace phát lại, không phải mong muốn:

| Lệnh | Không có OMNI | Có OMNI | Tiết kiệm |
|---|---|---|---|
| `cargo test` (490 đạt, 10 hỏng) | 16,5 KB đầu ra từng test | bản tóm tắt đạt/hỏng của chính runner | **92,9%** |
| `git status` (có thay đổi) | 496 B đầu ra porcelain | nhánh và các đường dẫn đã đổi | **61,7%** |
| `docker build` (nhiễu cache nặng) | 9,2 KB hash layer và thanh tiến trình | kết quả build, cache hit được gộp | **35,9%** |
| `git diff` (nhiều tệp) | lockfile, khoảng trắng, thay đổi do sinh mã | phần mã thực sự thay đổi | **25,2%** |
| `kubectl get pods` (35 pod, 5 crash) | toàn bộ bảng | toàn bộ bảng | **0%**, có chủ đích |

Mọi con số ở trên là payload **thực sự được giao**, đã tính cả dấu khôi phục khoảng
77 byte mà OMNI gắn vào mỗi khi nó bỏ đi thứ gì đó. Các bản phát hành trước trích đầu
ra của bộ chưng cất trước dấu đó, khiến các payload nhỏ trông đẹp hơn thực tế:
`git diff` đọc là 25,2% ở đây và 44,6% nếu không tính. Chính dấu đó làm cho phần bị cắt
khôi phục được, nên nó thuộc về con số.

Dòng đáng chú ý là `kubectl get pods`. Trước đây nó báo 9,3%; giờ nó không báo gì cả, vì
một bảng pod là một liệt kê mà mỗi dòng là một dữ liệu, không có nhiễu nào để bỏ. Mất
9,3% đó chính là bản sửa lỗi.

> **Nơi nó cố ý không làm gì.** Một lệnh thất bại được cho qua nguyên vẹn, vì một lỗi bị giấu đắt hơn một lỗi chưa nén. Đầu ra có cấu trúc (JSON, YAML, CSV) không bao giờ bị chạm vào, vì bước tiếp theo trong pipeline của bạn sẽ phân tích nó. OMNI xứng đáng có mặt ở phần lải nhải lặp lại của công cụ và tránh đường ở mọi chỗ khác, và đó là điều khiến việc bật nó cho mọi lệnh bạn chạy là an toàn.

### Không mất gì cả. Nó không bao giờ bịa ra điều gì.

Bốn bảo đảm, mỗi cái dẫn tới đoạn mã hoặc issue đã làm nó thành sự thật, chứ không phải
một câu xin bạn tin.

| Bảo đảm | Bằng cách nào | Bằng chứng |
|---|---|---|
| **Lấy lại bản gốc, từng byte một** | mọi thứ bị cắt đều được lưu trong **RewindStore** SQLite cục bộ (SHA-256 tới nội dung); agent nhận một hash và gọi `omni_retrieve` | [`Cách hoạt động`](#cách-hoạt-động) |
| **Không bao giờ bịa kết quả** | bộ chưng cất không phân tích được tín hiệu nào sẽ trả về đầu ra thô, chứ không phải một dòng xanh `no errors` hay `passed` | [#143](https://github.com/fajarhide/omni/issues/143) |
| **Thất bại không bao giờ bị che** | lệnh thoát với mã khác 0 được cho qua nguyên vẹn | [#120](https://github.com/fajarhide/omni/issues/120) |
| **Dữ liệu có cấu trúc không bị chạm** | JSON / YAML / NDJSON / CSV đi qua từng byte một | `pipeline::format` |
| **Số liệu là đo được, không phải kỳ vọng** | 6.656 trace thật phát lại trên bản binary phát hành, và 97,3% lệnh gọi không tiết kiệm được gì, con số đó chúng tôi cũng công bố | [`Đo đạc`](#đo-đạc) |

Đó là điều mà một tỉ lệ nén lớn hơn không mua được: **bạn luôn khôi phục được bản gốc, và nó sẽ không bao giờ nói dối agent của bạn.**

---

## Đo đạc

Đo trên bản binary phát hành bằng cách phát lại **6.656 lần thực thi lệnh thật**
trong khoảng **3 đến 10 tháng 8 năm 2026 UTC**, tất cả đều là đầu ra tới được mô hình.
Khoảng thời gian là một phần của con số: `execution_traces` bị cắt sau bảy ngày, nên
một tập dữ liệu biến mất một tuần sau khi được đo.

* Đầu ra build và test **76,9%**. Lớp lớn nhất là đọc lại tệp: bộ lọc lấy **0,0%**, ledger
  lấy **26,3%**, và chính khoảng cách đó là lý do ledger tồn tại.
* **97,3% lệnh gọi không tiết kiệm được gì**, và chúng tôi công bố vì đó là con số cho biết
  phần còn lại đáng giá bao nhiêu. **Không lệnh gọi nào làm đầu ra lớn hơn**
  trong lần đo này. Từng có 2 cho tới ([#398](https://github.com/fajarhide/omni/issues/398)), và chúng tôi đã công bố chúng suốt
  thời gian đó.
* **21 ms mỗi lệnh**, lớn dần theo lịch sử của bạn chứ không theo kích thước payload. Với
  cơ sở dữ liệu 205 MB con số là 61 ms.
<div align="center">
<img src="https://omni.weekndlabs.com/media/performance.png" alt="OMNI" width="600" />
</div>

Toàn bộ tập dữ liệu, phân tích theo lớp lệnh, fixture và bảng độ trễ:
**[docs/BENCHMARKS.md](../docs/BENCHMARKS.md)**. Tái lập bằng
`cargo test --release --test bench_replay -- --ignored`.

## Bắt đầu nhanh & Cài đặt

OMNI cực kỳ dễ thiết lập và tích hợp nguyên bản vào terminal của bạn.

**macOS / Linux:**
```bash
# 1. Cài qua Homebrew
brew install fajarhide/tap/omni

# 2. Thiết lập OMNI (menu tương tác cho Claude, VS Code, OpenCode, Codex, Antigravity)
omni init

# 3. Kiểm tra đã chạy
omni doctor

# 4. Hoặc tự động sửa nếu có vấn đề
omni doctor --fix

# 5. Xem trạng thái hiện tại
omni init --status
```

**Trình cài đặt chung (macOS / Linux / WSL):**
```bash 
curl -fsSL omni.weekndlabs.com/install | bash
```

**Windows (PowerShell):**
```powershell
irm omni.weekndlabs.com/install.ps1 | iex
```

---

---

## OMNI nhớ những gì, và trong bao lâu

Ba tầng, vốn đã nằm trong schema và tới giờ mới được viết ra. Câu trả lời ngắn cho "sau một
tháng vắng mặt, OMNI còn biết dự án của tôi không" là có với các kết luận, và không với
những byte thô.

| Tầng | Cái gì | Giữ |
|---|---|---|
| **Vĩnh viễn** | tri thức dự án, các mẫu lỗi lặp lại, engram, bộ nhớ mục tiêu | cho tới khi bạn xoá, trừ bộ nhớ mục tiêu vốn tôn trọng `ttl_days` của chính nó |
| **Làm việc, 30 ngày** | phiên, các dòng chưng cất, tệp nóng, RewindStore, chỉ mục sự kiện, sổ cái | cửa sổ trượt |
| **Nguyên văn, 7 ngày** | `execution_traces` và bản ghi phiên | ngắn hơn có chủ đích: nặng hơn hai bậc trên mỗi dòng |

Ranh giới nó đặt ra đáng được nói thẳng, vì đó là điều duy nhất một handle không thể hứa:
`omni_retrieve` cho nội dung lưu trữ quá 30 ngày sẽ không tìm thấy gì. Hãy giữ cửa sổ ngắn
nhất mở khi đang đo bằng `OMNI_TRACE_RETENTION_DAYS=90`.

`omni reset` xoá tất cả, và `omni doctor` hiển thị số liệu thực.

---

## Câu hỏi thường gặp

**OMNI có xóa vĩnh viễn log của tôi không?**  
Không. Log thô được nén và lưu cục bộ trong RewindStore SQLite. AI nhận một hash và có thể lấy lại toàn bộ log khi cần.

**Việc này có làm terminal của tôi chậm đi không?**  
Có, ở mức đo được, và chi phí lớn dần theo lịch sử. Bản thân pipeline chưng cất chạy trong vài mili giây một chữ số, nhưng mọi lệnh được hook cũng ghi vào RewindStore cục bộ: `git status` 496 byte mất khoảng 21 ms với cơ sở dữ liệu mới và khoảng 61 ms với cơ sở dữ liệu 205 MB, còn `cargo test` 16,5 KB mất khoảng 25 ms. Hãy tính vào ngân sách. `OMNI_PASSTHROUGH=1` bỏ qua toàn bộ pipeline khi bạn cần lại đầu ra thô.

**Tôi có thể thêm bộ lọc của riêng mình không?**  
Không, và đó là chủ ý từ 0.7.0. Bộ lọc được biên dịch vào binary, nên tập đang chạy đúng bằng tập mà kiểm thử bao phủ, và không có tệp nào trên đĩa đổi được thứ agent của bạn nhìn thấy. Nếu một công cụ cần signal, hãy mở issue; nó sẽ đi kèm binary cho tất cả mọi người.

**Lấy lại thứ OMNI đã gấp bằng cách nào?**
`omni retrieve <handle>`, với handle là 16 ký tự bên trong marker. Nó chạy trên mọi host, có hay không có MCP.

**Xem số liệu mà không cần gõ lệnh?**
`omni dashboard` phục vụ chúng ở `127.0.0.1`, chỉ đọc, từ chính cơ sở dữ liệu mà `omni stats` đọc.

**Làm sao xem mức tiết kiệm của chính tôi?**
Chạy `omni stats` sau vài ngày. `omni stats --share` in ra cùng những con số đó ở dạng
tiện sao chép.
`omni stats` mở đầu bằng tuổi thọ phiên, tức số lệnh một phiên đi được trước khi host đóng nó, vì đó mới là thứ cửa sổ ngữ cảnh thực sự tiêu tốn. Tỷ lệ chưng cất bên dưới là số liệu chẩn đoán cho pipeline của một host, không phải một tuyên bố về sản phẩm.

---

## Tìm hiểu thêm

* [Cách hoạt động và cái giá của nó](../docs/ARCHITECTURE.md): pipeline, RewindStore, Memory OS
* [Đo đạc đầy đủ](../docs/BENCHMARKS.md): tập dữ liệu, theo lớp lệnh, fixture, độ trễ
* [Đóng góp](../CONTRIBUTING.md): chạy được `make ci` là xong

---

```bash
brew install fajarhide/tap/omni && omni init
```

## Đóng góp & Giấy phép

Đây là một dự án làm vì đam mê, xây cho kỷ nguyên AI dạng agent. Dù bạn đến để tiết kiệm tiền token, thử các mô hình miễn phí, hay góp sức dựng nên bộ đồ nghề agent tối thượng, đóng góp luôn được chào đón!

- **Phát triển**: muốn build từ mã nguồn? Chạy `make ci` và `cargo build`. Đọc [CONTRIBUTING.md](../CONTRIBUTING.md) để biết chi tiết.
- **Giấy phép**: [MIT License](../LICENSE)

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
