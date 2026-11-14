# M3 技术门槛 Spike 报告

**日期：** 2026-11-14  
**里程碑：** M3（`epub://` + sandbox iframe + CSS 多栏分页原型）  
**执行方：** DeepSeek harness  
**结论：** `Gate proposal: PASS`（最终由 Codex/用户确认）

## 1. 环境

| 项目 | 值 |
|---|---|
| 操作系统 | macOS 26.6（25G72） |
| 机型 / CPU | Apple M5 |
| 内存 | 16 GB（17,179,869,184 bytes） |
| Rust | rustc 1.100.0，cargo 1.100.0 |
| Tauri | 2.12.5 |
| Node.js | v26.13.0 |
| Playwright | 1.65.0，WebKit build 2388 |
| 前端构建 | Vite production build（`npm run build`，reader-engine 输出到 `src-tauri/resources/reader-engine.js`，约 104 kB / gzip 30 kB） |
| Rust 测试 | cargo test 默认 debug 构建 |
| 打包 | `npm run tauri build` release，`Qurner.app` 7.14 MiB |

**证据边界：** 自动化在 Playwright WebKit + `http://` route 夹具中运行，模拟 `epub://` 文档、
CSP 与引擎脚本；真实 `epub://` 响应、session/token、HTML 改写与资源读取由 Rust
`protocol::handle` 测试覆盖。**没有在真实 Tauri WKWebView 中自动化加载 `epub://`**，
本报告不把 Playwright HTTP 上下文描述为真实 WKWebView 全链路。

## 2. 六类有效夹具矩阵

Rust 侧（`cargo test -p qurner --test m3_matrix`，3 passed）逐类验证：`parse_epub`、
会话创建、每个 spine 的 `protocol::handle` 返回 200、MIME 与 manifest 一致、CSP nonce、
唯一可信引擎标签、`data-spine-index` / `data-spine-href`；非 spine HTML 不注入；CSS/PNG/
字体/NCX 返回原始字节、`nosniff`、`no-store`。

WebKit 侧（`npm run test:e2e` 中 `m3-matrix.spec.ts`，7 passed）逐类加载合成等价页面，
验证 ready 恰好一次、init location、资源请求与渲染。

| 编号 | 夹具（Rust） | WebKit 页面 | Rust 证据 | WebKit 证据 |
|---|---|---|---|---|
| M3-F01 | MinimalEpub2（EPUB2/NCX） | `M3-F01` 第一章 | spine 200、NCX 200、CSP、引擎标签、toc 1 项 | ready=1、init location、请求 3、无出站 |
| M3-F02 | MinimalEpub3（EPUB3/nav） | `M3-F02` 第一章 | nav manifest 含 `properties=nav`、spine 200、nav 不注入 | ready=1、init location、请求 3 |
| M3-F03 | NestedToc（两层 TOC/fragments） | `M3-F03` 第一/二节 | spine 200、章节含 `#s1/#s2`、toc 2 顶层项 | ready=1、init location、请求 3 |
| M3-F04 | MissingMetadata（无书名/作者/封面） | `M3-F04` 正文 | title/authors 为空、spine 200、渲染不依赖封面 | ready=1、init location、请求 3 |
| M3-F05 | CssAndImage（CSS/PNG/font.ttf） | `M3-F05` 样式+图片+字体 | manifest 含 text/css、image/png、font/ttf；原始字节（CSS 含 font-family、PNG 签名） | 样式生效（text-indent 32px）、PNG 加载（naturalWidth≥1）、font.ttf 请求 200、请求 6 |
| M3-F06 | Bilingual（中文/English/emoji，多 spine） | `M3-F06` 中英+emoji | spine 2、spine 200、章节文本 | ready=1、init location、请求 3 |

资源请求总数：F01–F04/F06 各 3（父页、reader 文档、引擎）；F05 为 6（另有 CSS、PNG、
字体）。所有矩阵测试断言无父页/iframe 之外的出站请求。

