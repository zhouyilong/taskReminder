<template>
  <div v-if="payload" class="attention-stage" :class="stageClasses">
    <div class="attention-edge" aria-hidden="true"></div>
    <div v-if="payload.effect === 'banner'" class="attention-banner-anchor">
      <div class="attention-banner" role="status" aria-live="assertive">
        <span class="attention-bell" aria-hidden="true">
          <svg viewBox="0 0 24 24" width="22" height="22" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M6 8a6 6 0 0 1 12 0c0 7 3 9 3 9H3s3-2 3-9" />
            <path d="M10.3 21a1.94 1.94 0 0 0 3.4 0" />
          </svg>
        </span>
        <div class="attention-banner-body">
          <div class="attention-eyebrow">{{ eyebrow }}</div>
          <div class="attention-title">{{ title }}</div>
        </div>
        <div class="attention-hint">{{ hint }}</div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { listen } from "@tauri-apps/api/event";
import {
  ATTENTION_DURATION_MS,
  ATTENTION_FADE_MS,
  attentionEyebrow,
  attentionTitle,
  missedLabel,
  normalizeAttentionPayload,
  popupHint,
  resolveAttentionTheme,
  type AttentionQueueUpdate
} from "./attentionEffect";
import { safeStorage } from "./safeStorage";

// 预览“直到处理”时额外展示收起后的样子，与后端 `PREVIEW_PERSIST_EXTRA_MS` 一致。
const PREVIEW_PERSIST_EXTRA_MS = 5000;

type Phase = "enter" | "persist" | "leave";

const payload = ref(normalizeAttentionPayload((window as { __ATTENTION__?: unknown }).__ATTENTION__));
const phase = ref<Phase>("enter");
const shownAt = new Date();
const timers: number[] = [];
let unlistenQueue: (() => void) | null = null;

const missed = computed(() => {
  const current = payload.value;
  return !!current && !current.preview && missedLabel(current.scheduledTime, shownAt) !== "";
});
const eyebrow = computed(() => (payload.value ? attentionEyebrow(payload.value, shownAt) : ""));
const title = computed(() => attentionTitle(payload.value?.description));
const hint = computed(() => (payload.value ? popupHint(payload.value.popupDirection, payload.value.count) : ""));
const stageClasses = computed(() => [`is-${phase.value}`, { "is-missed": missed.value }]);

const applyTheme = () => {
  const systemLight = window.matchMedia?.("(prefers-color-scheme: light)").matches ?? false;
  const theme = resolveAttentionTheme(payload.value?.theme, safeStorage.getItem("appTheme"), systemLight);
  document.documentElement.classList.toggle("light-theme", theme === "light");
};

const later = (ms: number, action: () => void) => {
  timers.push(window.setTimeout(action, Math.max(0, ms)));
};

// 先闪烁约 6 秒：普通效果随后淡出，“直到处理”则收起横幅、保留缓慢呼吸的边缘光，
// 直到后端在队列清空时关闭窗口。预览“直到处理”时多展示一会儿收起后的样子再淡出。
const schedulePhases = () => {
  const current = payload.value;
  if (!current) return;
  if (current.untilHandled) {
    later(ATTENTION_DURATION_MS, () => (phase.value = "persist"));
    if (current.preview) {
      later(ATTENTION_DURATION_MS + PREVIEW_PERSIST_EXTRA_MS - ATTENTION_FADE_MS, () => (phase.value = "leave"));
    }
    return;
  }
  later(ATTENTION_DURATION_MS - ATTENTION_FADE_MS, () => (phase.value = "leave"));
};

onMounted(async () => {
  applyTheme();
  schedulePhases();
  try {
    unlistenQueue = await listen<AttentionQueueUpdate>("attention-queue", event => {
      if (payload.value && event.payload) {
        payload.value = { ...payload.value, ...event.payload };
      }
    });
  } catch (error) {
    console.warn("[attention] 监听队列更新失败", error);
  }
});

onBeforeUnmount(() => {
  timers.forEach(timer => window.clearTimeout(timer));
  unlistenQueue?.();
});
</script>

<style scoped>
.attention-stage {
  --attention-current-ring: var(--attention-ring);
  --attention-current-glow: var(--attention-glow);
  --attention-current-glow-soft: var(--attention-glow-soft);
  position: fixed;
  inset: 0;
  pointer-events: none;
  font-family: var(--font-sans);
  user-select: none;
}

.attention-stage.is-missed {
  --attention-current-ring: var(--attention-missed-ring);
  --attention-current-glow: var(--attention-missed-glow);
  --attention-current-glow-soft: var(--attention-missed-glow-soft);
}

