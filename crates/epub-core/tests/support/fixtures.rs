#![allow(dead_code)]
//! EPUB 测试夹具工厂。
//!
//! 在临时目录生成最小、有效、损坏和恶意的合成 EPUB，供 M1B–M1D 解析器测试复用。
//! 生成的 `.epub` 不会写入仓库。

use std::path::{Path, PathBuf};

use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

static FIXTURE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// 夹具专用临时目录（不依赖外部 crate）。
struct FixtureDir {
    path: PathBuf,
}

impl FixtureDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "qurner-fixture-{}-{}",
            std::process::id(),
            FIXTURE_COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("应能创建临时目录");
        Self { path }
    }

    fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl Drop for FixtureDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

use super::zip_writer::{ZipEntry, build_zip};

pub const MIMETYPE: &str = "application/epub+zip";

const CONTAINER_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>
"#;

const OPF2: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="2.0" unique-identifier="book-id">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>示例书籍</dc:title>
    <dc:creator>示例作者</dc:creator>
    <dc:language>zh-CN</dc:language>
    <dc:identifier id="book-id">urn:uuid:demo-epub2</dc:identifier>
  </metadata>
  <manifest>
    <item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/>
    <item id="chapter1" href="chapter1.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine toc="ncx">
    <itemref idref="chapter1"/>
  </spine>
</package>
"#;

const NCX: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1">
  <head>
    <meta name="dtb:uid" content="urn:uuid:demo-epub2"/>
  </head>
  <docTitle><text>示例书籍</text></docTitle>
  <navMap>
    <navPoint id="nav-1" playOrder="1">
      <navLabel><text>第一章</text></navLabel>
      <content src="chapter1.xhtml"/>
    </navPoint>
  </navMap>
</ncx>
"#;

const OPF3: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="book-id">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>示例书籍</dc:title>
    <dc:creator>示例作者</dc:creator>
    <dc:language>zh-CN</dc:language>
    <dc:identifier id="book-id">urn:uuid:demo-epub3</dc:identifier>
    <meta property="dcterms:modified">2026-11-14T00:00:00Z</meta>
  </metadata>
  <manifest>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="chapter1" href="chapter1.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine>
    <itemref idref="chapter1"/>
  </spine>
</package>
"#;

const OPF3_NESTED: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="book-id">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>嵌套目录示例</dc:title>
    <dc:creator>示例作者</dc:creator>
    <dc:language>zh-CN</dc:language>
    <dc:identifier id="book-id">urn:uuid:demo-nested</dc:identifier>
    <meta property="dcterms:modified">2026-11-14T00:00:00Z</meta>
  </metadata>
  <manifest>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="chapter1" href="chapter1.xhtml" media-type="application/xhtml+xml"/>
    <item id="chapter2" href="chapter2.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine>
    <itemref idref="chapter1"/>
    <itemref idref="chapter2"/>
  </spine>
</package>
"#;

const OPF3_MISSING_METADATA: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="book-id">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:identifier id="book-id">urn:uuid:demo-missing-metadata</dc:identifier>
    <meta property="dcterms:modified">2026-11-14T00:00:00Z</meta>
  </metadata>
  <manifest>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="chapter1" href="chapter1.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine>
    <itemref idref="chapter1"/>
  </spine>
</package>
"#;

const OPF3_CSS_IMAGE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="book-id">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>图文混排示例</dc:title>
    <dc:creator>示例作者</dc:creator>
    <dc:language>zh-CN</dc:language>
    <dc:identifier id="book-id">urn:uuid:demo-css-image</dc:identifier>
    <meta property="dcterms:modified">2026-11-14T00:00:00Z</meta>
  </metadata>
  <manifest>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="style" href="style.css" media-type="text/css"/>
    <item id="cover-image" href="cover.png" media-type="image/png" properties="cover-image"/>
    <item id="font" href="font.ttf" media-type="font/ttf"/>
    <item id="chapter1" href="chapter1.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine>
    <itemref idref="chapter1"/>
  </spine>
</package>
"#;

const OPF3_BILINGUAL: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="book-id">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>Bilingual Sample</dc:title>
    <dc:creator>示例作者</dc:creator>
    <dc:language>zh-CN</dc:language>
    <dc:identifier id="book-id">urn:uuid:demo-bilingual</dc:identifier>
    <meta property="dcterms:modified">2026-11-14T00:00:00Z</meta>
  </metadata>
  <manifest>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="chapter1" href="chapter1.xhtml" media-type="application/xhtml+xml"/>
    <item id="chapter2" href="chapter2.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine>
    <itemref idref="chapter1"/>
    <itemref idref="chapter2"/>
  </spine>
