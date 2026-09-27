<template>
  <div class="segmented priority-picker" role="radiogroup" aria-label="优先级">
    <button
      v-for="option in PRIORITY_OPTIONS"
      :key="option.value"
      type="button"
      role="radio"
      class="segmented-item"
      :class="[{ active: model === option.value }, option.value ? `is-p${option.value}` : '']"
      :aria-checked="model === option.value"
      @click="model = option.value"
    >
      {{ option.label }}
    </button>
  </div>
</template>

<script setup lang="ts">
import { PRIORITY_OPTIONS } from "../tasks";

const model = defineModel<number>({ required: true });
</script>