**测试页面/命令：**
- Rust：`cargo test -p qurner --test m3_matrix`；
- WebKit：`npx playwright test m3-matrix --project=webkit`（随 `npm run test:e2e`）。

## 3. 安全矩阵

| 攻击/风险 | 覆盖测试 | 结果 |
|---|---|---|
| 原书 script、on* 事件、javascript:、iframe/object/embed、base、meta refresh | `m3_security::hostile_html_removes_active_content_and_obfuscated_urls`、e2e `blocks inline and remote scripts, images and styles` | 移除；无执行、无请求 |
| 远程 HTTP/HTTPS 图片、CSS、脚本、字体 | e2e 同上（route 记录请求，断言 `evil.test` 0） | 0 请求 |
| 外部导航、协议相对 URL、控制字符混淆、`target=_top`、ping | e2e `external links and forms do not navigate`、`obfuscated links cannot navigate or execute` | 父页 URL 不变、无 evil 请求 |
| 伪造 source/origin/sessionId/token、乱序 init/ready | e2e `forged messages are ignored and do not break the handshake`；Vitest `messages.test.tsx`、`iframe.test.tsx`、`parent.test.tsx` | 被忽略；ready 仍恰好一次 |
| CSP 缺失/降级 | `m3_security` CSP 断言（nonce、base-uri、form-action） | 保持 |
| HTTP/HEAD/MIME/大小/路径边界 | `m3_protocol` 13 tests、`m3_security` 8 tests | 通过 |

安全测试总量：Rust `m3_security` 8 passed；WebKit `reader-security.spec.ts` 5 passed +
`empty-library` 1 passed。

## 4. 分页、滚动与 Locator

`reader-pagination.spec.ts`（7 passed）：

| 门槛 | 结果 |
|---|---|
| pageCount > 1；首页/末页 clamp | 500×750 内嵌视口下 pageCount 14；next/prev 正确 clamp |
| 末页可见正文、无空白尾页 | 末页 `scrollWidth - (pageCount-1)*step > 0`；文本 rect 与视口相交为 true |
| 末页 spine-request 去重 | 末页 next 只发送一次 `spine-request(next)`，重复输入不重复 |
| 滚动 turn 约一屏 | 翻屏位移 >0 且 ≤1.1 屏；turn 事件 textOffset 严格前进 |
| 滚动顶部/底部 spine-request | 各只发送一次 |
| resize 保持 locator | capture 的 quote 相同、textOffset 误差 ≤2 |
| 模式切换（分页→滚动→分页） | quote 保持；go-to 回到较早位置 |
| initialLocator | 恢复到中段，quote 命中、误差 ≤2、pageIndex>0 |
| 字体 ready 重排 | 受控延迟 `document.fonts.ready`，resolve 后产生 layout，locator 误差 ≤2 |
| 跨 spine / 文本缺失 | `E_LOCATOR_SPINE_MISMATCH` / `E_LOCATOR_NOT_FOUND`，各只发送一次；错误后位置事件回到章节开头 |

**性能（layout 内 turn-page）：** Playwright 在浏览器内测量 postMessage →
`page-changed` 的 20 次交替 next/prev；reader-engine 为 Vite production build（minify），
WebKit 1.65.0。

原始数据（ms，最近一次运行）：

```
[1, 1, 1, 1, 1, 1, 1, 2, 1, 1, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8]
```

- p50 = 8 ms，p95 = 8 ms，max = 8 ms；门槛 150 ms，余量约 16 倍。
- 前一次运行：`[1,1,2,1,1,1,6,8,8,9,7,8,8,8,8,9]` 等，p95 = 9 ms。
- 说明：测量在 WebKit + HTTP 夹具、无真实磁盘 I/O；真实 `epub://` 下资源已加载，
  但仍需 WKWebView 复核。

## 5. 100 MB EPUB 按需读取

`cargo test -p epub-core --test resource_read large_epub -- --nocapture`：

```
M3D_LARGE_READ bytes=2690 chapter_len=208
```

