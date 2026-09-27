import { onBeforeUnmount, onMounted, ref } from "vue";

/** 定时刷新的“当前时间”，用于区分已过与即将到来的提醒。 */
export const useNow = (intervalMs = 30_000) => {
  const now = ref(new Date());
  let timer: number | undefined;
  onMounted(() => {
    timer = window.setInterval(() => {
      now.value = new Date();
    }, intervalMs);
  });
  onBeforeUnmount(() => {
    if (timer !== undefined) {
      window.clearInterval(timer);
    }
  });
  return now;
};
