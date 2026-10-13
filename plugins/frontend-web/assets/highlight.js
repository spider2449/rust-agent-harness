// Syntax highlighting for the read-only file view. File contents are
// untrusted: tokens become text nodes and spans with fixed class names, never
// HTML. Each language is a list of [class, pattern] rules tried in order at
// each position; the patterns use no capturing groups, so one combined
// regular expression can tell which rule matched.
"use strict";

const Highlight = (() => {
  // Files larger than this stay plain: thousands of spans slow the page.
  const MAX_CHARS = 200 * 1024;

  const words = (list) => `\\b(?:${list.trim().split(/\s+/).join("|")})\\b`;
  const NUMBER = "\\b(?:0[xX][0-9a-fA-F_]+|0[bB][01_]+|0[oO][0-7_]+|\\d[\\d_]*(?:\\.\\d[\\d_]*)?(?:[eE][+-]?\\d+)?)[a-zA-Z0-9_]*\\b";
  const DQ = '"(?:\\\\[\\s\\S]|[^"\\\\\\n])*"';
  const SQ = "'(?:\\\\[\\s\\S]|[^'\\\\\\n])*'";
  const LINE_SLASH = "//[^\\n]*";
  const BLOCK = "/\\*[\\s\\S]*?\\*/";
  const LINE_HASH = "#[^\\n]*";
  const TYPE = "\\b[A-Z][A-Za-z0-9_]*\\b";
  const CALL = "\\b[A-Za-z_][A-Za-z0-9_]*(?=\\s*\\()";

  const C_KEYWORDS = `auto break case catch char class const continue default delete do double else enum explicit
    extends extern final float for friend goto if implements import inline instanceof int interface long namespace
    new override package private protected public register return short signed sizeof static struct super switch
    template this throw try typedef typename union unsigned using virtual void volatile while bool true false
    null nullptr func go chan defer fallthrough map range select type var string byte rune`;

  const LANGUAGES = {
    rust: [
      ["comment", `${LINE_SLASH}|${BLOCK}`],
      ["string", `b?r##"[\\s\\S]*?"##|b?r#"[\\s\\S]*?"#|b?r"[^"]*"|b?${DQ}|b?'(?:\\\\.|[^'\\\\\\n])'`],
      ["attr", "#!?\\[[^\\]\\n]*\\]"],
      ["function", "\\b[a-z_][a-z0-9_]*!"],
      ["keyword", words(`as async await break const continue crate dyn else enum extern false fn for if impl in let
        loop match mod move mut pub ref return self Self static struct super trait true type unsafe use where while`)],
      ["type", `${TYPE}|${words("u8 u16 u32 u64 u128 usize i8 i16 i32 i64 i128 isize f32 f64 bool char str")}`],
      ["number", NUMBER],
      ["function", CALL],
    ],
    js: [
      ["comment", `${LINE_SLASH}|${BLOCK}`],
      ["string", `\`(?:\\\\[\\s\\S]|[^\`\\\\])*\`|${DQ}|${SQ}`],
      ["keyword", words(`async await break case catch class const continue debugger default delete do else export
        extends false finally for from function if import in instanceof let new null of return static super switch
        this throw true try typeof undefined var void while with yield interface type enum implements declare
        readonly abstract private protected public namespace as keyof`)],
      ["type", TYPE],
      ["number", NUMBER],
      ["function", CALL],
    ],
    python: [
      ["comment", LINE_HASH],
      ["string", `[rRbBuUfF]{0,2}(?:"""[\\s\\S]*?"""|'''[\\s\\S]*?'''|${DQ}|${SQ})`],
      ["attr", "@[A-Za-z_][\\w.]*"],
      ["keyword", words(`and as assert async await break class continue def del elif else except False finally for
        from global if import in is lambda None nonlocal not or pass raise return True try while with yield self
        match case`)],
      ["type", TYPE],
      ["number", NUMBER],
      ["function", CALL],
    ],
    clike: [
      ["comment", `${LINE_SLASH}|${BLOCK}`],
      ["attr", "^[ \\t]*#[ \\t]*[a-z]+[^\\n]*|@[A-Za-z_]\\w*"],
      ["string", `\`[^\`]*\`|${DQ}|${SQ}`],
      ["keyword", words(C_KEYWORDS)],
      ["type", TYPE],
      ["number", NUMBER],
      ["function", CALL],
    ],
    toml: [
      ["comment", LINE_HASH],
      ["heading", "^[ \\t]*\\[\\[?[^\\]\\n]*\\]\\]?"],
      ["property", "^[ \\t]*[A-Za-z0-9_.\"'-]+(?=[ \\t]*=)"],
      ["string", `"""[\\s\\S]*?"""|'''[\\s\\S]*?'''|${DQ}|${SQ}`],
      ["keyword", words("true false")],
      ["number", NUMBER],
    ],
    yaml: [
      ["comment", "(?:^|[ \\t])#[^\\n]*"],
      ["property", "^[ \\t]*(?:- )?[A-Za-z0-9_.\"'/-]+(?=:(?:[ \\t]|$))"],
      ["string", `${DQ}|${SQ}`],
      ["keyword", words("true false null yes no on off")],
      ["number", NUMBER],
    ],
    json: [
      ["property", `${DQ}(?=\\s*:)`],
      ["string", DQ],
      ["keyword", words("true false null")],
      ["number", "-?" + NUMBER],
    ],
    markdown: [
      ["string", "^```[\\s\\S]*?^```[^\\n]*|`[^`\\n]+`"],
      ["heading", "^#{1,6}[ \\t][^\\n]*"],
      ["keyword", "\\*\\*[^*\\n]+\\*\\*|__[^_\\n]+__"],
      ["attr", "\\[[^\\]\\n]*\\]\\([^)\\n]*\\)"],
      ["comment", "^>[^\\n]*|<!--[\\s\\S]*?-->"],
    ],
    shell: [
      ["comment", LINE_HASH],
      ["string", `${DQ}|${SQ}`],
      ["property", "\\$\\{[^}\\n]*\\}|\\$[A-Za-z_][\\w:]*"],
      ["keyword", words(`if then else elif fi for while until do done case esac function return in local export
        echo exit set foreach param begin process end try catch finally throw switch`)],
      ["number", NUMBER],
    ],
    css: [
      ["comment", BLOCK],
      ["string", `${DQ}|${SQ}`],
      ["attr", "@[a-z-]+"],
      ["property", "--[\\w-]+|\\b[a-z-]+(?=\\s*:[^:])"],
      ["number", "#[0-9a-fA-F]{3,8}\\b|-?\\b\\d+(?:\\.\\d+)?(?:px|em|rem|vh|vw|%|s|ms|deg|fr)?"],
    ],
    // `git diff`: file headers before added and removed lines, since
    // `--- a/x` would otherwise read as a removed line.
    diff: [
      ["meta", "^(?:diff --git |index |--- |\\+\\+\\+ |new file mode|deleted file mode|similarity index|rename from|rename to|old mode|new mode|Binary files)[^\\n]*"],
      ["hunk", "^@@[^\\n]*"],
      ["inserted", "^\\+[^\\n]*"],
      ["deleted", "^-[^\\n]*"],
    ],
    html: [
      ["comment", "<!--[\\s\\S]*?-->"],
      ["tag", "</?[A-Za-z][\\w:-]*|/?>"],
      ["string", `${DQ}|${SQ}`],
      ["property", "\\b[A-Za-z_:][\\w:.-]*(?==)"],
    ],
  };

  const EXTENSIONS = {
    rs: "rust",
    js: "js", mjs: "js", cjs: "js", jsx: "js", ts: "js", tsx: "js",
    py: "python", pyi: "python",
    c: "clike", h: "clike", cc: "clike", cpp: "clike", hpp: "clike", cs: "clike", java: "clike", go: "clike",
    kt: "clike", swift: "clike",
    toml: "toml", ini: "toml", cfg: "toml", lock: "toml",
    yml: "yaml", yaml: "yaml",
    json: "json", jsonl: "json",
    md: "markdown", markdown: "markdown",
    sh: "shell", bash: "shell", zsh: "shell", ps1: "shell", psm1: "shell",
    css: "css",
    html: "html", htm: "html", xml: "html", svg: "html",
    diff: "diff", patch: "diff",
  };

  const compiled = new Map();

  function compile(name) {
    if (!compiled.has(name)) {
      const rules = LANGUAGES[name];
      const source = rules.map(([, pattern]) => `(${pattern})`).join("|");
      compiled.set(name, { classes: rules.map(([kind]) => kind), pattern: new RegExp(source, "gm") });
    }
    return compiled.get(name);
  }

  // The language of a path, or null when it has none known.
  function language(path) {
    const name = path.split("/").pop().toLowerCase();
    const dot = name.lastIndexOf(".");
    return dot < 0 ? null : EXTENSIONS[name.slice(dot + 1)] || null;
  }

  // A fragment of text nodes and token spans whose text is exactly `text`.
  function render(text, path) {
    const fragment = document.createDocumentFragment();
    const name = language(path);
    if (!name || text.length > MAX_CHARS) {
      fragment.append(text);
      return fragment;
    }
    const { classes, pattern } = compile(name);
    pattern.lastIndex = 0;
    let done = 0;
    for (let match = pattern.exec(text); match; match = pattern.exec(text)) {
      if (!match[0]) {
        // An empty match (a lookahead-only position): step over it.
        pattern.lastIndex += 1;
        continue;
      }
      if (match.index > done) fragment.append(text.slice(done, match.index));
      const rule = match.findIndex((group, index) => index > 0 && group !== undefined) - 1;
      const span = document.createElement("span");
      span.className = `tok-${classes[rule]}`;
      span.textContent = match[0];
      fragment.append(span);
      done = match.index + match[0].length;
    }
    if (done < text.length) fragment.append(text.slice(done));
    return fragment;
  }

  return { language, render };
})();
