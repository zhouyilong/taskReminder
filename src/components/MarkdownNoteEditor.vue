<template>
  <div
    class="markdown-note-editor"
    :class="[`variant-${variant}`, `theme-${theme}`, { 'is-readonly': readonly }]"
    @mousedown.stop
  >
    <div ref="editorRoot" class="markdown-note-editor-root"></div>
  </div>
</template>

<script setup lang="ts">
import { Crepe } from "@milkdown/crepe";
import { nextTick, onBeforeUnmount, onMounted, ref, shallowRef, watch } from "vue";
import "@milkdown/crepe/theme/common/style.css";

const props = withDefaults(defineProps<{
  modelValue: string;
  placeholder?: string;
  readonly?: boolean;
  theme?: "light" | "dark";
  variant?: "card" | "ghost";
}>(), {
  placeholder: "",
  readonly: false,
  theme: "dark",
  variant: "card"
});

const emit = defineEmits<{
  "update:modelValue": [value: string];
}>();

const editorRoot = ref<HTMLDivElement | null>(null);
const crepe = shallowRef<Crepe | null>(null);
const currentMarkdown = ref(props.modelValue ?? "");
let lastEmittedMarkdown = props.modelValue ?? "";
let buildToken = 0;

const destroyEditor = async () => {
  const instance = crepe.value;
  crepe.value = null;
  if (instance) {
    await instance.destroy();
  }
  if (editorRoot.value) {
    editorRoot.value.innerHTML = "";
  }
};

const buildEditor = async (markdown: string) => {
  const token = ++buildToken;
  await nextTick();
  if (!editorRoot.value) {
    return;
  }

  await destroyEditor();
  if (token !== buildToken || !editorRoot.value) {
    return;
  }

  const instance = new Crepe({
    root: editorRoot.value,
    defaultValue: markdown,
    features: {
      [Crepe.Feature.ImageBlock]: false,
      [Crepe.Feature.Latex]: false,
      [Crepe.Feature.Table]: false
    },
    featureConfigs: {
      [Crepe.Feature.Placeholder]: {
        text: props.placeholder,
        mode: "doc"
      }
    }
  });

  instance.on(listener => {
    listener.markdownUpdated((_ctx, nextMarkdown) => {
      currentMarkdown.value = nextMarkdown;
      lastEmittedMarkdown = nextMarkdown;
      emit("update:modelValue", nextMarkdown);
    });
  });

  await instance.create();
  if (token !== buildToken) {
    await instance.destroy();
    return;
  }

  instance.setReadonly(props.readonly);
  crepe.value = instance;
  currentMarkdown.value = instance.getMarkdown();
  lastEmittedMarkdown = currentMarkdown.value;
};

onMounted(() => {
  void buildEditor(props.modelValue ?? "");
});

onBeforeUnmount(() => {
  buildToken += 1;
  void destroyEditor();
});

watch(() => props.readonly, value => {
  crepe.value?.setReadonly(value);
});

watch(() => props.modelValue, value => {
  const nextMarkdown = value ?? "";
  if (nextMarkdown === lastEmittedMarkdown || nextMarkdown === currentMarkdown.value) {
    return;
  }
  currentMarkdown.value = nextMarkdown;
  void buildEditor(nextMarkdown);
});

watch(() => props.placeholder, () => {
  if (!crepe.value) {
    return;
  }
  void buildEditor(props.modelValue ?? "");
});
</script>

<style scoped>
.markdown-note-editor {
  --md-editor-padding-x: 14px;
  --md-editor-padding-y: 12px;
  position: relative;
  width: 100%;
  height: 100%;
  min-height: 136px;
  overflow: hidden;
  color: inherit;
}

.markdown-note-editor.variant-card {
  border: 1px solid var(--border);
  border-radius: var(--radius-xs);
  background: var(--bg-panel, var(--bg-base));
  transition:
    border-color 0.16s ease,
    box-shadow 0.16s ease,
    background-color 0.16s ease;
}

.markdown-note-editor.variant-card:hover {
  border-color: var(--border-strong);
}

.markdown-note-editor.variant-card:focus-within {
  border-color: var(--primary);
  box-shadow: 0 0 0 3px var(--focus-ring);
}

.markdown-note-editor.variant-ghost {
  min-height: 0;
  background: transparent;
}

.markdown-note-editor-root {
  width: 100%;
  height: 100%;
  min-height: inherit;
}

