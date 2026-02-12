import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import { resolve } from "path";

// 使用多入口以支持通知窗口
export default defineConfig({
  // Use relative asset paths so packaged desktop builds can always resolve JS/CSS.
  base: "./",
  plugins: [vue()],
  server: {
    host: "127.0.0.1",
    port: 5173,
    strictPort: true,
    // Avoid exhausting inotify watches when Rust toolchain lives under the repo
    watch: {
      ignored: ["**/.dev/**", "**/src-tauri/target/**"]
    }
  },
  build: {
    target: process.env.TAURI_ENV_PLATFORM === "windows" ? "chrome105" : "safari13",
    rollupOptions: {
      input: {
        main: resolve(__dirname, "index.html"),
        notification: resolve(__dirname, "notification.html"),
        stickyNote: resolve(__dirname, "sticky-note.html")
      }
    }
  }
});
