<template>
  <div v-if="open" class="modal-mask" @click.self="onClose">
    <div class="modal">
      <div class="modal-header">
        <span>{{ title }}</span>
        <div class="modal-header-actions">
          <button
            v-if="showDelete"
            class="modal-delete"
            type="button"
            @click="onDelete"
          >
            删除
          </button>
        </div>
      </div>
      <div class="modal-body">
        <slot />
      </div>
      <div class="modal-actions" style="margin-top: 16px; display: flex; justify-content: flex-end; gap: 8px;">
        <button class="button secondary" type="button" @click="onClose">取消</button>
        <button class="button" type="button" @click="onConfirm">确认</button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
const props = defineProps<{
  open: boolean;
  title: string;
  showDelete?: boolean;
}>();

const emit = defineEmits(["close", "confirm", "delete"]);

const onClose = () => emit("close");
const onConfirm = () => emit("confirm");
const onDelete = () => emit("delete");
</script>
