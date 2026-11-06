<template>
  <div class="pagination">
    <span>共 {{ total }} 条</span>
    <select class="select" title="每页条数" v-model.number="pageSize">
      <option :value="10">10 条/页</option>
      <option :value="20">20 条/页</option>
      <option :value="50">50 条/页</option>
      <option :value="100">100 条/页</option>
    </select>
    <button class="button secondary" title="首页" aria-label="首页" :disabled="pageIndex <= 1" @click="pageIndex = 1">
      <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M17 6.5L11.5 12l5.5 5.5M8 6.5v11" /></svg>
    </button>
    <button class="button secondary" :disabled="pageIndex <= 1" @click="pageIndex--">
      <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M14.5 6.5L9 12l5.5 5.5" /></svg>上一页
    </button>
    <span class="pagination-page">第 {{ pageIndex }} / {{ totalPages }} 页</span>
    <button class="button secondary" :disabled="pageIndex >= totalPages" @click="pageIndex++">
      下一页<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M9.5 6.5L15 12l-5.5 5.5" /></svg>
    </button>
    <button class="button secondary" title="末页" aria-label="末页" :disabled="pageIndex >= totalPages" @click="pageIndex = totalPages">
      <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M7 6.5l5.5 5.5L7 17.5M16 6.5v11" /></svg>
    </button>
    <label class="pagination-jump">
      跳至
      <input
        v-model="jumpText"
        class="input pagination-jump-input"
        type="text"
        inputmode="numeric"
        aria-label="跳转到第几页"
        :placeholder="String(pageIndex)"
        :disabled="totalPages <= 1"
        @keydown.enter.prevent="jump"
        @keydown.esc.stop.prevent="jumpText = ''"
      />
      页
    </label>
    <button class="button secondary" type="button" :disabled="totalPages <= 1 || !jumpText.trim()" @click="jump">跳转</button>
  </div>
</template>

<script setup lang="ts">
// 表格分页条：首页 / 上一页 / 下一页 / 末页，输入页码回车或点“跳转”（v2.2.8，超出范围时跳到首页或末页）。
import { ref } from "vue";
import { clampPage } from "../pagination";

const props = defineProps<{ total: number; totalPages: number }>();
const pageIndex = defineModel<number>("pageIndex", { required: true });
const pageSize = defineModel<number>("pageSize", { required: true });

const jumpText = ref("");

const jump = () => {
  const target = clampPage(jumpText.value, props.totalPages);
  jumpText.value = "";
  if (target !== null) pageIndex.value = target;
};
</script>
