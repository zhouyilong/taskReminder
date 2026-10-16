<template>
  <Modal :open="labelManagerOpen" title="管理标签与项目" @close="close" @confirm="close">
    <div v-for="section in sections" :key="section.kind" class="modal-section">
      <div class="label-section-title">
        {{ section.title }}
        <span class="tag">{{ section.items.length }}</span>
      </div>
      <div v-if="!section.items.length" class="field-hint">{{ section.empty }}</div>
      <ul v-else class="label-list">
        <li v-for="item in section.items" :key="item.name" class="label-item">
          <template v-if="editing && editing.kind === section.kind && editing.name === item.name">
            <span class="label-prefix" aria-hidden="true">{{ section.prefix }}</span>
            <input
              ref="inputEl"
              v-model="draft"
              class="input label-input"
              :maxlength="section.kind === 'tag' ? 24 : MAX_PROJECT_CHARS"
              :aria-label="`${section.title}的新名称`"
              @keydown.enter.prevent="submitRename"
              @keydown.esc.stop.prevent="editing = null"
            />
            <span class="label-hint" :class="{ 'is-error': renameError }">{{ renameHint }}</span>
            <button class="button" type="button" :disabled="!canSubmit || busy" @click="submitRename">
              {{ mergeTarget ? "合并" : "确定" }}
            </button>
            <button class="button secondary" type="button" @click="editing = null">取消</button>
          </template>
          <template v-else>
            <span class="label-name" :title="item.name">{{ section.prefix }}{{ item.name }}</span>
            <span class="label-count">{{ item.count }} 条</span>
            <button class="button ghost" type="button" :disabled="busy" @click="startRename(section.kind, item.name)">
              重命名
            </button>
            <button
              class="button ghost is-danger"
              type="button"
              :disabled="busy"
              @click="confirmRemove(section.kind, item.name, item.count)"
            >
              {{ section.kind === "tag" ? "移除" : "移出" }}
            </button>
          </template>
        </li>
      </ul>
    </div>
    <div class="modal-section">
      <div class="field-hint">
        改成已有的名称即合并。改动写入每条相关的待办（含已完成）与循环提醒，开启云同步时会同步到其他设备；移除标签、移出项目不会删除条目。
      </div>
      <div v-if="message" class="field-hint label-message" :class="{ 'is-error': messageIsError }">{{ message }}</div>
    </div>
  </Modal>
</template>

<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import Modal from "./Modal.vue";
import { api } from "../api";
import { errorMessage } from "../format";
import { MAX_PROJECT_CHARS, collectLabelUsage, findOtherLabel, normalizeProject, normalizeTags } from "../tasks";
import { useAppData } from "../composables/useAppData";
import { useDialogs } from "../composables/useDialogs";
import { useLabelManager, type LabelChange } from "../composables/useLabelManager";

type LabelKind = LabelChange["kind"];

const { tasks, completedTasks, recurringTasks, refreshAll } = useAppData();
const { confirmAction } = useDialogs();
const { labelManagerOpen, lastChange } = useLabelManager();

const usage = computed(() => collectLabelUsage(tasks.value, completedTasks.value, recurringTasks.value));

const sections = computed(() => [
  {
    kind: "tag" as const,
    title: "标签",
    prefix: "#",
    empty: "还没有标签。新建待办时输入 #标签 即可添加。",
    items: usage.value.tags.map(item => ({ name: item.tag, count: item.count }))
  },
  {
    kind: "project" as const,
    title: "项目",
    prefix: "@",
    empty: "还没有项目。新建待办时输入 @项目 即可归入项目。",
    items: usage.value.projects.map(item => ({ name: item.project, count: item.count }))
  }
]);

const busy = ref(false);
const message = ref("");
const messageIsError = ref(false);
const editing = ref<{ kind: LabelKind; name: string } | null>(null);
const draft = ref("");
const inputEl = ref<HTMLInputElement[] | null>(null);

watch(labelManagerOpen, open => {
  if (open) {
    message.value = "";
    editing.value = null;
  }
});

const close = () => {
  labelManagerOpen.value = false;
};

const normalizeName = (kind: LabelKind, value: string) =>
  kind === "tag" ? (normalizeTags([value])[0] ?? "") : normalizeProject(value);