</package>
"#;

const STYLE_CSS: &str = "body { font-family: serif; }\np { text-indent: 2em; }\n";

const FONT_TTF: &[u8] = &[
    0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00,
];

const COVER_PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae,
    0x42, 0x60, 0x82,
];

/// 生成后的临时 EPUB 文件；持有 `TempDir`，drop 时自动清理。
pub struct Fixture {
    dir: FixtureDir,
    path: PathBuf,
}

impl Fixture {
    /// 临时 `.epub` 文件路径。
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 临时目录路径。
    pub fn dir(&self) -> &Path {
        self.dir.path()
    }

    /// 读取整个文件字节，便于检查不安全条目名等结构。
    pub fn read_bytes(&self) -> Vec<u8> {
        std::fs::read(&self.path).expect("夹具文件应可读取")
    }
}

/// 已声明的夹具类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixtureKind {
    /// 最小 EPUB2：container、OPF v2、NCX 和一章 XHTML。
    MinimalEpub2,
    /// 最小 EPUB3：nav、OPF v3 和一章 XHTML。
    MinimalEpub3,
    /// 至少两层目录节点。
    NestedToc,
    /// 缺少书名、作者和封面。
    MissingMetadata,
    /// 内嵌 CSS 与 PNG 封面图片。
    CssAndImage,
    /// 中文和英文正文。
    Bilingual,
    /// container 指向的 OPF 缺失。
    MissingOpf,
    /// 包含 `..`、绝对路径和 Windows 盘符条目名。
    UnsafePaths,
    /// 包含两个同名 ZIP 条目。
    DuplicateEntries,
}

impl FixtureKind {
    /// 全部夹具类型，供穷举测试使用。
    pub const ALL: [Self; 9] = [
        Self::MinimalEpub2,
        Self::MinimalEpub3,
        Self::NestedToc,
        Self::MissingMetadata,
        Self::CssAndImage,
        Self::Bilingual,
        Self::MissingOpf,
        Self::UnsafePaths,
        Self::DuplicateEntries,
    ];

