import { reactive } from "vue";

export type ContextMenuItem = { label: string; action: () => void; danger?: boolean };

const state = reactive({
  visible: false,
  x: 0,
  y: 0,
  items: [] as ContextMenuItem[],
});

const hideContextMenu = () => {
  state.visible = false;
};

const showContextMenu = (event: MouseEvent, items: ContextMenuItem[]) => {
  state.x = event.clientX;
  state.y = event.clientY;
  state.items = items.map(item => ({
    ...item,
    action: () => {
      hideContextMenu();
      item.action();
    }
  }));
  state.visible = true;
};

export const useContextMenu = () => ({ contextMenu: state, showContextMenu, hideContextMenu });
