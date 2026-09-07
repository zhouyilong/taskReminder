import { createApp } from "vue";
import "./styles.css";
import { renderStartupError } from "./startupError";

document.documentElement.classList.add("sticky-note-mode");
document.body.classList.add("sticky-note-mode");
if (typeof navigator !== "undefined" && /linux/i.test(navigator.userAgent)) {
  document.documentElement.classList.add("is-linux");
  document.body.classList.add("is-linux");
}

const bootstrap = async () => {
  try {
    const { default: StickyNoteItemApp } = await import("./StickyNoteItemApp.vue");
    createApp(StickyNoteItemApp).mount("#sticky-note-item");
  } catch (error) {
    renderStartupError("#sticky-note-item", "独立便签初始化失败", error);
  }
};

void bootstrap();
