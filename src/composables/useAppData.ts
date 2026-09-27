// 主窗口共享的数据仓库（模块级单例）：各视图读取同一份列表，写操作后统一刷新。
import { computed, ref } from "vue";
import { api } from "../api";
import type { RecurringTask, ReminderRecord, Task } from "../types";

const tasks = ref<Task[]>([]);
const completedTasks = ref<Task[]>([]);
const recurringTasks = ref<RecurringTask[]>([]);
const reminderRecords = ref<ReminderRecord[]>([]);
const holidayYears = ref<number[]>([]);
/** 每次刷新后递增，供需要额外加载数据的视图（今天、回收站）监听。 */
const dataVersion = ref(0);

const activeRecurringCount = computed(() => recurringTasks.value.filter(task => !task.isPaused).length);

const refreshAll = async () => {
  const [active, completed, recurring, records] = await Promise.all([
    api.listActiveTasks(),
    api.listCompletedTasks(),
    api.listRecurringTasks(),
    api.listReminderRecords(),
  ]);
  tasks.value = active;
  completedTasks.value = completed;
  recurringTasks.value = recurring;
  reminderRecords.value = records;
  dataVersion.value += 1;
};

const loadHolidayYears = async () => {
  holidayYears.value = await api.getHolidayYears();
};

export const useAppData = () => ({
  tasks,
  completedTasks,
  recurringTasks,
  reminderRecords,
  holidayYears,
  dataVersion,
  activeRecurringCount,
  refreshAll,
  loadHolidayYears,
});