    /// 夹具文件名。
    pub const fn filename(self) -> &'static str {
        match self {
            Self::MinimalEpub2 => "minimal-epub2.epub",
            Self::MinimalEpub3 => "minimal-epub3.epub",
            Self::NestedToc => "nested-toc.epub",
            Self::MissingMetadata => "missing-metadata.epub",
            Self::CssAndImage => "css-and-image.epub",
            Self::Bilingual => "bilingual.epub",
            Self::MissingOpf => "missing-opf.epub",
            Self::UnsafePaths => "unsafe-paths.epub",
            Self::DuplicateEntries => "duplicate-entries.epub",
        }
    }

    /// 生成对应的临时 EPUB 文件。
    pub fn build(self) -> Fixture {
        match self {
            Self::MinimalEpub2 => write_fixture(
                self.filename(),
                vec![
                    entry("META-INF/container.xml", CONTAINER_XML),
                    entry("OEBPS/content.opf", OPF2),
                    entry("OEBPS/toc.ncx", NCX),
                    entry("OEBPS/chapter1.xhtml", chapter("这是第一章。")),
                ],
            ),
            Self::MinimalEpub3 => write_fixture(
                self.filename(),
                vec![
                    entry("META-INF/container.xml", CONTAINER_XML),
                    entry("OEBPS/content.opf", OPF3),
                    entry(
                        "OEBPS/nav.xhtml",
                        nav("<ol><li><a href=\"chapter1.xhtml\">第一章</a></li></ol>"),
                    ),
                    entry("OEBPS/chapter1.xhtml", chapter("这是第一章。")),
                ],
            ),
            Self::NestedToc => write_fixture(
                self.filename(),
                vec![
                    entry("META-INF/container.xml", CONTAINER_XML),
                    entry("OEBPS/content.opf", OPF3_NESTED),
                    entry(
                        "OEBPS/nav.xhtml",
                        nav(concat!(
                            "<ol>",
                            "<li><a href=\"chapter1.xhtml\">第一部分</a>",
                            "<ol>",
                            "<li><a href=\"chapter1.xhtml#s1\">第一节</a></li>",
                            "<li><a href=\"chapter1.xhtml#s2\">第二节</a></li>",
                            "</ol>",
                            "</li>",
                            "<li><a href=\"chapter2.xhtml\">第二部分</a></li>",
                            "</ol>"
                        )),
                    ),
                    entry(
                        "OEBPS/chapter1.xhtml",
                        chapter("<h1 id=\"s1\">第一节</h1><h2 id=\"s2\">第二节</h2>"),
                    ),
                    entry("OEBPS/chapter2.xhtml", chapter("第二部分正文。")),
                ],
            ),
            Self::MissingMetadata => write_fixture(
                self.filename(),
                vec![
                    entry("META-INF/container.xml", CONTAINER_XML),
                    entry("OEBPS/content.opf", OPF3_MISSING_METADATA),
                    entry(
                        "OEBPS/nav.xhtml",
                        nav("<ol><li><a href=\"chapter1.xhtml\">第一章</a></li></ol>"),
                    ),
                    entry("OEBPS/chapter1.xhtml", chapter("没有元数据。")),
                ],
            ),
            Self::CssAndImage => write_fixture(
                self.filename(),
                vec![
                    entry("META-INF/container.xml", CONTAINER_XML),
                    entry("OEBPS/content.opf", OPF3_CSS_IMAGE),
                    entry(
                        "OEBPS/nav.xhtml",
                        nav("<ol><li><a href=\"chapter1.xhtml\">第一章</a></li></ol>"),
                    ),
                    entry("OEBPS/style.css", STYLE_CSS),
                    ZipEntry::new("OEBPS/cover.png", COVER_PNG.to_vec()),
                    ZipEntry::new("OEBPS/font.ttf", FONT_TTF.to_vec()),
                    entry(
                        "OEBPS/chapter1.xhtml",
                        chapter("<p>带样式的正文。</p><img src=\"cover.png\" alt=\"封面\"/>"),
                    ),
                ],
            ),
            Self::Bilingual => write_fixture(
                self.filename(),
                vec![
                    entry("META-INF/container.xml", CONTAINER_XML),
                    entry("OEBPS/content.opf", OPF3_BILINGUAL),
                    entry(
                        "OEBPS/nav.xhtml",
                        nav(concat!(
                            "<ol>",
                            "<li><a href=\"chapter1.xhtml\">第一章</a></li>",
                            "<li><a href=\"chapter2.xhtml\">Chapter Two</a></li>",
                            "</ol>"
                        )),
                    ),
                    entry("OEBPS/chapter1.xhtml", chapter("这是中文第一章。")),
                    entry("OEBPS/chapter2.xhtml", chapter("This is chapter two.")),
                ],
            ),
            Self::MissingOpf => write_fixture(
                self.filename(),
                vec![
                    entry("META-INF/container.xml", CONTAINER_XML),
                    entry("OEBPS/chapter1.xhtml", chapter("缺少 OPF。")),
                ],
            ),
            Self::UnsafePaths => write_fixture(
                self.filename(),
                vec![
                    entry("META-INF/container.xml", CONTAINER_XML),
                    entry("OEBPS/content.opf", OPF3),
                    entry(
                        "OEBPS/nav.xhtml",
                        nav("<ol><li><a href=\"chapter1.xhtml\">第一章</a></li></ol>"),
                    ),
                    entry("OEBPS/chapter1.xhtml", chapter("正常章节。")),
                    entry("../evil.txt", "escaped"),
                    entry("/absolute.txt", "absolute"),
                    entry("C:\\windows\\evil.txt", "drive"),
                    entry("OEBPS/../../escape.txt", "escape"),
                ],
            ),
            Self::DuplicateEntries => write_fixture(
                self.filename(),
                vec![
                    entry("META-INF/container.xml", CONTAINER_XML),
                    entry("OEBPS/content.opf", OPF3),
                    entry(
                        "OEBPS/nav.xhtml",
                        nav("<ol><li><a href=\"chapter1.xhtml\">第一章</a></li></ol>"),
                    ),
                    entry("OEBPS/chapter1.xhtml", chapter("第一次出现。")),
                    entry("OEBPS/chapter1.xhtml", chapter("第二次出现。")),
                ],
            ),
        }
    }
}

fn entry(name: &str, data: impl Into<Vec<u8>>) -> ZipEntry {
    ZipEntry::new(name, data)
}

fn write_fixture(filename: &str, body: Vec<ZipEntry>) -> Fixture {
    let mut entries = Vec::with_capacity(body.len() + 1);
    entries.push(ZipEntry::new("mimetype", MIMETYPE));
    entries.extend(body);

    let dir = FixtureDir::new();
    let path = dir.path().join(filename);
    std::fs::write(&path, build_zip(&entries)).expect("应能写入夹具文件");
    Fixture { dir, path }
}

