<div align="center">
  <img src="../media/logo.png" alt="OMNI Logo" width="300" />

<h1>OMNI</h1>
<p align="center">
    <em><b>Đừng trả tiền để Claude đọc 10.000 dòng nhiễu terminal nữa.</b> OMNI cắt <code>git</code> 89%, <code>cargo</code> 91% và <code>kubectl</code> 77% trước khi agent của bạn kịp nhìn thấy. Mọi thứ còn lại đi qua nguyên vẹn. Không mất gì cả, và nó không bao giờ bịa ra kết quả.</em>
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
<code>git</code> 89% &middot; <code>cargo</code> 91% &middot; <code>kubectl</code> 77% &middot; 21 ms mỗi lệnh &middot; 0 trong 9.965 lệnh gọi làm đầu ra lớn hơn &middot; mọi phần bị cắt đều khôi phục được từng byte &middot; bộ nhớ xuyên phiên </b>

</br></br>

```bash
brew install fajarhide/tap/omni && omni init
```

Chưng cất đầu ra lệnh trên Claude Code. Cài hook, máy chủ MCP và trạng thái phiên dùng chung trên Cursor, Windsurf, Codex và Roo, nơi việc ghi đè phụ thuộc vào host: Cursor không cho phép hook thay thế đầu ra của công cụ tích hợp.

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

Hai lời hứa, và cả hai nằm trong mã nguồn chứ không nằm trong đoạn văn này.

**Không mất gì cả.** Mọi byte OMNI cắt đi đều được lưu cục bộ trong RewindStore, khóa bằng SHA-256. Agent nhận một hash kèm đầu ra đã chưng cất và có thể gọi `omni_retrieve` để kéo bản gốc về từng byte một, ngay giữa cuộc hội thoại, mà không cần chạy lại lệnh của bạn.

**Nó không bao giờ bịa ra điều gì.** Bộ chưng cất không nhận ra gì trong đầu vào sẽ trả lại đúng đầu vào thô. Đó là một kiểu dữ liệu, không phải một quy ước: `distill` trả về `Option<String>` và lớp định tuyến quay về bản gốc mỗi khi nhận `None`. Không có đường mã nào tạo ra một dòng xanh "no errors" mà OMNI chưa đọc.

Các bộ nén khác đề nghị bạn *tin* rằng thứ họ cắt đi không quan trọng. OMNI đưa cho bạn biên nhận:

