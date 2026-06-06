// Shared YAML parser for Hayate/proto/protocol.yaml (block-only, fixed indentation).

export function parseYaml(text) {
  const result = {};
  let section = null;
  let currentItem = null;
  let currentParam = null;

  function setKv(obj, key, rawVal) {
    const val = rawVal.replace(/^"(.*)"$/, '$1');
    obj[key] = val;
  }

  for (const rawLine of text.split('\n')) {
    const noComment = rawLine.replace(/#.*$/, '').trimEnd();
    if (!noComment.trim()) continue;

    const indent = noComment.length - noComment.trimStart().length;
    const content = noComment.trim();

    if (indent === 0) {
      if (content.endsWith(':')) {
        const name = content.slice(0, -1);
        if (name !== 'version') {
          section = name;
          result[section] = [];
        }
      }
      currentItem = null;
      currentParam = null;
      continue;
    }

    if (!section) continue;

    if (indent === 2) {
      if (content.startsWith('- ')) {
        const rest = content.slice(2);
        currentItem = {};
        result[section].push(currentItem);
        currentParam = null;
        if (rest.includes(':')) {
          const colonIdx = rest.indexOf(':');
          const k = rest.slice(0, colonIdx).trim();
          const v = rest.slice(colonIdx + 1).trim();
          setKv(currentItem, k, v);
        }
      }
      continue;
    }

    if (!currentItem) continue;

    if (indent === 4) {
      if (content.startsWith('- ')) {
        const rest = content.slice(2);
        currentParam = {};
        if (!Array.isArray(currentItem._cur_list)) currentItem._cur_list = [];
        currentItem._cur_list.push(currentParam);
        if (rest.includes(':')) {
          const colonIdx = rest.indexOf(':');
          const k = rest.slice(0, colonIdx).trim();
          const v = rest.slice(colonIdx + 1).trim();
          setKv(currentParam, k, v);
        }
      } else if (content.includes(':')) {
        const colonIdx = content.indexOf(':');
        const key = content.slice(0, colonIdx).trim();
        const val = content.slice(colonIdx + 1).trim();
        if (val === '') {
          currentItem[key] = [];
          currentItem._active_list = key;
          currentParam = null;
        } else {
          const cleanVal = val.replace(/^"(.*)"$/, '$1');
          currentItem[key] = cleanVal;
        }
      }
      continue;
    }

    if (indent === 6) {
      if (content.startsWith('- ')) {
        const rest = content.slice(2);
        currentParam = {};
        const listKey = currentItem._active_list;
        if (listKey && Array.isArray(currentItem[listKey])) {
          currentItem[listKey].push(currentParam);
        }
        if (rest.includes(':')) {
          const colonIdx = rest.indexOf(':');
          const k = rest.slice(0, colonIdx).trim();
          const v = rest.slice(colonIdx + 1).trim();
          setKv(currentParam, k, v);
        }
      } else if (content.includes(':') && currentParam) {
        const colonIdx = content.indexOf(':');
        const k = content.slice(0, colonIdx).trim();
        const v = content.slice(colonIdx + 1).trim();
        setKv(currentParam, k, v);
      }
      continue;
    }

    if (indent === 8 && currentParam) {
      if (content.includes(':')) {
        const colonIdx = content.indexOf(':');
        const k = content.slice(0, colonIdx).trim();
        const v = content.slice(colonIdx + 1).trim();
        setKv(currentParam, k, v);
      }
    }
  }

  for (const items of Object.values(result)) {
    for (const item of items) {
      delete item._active_list;
      delete item._cur_list;
    }
  }

  return result;
}

export function toCamelCase(s) {
  return s.replace(/_([a-z])/g, (_, c) => c.toUpperCase());
}

export function tagToPatchKey(name) {
  const lower = name.toLowerCase();
  return lower.replace(/_([a-z])/g, (_, c) => c.toUpperCase());
}

export function toKebabCase(camel) {
  return camel.replace(/[A-Z]/g, (m) => `-${m.toLowerCase()}`);
}
