// 带自然语言识别的新建待办：输入框实时识别时间、循环规则、标签与优先级，
// 提交时按识别结果创建一次性待办或循环提醒。待办视图与日历视图共用。
import { computed, ref, watch, type Ref } from "vue";
import { api } from "../api";
import { errorMessage, toLocalDateTimeString } from "../format";
import { describeParsedSchedule, hasParsedMeta, parseQuickInput, type ParsedInput } from "../nlp";
import { buildRecurringPayload, validateRecurringDraft } from "../recurring";
import { useAppData } from "./useAppData";

export interface SmartAddOptions {
  stickyContent?: string | null;
  /** 没有识别出时间时使用的提醒时间（如日历中选中的日期）。 */
  fallbackReminder?: Date | null;
}

export const useSmartAdd = (text: Ref<string>) => {
  const { refreshAll } = useAppData();
  /** 为 false 时按原文添加，不做识别；清空输入后自动恢复。 */
  const enabled = ref(true);
  const error = ref("");
  const saving = ref(false);

  watch(text, value => {
    error.value = "";
    if (!value.trim()) {
      enabled.value = true;
    }
  });

  const parse = (now = new Date()): ParsedInput | null => (enabled.value ? parseQuickInput(text.value, now) : null);

  const parsed = computed(() => parse());
  const hasMeta = computed(() => Boolean(parsed.value && hasParsedMeta(parsed.value)));
  const scheduleText = computed(() => (parsed.value ? describeParsedSchedule(parsed.value) : ""));
  const title = computed(() => (parsed.value ? parsed.value.title : text.value.trim()));

  /** 成功返回 true；失败时写入 error。 */
  const submit = async (options: SmartAddOptions = {}): Promise<boolean> => {
    if (saving.value) {
      return false;
    }
    const now = new Date();
    const result = parse(now);
    const description = result ? result.title : text.value.trim();
    if (!description) {
      error.value = "请输入待办标题";
      return false;
    }
    saving.value = true;
    error.value = "";
    try {
      if (result?.recurring) {
        const draft = { ...result.recurring, description, tags: result.tags };
        const invalid = validateRecurringDraft(draft);
        if (invalid) {
          error.value = invalid;
          return false;
        }
        await api.createRecurringTask(buildRecurringPayload(draft));
      } else {
        const due = result?.dueTime ?? null;
        let reminder = result?.reminderTime ?? options.fallbackReminder ?? null;
        // 有截止时间而提前量算出的提醒时间已过：只记截止时间、不提醒。
        if (due && reminder && reminder.getTime() <= now.getTime() && due.getTime() > now.getTime()) {
          reminder = null;
        }
        if (reminder && reminder.getTime() <= now.getTime()) {
          error.value = "提醒时间需晚于当前时间";
          return false;
        }
        await api.createTask({
          description,
          stickyContent: options.stickyContent?.trim() ? options.stickyContent : null,
          tags: result?.tags ?? [],
          priority: result?.priority ?? 0,
          reminderTime: reminder ? toLocalDateTimeString(reminder) : null,
          dueAt: due ? toLocalDateTimeString(due) : null,
        });
      }
      text.value = "";
      enabled.value = true;
      await refreshAll();
      return true;
    } catch (caught) {
      error.value = errorMessage(caught);
      return false;
    } finally {
      saving.value = false;
    }
  };

  return { enabled, error, saving, parsed, hasMeta, scheduleText, title, submit };
};
