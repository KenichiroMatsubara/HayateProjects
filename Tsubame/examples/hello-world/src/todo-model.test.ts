import { describe, expect, it } from 'vitest';
import {
  add,
  clearDone,
  completion,
  deserialize,
  editText,
  filterTodos,
  loadTodos,
  moveDown,
  moveUp,
  remove,
  saveTodos,
  SEED,
  serialize,
  sortTodos,
  type StorageLike,
  toggleDone,
  type Todo,
  visibleTodos,
} from './todo-model.js';

function fakeStorage(initial: Record<string, string> = {}): StorageLike {
  const map = new Map(Object.entries(initial));
  return {
    getItem: (key) => map.get(key) ?? null,
    setItem: (key, value) => {
      map.set(key, value);
    },
  };
}

const t = (id: number, text: string, prio: 1 | 2 | 3, done: boolean): Todo => ({
  id,
  text,
  prio,
  done,
});

describe('add', () => {
  it('prepends a new incomplete todo with the given priority', () => {
    const todos = [t(1, 'first', 2, false)];
    const next = add(todos, { id: 2, text: 'second', prio: 3 });
    expect(next).toEqual([
      { id: 2, text: 'second', prio: 3, done: false },
      { id: 1, text: 'first', prio: 2, done: false },
    ]);
  });

  it('trims surrounding whitespace from the text', () => {
    const next = add([], { id: 1, text: '  buy milk  ', prio: 2 });
    expect(next[0].text).toBe('buy milk');
  });

  it('rejects empty or whitespace-only text, leaving the list unchanged', () => {
    const todos = [t(1, 'first', 2, false)];
    expect(add(todos, { id: 2, text: '   ', prio: 1 })).toEqual(todos);
    expect(add(todos, { id: 2, text: '', prio: 1 })).toEqual(todos);
  });
});

describe('toggleDone', () => {
  it('flips the done flag of the matching todo, leaving others alone', () => {
    const todos = [t(1, 'a', 2, false), t(2, 'b', 2, true)];
    expect(toggleDone(todos, 1)).toEqual([t(1, 'a', 2, true), t(2, 'b', 2, true)]);
    expect(toggleDone(todos, 2)).toEqual([t(1, 'a', 2, false), t(2, 'b', 2, false)]);
  });

  it('does not mutate the input array or its todos', () => {
    const todos = [t(1, 'a', 2, false)];
    toggleDone(todos, 1);
    expect(todos[0].done).toBe(false);
  });

  it('returns an unchanged list when no id matches', () => {
    const todos = [t(1, 'a', 2, false)];
    expect(toggleDone(todos, 99)).toEqual(todos);
  });
});

describe('editText', () => {
  it('replaces the matching todo text, trimming whitespace', () => {
    const todos = [t(1, 'old', 2, false), t(2, 'keep', 1, true)];
    expect(editText(todos, 1, '  new  ')).toEqual([t(1, 'new', 2, false), t(2, 'keep', 1, true)]);
  });

  it('ignores empty or whitespace-only text, leaving the list unchanged', () => {
    const todos = [t(1, 'old', 2, false)];
    expect(editText(todos, 1, '   ')).toEqual(todos);
  });
});

describe('remove', () => {
  it('drops the todo with the matching id', () => {
    const todos = [t(1, 'a', 2, false), t(2, 'b', 1, true)];
    expect(remove(todos, 1)).toEqual([t(2, 'b', 1, true)]);
  });
});

describe('clearDone', () => {
  it('drops every completed todo, keeping the active ones in order', () => {
    const todos = [t(1, 'a', 2, true), t(2, 'b', 1, false), t(3, 'c', 3, true)];
    expect(clearDone(todos)).toEqual([t(2, 'b', 1, false)]);
  });
});

describe('moveUp / moveDown', () => {
  const todos = [t(1, 'a', 2, false), t(2, 'b', 2, false), t(3, 'c', 2, false)];

  it('moveUp swaps a todo with the one above it', () => {
    expect(moveUp(todos, 2).map((x) => x.id)).toEqual([2, 1, 3]);
  });

  it('moveDown swaps a todo with the one below it', () => {
    expect(moveDown(todos, 2).map((x) => x.id)).toEqual([1, 3, 2]);
  });

  it('leaves the list unchanged at the edges', () => {
    expect(moveUp(todos, 1)).toEqual(todos);
    expect(moveDown(todos, 3)).toEqual(todos);
  });

  it('leaves the list unchanged for an unknown id', () => {
    expect(moveUp(todos, 99)).toEqual(todos);
    expect(moveDown(todos, 99)).toEqual(todos);
  });
});