const namesOf = (kind: LabelKind) =>
  kind === "tag" ? usage.value.tags.map(item => item.tag) : usage.value.projects.map(item => item.project);

const countOf = (kind: LabelKind, name: string) =>
  sections.value.find(section => section.kind === kind)?.items.find(item => item.name === name)?.count ?? 0;

const nextName = computed(() => (editing.value ? normalizeName(editing.value.kind, draft.value) : ""));

const mergeTarget = computed(() =>
  editing.value && nextName.value ? findOtherLabel(namesOf(editing.value.kind), editing.value.name, nextName.value) : null
);

const renameError = computed(() => Boolean(editing.value) && !nextName.value);

const canSubmit = computed(() => Boolean(editing.value && nextName.value && nextName.value !== editing.value.name));

const renameHint = computed(() => {
  if (!editing.value) return "";
  if (!nextName.value) return "名称不能为空";
  const prefix = editing.value.kind === "tag" ? "#" : "@";
  return mergeTarget.value ? `将合并到已有的 ${prefix}${mergeTarget.value}` : "";
});

const startRename = async (kind: LabelKind, name: string) => {
  editing.value = { kind, name };
  draft.value = name;
  message.value = "";
  await nextTick();
  inputEl.value?.[0]?.select();
};

const apply = (change: LabelChange, action: () => Promise<number>) => {
  if (busy.value) return Promise.resolve();
  busy.value = true;
  return action()
    .then(async changed => {
      editing.value = null;
      lastChange.value = change;
      await refreshAll();
      message.value = `已修改 ${changed} 条`;
      messageIsError.value = false;
    })
    .catch(error => {
      message.value = errorMessage(error);
      messageIsError.value = true;
    })
    .finally(() => {
      busy.value = false;
    });
};

const submitRename = () => {
  if (!editing.value || !canSubmit.value) return;
  const { kind, name } = editing.value;
  const to = mergeTarget.value ?? nextName.value;
  const count = countOf(kind, name);
  const isTag = kind === "tag";
  const merge = Boolean(mergeTarget.value);
  const verb = merge ? "合并到已有的" : "改名为";
  const question = isTag ? `把 #${name} ${verb} #${to}？` : `把项目「${name}」${verb}${merge ? "项目" : ""}「${to}」？`;
  const scope = isTag ? `${count} 条待办与循环提醒` : `${count} 条待办`;
  confirmAction({
    title: merge ? (isTag ? "合并标签" : "合并项目") : isTag ? "重命名标签" : "重命名项目",
    message: `${question}将修改 ${scope}。`,
    action: () =>
      apply({ kind, from: name, to }, () => (isTag ? api.renameTag(name, to) : api.renameProject(name, to)))
  });
};

const confirmRemove = (kind: LabelKind, name: string, count: number) => {
  editing.value = null;
  confirmAction({
    title: kind === "tag" ? "移除标签" : "移出项目",
    message:
      kind === "tag"
        ? `从 ${count} 条待办与循环提醒上移除 #${name}？条目本身不会删除。`
        : `把项目「${name}」中的 ${count} 条待办移出到未分组？待办本身不会删除。`,
    action: () =>
      apply({ kind, from: name, to: "" }, () => (kind === "tag" ? api.removeTag(name) : api.renameProject(name, "")))
  });
};
</script>

<style scoped>
.label-section-title {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
  color: var(--text-title);
  font-size: var(--font-body);
  font-weight: var(--weight-bold);
}

.label-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
  max-height: 220px;
  margin: 0;
  padding: 0;
  overflow-y: auto;
  list-style: none;
}

.label-item {
  display: flex;
  align-items: center;
  gap: 8px;
  min-height: 36px;
  padding: 2px 4px 2px 10px;
  border-radius: var(--radius-xs);
  background: var(--bg-muted);
  font-size: var(--font-meta);
}

.label-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  color: var(--text-base);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.label-count {
  color: var(--text-muted);
  font-variant-numeric: tabular-nums;
}

.label-prefix {
  color: var(--text-muted);
}

.label-input {
  flex: 1;
  min-width: 80px;
}

.label-hint {
  color: var(--text-muted);
  white-space: nowrap;
}

.label-hint.is-error {
  color: var(--danger-text);
}

.label-message {
  margin-top: 6px;
}
</style>
