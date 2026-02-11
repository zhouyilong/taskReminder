import { createApp } from "vue";
import "./styles.css";
import { renderStartupError } from "./startupError";

const bootstrap = async () => {
  try {
    const { default: App } = await import("./App.vue");
    createApp(App).mount("#app");
  } catch (error) {
    renderStartupError("#app", "主界面初始化失败", error);
  }
};

void bootstrap();