.markdown-note-editor :deep(.milkdown) {
  height: 100%;
  color: inherit;
  background: transparent;
  font-family: inherit;
  --crepe-font-default: "Source Han Sans SC", "PingFang SC", "Noto Sans SC", "Segoe UI", sans-serif;
  --crepe-font-title: "Source Han Sans SC", "PingFang SC", "Noto Sans SC", "Segoe UI", sans-serif;
  --crepe-font-code: "JetBrains Mono", "Cascadia Code", "Consolas", monospace;
}

.markdown-note-editor.theme-light :deep(.milkdown) {
  --crepe-color-background: #ffffff;
  --crepe-color-on-background: #18181b;
  --crepe-color-surface: #ffffff;
  --crepe-color-surface-low: #f8fafc;
  --crepe-color-on-surface: #18181b;
  --crepe-color-on-surface-variant: #52525b;
  --crepe-color-outline: #cbd5e1;
  --crepe-color-primary: #2563eb;
  --crepe-color-secondary: #dbeafe;
  --crepe-color-on-secondary: #1e3a8a;
  --crepe-color-inverse: #18181b;
  --crepe-color-on-inverse: #fafafa;
  --crepe-color-inline-code: #b91c1c;
  --crepe-color-error: #dc2626;
  --crepe-color-hover: #eff6ff;
  --crepe-color-selected: #dbeafe;
  --crepe-color-inline-area: #f1f5f9;
  --crepe-shadow-1: 0 10px 24px rgba(15, 23, 42, 0.08);
  --crepe-shadow-2: 0 16px 34px rgba(15, 23, 42, 0.14);
}

.markdown-note-editor.theme-dark :deep(.milkdown) {
  --crepe-color-background: #121826;
  --crepe-color-on-background: #e8eefc;
  --crepe-color-surface: #161f32;
  --crepe-color-surface-low: #111827;
  --crepe-color-on-surface: #eef2ff;
  --crepe-color-on-surface-variant: #b9c3de;
  --crepe-color-outline: rgba(173, 190, 230, 0.45);
  --crepe-color-primary: #8ab4ff;
  --crepe-color-secondary: rgba(59, 92, 163, 0.42);
  --crepe-color-on-secondary: #e8eefc;
  --crepe-color-inverse: #eef2ff;
  --crepe-color-on-inverse: #111827;
  --crepe-color-inline-code: #ffb4ab;
  --crepe-color-error: #ff8f8f;
  --crepe-color-hover: rgba(60, 89, 146, 0.18);
  --crepe-color-selected: rgba(86, 126, 205, 0.24);
  --crepe-color-inline-area: rgba(18, 24, 38, 0.72);
  --crepe-shadow-1: 0 12px 28px rgba(2, 6, 23, 0.4);
  --crepe-shadow-2: 0 18px 40px rgba(2, 6, 23, 0.5);
}

.markdown-note-editor :deep(.milkdown .crepe-placeholder) {
  font-style: normal;
}

.markdown-note-editor :deep(.milkdown .ProseMirror) {
  min-height: inherit;
  padding: var(--md-editor-padding-y) var(--md-editor-padding-x);
  outline: none;
  white-space: pre-wrap;
}

.markdown-note-editor.variant-ghost :deep(.milkdown .ProseMirror) {
  padding: 14px 20px 18px;
}

.markdown-note-editor :deep(.milkdown .ProseMirror h1),
.markdown-note-editor :deep(.milkdown .ProseMirror h2),
.markdown-note-editor :deep(.milkdown .ProseMirror h3),
.markdown-note-editor :deep(.milkdown .ProseMirror h4),
.markdown-note-editor :deep(.milkdown .ProseMirror h5),
.markdown-note-editor :deep(.milkdown .ProseMirror h6) {
  line-height: 1.3;
}

.markdown-note-editor :deep(.milkdown .ProseMirror pre) {
  border-radius: 12px;
}

.markdown-note-editor :deep(.milkdown .ProseMirror code) {
  border-radius: 6px;
}

.markdown-note-editor :deep(.milkdown .ProseMirror ul[data-type="taskList"]) {
  padding-left: 0.2rem;
}

.markdown-note-editor.variant-ghost :deep(.milkdown .milkdown-toolbar) {
  z-index: 8;
}

.markdown-note-editor.is-readonly {
  pointer-events: none;
}
</style>
