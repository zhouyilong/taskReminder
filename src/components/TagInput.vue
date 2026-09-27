<template>
  <div class="tag-input" :class="{ 'is-focused': focused }" @click="inputRef?.focus()">
    <span v-for="tag in model" :key="tag" class="task-tag is-removable">
      #{{ tag }}
      <button type="button" class="task-tag-remove" :title="`移除 #${tag}`" @click.stop="remove(tag)">
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M7 7l10 10M17 7L7 17" /></svg>
      </button>
    </span>
    <input
      ref="inputRef"
      v-model="draft"
      class="tag-input-field"
      :list="listId"
      :placeholder="model.length ? '' : placeholder"
      maxlength="24"
      @focus="focused = true"
      @blur="handleBlur"
      @keydown.enter.prevent="commit"
      @keydown="handleKeydown"
    />
    <datalist :id="listId">
      <option v-for="tag in availableSuggestions" :key="tag" :value="tag" />
    </datalist>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { normalizeTags } from "../tasks";

const props = withDefaults(defineProps<{ suggestions?: string[]; placeholder?: string }>(), {
  suggestions: () => [],
  placeholder: "添加标签，回车确认",
});

const model = defineModel<string[]>({ required: true });

const listId = `tag-suggestions-${Math.random().toString(36).slice(2, 8)}`;
const inputRef = ref<HTMLInputElement | null>(null);
const draft = ref("");
const focused = ref(false);

const availableSuggestions = computed(() => {
  const existing = new Set(model.value.map(tag => tag.toLowerCase()));
  return props.suggestions.filter(tag => !existing.has(tag.toLowerCase()));
});

const commit = () => {
  const parts = draft.value.split(/[\s,，]+/).filter(Boolean);
  if (parts.length) {
    model.value = normalizeTags([...model.value, ...parts]);
  }
  draft.value = "";
};

const remove = (tag: string) => {
  model.value = model.value.filter(item => item !== tag);
};

const handleKeydown = (event: KeyboardEvent) => {
  if (event.key === "," || event.key === "，" || (event.key === " " && draft.value.trim())) {
    event.preventDefault();
    commit();
  } else if (event.key === "Backspace" && !draft.value && model.value.length) {
    model.value = model.value.slice(0, -1);
  }
};

const handleBlur = () => {
  focused.value = false;
  commit();
};
</script>
