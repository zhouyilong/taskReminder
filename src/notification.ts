import { createApp } from "vue";
import "./styles.css";
import { renderStartupError } from "./startupError";

document.documentElement.classList.add("notification-mode");
document.body.classList.add("notification-mode");

const bootstrap = async () => {
  try {
    const { default: NotificationApp } = await import("./NotificationApp.vue");
    createApp(NotificationApp).mount("#notification");
  } catch (error) {
    renderStartupError("#notification", "通知窗口初始化失败", error);
  }
};

void bootstrap();
