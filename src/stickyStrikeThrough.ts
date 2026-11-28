import { isHistoryTransaction } from "@milkdown/kit/prose/history";
import { Fragment, type Node } from "@milkdown/kit/prose/model";
import { Plugin, Selection, TextSelection, type Transaction } from "@milkdown/kit/prose/state";
import { AddMarkStep, Mapping } from "@milkdown/kit/prose/transform";
import { $prose } from "@milkdown/kit/utils";

const STRIKE = "strike_through";

interface Range {
  from: number;
  to: number;
}

interface Line extends Range {
  breakAfter?: Node;
}

interface LinePlacement {
  source: Node;
  range: Range;
  target: Node;
  offset: number;
}

function addedStrikeRanges(transactions: readonly Transaction[]): Range[] {
  const mapping = new Mapping();
  const ranges: Range[] = [];
  transactions.forEach(transaction => mapping.appendMapping(transaction.mapping));
  let stepIndex = 0;
  for (const transaction of transactions) {
    for (const step of transaction.steps) {
      stepIndex += 1;
      if (step instanceof AddMarkStep && step.mark.type.name === STRIKE) {
        const after = mapping.slice(stepIndex);
        ranges.push({ from: after.map(step.from, 1), to: after.map(step.to, -1) });
      }
    }
  }
  return ranges;
}

function isNewlyStruck(content: Fragment, start: number, ranges: readonly Range[]): boolean {
  if (!ranges.some(range => range.from < start + content.size && range.to > start)) return false;
  let hasText = false;
  let fullyStruck = true;
  content.forEach(node => {
    if ((node.isText && !node.text?.trim()) || node.type.name === "hardbreak") return;
    hasText = true;
    fullyStruck &&= node.marks.some(mark => mark.type.name === STRIKE);
  });
  return hasText && fullyStruck;
}

function linesIn(block: Node): Line[] {
  const lines: Line[] = [];
  let from = 0;
  block.forEach((node, offset) => {
    if (node.type.name !== "hardbreak" || node.attrs.isInline) return;
    lines.push({ from, to: offset, breakAfter: node });
    from = offset + node.nodeSize;
  });
  lines.push({ from, to: block.content.size });
  return lines;
}

function copyLines(block: Node, lines: Line[], placements: LinePlacement[], fallbackBreak?: Node): Node {
  let content = Fragment.empty;
  const offsets = lines.map((line, index) => {
    if (index && fallbackBreak) {
      content = content.append(Fragment.from(lines[index - 1].breakAfter ?? fallbackBreak));
    }
    const offset = content.size;
    content = content.append(block.content.cut(line.from, line.to));
    return offset;
  });
  const target = block.copy(content);
  lines.forEach((range, index) => placements.push({ source: block, range, target, offset: offsets[index] }));
  return target;
}

function splitList(list: Node, start: number, ranges: readonly Range[]) {
  const pending: Node[] = [];
  const completed: Node[] = [];
  list.forEach((item, offset) => {
    const first = item.firstChild;
    const itemStart = start + offset + 1;
    const struck = first?.isTextblock && isNewlyStruck(first.content, itemStart + 2, ranges);
    const blocks: Node[] = [];
    item.forEach((block, blockOffset) => {
      if (!isList(block)) {
        blocks.push(block);
        return;
      }
      const nested = splitList(block, itemStart + blockOffset + 1, ranges);
      blocks.push(block.copy(Fragment.fromArray([...nested.pending, ...nested.completed])));
    });
    (struck ? completed : pending).push(item.copy(Fragment.fromArray(blocks)));
  });
  return { pending, completed };
}

function isList(node: Node): boolean {
  return node.type.name === "bullet_list" || node.type.name === "ordered_list";
}

function positionOf(doc: Node, target: Node): number | undefined {
  let result: number | undefined;
  doc.descendants((node, pos) => {
    if (node === target && result === undefined) result = pos + 1;
    return result === undefined;
  });
  return result;
}

