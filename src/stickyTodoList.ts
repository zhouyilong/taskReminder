import { createApp } from "vue";
import "./styles.css";
import { renderStartupError } from "./startupError";

// 与普通便签共用纸面样式与透明背景（Linux 下不透明，见 styles.css）。
document.documentElement.classList.add("sticky-note-mode");
document.body.classList.add("sticky-note-mode");
if (typeof navigator !== "undefined" && /linux/i.test(navigator.userAgent)) {
  document.documentElement.classList.add("is-linux");
  document.body.classList.add("is-linux");
}

const bootstrap = async () => {
  try {
    const { default: StickyTodoListApp } = await import("./StickyTodoListApp.vue");
    createApp(StickyTodoListApp).mount("#sticky-todo-list");
  } catch (error) {
    renderStartupError("#sticky-todo-list", "待办清单便签初始化失败", error);
  }
};

void bootstrap();