describe('filterTodos', () => {
  const todos = [t(1, 'a', 2, false), t(2, 'b', 2, true), t(3, 'c', 2, false)];

  it('returns every todo for "all"', () => {
    expect(filterTodos(todos, 'all')).toEqual(todos);
  });

  it('returns only incomplete todos for "active"', () => {
    expect(filterTodos(todos, 'active').map((x) => x.id)).toEqual([1, 3]);
  });

  it('returns only completed todos for "done"', () => {
    expect(filterTodos(todos, 'done').map((x) => x.id)).toEqual([2]);
  });
});

describe('sortTodos', () => {
  it('preserves the existing order for "manual" without mutating the input', () => {
    const todos = [t(3, 'c', 1, false), t(1, 'a', 2, false), t(2, 'b', 3, false)];
    expect(sortTodos(todos, 'manual').map((x) => x.id)).toEqual([3, 1, 2]);
    expect(todos.map((x) => x.id)).toEqual([3, 1, 2]);
  });

  it('sorts by name using Japanese collation for "name"', () => {
    const todos = [t(1, 'りんご', 2, false), t(2, 'あんず', 2, false), t(3, 'みかん', 2, false)];
    expect(sortTodos(todos, 'name').map((x) => x.text)).toEqual(['あんず', 'みかん', 'りんご']);
  });

  it('sorts by descending priority for "prio"', () => {
    const todos = [t(1, 'a', 1, false), t(2, 'b', 3, false), t(3, 'c', 2, false)];
    expect(sortTodos(todos, 'prio').map((x) => x.id)).toEqual([2, 3, 1]);
  });

  it('does not mutate the input array when sorting', () => {
    const todos = [t(1, 'b', 1, false), t(2, 'a', 1, false)];
    sortTodos(todos, 'name');
    expect(todos.map((x) => x.id)).toEqual([1, 2]);
  });
});

describe('visibleTodos', () => {
  it('applies the filter before sorting so the list shows only matching, ordered todos', () => {
    const todos = [
      t(1, 'low active', 1, false),
      t(2, 'high done', 3, true),
      t(3, 'high active', 3, false),
      t(4, 'mid active', 2, false),
    ];
    // active hides the done todo; prio then orders the survivors high->low.
    expect(visibleTodos(todos, 'active', 'prio').map((x) => x.id)).toEqual([3, 4, 1]);
  });

  it('does not mutate the input list', () => {
    const todos = [t(1, 'b', 1, false), t(2, 'a', 1, false)];
    visibleTodos(todos, 'all', 'name');
    expect(todos.map((x) => x.id)).toEqual([1, 2]);
  });
});

describe('completion', () => {
  it('reports total, remaining, and a rounded percent', () => {
    const todos = [t(1, 'a', 2, true), t(2, 'b', 2, false), t(3, 'c', 2, false)];
    expect(completion(todos)).toEqual({ total: 3, remaining: 2, percent: 33 });
  });

  it('is 100% when every todo is done', () => {
    expect(completion([t(1, 'a', 2, true), t(2, 'b', 2, true)])).toEqual({
      total: 2,
      remaining: 0,
      percent: 100,
    });
  });

  it('is 0% for an empty list (no division by zero)', () => {
    expect(completion([])).toEqual({ total: 0, remaining: 0, percent: 0 });
  });
});

describe('serialize / deserialize', () => {
  it('round-trips a list of todos back to an equal list', () => {
    const todos = [t(1, 'a', 3, false), t(2, 'b', 1, true)];
    expect(deserialize(serialize(todos))).toEqual(todos);
  });

  it('preserves an empty list as a valid state', () => {
    expect(deserialize(serialize([]))).toEqual([]);
  });

  it('falls back to a copy of the seed when storage is empty (null)', () => {
    expect(deserialize(null)).toEqual(SEED);
    expect(deserialize(null)).not.toBe(SEED);
  });

  it('falls back to the seed on malformed JSON', () => {
    expect(deserialize('{not json')).toEqual(SEED);
  });

  it('falls back to the seed when the payload is not an array', () => {
    expect(deserialize('{"id":1}')).toEqual(SEED);
  });

  it('falls back to the seed when any item has the wrong shape', () => {
    expect(deserialize('[{"id":1,"text":"a","prio":2,"done":false},{"id":2}]')).toEqual(SEED);
    expect(deserialize('[{"id":"x","text":"a","prio":2,"done":false}]')).toEqual(SEED);
  });
});

describe('loadTodos / saveTodos', () => {
  it('round-trips todos through a storage backend', () => {
    const storage = fakeStorage();
    const todos = [t(1, 'a', 3, false), t(2, 'b', 1, true)];
    saveTodos(storage, todos);
    expect(loadTodos(storage)).toEqual(todos);
  });

  it('returns the seed when the storage has no saved value', () => {
    expect(loadTodos(fakeStorage())).toEqual(SEED);
  });

  it('returns the seed when the stored value is corrupt', () => {
    expect(loadTodos(fakeStorage({ 'pop-todo-items-v1': 'oops' }))).toEqual(SEED);
  });
});