fn chapter(body: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xml:lang="zh-CN" lang="zh-CN">
  <head><title>章节</title></head>
  <body>{body}</body>
</html>
"#
    )
}

fn nav(items: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops">
  <head><title>目录</title></head>
  <body>
    <nav epub:type="toc" id="toc">
      <h1>目录</h1>
      {items}
    </nav>
  </body>
</html>
"#
    )
}

/// 加密清单的 XML 书写风格，用于验证解析器不依赖字符串格式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncryptionStyle {
    /// 双引号、`enc:` 命名空间前缀、标准属性顺序。
    DoubleQuoted,
    /// 单引号、额外的命名空间属性位于 URI 之前。
    SingleQuoted,
    /// 不同前缀（`x:`）且属性顺序被打乱。
    Prefixed,
}

/// 生成包含 `META-INF/encryption.xml` 的 EPUB3 夹具。
pub fn encrypted_epub(uri: &str, style: EncryptionStyle) -> Fixture {
    let header = r#"<?xml version="1.0" encoding="UTF-8"?>
<encryption xmlns="urn:oasis:names:tc:opendocument:xmlns:container"
            xmlns:enc="http://www.w3.org/2001/04/xmlenc#">"#;
    let xml = match style {
        EncryptionStyle::DoubleQuoted => format!(
            r#"{header}
  <enc:EncryptedData>
    <enc:EncryptionMethod Algorithm="http://www.w3.org/2001/04/xmlenc#aes128-cbc"/>
    <enc:CipherData>
      <enc:CipherReference URI="{uri}"/>
    </enc:CipherData>
  </enc:EncryptedData>
</encryption>
"#
        ),
        EncryptionStyle::SingleQuoted => format!(
            r#"{header}
  <enc:EncryptedData>
    <enc:CipherData>
      <enc:CipherReference xmlns:foo="urn:example" foo:name="value" URI='{uri}'/>
    </enc:CipherData>
  </enc:EncryptedData>
</encryption>
"#
        ),
        EncryptionStyle::Prefixed => format!(
            r#"{header}
  <enc:EncryptedData>
    <enc:CipherData>
      <enc:CipherReference data-order="reversed" URI="{uri}" xmlns:x="urn:example" x:flag="1"/>
    </enc:CipherData>
  </enc:EncryptedData>
</encryption>
"#
        ),
    };

    write_fixture(
        "encrypted.epub",
        vec![
            entry("META-INF/container.xml", CONTAINER_XML),
            entry("OEBPS/content.opf", OPF3),
            entry(
                "OEBPS/nav.xhtml",
                nav("<ol><li><a href=\"chapter1.xhtml\">第一章</a></li></ol>"),
            ),
            entry("OEBPS/chapter1.xhtml", chapter("加密测试。")),
            entry("META-INF/encryption.xml", xml),
        ],
    )
}

const OPF3_LARGE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="book-id">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>大文件样本</dc:title>
    <dc:creator>示例作者</dc:creator>
    <dc:language>zh-CN</dc:language>
    <dc:identifier id="book-id">urn:uuid:demo-large</dc:identifier>
    <meta property="dcterms:modified">2026-11-14T00:00:00Z</meta>
  </metadata>
  <manifest>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="chapter1" href="chapter1.xhtml" media-type="application/xhtml+xml"/>
    <item id="big" href="images/big.bin" media-type="application/octet-stream"/>
  </manifest>
  <spine>
    <itemref idref="chapter1"/>
  </spine>
</package>
"#;

/// 生成约 100 MB 的有效 EPUB，用于预检读取量和按需读取回归。
pub fn large_epub() -> Fixture {
    let mut big = vec![0_u8; 100 * 1024 * 1024];
    for (index, byte) in big.iter_mut().enumerate() {
        *byte = (index % 251) as u8;
    }
    write_fixture(
        "large.epub",
        vec![
            entry("META-INF/container.xml", CONTAINER_XML),
            entry("OEBPS/content.opf", OPF3_LARGE),
            entry(
                "OEBPS/nav.xhtml",
                nav("<ol><li><a href=\"chapter1.xhtml\">第一章</a></li></ol>"),
            ),
            entry("OEBPS/chapter1.xhtml", chapter("大文件样本。")),
            ZipEntry::new("OEBPS/images/big.bin", big),
        ],
    )
}