| Bảo đảm | Bằng cách nào | Bằng chứng |
|---|---|---|
| **Lấy lại bản gốc, từng byte một** | mọi thứ bị cắt đều được lưu trong **RewindStore** SQLite cục bộ (SHA-256 tới nội dung); agent nhận một hash và gọi `omni_retrieve` | [`Cách hoạt động`](#cách-hoạt-động) |
| **Không bao giờ bịa kết quả** | bộ chưng cất không phân tích được tín hiệu nào sẽ trả về đầu ra thô, chứ không phải một dòng xanh `no errors` hay `passed` | [#143](https://github.com/fajarhide/omni/issues/143) |
| **Thất bại không bao giờ bị che** | lệnh thoát với mã khác 0 được cho qua nguyên vẹn | [#120](https://github.com/fajarhide/omni/issues/120) |
| **Dữ liệu có cấu trúc không bị chạm** | JSON / YAML / NDJSON / CSV đi qua từng byte một | `pipeline::format` |
| **Số liệu là đo được, không phải kỳ vọng** | 9.965 trace thật phát lại trên bản binary phát hành, và 90,0% lệnh gọi không tiết kiệm được gì, con số đó chúng tôi cũng công bố | [`Đo đạc`](#đo-đạc) |

Đó là điều mà một tỉ lệ nén lớn hơn không mua được: **bạn luôn khôi phục được bản gốc, và nó sẽ không bao giờ nói dối agent của bạn.**

---

## Đo đạc

Đo trên bản binary phát hành bằng cách phát lại **9.965 lần thực thi lệnh thật** từ
thói quen sử dụng của một lập trình viên:

* **Trên những lệnh thực sự sinh nhiễu, 76 đến 91%.** `cargo` 91,4%, `git` 89,2%,
  `kubectl` 76,5%. Đó là nơi ngân sách ngữ cảnh của bạn tiêu hết, và cũng là nơi OMNI
  làm việc.
* **OMNI ra tay với 1 lệnh trong 10, và thêm 0 byte vào 9 lệnh còn lại.** Nó là bộ lọc,
  không phải bộ tóm tắt. Khi không có gì để cắt, nó tránh đường hoàn toàn.
* **Không một lệnh gọi nào trong 9.965 làm đầu ra lớn hơn.**
* **Giảm 43,3% số byte** trên toàn bộ tổ hợp, cả lệnh ồn ào lẫn lệnh yên tĩnh.
* **21 ms mỗi lệnh** từ đầu tới cuối, lớn dần theo lịch sử của bạn chứ không theo kích
  thước payload. Với cơ sở dữ liệu 205 MB con số là 61 ms.

<div align="center">
<img src="https://omni.weekndlabs.com/media/performance.png" alt="OMNI" width="600" />
</div>

Toàn bộ tập dữ liệu, phân tích theo lệnh, fixture và bảng độ trễ:
**[docs/BENCHMARKS.md](../docs/BENCHMARKS.md)**. Tái lập bằng
`cargo test --release --test bench_replay -- --ignored`.

### Cách đọc một con số tiết kiệm, kể cả của chúng tôi

Công cụ nào trong nhóm này cũng công bố một tỉ lệ phần trăm. Đây là năm câu hỏi quyết
định con số đó có nghĩa gì, cùng câu trả lời của chúng tôi:

| Câu hỏi | Vì sao quan trọng | OMNI |
|---|---|---|
| Bao nhiêu phần trăm lệnh gọi **không** tiết kiệm được gì? | Công cụ tiết kiệm trên mọi lệnh là đang tóm tắt phần đầu ra bạn cần | **90,0%**, có công bố |
| Có lệnh gọi nào làm đầu ra **lớn hơn** không? | Dấu và tiêu đề đều tốn byte, và không ai báo cáo những lần phản tác dụng | **0 trong 9.965** |
| Đã đo trên **tập hợp** nào? | Đếm cả byte terminal không mô hình nào đọc là cách thổi phồng miễn phí | chỉ phần tới được mô hình, và nói ra điều đó khiến chúng tôi mất 36 điểm |
| Bạn có **chạy lại** được không? | Con số không tái lập được là một tuyên bố, không phải một phép đo | một lệnh, trên dữ liệu của chính bạn |
| Phần bị cắt có **khôi phục** được không? | Có mất mát thì ổn nếu đảo ngược được, và chí mạng nếu không | từng byte một, qua `omni_retrieve` |

Chúng tôi công bố tỉ lệ lệnh gọi mà mình không làm gì cả, vì đó là con số cho bạn biết
những con số còn lại đáng giá bao nhiêu.

---

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

## Câu hỏi thường gặp

**OMNI có xóa vĩnh viễn log của tôi không?**  
Không. Log thô được nén và lưu cục bộ trong RewindStore SQLite. AI nhận một hash và có thể lấy lại toàn bộ log khi cần.

**Việc này có làm terminal của tôi chậm đi không?**  
Có, ở mức đo được, và chi phí lớn dần theo lịch sử. Bản thân pipeline chưng cất chạy trong vài mili giây một chữ số, nhưng mọi lệnh được hook cũng ghi vào RewindStore cục bộ: `git status` 496 byte mất khoảng 21 ms với cơ sở dữ liệu mới và khoảng 61 ms với cơ sở dữ liệu 205 MB, còn `cargo test` 16,5 KB mất khoảng 25 ms. Hãy tính vào ngân sách. `OMNI_PASSTHROUGH=1` bỏ qua toàn bộ pipeline khi bạn cần lại đầu ra thô.

**Tôi có thể thêm bộ lọc của riêng mình không?**  
Có. Bạn có thể dạy OMNI bóc phần nhiễu riêng của công cụ nội bộ bằng TOML:
```toml
# ~/.omni/signals/custom.toml
[filters.my_tool]
match_command = "^internal-tool\\b"
strip_lines_matching = ["^DEBUG", "syncing..."]
```

**Làm sao xem mức tiết kiệm của chính tôi?**
Chạy `omni stats` sau vài ngày. `omni stats --share` in ra cùng những con số đó ở dạng
tiện sao chép.

---

## Tìm hiểu thêm

* [Cách hoạt động và cái giá của nó](../docs/ARCHITECTURE.md): pipeline, RewindStore, Memory OS
* [Đo đạc đầy đủ](../docs/BENCHMARKS.md): tập dữ liệu, theo lệnh, fixture, độ trễ
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
