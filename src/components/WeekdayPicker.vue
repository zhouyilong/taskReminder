<template>
  <div class="weekday-picker" role="group" aria-label="选择周几">
    <button
      v-for="(label, index) in WEEKDAY_LABELS"
      :key="label"
      type="button"
      class="weekday-chip"
      :class="{ 'is-active': isActive(index + 1) }"
      :aria-pressed="isActive(index + 1)"
      @click="toggle(index + 1)"
    >
      {{ label }}
    </button>
    <span class="weekday-divider" aria-hidden="true"></span>
    <button
      v-for="preset in presets"
      :key="preset.label"
      type="button"
      class="weekday-preset"
      :class="{ 'is-active': modelValue === preset.mask }"
      @click="emit('update:modelValue', preset.mask)"
    >
      {{ preset.label }}
    </button>
  </div>
</template>

<script setup lang="ts">
import {
  WEEKDAY_LABELS,
  WEEKDAY_MASK_ALL,
  WEEKDAY_MASK_WEEKEND,
  WEEKDAY_MASK_WORKDAYS,
  weekdayBit
} from "../weekdays";

const props = defineProps<{ modelValue: number }>();
const emit = defineEmits<{ (event: "update:modelValue", value: number): void }>();

const presets = [
  { label: "工作日", mask: WEEKDAY_MASK_WORKDAYS },
  { label: "周末", mask: WEEKDAY_MASK_WEEKEND },
  { label: "每天", mask: WEEKDAY_MASK_ALL }
];

const isActive = (weekday: number) => (props.modelValue & weekdayBit(weekday)) !== 0;

const toggle = (weekday: number) => {
  emit("update:modelValue", props.modelValue ^ weekdayBit(weekday));
};
</script>

<style scoped>
.weekday-picker {
  display: inline-flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 4px;
}

.weekday-chip,
.weekday-preset {
  height: 30px;
  border-radius: 8px;
  border: 1px solid var(--border);
  background: var(--bg-muted);
  color: var(--text-muted);
  font: inherit;
  font-size: 12px;
  cursor: pointer;
  transition: background 0.15s ease, border-color 0.15s ease, color 0.15s ease;
}

.weekday-chip {
  width: 30px;
  padding: 0;
  font-weight: 600;
}

.weekday-preset {
  padding: 0 10px;
}

.weekday-chip:hover,
.weekday-preset:hover {
  border-color: var(--primary);
  color: var(--text-base);
}

.weekday-chip.is-active {
  border-color: var(--primary);
  background: var(--primary);
  color: #fff;
}

.weekday-preset.is-active {
  border-color: var(--primary);
  background: var(--primary-soft);
  color: var(--primary-text);
}

.weekday-divider {
  width: 1px;
  height: 18px;
  margin: 0 4px;
  background: var(--border);
}
</style>
