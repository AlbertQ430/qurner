import { EmptyLibrary } from "../features/library/EmptyLibrary";

export default function App() {
  return (
    <main className="app">
      <header className="app-header">
        <h1>Qurner</h1>
        <p className="app-subtitle">个人电子书阅读器</p>
      </header>
      <EmptyLibrary />
    </main>
  );
}
