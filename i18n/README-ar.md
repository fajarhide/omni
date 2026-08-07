<div align="center">
  <img src="../media/logo.png" alt="OMNI Logo" width="300" />

<h1>OMNI</h1>
<p align="center" dir="rtl">
    <em><b>توقّف عن الدفع مقابل أن يقرأ Claude عشرة آلاف سطر من ضوضاء الطرفية.</b> يقصّ OMNI من <code>git</code> نسبة 89٪، ومن <code>cargo</code> نسبة 91٪، ومن <code>kubectl</code> نسبة 77٪ قبل أن يراها وكيلك. وكل ما عداها يمرّ دون مساس. لا يضيع شيء أبدًا، ولا يختلق نتيجة.</em>
</p>

[🇺🇸 English](../README.md) | [🇯🇵 日本語](README-ja.md) | [🇨🇳 简体中文](README-zh.md) | [🇸🇦 العربية](README-ar.md) | [🇮🇩 Bahasa Indonesia](README-id.md) | [🇻🇳 Tiếng Việt](README-vi.md) | [🇰🇷 한국어](README-ko.md)

[![CI](https://github.com/fajarhide/omni/actions/workflows/ci.yml/badge.svg)](https://github.com/fajarhide/omni/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/fajarhide/omni)](https://github.com/fajarhide/omni/releases)
  [![Rust](https://img.shields.io/badge/built_with-Rust-dca282.svg)](https://www.rust-lang.org/)
  [![MCP](https://img.shields.io/badge/MCP-compatible-green.svg?style=flat-square)](https://modelcontextprotocol.io/)
  [![License: MIT](https://img.shields.io/github/license/fajarhide/omni)](https://github.com/fajarhide/omni/blob/main/LICENSE)
  [![Hits](https://hits.sh/github.com/fajarhide/omni.svg)](https://hits.sh/github.com/fajarhide/omni/)
</br></br>
<b dir="rtl">
<code>git</code> ‏89٪ &middot; <code>cargo</code> ‏91٪ &middot; <code>kubectl</code> ‏77٪ &middot; ‏21 مللي ثانية لكل أمر &middot; صفر من 9,965 استدعاءً كبّر المخرجات &middot; كل ما يُقصّ قابل للاسترجاع بايتًا ببايت &middot; ذاكرة عابرة للجلسات </b>

</br></br>

```bash
brew install fajarhide/tap/omni && omni init
```

يقوم بتقطير مخرجات الأوامر في Claude Code. ويثبّت الخطافات وخادم MCP وحالة الجلسة المشتركة في Cursor وWindsurf وCodex وRoo، حيث تعتمد إعادة الكتابة على المضيف: لا يسمح Cursor للخطاف باستبدال مخرجات الأدوات المدمجة.

</br>
<img src="../media/demo.gif" alt="‏OMNI يُقطّر تشغيل cargo test مزدحمًا بالضوضاء حتى الحكم النهائي، ثم يعرض omni stats" width="820" />
</div>

---

<div dir="rtl">

يقرأ وكيلك كل سطر تطبعه طرفيتك: سجلات البناء، وسجلات Docker، وسجلات CI، وأشرطة
التقدم، وألوان ANSI. آلاف التوكنز للعثور على سطر واحد. ليست تكلفة Claude هي
المرتفعة، بل طرفيتك.

ثم ينسى ذلك كله بين ليلة وضحاها. تعيد تشغيل Cursor، أو تنتقل إلى Claude Code، فتشرح
المشروع من الصفر مرة أخرى.

يعالج OMNI الأمرين معًا، ويبتعد عن الطريق في كل ما عداهما.

</div>

---

## الفرق

<div dir="rtl">

**المشكلة الأولى: طرفيتك تُغرِق الإشارة**

نفس أمر `git log` جنبًا إلى جنب. بدون OMNI، يملأ حقل `Author` و`Date` ونص رسالة
كوميت واحدة الشاشة. مع OMNI، **تبقى كل كوميت موجودة**، بسطر واحد على هيئة
`hash subject`، وبحجم أصغر بنسبة 94٪. لم يُلخَّص شيء بعيدًا؛ والرقم في التذييل مقيس
من عدد البايتات الحقيقي، لا موعود به.

</div>

<table>
<tr>
<td align="center"><b>بدون OMNI</b><br/><sub><code>git log -15</code> خام</sub></td>
<td align="center"><b>مع OMNI</b><br/><sub>كل كوميت محفوظة، أصغر بنسبة 94٪</sub></td>
</tr>
<tr>
<td valign="top"><img src="../media/demo-git-without.gif" alt="‏git log -15 خام ومُطوَّل: حقول Author وDate ونص كوميت واحدة تملأ الشاشة" width="400" /></td>
<td valign="top"><img src="../media/demo-git-with.gif" alt="نفس git log -15 عبر OMNI: كل كوميت في سطر hash وsubject، أصغر بنسبة 94٪" width="400" /></td>
</tr>
</table>

<div dir="rtl">

أرقام حقيقية، مقيسة على `tests/fixtures/` وعلى تتبّعات أُعيد تشغيلها، لا طموحات:

</div>

| الأمر | بدون OMNI | مع OMNI | التوفير |
|---|---|---|---|
| `cargo test` (نجح 490، فشل 10) | ‏16.5 كيلوبايت من مخرجات كل اختبار | ملخّص النجاح والفشل من المُشغّل نفسه | **92.9٪** |
| `git status` (فيه تغييرات) | ‏496 بايت من مخرجات porcelain | الفرع والمسارات التي تغيّرت | **61.7٪** |
| `docker build` (ضوضاء تخزين مؤقت كثيفة) | ‏9.2 كيلوبايت من تجزئات الطبقات وأشرطة التقدم | نتيجة البناء، مع طيّ إصابات الذاكرة المؤقتة | **35.9٪** |
| `git diff` (ملفات متعددة) | ملفات القفل والمسافات والتغييرات المولّدة | الشيفرة التي تغيّرت فعلًا | **25.2٪** |
| `kubectl get pods` (‏35 pod، 5 متعطّلة) | الجدول كاملًا | الجدول كاملًا | **0٪**، عن قصد |

<div dir="rtl">

كل رقم أعلاه هو الحمولة **المُسلَّمة فعلًا**، شاملةً علامة الاسترجاع البالغة نحو 77 بايتًا
التي يرفقها OMNI كلما أسقط شيئًا. كانت الإصدارات السابقة تقتبس مخرجات المُقطِّر قبل تلك
العلامة، فتبدو الحمولات الصغيرة أفضل مما هي: يُقرأ `git diff` هنا 25.2٪ وبدونها 44.6٪.
العلامة هي ما يجعل القصّ قابلًا للعكس، فمكانها داخل الرقم.

الصف اللافت هو `kubectl get pods`. كان يبلّغ عن 9.3٪، وصار الآن لا يبلّغ عن شيء، لأن جدول
الـ pods تعداد كل سطر فيه بيانة، ولا ضوضاء فيه تُحذف. خسارة تلك الـ 9.3٪ هي الإصلاح نفسه.

</div>

<div dir="rtl">

> **حيث لا يفعل شيئًا، عن قصد.** الأمر الذي يفشل يمرّ حرفيًا، لأن خطأً مخفيًا أغلى من خطأ غير مضغوط. والمخرجات المهيكلة (JSON وYAML وCSV) لا تُمسّ أبدًا، لأن الخطوة التالية في مسارك ستحلّلها. يكسب OMNI مكانه في ثرثرة الأدوات المتكررة ويبتعد عن الطريق في كل ما عداها، وهذا ما يجعل تركه مفعّلًا على كل أمر تشغّله آمنًا.

### لا يضيع شيء أبدًا. ولا يختلق شيئًا.

وعدان، وكلاهما في الشيفرة لا في هذه الفقرة.

**لا يضيع شيء أبدًا.** كل بايت يقصّه OMNI يُؤرشَف محليًا في RewindStore بمفتاح SHA-256. يتلقى الوكيل تجزئة مع المخرجات المُقطَّرة، وبإمكانه استدعاء `omni_retrieve` لسحب الأصل بايتًا ببايت في منتصف المحادثة، دون إعادة تشغيل أمرك.

**ولا يختلق شيئًا.** المُقطِّر الذي لا يتعرّف على شيء في مدخلاته يعيد المدخل الخام. هذا نوع بياني لا عُرف: تعيد `distill` قيمة `Option<String>`، وتعود طبقة التوجيه إلى الأصل كلما تلقّت `None`. لا يوجد مسار شيفرة ينتج سطرًا أخضر بعبارة "no errors" لم يقرأه OMNI.

يطلب منك كل ضاغط آخر أن *تثق* بأن ما اقتطعه لم يكن مهمًا. أما OMNI فيسلّمك الإيصال:

</div>

| الضمانة | كيف | الدليل |
|---|---|---|
| **استعادة الأصل بايتًا ببايت** | كل ما يُقتطع يُؤرشف في **RewindStore** المحلي على SQLite (من SHA-256 إلى المحتوى)؛ يتلقى الوكيل تجزئة ويستدعي `omni_retrieve` | [`كيف يعمل`](#كيف-يعمل) |
| **لا يختلق نتيجة أبدًا** | المُقطِّر الذي لم يحلّل أي إشارة يعيد المخرجات الخام، لا سطرًا أخضر مثل `no errors` أو `passed` | [#143](https://github.com/fajarhide/omni/issues/143) |
| **لا يُخفي الإخفاقات أبدًا** | الأمر الذي ينتهي برمز خروج غير صفري يمرّ حرفيًا | [#120](https://github.com/fajarhide/omni/issues/120) |
| **لا تُمسّ البيانات المهيكلة** | ‏JSON و YAML و NDJSON و CSV تمرّ بايتًا ببايت | `pipeline::format` |
| **الأرقام مقيسة لا مأمولة** | ‏9,965 تتبّعات حقيقية أُعيد تشغيلها على الملف التنفيذي للإصدار، و90.0٪ من الاستدعاءات لم توفّر شيئًا، وهو رقم ننشره أيضًا | [`القياسات`](#القياسات) |

<div dir="rtl">

هذا ما لا تشتريه نسبة ضغط أكبر: **يمكنك دائمًا استعادة الأصل، ولن يكذب على وكيلك أبدًا.**

</div>

---

## القياسات

<div dir="rtl">

مقيسًا على الملف التنفيذي للإصدار بإعادة تشغيل **9,965 تنفيذًا حقيقيًا للأوامر** من
استخدام فعلي لمطوّر واحد:

* **على الأوامر التي تولّد ضوضاء فعلًا، من 76٪ إلى 91٪.** ‏`cargo` ‏91.4٪، و`git` ‏89.2٪،
  و`kubectl` ‏76.5٪. هناك تذهب ميزانية سياقك، وهناك يعمل OMNI.
* **يتدخّل OMNI في أمر واحد من كل عشرة، ولا يضيف بايتًا واحدًا إلى التسعة الباقية.**
  إنه مرشّح لا مُلخِّص. حين لا يوجد ما يُقصّ يبتعد تمامًا.
* **لم يكبّر أي استدعاء من الـ 9,965 مخرجاتِه.**
* **‏43.3٪ بايتات أقل** عبر المزيج كله، بالأوامر المزعجة والهادئة معًا.
* **‏21 مللي ثانية لكل أمر** من الطرف إلى الطرف، تنمو مع تاريخك لا مع حجم الحمولة،
  وتصير 61 مللي ثانية مع قاعدة بحجم 205 ميغابايت.

</div>

<div align="center">
<img src="https://omni.weekndlabs.com/media/performance.png" alt="OMNI" width="600" />
</div>

<div dir="rtl">

المجموعة كاملة، والتفصيل حسب الأمر، والعيّنات وجداول زمن الاستجابة في
**[docs/BENCHMARKS.md](../docs/BENCHMARKS.md)**. وأعد إنتاجها بـ
`cargo test --release --test bench_replay -- --ignored`.

### كيف تقرأ رقم توفير، ورقمنا منها

كل أداة في هذه الفئة تنشر نسبة مئوية. وهذه خمسة أسئلة تقرّر إن كان للرقم معنى، ومعها
إجاباتنا:

</div>

| السؤال | لماذا يهم | OMNI |
|---|---|---|
| ما حصة الاستدعاءات التي **لم** توفّر شيئًا؟ | الأداة التي توفّر في كل أمر إنما تلخّص مخرجات كنت تحتاجها | **‏90.0٪**، ننشرها |
| هل كبّر أي استدعاء المخرجات؟ | العلامات والترويسات تكلّف بايتات، ولا أحد يبلّغ عن الحالات التي انقلبت عليه | **صفر من 9,965** |
| أي **مجتمع** قيس؟ | عدّ بايتات الطرفية التي لا يقرأها نموذج ينفخ الرقم مجانًا | ما وصل إلى نموذج فقط، وقول ذلك كلّفنا 36 نقطة |
| هل يمكنك **إعادة تشغيله**؟ | رقم لا يمكن إعادة إنتاجه ادّعاء لا قياس | أمر واحد، على بياناتك أنت |
| هل القصّ **قابل للاسترجاع**؟ | الفقدان الجزئي مقبول ما دام عكسيًا، وقاتل إن لم يكن | بايتًا ببايت، عبر `omni_retrieve` |

<div dir="rtl">

ننشر حصة الاستدعاءات التي لم نفعل فيها شيئًا، لأنها الرقم الذي يخبرك بقيمة بقية
الأرقام.

</div>

---

## البدء السريع والتثبيت

<div dir="rtl">

إعداد OMNI سهل للغاية، وهو يتكامل مع طرفيتك بشكل أصيل.

</div>

**macOS / Linux:**
```bash
# 1. التثبيت عبر Homebrew
brew install fajarhide/tap/omni

# 2. إعداد OMNI (قائمة تفاعلية لـ Claude و VS Code و OpenCode و Codex و Antigravity)
omni init

# 3. التحقق من أنه يعمل
omni doctor

# 4. أو الإصلاح التلقائي لأي مشكلة
omni doctor --fix

# 5. عرض الحالة الحالية
omni init --status
```

**مثبّت شامل (macOS / Linux / WSL):**
```bash 
curl -fsSL omni.weekndlabs.com/install | bash
```

**Windows (PowerShell):**
```powershell
irm omni.weekndlabs.com/install.ps1 | iex
```

---

---

## الأسئلة الشائعة

<div dir="rtl">

**هل يحذف OMNI سجلاتي نهائيًا؟**  
لا. تُضغط السجلات الخام وتُخزَّن محليًا في RewindStore على SQLite. يتلقى الذكاء الاصطناعي تجزئة ويمكنه استرجاع السجل الكامل عند الحاجة.

**هل سيبطئ هذا طرفيتي؟**  
نعم، بقدر قابل للقياس، والتكلفة تنمو مع تاريخك. خط التقطير نفسه يعمل في أجزاء من الألف من الثانية بخانة واحدة، لكن كل أمر مربوط بخُطّاف يكتب أيضًا في RewindStore المحلي: يستغرق `git status` بحجم 496 بايت نحو 21 مللي ثانية مقابل قاعدة جديدة، ونحو 61 مللي ثانية مقابل قاعدة بحجم 205 ميغابايت، ويستغرق `cargo test` بحجم 16.5 كيلوبايت نحو 25 مللي ثانية. ضَعْ ذلك في الحسبان. ويتخطى `OMNI_PASSTHROUGH=1` خط المعالجة بالكامل حين تحتاج إلى المخرجات الخام.

**هل يمكنني إضافة مرشّحاتي الخاصة؟**  
نعم. يمكنك تعليم OMNI إزالة الضوضاء الخاصة بأدواتك الداخلية عبر TOML:

</div>

```toml
# ~/.omni/signals/custom.toml
[filters.my_tool]
match_command = "^internal-tool\\b"
strip_lines_matching = ["^DEBUG", "syncing..."]
```

<div dir="rtl">

**كيف أرى توفيري أنا؟**
شغّل `omni stats` بعد أيام من الاستخدام. ويطبع `omni stats --share` الأرقام نفسها في
صيغة جاهزة للنسخ.

</div>

---

## اقرأ أكثر

<div dir="rtl">

* [كيف يعمل، وكم يكلّف](../docs/ARCHITECTURE.md): خط المعالجة، وRewindStore، وMemory OS
* [القياسات كاملة](../docs/BENCHMARKS.md): المجموعة، والتفصيل حسب الأمر، والعيّنات، وزمن الاستجابة
* [المساهمة](../CONTRIBUTING.md): اجعل `make ci` يمرّ وأنت معنا

</div>

---

```bash
brew install fajarhide/tap/omni && omni init
```

## المساهمة والترخيص

<div dir="rtl">

هذا مشروع شغف بُني لعصر الذكاء الاصطناعي الوكيل. سواء جئت لتوفير تكاليف التوكنز، أو لتجريب النماذج المجانية، أو للمساعدة في بناء حزمة أدوات الوكلاء المثلى، فالمساهمات مرحّب بها دائمًا!

- **التطوير**: تريد البناء من المصدر؟ شغّل `make ci` و`cargo build`. اقرأ [CONTRIBUTING.md](../CONTRIBUTING.md) للتفاصيل.
- **الترخيص**: [MIT License](../LICENSE)

</div>

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
