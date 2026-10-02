<template>
  <Transition name="modal">
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
        <div class="modal-actions">
          <button class="button secondary" type="button" @click="onClose">取消</button>
          <button class="button" type="button" @click="onConfirm">确认</button>
        </div>
      </div>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { onBeforeUnmount, watch } from "vue";
import { pushModal } from "../modalStack";
const props = defineProps<{
  open: boolean;
  title: string;
  showDelete?: boolean;
}>();

const emit = defineEmits(["close", "confirm", "delete"]);

const onClose = () => emit("close");
const onConfirm = () => emit("confirm");
const onDelete = () => emit("delete");

// 打开时登记到弹窗栈，Esc（App.vue 的快捷键）关闭最上层的弹窗。
let release: (() => void) | null = null;
watch(
  () => props.open,
  open => {
    release?.();
    release = open ? pushModal(onClose) : null;
  },
  { immediate: true }
);
onBeforeUnmount(() => release?.());
</script>
