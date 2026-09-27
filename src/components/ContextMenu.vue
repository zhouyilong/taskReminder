<template>
  <div
    v-if="contextMenu.visible"
    class="context-menu"
    :style="{ top: contextMenu.y + 'px', left: contextMenu.x + 'px' }"
    @click.stop
  >
    <button
      v-for="item in contextMenu.items"
      :key="item.label"
      class="context-menu-item"
      :class="{ danger: item.danger }"
      type="button"
      @click="item.action()"
    >
      {{ item.label }}
    </button>
  </div>
</template>

<script setup lang="ts">
import { onBeforeUnmount, onMounted } from "vue";
import { useContextMenu } from "../composables/useContextMenu";

const { contextMenu, hideContextMenu } = useContextMenu();

onMounted(() => {
  window.addEventListener("click", hideContextMenu);
});

onBeforeUnmount(() => {
  window.removeEventListener("click", hideContextMenu);
});
</script>
