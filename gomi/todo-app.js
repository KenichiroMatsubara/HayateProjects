/* ============================================================
   POP TODO — SolidJS app
   solid-js/html (タグ付きテンプレート) を使用、ビルド不要。
   ============================================================ */
import { createSignal, createMemo, For, Show } from "https://esm.sh/solid-js@1.9.5";
import { render } from "https://esm.sh/solid-js@1.9.5/web";
import html from "https://esm.sh/solid-js@1.9.5/html";

/* ---------- テーマ & アクセント色 (vanilla) ---------- */
const root = document.documentElement;
const THEME_KEY = "pop-todo-theme";
const HUE_KEY = "pop-todo-hue";

function applyTheme(theme) {
  root.setAttribute("data-theme", theme);
  const btn = document.querySelector(".theme-toggle");
  if (btn) btn.textContent = theme === "dark" ? "☀ ライト" : "● ダーク";
}
applyTheme(localStorage.getItem(THEME_KEY) || "light");

document.querySelector(".theme-toggle").addEventListener("click", () => {
  const next = root.getAttribute("data-theme") === "dark" ? "light" : "dark";
  localStorage.setItem(THEME_KEY, next);
  applyTheme(next);
});

function applyHue(hue) {
  root.style.setProperty("--hue", hue);
  document.querySelectorAll(".hue-swatch").forEach((b) => {
    b.setAttribute("aria-pressed", b.dataset.hue === String(hue) ? "true" : "false");
  });
}
applyHue(localStorage.getItem(HUE_KEY) || "330");

document.querySelectorAll(".hue-swatch").forEach((btn) => {
  btn.addEventListener("click", () => {
    localStorage.setItem(HUE_KEY, btn.dataset.hue);
    applyHue(btn.dataset.hue);
  });
});

document.querySelector(".scroll-top").addEventListener("click", () => {
  window.scrollTo({ top: 0, behavior: "smooth" });
});

/* ---------- TODO データ ---------- */
const STORAGE = "pop-todo-items-v1";
const seed = [
  { id: 1, text: "レイアウトエンジンに flex-wrap を実装", prio: 3, done: false },
  { id: 2, text: "box-shadow の描画を確認する", prio: 2, done: true },
  { id: 3, text: "ドラッグで並べ替えできるかテスト", prio: 2, done: false },
  { id: 4, text: "ダークモードの配色を調整", prio: 1, done: false },
  { id: 5, text: "sticky ヘッダーの挙動チェック", prio: 3, done: true },
];

function load() {
  try {
    const raw = localStorage.getItem(STORAGE);
    if (raw) {
      const arr = JSON.parse(raw);
      if (Array.isArray(arr) && arr.length) return arr;
    }
  } catch (e) { /* ignore */ }
  return seed;
}