- 请求小 XHTML 时实际读取 2,690 字节（<1 MB），章节 208 字节；未读取 100 MB 大条目，
  未创建整书副本。
- 测试构建：cargo test 默认 debug；测量方式为 `CountingReader` 统计 `Read` 字节。

## 6. 嵌套目录映射

`cargo test -p epub-core --test toc_mapping`（2 passed），`M3D_TOC` 输出：

| 目录项 | 解析资源 | fragment | spineIndex | spineHref |
|---|---|---|---|---|
| 第一部分 | `OEBPS/chapter1.xhtml` | — | 0 | `OEBPS/chapter1.xhtml` |
| 第一节 | `OEBPS/chapter1.xhtml` | `s1` | 0 | `OEBPS/chapter1.xhtml` |
| 第二节 | `OEBPS/chapter1.xhtml` | `s2` | 0 | `OEBPS/chapter1.xhtml` |
| 第二部分 | `OEBPS/chapter2.xhtml` | — | 1 | `OEBPS/chapter2.xhtml` |

- href 按 OPF 目录自行解析（非文件名模糊匹配），fragment 与章节 `id="s1"/"s2"` 交叉核对。

## 7. 已知差异、未验证项与风险

**未验证：**
- 真实 Tauri WKWebView 中 `epub://` 全链路（自定义协议、sandbox iframe、CSP）自动化；
  只有 Playwright HTTP 夹具 + Rust mock 协议。
- 真实复杂书籍（图片、表格、绝对定位、字体、固定宽度元素）的分页空白/溢出。
- Playwright 字体渲染：F05 使用最小伪 TTF，验证请求/加载路径与 CSS 应用，但未验证
  自定义字体字形渲染（WebKit 使用 fallback）。
- 滚动模式方向键、跨章节串联（只发 `spine-request`）。
- pageCount 由 `scrollWidth` 四舍五入，亚像素布局可能有 ±1 误差。
- 大文本节点内字符级 Locator 精度（当前允许一个文本节点误差）。

**风险：**
- CSS 多栏在复杂原书样式下可能产生空白页或横向溢出，需 M6 前真实书籍复核。
- `caretRangeFromPoint` 依赖 WebKit 实现；其他引擎需回退路径。
- resize/字体 layout 可能产生多次 layout 事件，前端以最后一次为准。
- 引擎约 104 kB，无缓存；每次文档加载重新执行。
- 跨 spine Locator 当前只报 `E_LOCATOR_SPINE_MISMATCH`，父桥需在 M5 串联章节。

## 8. Gate 结论与建议 ADR

**Gate proposal: PASS**

- 六类合成有效 EPUB 在 Rust 协议与 WebKit 引擎矩阵中均可读；安全边界保持。
- 分页、末页、无空白尾页、resize、模式切换、initialLocator、字体重排、跨 spine 错误与
  滚动边界均有自动化证据。
- 20 次 turn-page p95 = 8 ms，满足 ≤150 ms。
- 100 MB EPUB 只读取 2,690 字节。
- 嵌套目录可解析到 spine/fragment。

**建议 ADR 方向（由 Codex/用户决定）：**
1. 保留 `epub://` + sandbox iframe + 受控脚本注入 + CSS 多栏分页的架构；
2. 在 M4/M5 前补充真实 Tauri WKWebView 的 `epub://` 自动化或人工冒烟；
3. 真实书籍兼容性回归作为 M6 门槛，重点关注空白页、字体、表格与绝对定位；
4. Locator 继续使用 version 1 的文本 quote/offset 方案，跨章节由导航层串联。

## 9. 复现命令

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p epub-core
cargo test -p reader-model
cargo test -p qurner --lib
cargo test -p qurner --test m3_protocol
cargo test -p qurner --test m3_security
cargo test -p qurner --test m3_matrix
cargo test -p epub-core --test toc_mapping
cargo test -p epub-core --test resource_read large_epub -- --nocapture
cargo test --workspace
npm run check
npm run test:e2e
npm run build
npm run tauri build
```
