<template>
  <div class="batch-bar" role="toolbar" aria-label="批量操作">
    <span class="batch-count">已选 {{ count }} {{ unit }}</span>
    <button v-if="count < total" class="button secondary" type="button" @click="emit('selectAll')">
      全选筛选结果（{{ total }} {{ unit }}）
    </button>
    <slot />
    <span class="batch-spacer" />
    <button class="button ghost" type="button" :title="`${cancelLabel}（Esc）`" @click="emit('cancel')">{{ cancelLabel }}</button>
  </div>
</template>

<script setup lang="ts">
// 列表多选的批量操作栏（v2.2.8）：已选数量、全选筛选结果、取消；具体操作按钮放在默认插槽中。
withDefaults(defineProps<{ count: number; total: number; unit?: string; cancelLabel?: string }>(), {
  unit: "条",
  cancelLabel: "取消选择"
});
const emit = defineEmits<{ (event: "selectAll"): void; (event: "cancel"): void }>();
</script>
