const LINK_PATTERN = /!?\[([^\]]*)\]\(([^)]+)\)/g;
const CODE_FENCE_PATTERN = /```[\s\S]*?```/g;
const INLINE_CODE_PATTERN = /`([^`]+)`/g;
const HEADING_PATTERN = /^\s{0,3}#{1,6}\s+/gm;
const BLOCK_PREFIX_PATTERN = /^\s{0,3}(?:>+\s*|[-*+]\s+|\d+\.\s+|[-*+]\s+\[(?: |x|X)\]\s+)/gm;
const HTML_TAG_PATTERN = /<[^>]+>/g;
const EMPHASIS_PATTERN = /(\*\*|__|\*|_|~~)/g;
const MULTI_SPACE_PATTERN = /\s+/g;

export const markdownToPlainText = (value: string | null | undefined): string => {
  if (!value) {
    return "";
  }

  return value
    .replace(CODE_FENCE_PATTERN, match =>
      match
        .replace(/```/g, "")
        .replace(/^[a-z0-9_-]+\s*$/gim, "")
    )
    .replace(LINK_PATTERN, (_match, label: string) => label || "")
    .replace(INLINE_CODE_PATTERN, "$1")
    .replace(HEADING_PATTERN, "")
    .replace(BLOCK_PREFIX_PATTERN, "")
    .replace(HTML_TAG_PATTERN, "")
    .replace(EMPHASIS_PATTERN, "")
    .replace(MULTI_SPACE_PATTERN, " ")
    .trim();
};

export const markdownToPreviewText = (value: string | null | undefined, fallback = "-"): string => {
  const text = markdownToPlainText(value);
  return text || fallback;
};
