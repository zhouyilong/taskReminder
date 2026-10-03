import { ref } from "vue";

// 全局搜索面板（Ctrl+K / 标题栏按钮）的开关，模块级单例。
const searchOpen = ref(false);

export const useSearchPalette = () => ({
  searchOpen,
  openSearch: () => {
    searchOpen.value = true;
  },
  closeSearch: () => {
    searchOpen.value = false;
  }
});
