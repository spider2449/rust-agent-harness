// Markdown for the model's replies. Model output is untrusted: everything
// becomes DOM nodes with text content and fixed tags, never HTML. Raw HTML
// in the source stays text, links work only for http, https and mailto
// URLs, and images are shown as links (nothing is loaded from elsewhere).
//
// Blocks: ATX headings, paragraphs, fenced code (highlighted when the
// language is known), block quotes, nested bullet, numbered and task lists,
// pipe tables and thematic breaks. Inline: code, strong, emphasis,
// strikethrough, links, autolinks and bare URLs. A single line break inside
// a paragraph is kept, as in chat.
"use strict";

const Markdown = (() => {
  // Replies larger than this stay plain text.
  const MAX_CHARS = 200 * 1024;

  // Fence languages to a file extension the highlighter knows.
  const FENCE = {
    rust: "rs", rs: "rs",
    javascript: "js", js: "js", jsx: "js", typescript: "ts", ts: "ts", tsx: "ts",
    python: "py", py: "py",
    c: "c", h: "c", cpp: "cpp", "c++": "cpp", csharp: "cs", cs: "cs", java: "java", go: "go", kotlin: "kt", swift: "swift",
    toml: "toml", ini: "ini", yaml: "yaml", yml: "yaml", json: "json", jsonl: "json",
    markdown: "md", md: "md",
    sh: "sh", bash: "sh", shell: "sh", zsh: "sh", console: "sh", powershell: "ps1", ps1: "ps1", pwsh: "ps1",
    css: "css", html: "html", xml: "xml", svg: "svg", diff: "diff", patch: "diff",
  };

  const node = (tag, className) => {
    const element = document.createElement(tag);
    if (className) element.className = className;
    return element;
  };

  const FENCE_OPEN = /^ {0,3}(`{3,}|~{3,})\s*([^\s`]*)[^`]*$/;
  const HEADING = /^ {0,3}(#{1,6})(?:\s+(.*?))?(?:\s+#+)?\s*$/;
  const RULE = /^ {0,3}([-*_])(?:\s*\1){2,}\s*$/;
  const QUOTE = /^ {0,3}> ?(.*)$/;
  const ITEM = /^( *)([-*+]|\d{1,9}[.)])(?:\s+(.*))?$/;
  const TABLE_RULE = /^\s*\|?\s*:?-+:?\s*(?:\|\s*:?-+:?\s*)*\|?\s*$/;

  function render(text) {
    const fragment = document.createDocumentFragment();
    if (text.length > MAX_CHARS) {
      const pre = node("pre");
      pre.textContent = text;
      fragment.append(pre);
      return fragment;
    }
    blocks(text.replace(/\r\n?/g, "\n").split("\n"), fragment);
    return fragment;
  }

  function startsBlock(line, next) {
    return FENCE_OPEN.test(line) || HEADING.test(line) || RULE.test(line) || QUOTE.test(line)
      || ITEM.test(line) || (line.includes("|") && next !== undefined && TABLE_RULE.test(next) && next.includes("-"));
  }

  function blocks(lines, parent) {
    let i = 0;
    while (i < lines.length) {
      const line = lines[i];
      if (!line.trim()) {
        i += 1;
        continue;
      }
      let match;
      if ((match = FENCE_OPEN.exec(line))) {
        const fence = match[1];
        // Closed by a line of at least as many of the same character.
        const close = new RegExp(`^ {0,3}${fence[0] === "`" ? "`" : "~"}{${fence.length},}\\s*$`);
        const body = [];
        i += 1;
        while (i < lines.length && !close.test(lines[i])) {
          body.push(lines[i]);
          i += 1;
        }
        i += 1;
        parent.append(code(body.join("\n"), match[2].toLowerCase()));
      } else if ((match = HEADING.exec(line))) {
        const heading = node(`h${match[1].length}`);
        inline(match[2] || "", heading);
        parent.append(heading);
        i += 1;
      } else if (RULE.test(line)) {
        parent.append(node("hr"));
        i += 1;
      } else if (QUOTE.test(line)) {
        const inner = [];
        while (i < lines.length && lines[i].trim() && (match = QUOTE.exec(lines[i]))) {
          inner.push(match[1]);
          i += 1;
        }
        const quote = node("blockquote");
        blocks(inner, quote);
        parent.append(quote);
      } else if (ITEM.test(line)) {
        i = list(lines, i, parent);
      } else if (line.includes("|") && i + 1 < lines.length && TABLE_RULE.test(lines[i + 1]) && lines[i + 1].includes("-")) {
        i = table(lines, i, parent);
      } else {
        const text = [line];
        i += 1;
        while (i < lines.length && lines[i].trim() && !startsBlock(lines[i], lines[i + 1])) {
          text.push(lines[i]);
          i += 1;
        }
        const paragraph = node("p");
        inline(text.map((part) => part.trim()).join("\n"), paragraph);
        parent.append(paragraph);
      }
    }
  }

  function code(text, language) {
    const pre = node("pre", "md-code");
    const element = node("code");
    const extension = FENCE[language];
    element.append(extension && typeof Highlight !== "undefined" ? Highlight.render(text, `snippet.${extension}`) : text);
    if (language) pre.dataset.language = language;
    pre.append(element);
    return pre;
  }

  // One list starting at `start`; returns the index after it.
  function list(lines, start, parent) {
    const first = ITEM.exec(lines[start]);
    const indent = first[1].length;
    const ordered = /\d/.test(first[2]);
    const element = node(ordered ? "ol" : "ul");
    if (ordered && parseInt(first[2], 10) !== 1) element.start = parseInt(first[2], 10);
    let i = start;
    let loose = false;
    const items = [];
    while (i < lines.length) {
      const match = ITEM.exec(lines[i]);
      if (!match || match[1].length !== indent || /\d/.test(match[2]) !== ordered) break;
      const width = indent + match[2].length + 1;
      const content = [match[3] || ""];
      i += 1;
      while (i < lines.length) {
        const line = lines[i];
        if (!line.trim()) {
          // A blank line continues the item only when indented text follows.
          const next = lines.slice(i + 1).find((later) => later.trim());
          if (next !== undefined && leading(next) >= width) {
            content.push("");
            loose = true;
            i += 1;
            continue;
          }
          break;
        }
        const nested = ITEM.exec(line);
        if (nested && nested[1].length <= indent) break;
        if (leading(line) < width && !nested && startsBlock(line, lines[i + 1])) break;
        content.push(line.slice(Math.min(leading(line), width)));
        i += 1;
      }
      items.push(content);
      // Blank lines between items make the list loose.
      if (i < lines.length && !lines[i].trim()) {
        const next = lines.slice(i).findIndex((later) => later.trim());
        const following = next < 0 ? undefined : ITEM.exec(lines[i + next]);
        if (following && following[1].length === indent && /\d/.test(following[2]) === ordered) {
          loose = true;
          i += next;
        }
      }
    }
    for (const content of items) {
      const item = node("li");
      const task = /^\[([ xX])\]\s+/.exec(content[0]);
      if (task) {
        const box = node("input");
        box.type = "checkbox";
        box.disabled = true;
        box.checked = task[1] !== " ";
        item.className = "task";
        item.append(box);
        content[0] = content[0].slice(task[0].length);
      }
      blocks(content, item);
      // Tight lists show their single paragraphs without spacing.
      if (!loose) {
        for (const paragraph of [...item.children].filter((child) => child.tagName === "P")) {
          paragraph.replaceWith(...paragraph.childNodes);
        }
      }
      element.append(item);
    }
    parent.append(element);
    return i;
  }

  function leading(line) {
    return line.length - line.trimStart().length;
  }

  function cells(line) {
    let row = line.trim();
    if (row.startsWith("|")) row = row.slice(1);
    if (row.endsWith("|") && !row.endsWith("\\|")) row = row.slice(0, -1);
    return row.split(/(?<!\\)\|/).map((cell) => cell.trim().replace(/\\\|/g, "|"));
  }

  function table(lines, start, parent) {
    const head = cells(lines[start]);
    const aligns = cells(lines[start + 1]).map((rule) =>
      rule.startsWith(":") && rule.endsWith(":") ? "center" : rule.endsWith(":") ? "right" : rule.startsWith(":") ? "left" : "");
    const wrap = node("div", "md-table");
    const element = node("table");
    const row = (values, tag) => {
      const tr = node("tr");
      head.forEach((_, index) => {
        const cell = node(tag);
        if (aligns[index]) cell.style.textAlign = aligns[index];
        inline(values[index] || "", cell);
        tr.append(cell);
      });
      return tr;
    };
    const thead = node("thead");
    thead.append(row(head, "th"));
    const tbody = node("tbody");
    let i = start + 2;
    while (i < lines.length && lines[i].trim() && lines[i].includes("|")) {
      tbody.append(row(cells(lines[i]), "td"));
      i += 1;
    }
    element.append(thead, tbody);
    wrap.append(element);
    parent.append(wrap);
    return i;
  }

  function safeUrl(url) {
    try {
      const parsed = new URL(url);
      return ["http:", "https:", "mailto:"].includes(parsed.protocol) ? parsed.href : null;
    } catch {
      return null;
    }
  }

  function link(url, label, parent) {
    const safe = safeUrl(url);
    if (!safe) {
      inline(label, parent);
      return;
    }
    const anchor = node("a");
    anchor.href = safe;
    anchor.target = "_blank";
    anchor.rel = "noopener noreferrer";
    anchor.title = safe;
    inline(label, anchor);
    parent.append(anchor);
  }

  // The closing `delimiter` for one that opens at `from`, or -1.
  function closing(text, delimiter, from) {
    let at = text.indexOf(delimiter, from);
    while (at >= 0) {
      const before = text[at - 1];
      if (at > from && before !== " " && before !== "\\" && !(delimiter.length === 1 && text[at + 1] === delimiter)) {
        return at;
      }
      at = text.indexOf(delimiter, at + delimiter.length);
    }
    return -1;
  }

  const ESCAPABLE = "\\`*_{}[]()#+-.!|~<>\"'";
  const BARE_URL = /^https?:\/\/[^\s<>"]+/;
  const AUTOLINK = /^<((?:https?:\/\/|mailto:)[^\s<>]+)>/;

  function inline(text, parent) {
    let plain = "";
    const flush = () => {
      if (plain) parent.append(plain);
      plain = "";
    };
    let i = 0;
    while (i < text.length) {
      const c = text[i];
      const rest = text.slice(i);
      let match;
      if (c === "\\" && i + 1 < text.length && ESCAPABLE.includes(text[i + 1])) {
        plain += text[i + 1];
        i += 2;
      } else if (c === "\\" && text[i + 1] === "\n") {
        flush();
        parent.append(node("br"));
        i += 2;
      } else if (c === "\n") {
        flush();
        parent.append(node("br"));
        i += 1;
      } else if (c === "`") {
        const run = /^`+/.exec(rest)[0];
        const end = text.indexOf(run, i + run.length);
        if (end < 0) {
          plain += run;
          i += run.length;
        } else {
          flush();
          const element = node("code");
          let inner = text.slice(i + run.length, end);
          if (/^ .* $/.test(inner)) inner = inner.slice(1, -1);
          element.textContent = inner;
          parent.append(element);
          i = end + run.length;
        }
      } else if ((rest.startsWith("**") || rest.startsWith("__") || rest.startsWith("~~")) && text[i + 2] && text[i + 2] !== " ") {
        const delimiter = rest.slice(0, 2);
        const end = closing(text, delimiter, i + 2);
        if (end < 0) {
          plain += delimiter;
          i += 2;
        } else {
          flush();
          const element = node(delimiter === "~~" ? "del" : "strong");
          inline(text.slice(i + 2, end), element);
          parent.append(element);
          i = end + 2;
        }
      } else if ((c === "*" || (c === "_" && !/\w/.test(text[i - 1] || ""))) && text[i + 1] && text[i + 1] !== " ") {
        const end = closing(text, c, i + 1);
        if (end < 0 || (c === "_" && /\w/.test(text[end + 1] || ""))) {
          plain += c;
          i += 1;
        } else {
          flush();
          const element = node("em");
          inline(text.slice(i + 1, end), element);
          parent.append(element);
          i = end + 1;
        }
      } else if ((c === "[" || (c === "!" && text[i + 1] === "[")) && (match = /^!?\[((?:\\.|[^\]\\])*)\]\(\s*<?((?:[^\s()<>]|\([^\s()<>]*\))+)>?(?:\s+"[^"]*")?\s*\)/.exec(rest))) {
        flush();
        const label = match[1] || match[2];
        link(match[2], c === "!" ? `🖼 ${label}` : label, parent);
        i += match[0].length;
      } else if (c === "<" && (match = AUTOLINK.exec(rest))) {
        flush();
        link(match[1], match[1], parent);
        i += match[0].length;
      } else if (c === "h" && !/\w/.test(text[i - 1] || "") && (match = BARE_URL.exec(rest))) {
        // Trailing punctuation ends the sentence, not the URL.
        let url = match[0].replace(/[.,;:!?)\]'"]+$/, "");
        if (match[0].length > url.length && match[0][url.length] === ")" && url.includes("(")) url += ")";
        flush();
        link(url, url, parent);
        i += url.length;
      } else {
        plain += c;
        i += 1;
      }
    }
    flush();
  }

  return { render };
})();
