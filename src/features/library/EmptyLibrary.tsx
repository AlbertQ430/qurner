import { ImportButton } from "../import/ImportButton";

export function EmptyLibrary() {
  return (
    <section className="library-empty" aria-labelledby="library-empty-title">
      <p id="library-empty-title" className="library-empty-title">
        书库还是空的
      </p>
      <p>稍后可以通过导入添加书籍。</p>
      <ImportButton />
    </section>
  );
}
