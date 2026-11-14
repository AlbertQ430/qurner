mod support;

use epub_core::{MAX_RESOURCE_BYTES, ParsedBook, ResourcePath, TocNode, parse_epub, read_resource};
use support::fixtures::FixtureKind;

fn resolve(book: &ParsedBook, href: &str) -> (ResourcePath, Option<String>) {
    let (path, fragment) = match href.split_once('#') {
        Some((path, fragment)) => (path, Some(fragment.to_string())),
        None => (href, None),
    };
    let base = book
        .package_path
        .as_str()
        .rsplit_once('/')
        .map(|(directory, _)| directory)
        .unwrap_or("");
    let full = if path.is_empty() {
        base.to_string()
    } else if base.is_empty() || path.starts_with(&format!("{base}/")) {
        path.to_string()
    } else {
        format!("{base}/{path}")
    };
    (
        ResourcePath::new(&full).expect("目录 href 应解析为合法资源路径"),
        fragment,
    )
}

fn walk(
    book: &ParsedBook,
    nodes: &[TocNode],
    output: &mut Vec<(String, ResourcePath, Option<String>)>,
) {
    for node in nodes {
        let (resource, fragment) = resolve(book, &node.href);
        output.push((node.label.clone(), resource, fragment));
        walk(book, &node.children, output);
    }
}

#[test]
fn nested_toc_maps_to_spine_and_fragments() {
    let fixture = FixtureKind::NestedToc.build();
    let book = parse_epub(fixture.path()).expect("应能解析嵌套目录夹具");

    let mut entries = Vec::new();
    walk(&book, &book.toc, &mut entries);

    let find = |label: &str| {
        entries
            .iter()
            .find(|(entry_label, _, _)| entry_label == label)
            .expect("目录项应存在")
    };

    let (_, part1, part1_fragment) = find("第一部分");
    assert_eq!(part1.as_str(), "OEBPS/chapter1.xhtml");
    assert!(part1_fragment.is_none());
    let part1_index = book
        .spine
        .iter()
        .position(|item| &item.href == part1)
        .expect("第一部分应映射到 spine");
    assert_eq!(part1_index, 0);

    let (_, s1, s1_fragment) = find("第一节");
    assert_eq!(s1.as_str(), "OEBPS/chapter1.xhtml");
    assert_eq!(s1_fragment.as_deref(), Some("s1"));
    let (_, s2, s2_fragment) = find("第二节");
    assert_eq!(s2.as_str(), "OEBPS/chapter1.xhtml");
    assert_eq!(s2_fragment.as_deref(), Some("s2"));

    let (_, part2, part2_fragment) = find("第二部分");
    assert_eq!(part2.as_str(), "OEBPS/chapter2.xhtml");
    assert!(part2_fragment.is_none());
    let part2_index = book
        .spine
        .iter()
        .position(|item| &item.href == part2)
        .expect("第二部分应映射到 spine");
    assert_eq!(part2_index, 1);

    let chapter = read_resource(fixture.path(), s1, MAX_RESOURCE_BYTES).expect("应能读取章节");
    let text = String::from_utf8(chapter).expect("章节应为 UTF-8");
    assert!(text.contains("id=\"s1\""));
    assert!(text.contains("id=\"s2\""));

    eprintln!("M3D_TOC entries={entries:?}");
}
