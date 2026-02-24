import { createApp } from "vue";
import "./styles.css";
import { renderStartupError } from "./startupError";

document.documentElement.classList.add("sticky-note-mode");
document.body.classList.add("sticky-note-mode");
document.documentElement.classList.add("sticky-note-manager-mode");
document.body.classList.add("sticky-note-manager-mode");

const bootstrap = async () => {
  try {
    const { default: StickyNoteApp } = await import("./StickyNoteApp.vue");
    createApp(StickyNoteApp).mount("#sticky-note");
  } catch (error) {
    renderStartupError("#sticky-note", "桌面便签初始化失败", error);
  }
};

void bootstrap();