function restoredPosition(doc: Node, parent: Node, offset: number, placements: readonly LinePlacement[]) {
  const line = placements.find(({ source, range }) => source === parent && range.from <= offset && offset <= range.to);
  const start = positionOf(doc, line?.target ?? parent);
  if (start === undefined) return undefined;
  return start + (line ? line.offset + offset - line.range.from : offset);
}

function replaceReorderedBlocks(tr: Transaction, content: Fragment): void {
  const original = tr.doc;
  let first = 0;
  let start = 0;
  let oldEnd = original.childCount;
  let newEnd = content.childCount;
  let unchangedTail = 0;
  while (first < oldEnd && first < newEnd && original.child(first).eq(content.child(first))) {
    start += original.child(first).nodeSize;
    first += 1;
  }
  while (oldEnd > first && newEnd > first && original.child(oldEnd - 1).eq(content.child(newEnd - 1))) {
    unchangedTail += original.child(oldEnd - 1).nodeSize;
    oldEnd -= 1;
    newEnd -= 1;
  }
  tr.replaceWith(start, original.content.size - unchangedTail, content.cut(start, content.size - unchangedTail));
}

// Append the move to the mark transaction so the edit is saved and undone as one action.
export const stickyStrikeThrough = $prose(() => new Plugin({
  appendTransaction(transactions, _oldState, state) {
    if (transactions.some(isHistoryTransaction)) return null;
    const ranges = addedStrikeRanges(transactions);
    if (!ranges.length) return null;

    const { doc, selection } = state;
    const kept: Node[] = [];
    const completed: Node[] = [];
    const trailing: Node[] = [];
    const placements: LinePlacement[] = [];
    let last = doc.childCount - 1;
    while (last >= 0 && doc.child(last).type.name === "paragraph" && doc.child(last).content.size === 0) {
      last -= 1;
    }

    doc.forEach((block, pos, index) => {
      if (index > last) {
        trailing.push(block);
        return;
      }
      const isLast = index === last && completed.length === 0;
      if (isList(block)) {
        const items = splitList(block, pos, ranges);
        if (isLast) {
          kept.push(block.copy(Fragment.fromArray([...items.pending, ...items.completed])));
          return;
        }
        if (items.pending.length) kept.push(block.copy(Fragment.fromArray(items.pending)));
        if (items.completed.length) completed.push(block.copy(Fragment.fromArray(items.completed)));
        return;
      }
      if (!block.isTextblock || block.type.spec.code) {
        kept.push(block);
        return;
      }
      const lines = linesIn(block);
      const pending: Line[] = [];
      const struck: Line[] = [];
      for (const line of lines) {
        const formatted = isNewlyStruck(block.content.cut(line.from, line.to), pos + 1 + line.from, ranges);
        (formatted ? struck : pending).push(line);
      }
      if (!struck.length) {
        kept.push(block);
      } else if (!pending.length) {
        completed.push(block);
      } else if (isLast) {
        kept.push(copyLines(block, [...pending, ...struck], placements, lines[0].breakAfter));
      } else {
        kept.push(copyLines(block, pending, placements, lines[0].breakAfter));
        completed.push(copyLines(block, struck, placements, lines[0].breakAfter));
      }
    });

    const content = Fragment.fromArray([...kept, ...completed, ...trailing]);
    if (content.eq(doc.content)) return null;
    const tr = state.tr;
    replaceReorderedBlocks(tr, content);
    const anchor = restoredPosition(tr.doc, selection.$anchor.parent, selection.$anchor.parentOffset, placements);
    const head = restoredPosition(tr.doc, selection.$head.parent, selection.$head.parentOffset, placements);
    if (selection instanceof TextSelection && anchor !== undefined && head !== undefined) {
      tr.setSelection(TextSelection.create(tr.doc, anchor, head));
    } else {
      tr.setSelection(Selection.atEnd(tr.doc));
    }
    return tr.scrollIntoView();
  }
}));
