<template>
  <div class="pagination">
    <span>共 {{ total }} 条</span>
    <select class="select" title="每页条数" v-model.number="pageSize">
      <option :value="10">10 条/页</option>
      <option :value="20">20 条/页</option>
      <option :value="50">50 条/页</option>
    </select>
    <button class="button secondary" :disabled="pageIndex <= 1" @click="pageIndex--">
      <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M14.5 6.5L9 12l5.5 5.5" /></svg>上一页
    </button>
    <span class="pagination-page">第 {{ pageIndex }} / {{ totalPages }} 页</span>
    <button class="button secondary" :disabled="pageIndex >= totalPages" @click="pageIndex++">
      下一页<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M9.5 6.5L15 12l-5.5 5.5" /></svg>
    </button>
  </div>
</template>

<script setup lang="ts">
defineProps<{ total: number; totalPages: number }>();
const pageIndex = defineModel<number>("pageIndex", { required: true });
const pageSize = defineModel<number>("pageSize", { required: true });
</script>
