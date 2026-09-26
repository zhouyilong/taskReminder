import { createApp } from "vue";
import "./styles.css";
import { renderStartupError } from "./startupError";

document.documentElement.classList.add("quick-add-mode");
document.body.classList.add("quick-add-mode");

const bootstrap = async () => {
  try {
    const { default: QuickAddApp } = await import("./QuickAddApp.vue");
    createApp(QuickAddApp).mount("#quick-add");
  } catch (error) {
    renderStartupError("#quick-add", "快速添加窗口初始化失败", error);
  }
};

void bootstrap();
