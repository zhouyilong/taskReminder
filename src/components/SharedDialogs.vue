<template>
  <TaskEditModal />
  <RecurringEditModal />

  <Modal :open="detailState.open" :title="detailState.title" @close="detailState.open = false" @confirm="detailState.open = false">
    <div class="detail-list">
      <div v-for="item in detailState.items" :key="item.label" class="detail-item">
        <span class="detail-label">{{ item.label }}</span>
        <span class="detail-value">{{ item.value }}</span>
      </div>
    </div>
  </Modal>

  <Modal :open="confirmState.open" :title="confirmState.title" @close="closeConfirm" @confirm="runConfirm">
    <div class="modal-text">{{ confirmState.message }}</div>
  </Modal>

  <ContextMenu />
</template>

<script setup lang="ts">
import Modal from "./Modal.vue";
import ContextMenu from "./ContextMenu.vue";
import TaskEditModal from "./TaskEditModal.vue";
import RecurringEditModal from "./RecurringEditModal.vue";
import { useDialogs } from "../composables/useDialogs";

const { detailState, confirmState, closeConfirm, runConfirm } = useDialogs();
</script>
