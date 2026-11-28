import type { MarkdownNode } from "@milkdown/kit/transformer";
import { $markSchema, $remark } from "@milkdown/kit/utils";

export const NOTE_TEXT_COLORS = [
  { value: "red", label: "红色" },
  { value: "amber", label: "琥珀色" },
  { value: "green", label: "绿色" },
  { value: "blue", label: "蓝色" },
  { value: "violet", label: "紫色" }
] as const;

export type NoteTextColor = typeof NOTE_TEXT_COLORS[number]["value"];

const COLOR_MARK = "noteTextColor";
const COLOR_OPEN = /^<span data-note-color="([a-z]+)">$/;
const SPAN_OPEN = /^<span(?:\s|>)/i;
const SPAN_CLOSE = /^<\/span\s*>$/i;

function isNoteTextColor(value: unknown): value is NoteTextColor {
  return NOTE_TEXT_COLORS.some(option => option.value === value);
}

// CommonMark parses inline HTML tags separately from the Markdown between them.
// Fold only our spans into marks and leave other HTML to Milkdown's normal parser.
function parseColorSpans(node: MarkdownNode): void {
  if (!node.children) return;
  const result: MarkdownNode[] = [];
  const spans: { opening: MarkdownNode; children: MarkdownNode[] }[] = [];
  const output = () => spans[spans.length - 1]?.children ?? result;

  for (const child of node.children) {
    const tag = child.type === "html" && typeof child.value === "string" ? child.value : "";
    if (SPAN_OPEN.test(tag) && !tag.endsWith("/>")) {
      spans.push({ opening: child, children: [] });
    } else if (SPAN_CLOSE.test(tag) && spans.length) {
      const { opening, children } = spans.pop()!;
      const color = COLOR_OPEN.exec(String(opening.value))?.[1];
      if (isNoteTextColor(color)) {
        output().push({ type: COLOR_MARK, color, children });
      } else {
        output().push(opening, ...children, child);
      }
    } else {
      parseColorSpans(child);
      output().push(child);
    }
  }
  while (spans.length) {
    const { opening, children } = spans.pop()!;
    output().push(opening, ...children);
  }
  node.children = result;
}

export const remarkNoteTextColor = $remark(COLOR_MARK, () => function () {
  const data = this.data();
  const extensions = data.toMarkdownExtensions ??= [];
  type Extension = typeof extensions[number];
  type Handler = NonNullable<NonNullable<Extension["handlers"]>["emphasis"]>;
  const handlers: Record<string, Handler> = {
    [COLOR_MARK](node, _parent, state, info) {
      const color = (node as MarkdownNode).color;
      const content = state.containerPhrasing(node as Parameters<typeof state.containerPhrasing>[0], {
        ...info, before: ">", after: "<"
      });
      return isNoteTextColor(color) ? `<span data-note-color="${color}">${content}</span>` : content;
    }
  };
  extensions.push({ handlers });
  return root => parseColorSpans(root as unknown as MarkdownNode);
});

export const noteTextColorSchema = $markSchema(COLOR_MARK, () => ({
  attrs: { color: {} },
  inclusive: false,
  priority: 0,
  parseDOM: [{
    tag: "span[data-note-color]",
    getAttrs: element => {
      const color = (element as HTMLElement).getAttribute("data-note-color");
      return isNoteTextColor(color) ? { color } : false;
    }
  }],
  toDOM: mark => ["span", { "data-note-color": mark.attrs.color }, 0],
  parseMarkdown: {
    match: node => node.type === COLOR_MARK,
    runner: (state, node, type) => {
      state.openMark(type, { color: node.color });
      state.next(node.children);
      state.closeMark(type);
    }
  },
  toMarkdown: {
    match: mark => mark.type.name === COLOR_MARK,
    runner: (state, mark) => { state.withMark(mark, COLOR_MARK, undefined, { color: mark.attrs.color }); }
  }
}));