/* ---------- アプリ ---------- */
function App() {
  const [todos, setTodos] = createSignal(load());
  const [filter, setFilter] = createSignal("all");
  const [sortMode, setSortMode] = createSignal("manual");
  const [newPrio, setNewPrio] = createSignal(2);
  const [editingId, setEditingId] = createSignal(null);
  const [dragId, setDragId] = createSignal(null);
  const [overId, setOverId] = createSignal(null);

  const update = (fn) =>
    setTodos((t) => {
      const next = fn(t);
      localStorage.setItem(STORAGE, JSON.stringify(next));
      return next;
    });

  const remaining = createMemo(() => todos().filter((t) => !t.done).length);
  const pct = createMemo(() => {
    const total = todos().length;
    return total === 0 ? 0 : Math.round(((total - remaining()) / total) * 100);
  });

  const filtered = createMemo(() => {
    let list = todos();
    if (filter() === "active") list = list.filter((t) => !t.done);
    if (filter() === "done") list = list.filter((t) => t.done);
    if (sortMode() === "name")
      list = [...list].sort((a, b) => a.text.localeCompare(b.text, "ja"));
    if (sortMode() === "prio") list = [...list].sort((a, b) => b.prio - a.prio);
    return list;
  });

  const addTodo = (e) => {
    e.preventDefault();
    const input = e.currentTarget.querySelector(".add-input");
    const text = input.value.trim();
    if (!text) return;
    update((t) => [
      { id: Date.now(), text, prio: newPrio(), done: false },
      ...t,
    ]);
    input.value = "";
    input.focus();
  };

  const toggleDone = (id) =>
    update((t) => t.map((x) => (x.id === id ? { ...x, done: !x.done } : x)));

  const removeTodo = (id) => update((t) => t.filter((x) => x.id !== id));

  const commitEdit = (id, value) => {
    const text = value.trim();
    if (text) update((t) => t.map((x) => (x.id === id ? { ...x, text } : x)));
    setEditingId(null);
  };

  const clearDone = () => update((t) => t.filter((x) => !x.done));

  const dropOn = (targetId) => {
    const from = dragId();
    setDragId(null);
    setOverId(null);
    if (from == null || from === targetId) return;
    update((t) => {
      const list = [...t];
      const fromIdx = list.findIndex((x) => x.id === from);
      const [moved] = list.splice(fromIdx, 1);
      const toIdx = list.findIndex((x) => x.id === targetId);
      list.splice(toIdx, 0, moved);
      return list;
    });
  };

  const prioBtn = (val, label) => html`
    <button
      type="button"
      class="prio-btn"
      data-prio=${String(val)}
      aria-pressed=${() => (newPrio() === val ? "true" : "false")}
      onclick=${() => setNewPrio(val)}
    >${label}</button>`;

  const filterChip = (val, label) => html`
    <button
      type="button"
      class="chip"
      aria-pressed=${() => (filter() === val ? "true" : "false")}
      onclick=${() => setFilter(val)}
    >${label}</button>`;

  const sortChip = (val, label) => html`
    <button
      type="button"
      class="chip"
      aria-pressed=${() => (sortMode() === val ? "true" : "false")}
      onclick=${() => setSortMode(val)}
    >${label}</button>`;

  const item = (todo) => html`
    <li
      class=${() =>
        "todo-item" +
        (todo.done ? " done" : "") +
        (dragId() === todo.id ? " dragging" : "") +
        (overId() === todo.id && dragId() !== todo.id ? " drag-over" : "")}
      data-prio=${String(todo.prio)}
      draggable=${() => (sortMode() === "manual" && editingId() !== todo.id ? "true" : "false")}
      ondragstart=${(e) => {
        setDragId(todo.id);
        e.dataTransfer.effectAllowed = "move";
      }}
      ondragend=${() => { setDragId(null); setOverId(null); }}
      ondragover=${(e) => {
        if (sortMode() !== "manual") return;
        e.preventDefault();
        setOverId(todo.id);
      }}
      ondragleave=${() => { if (overId() === todo.id) setOverId(null); }}
      ondrop=${(e) => { e.preventDefault(); dropOn(todo.id); }}
    >
      <button type="button" class="drag-handle" title="ドラッグで並べ替え(手動のとき)" tabindex="-1">⠿</button>
      <button
        type="button"
        class="check"
        role="checkbox"
        aria-checked=${todo.done ? "true" : "false"}
        aria-label="完了にする"
        onclick=${() => toggleDone(todo.id)}
      ></button>
      <span class="prio-dot" title=${"優先度 " + ["", "低", "中", "高"][todo.prio]}></span>
      <${Show}
        when=${() => editingId() === todo.id}
        fallback=${html`<span class="todo-text" ondblclick=${() => setEditingId(todo.id)}>${todo.text}</span>`}
      >
        <form class="edit-form" onsubmit=${(e) => {
          e.preventDefault();
          commitEdit(todo.id, e.currentTarget.querySelector(".edit-input").value);
        }}>
          <input
            class="edit-input"
            value=${todo.text}
            ref=${(el) => setTimeout(() => { el.focus(); el.select(); })}
            onblur=${(e) => commitEdit(todo.id, e.target.value)}
            onkeydown=${(e) => { if (e.key === "Escape") setEditingId(null); }}
          />
        </form>
      <//>
      <div class="item-actions">
        <button type="button" class="icon-btn" title="編集" onclick=${() => setEditingId(todo.id)}>✎</button>
        <button type="button" class="icon-btn danger" title="削除" onclick=${() => removeTodo(todo.id)}>✕</button>
      </div>
    </li>`;

  return html`
    <div class="todo-card">
      <div class="todo-head">
        <div class="todo-head-row">
          <h2 class="todo-title">きょうのタスク</h2>
          <span class="todo-count">${() => "残り " + remaining() + " 件 / 全 " + todos().length + " 件"}</span>
        </div>
        <div class="progress-track" title="完了率">
          <div class="progress-fill" style=${() => "width:" + pct() + "%"}></div>
        </div>
      </div>

      <form class="add-form" onsubmit=${addTodo}>
        <input class="add-input" type="text" placeholder="新しいタスクを入力…" aria-label="新しいタスク" />
        <div class="prio-seg" role="group" aria-label="優先度">
          ${prioBtn(3, "高")}${prioBtn(2, "中")}${prioBtn(1, "低")}
        </div>
        <button type="submit" class="add-btn">追加</button>
      </form>

      <div class="todo-toolbar">
        <span class="toolbar-label">表示</span>
        ${filterChip("all", "すべて")}${filterChip("active", "未完了")}${filterChip("done", "完了済み")}
        <span class="toolbar-divider"></span>
        <span class="toolbar-label">並び</span>
        ${sortChip("manual", "手動")}${sortChip("name", "名前")}${sortChip("prio", "優先度")}
      </div>

      <ul class="todo-list">
        <${For} each=${filtered}>${item}<//>
        <${Show} when=${() => filtered().length === 0}>
          <li class="empty">表示するタスクがありません</li>
        <//>
      </ul>

      <div class="todo-foot">
        <span>${() => pct() + "% 完了"}</span>
        <span class="spacer"></span>
        <span class="mono">ダブルクリックで編集 / ⠿ をドラッグ</span>
        <button type="button" class="clear-btn" onclick=${clearDone}>完了を消す</button>
      </div>
    </div>`;
}

render(App, document.getElementById("todo-app"));