/* 屏幕四边的光晕：先快速闪 3 次，之后淡出或缓慢呼吸。 */
.attention-edge {
  position: absolute;
  inset: 0;
  box-shadow:
    inset 0 0 0 3px var(--attention-current-ring),
    inset 0 0 48px 12px var(--attention-current-glow),
    inset 0 0 180px 56px var(--attention-current-glow-soft);
  opacity: 0.25;
  will-change: opacity;
  animation: attention-flash 0.9s ease-in-out 6 alternate both;
}

.is-persist .attention-edge {
  animation: attention-breathe 2.4s ease-in-out infinite alternate;
}

.is-leave .attention-edge {
  animation: attention-fade-out 0.6s ease forwards;
}

.attention-banner-anchor {
  position: absolute;
  top: 28px;
  left: 50%;
  width: min(720px, calc(100vw - 48px));
  transform: translateX(-50%);
  transform-origin: top center;
  display: flex;
  justify-content: center;
  transition: transform 0.4s ease;
}

.is-persist .attention-banner-anchor {
  transform: translateX(-50%) scale(0.88);
}

.attention-banner {
  display: flex;
  align-items: center;
  gap: 14px;
  max-width: 100%;
  padding: 14px 20px 14px 14px;
  border-radius: 18px;
  border: 1px solid var(--attention-banner-border);
  background: var(--attention-banner-bg);
  box-shadow: var(--attention-banner-shadow);
  backdrop-filter: blur(14px);
  animation: attention-drop 0.6s cubic-bezier(0.2, 1.3, 0.35, 1) both;
  transition: padding 0.4s ease;
}

.is-persist .attention-banner {
  padding: 10px 16px 10px 10px;
}

.is-leave .attention-banner {
  animation: attention-rise 0.5s ease forwards;
}

.attention-bell {
  flex: none;
  display: grid;
  place-items: center;
  width: 42px;
  height: 42px;
  border-radius: 50%;
  color: var(--primary-text);
  background: var(--primary-soft);
  box-shadow: 0 0 0 1px var(--attention-banner-border);
}

.attention-bell svg {
  transform-origin: 50% 8%;
  animation: attention-bell 1.2s ease-in-out 0.3s 2;
}

.is-missed .attention-banner {
  border-color: var(--attention-missed-glow);
}

.is-missed .attention-bell {
  color: var(--warning-text);
  background: var(--warning-soft);
}

.attention-banner-body {
  min-width: 0;
  flex: 1;
}

.attention-eyebrow {
  font-size: var(--font-meta);
  font-weight: var(--weight-semibold);
  color: var(--text-muted);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.is-missed .attention-eyebrow {
  color: var(--warning-text);
}

.attention-title {
  margin-top: 2px;
  font-size: 17px;
  line-height: 1.35;
  font-weight: var(--weight-bold);
  color: var(--text-title);
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
  word-break: break-word;
}

.is-persist .attention-title {
  -webkit-line-clamp: 1;
}

.attention-hint {
  flex: none;
  max-width: 220px;
  padding-left: 14px;
  border-left: 1px solid var(--border-strong);
  font-size: var(--font-meta);
  color: var(--text-muted);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  transition: opacity 0.3s ease, max-width 0.4s ease, padding 0.4s ease;
}

.is-persist .attention-hint {
  opacity: 0;
  max-width: 0;
  padding-left: 0;
  border-left-color: transparent;
}

@keyframes attention-flash {
  from {
    opacity: 0.25;
  }
  to {
    opacity: 1;
  }
}

@keyframes attention-breathe {
  from {
    opacity: 0.25;
  }
  to {
    opacity: 0.6;
  }
}

@keyframes attention-fade-out {
  from {
    opacity: 0.25;
  }
  to {
    opacity: 0;
  }
}

@keyframes attention-drop {
  from {
    opacity: 0;
    transform: translateY(-140%);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

@keyframes attention-rise {
  from {
    opacity: 1;
    transform: translateY(0);
  }
  to {
    opacity: 0;
    transform: translateY(-140%);
  }
}

@keyframes attention-bell {
  0%,
  100% {
    transform: rotate(0);
  }
  15% {
    transform: rotate(16deg);
  }
  30% {
    transform: rotate(-14deg);
  }
  45% {
    transform: rotate(10deg);
  }
  60% {
    transform: rotate(-6deg);
  }
  75% {
    transform: rotate(2deg);
  }
}

/* 减少动态效果：不闪烁、不滑入，只显示静止的光晕与横幅。 */
@media (prefers-reduced-motion: reduce) {
  .attention-edge,
  .is-persist .attention-edge {
    animation: none;
    opacity: 0.7;
  }

  .is-persist .attention-edge {
    opacity: 0.4;
  }

  .is-leave .attention-edge,
  .is-leave .attention-banner {
    animation: none;
    opacity: 0;
  }

  .attention-banner,
  .attention-banner-anchor,
  .attention-bell svg {
    animation: none;
    transition: none;
  }
}
</style>
