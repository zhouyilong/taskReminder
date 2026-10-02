// 提醒弹窗的提示音：后端在提醒入队时按“提示音”设置与勿扰时段决定 `sound`，
// 弹窗只为第一次出现的提醒响一次（同一批同时到达的多条只响一次）。
import type { NotificationPayload } from "./types";

type SoundItem = Pick<NotificationPayload, "recordId" | "sound">;

/** 根据上一次看到的提醒记录，判断这次队列更新是否需要响铃，并返回新的“已看到”集合。 */
export const nextSoundState = (seen: ReadonlySet<string>, items: readonly SoundItem[]) => {
  const play = items.some(item => item.sound === true && !seen.has(item.recordId));
  return { play, seen: new Set(items.map(item => item.recordId)) };
};

let context: AudioContext | null = null;

/** 播放一段约 0.6 秒的双音提示（Web Audio 合成，不需要音频文件）。失败时静默忽略。 */
export const playChime = async () => {
  try {
    const AudioCtor =
      window.AudioContext ?? (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
    if (!AudioCtor) return;
    context ??= new AudioCtor();
    if (context.state === "suspended") await context.resume();
    const start = context.currentTime + 0.02;
    const notes: Array<[number, number]> = [
      [880, 0],
      [1318.5, 0.16]
    ];
    for (const [frequency, offset] of notes) {
      const oscillator = context.createOscillator();
      const gain = context.createGain();
      oscillator.type = "sine";
      oscillator.frequency.value = frequency;
      const at = start + offset;
      gain.gain.setValueAtTime(0.0001, at);
      gain.gain.exponentialRampToValueAtTime(0.22, at + 0.02);
      gain.gain.exponentialRampToValueAtTime(0.0001, at + 0.45);
      oscillator.connect(gain).connect(context.destination);
      oscillator.start(at);
      oscillator.stop(at + 0.5);
    }
  } catch (error) {
    console.warn("[notification] 播放提示音失败", error);
  }
};
