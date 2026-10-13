import { createApp } from "vue";
import "./styles.css";
import { renderStartupError } from "./startupError";

document.documentElement.classList.add("attention-mode");
document.body.classList.add("attention-mode");

const bootstrap = async () => {
  try {
    const { default: AttentionApp } = await import("./AttentionApp.vue");
    createApp(AttentionApp).mount("#attention");
  } catch (error) {
    renderStartupError("#attention", "全屏提醒效果初始化失败", error);
  }
};

void bootstrap();
