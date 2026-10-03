// 浏览器中的 Tauri 运行时模拟（端到端测试与界面预览使用）：在页面脚本之前注入，
// 提供 `window.__TAURI_INTERNALS__`（invoke、transformCallback、metadata），
// 命令在内存中的示例数据上执行，并把每次调用记在 `window.__TAURI_MOCK__.calls` 中供断言。
//
// 示例数据的时间相对于“现在”生成；页面加载前可设置 `window.__TAURI_MOCK_SEED__ = { tasks, ... }` 覆盖。
(() => {
  const pad = value => String(value).padStart(2, "0");
  const fmt = date =>
    `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}T${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`;
  const at = (dayOffset, hour, minute = 0) => {
    const date = new Date();
    date.setDate(date.getDate() + dayOffset);
    date.setHours(hour, minute, 0, 0);
    return fmt(date);
  };
  const now = () => fmt(new Date());
  let nextId = 1;
  const newId = prefix => `${prefix}-${Date.now().toString(36)}-${nextId++}`;

  const task = (id, description, extra = {}) => ({
    id,
    description,
    type: "ONE_TIME",
    status: "PENDING",
    createdAt: at(-3, 9),
    completedAt: null,
    reminderTime: null,
    updatedAt: at(-3, 9),
    deletedAt: null,
    stickyContent: "",
    tags: [],
    priority: 0,
    dueAt: null,
    stickyColor: "",
    sortOrder: null,
    ...extra
  });
  const recurring = (id, description, extra = {}) => ({
    id,
    description,
    type: "RECURRING",
    status: "PENDING",
    createdAt: at(-30, 9),
    intervalMinutes: 60,
    lastTriggered: at(-1, 9),
    nextTrigger: at(1, 9),
    isPaused: false,
    repeatMode: "DAILY",
    scheduleTime: "09:00",
    tags: [],
    ...extra
  });
  const record = (id, reminderId, description, type, triggerTime, action) => ({
    id,
    reminderId,
    description,
    type,
    triggerTime,
    closeTime: triggerTime,
    action,
    updatedAt: triggerTime,
    deletedAt: null
  });

  const defaultSeed = () => {
    const tasks = [
      task("t-report", "写季度报告", {
        tags: ["工作"],
        priority: 3,
        dueAt: at(2, 18),
        reminderTime: at(2, 17),
        stickyContent: "- [x] 收集数据\n- [x] 写初稿\n- [ ] 请组长审阅\n- [ ] 提交",
        stickyColor: "blue"
      }),
      task("t-groceries", "买菜", {
        tags: ["生活"],
        reminderTime: at(0, 19),
        stickyContent: "- [ ] 牛奶\n- [ ] 鸡蛋\n- [ ] 西红柿"
      }),
      task("t-dentist", "预约牙医", { tags: ["健康"], priority: 1, reminderTime: at(-1, 10) }),
      task("t-ideas", "读书笔记", { stickyContent: "《置身事内》第三章：土地财政与地方债。" }),
      task("t-done-1", "交电费", { status: "COMPLETED", completedAt: at(-1, 20), tags: ["生活"] }),
      task("t-done-2", "提交报销单", { status: "COMPLETED", completedAt: at(-9, 11), tags: ["工作"] }),
      task("t-done-3", "修自行车", { status: "COMPLETED", completedAt: at(-20, 16) })
    ];
    const recurringTasks = [
      recurring("r-standup", "站会", { repeatMode: "WORKDAY", scheduleTime: "09:30", tags: ["工作"] }),
      recurring("r-water", "喝水", { repeatMode: "INTERVAL_RANGE", intervalMinutes: 90, startTime: "09:00", endTime: "18:00", tags: ["健康"] }),
      recurring("r-rent", "交房租", { repeatMode: "MONTHLY_LAST_DAY", scheduleTime: "20:00", tags: ["生活"] })
    ];
    const reminderRecords = [];
    // 近 12 周的提醒记录：站会每个工作日一次，喝水每天两次，待办偶尔推迟。
    for (let day = -80; day <= 0; day += 1) {
      const date = new Date();
      date.setDate(date.getDate() + day);
      const weekday = date.getDay();
      if (weekday >= 1 && weekday <= 5) {
        const action = day % 7 === 0 ? "SNOOZED" : day % 5 === 0 ? "DISMISSED" : "COMPLETED";
        reminderRecords.push(record(`rec-standup${day}`, "r-standup", "站会", "RECURRING", at(day, 9, 30), action));
      }
      reminderRecords.push(record(`rec-water-a${day}`, "r-water", "喝水", "RECURRING", at(day, 10, 30), day % 3 === 0 ? "SNOOZED" : "DISMISSED"));
      if (day % 2 === 0) {
        reminderRecords.push(record(`rec-water-b${day}`, "r-water", "喝水", "RECURRING", at(day, 15), "COMPLETED"));
      }
    }
    reminderRecords.push(record("rec-dentist", "t-dentist", "预约牙医", "TASK", at(-1, 10), "SNOOZED"));
    reminderRecords.push(record("rec-power", "t-done-1", "交电费", "TASK", at(-1, 19), "COMPLETED"));
    return { tasks, recurringTasks, reminderRecords };
  };

  const seed = window.__TAURI_MOCK_SEED__ ?? defaultSeed();
  const state = {
    tasks: seed.tasks ?? [],
    recurringTasks: seed.recurringTasks ?? [],
    reminderRecords: seed.reminderRecords ?? [],
    settings: {
      autoStartEnabled: false,
      soundEnabled: true,
      snoozeMinutes: 10,
      stickyNoteEnabled: true,
      stickyNoteContent: "",
      stickyNoteWidth: 360,
      stickyNoteHeight: 520,
      stickyNoteX: null,
      stickyNoteY: null,
      stickyNoteOpacity: 0.95,
      windowOpacity: 1,
      webdavEnabled: false,
      webdavUrl: "",
      webdavUsername: "",
      webdavPassword: "",
      webdavRootPath: "",
      webdavSyncIntervalMinutes: 60,
      webdavDeviceId: "mock-device",
      notificationTheme: "app",
      quickAddEnabled: true,
      quickAddShortcut: "CommandOrControl+Alt+N",
      syncEncryptionEnabled: false,
      syncPassphrase: "",
      quietHoursEnabled: false,
      quietHoursStart: "22:00",
      quietHoursEnd: "08:00",
      nativeNotificationEnabled: false,
      stickyToggleShortcut: "",
      stickySnapEnabled: true,
      holidayAutoUpdate: true,
      completedRetentionDays: 30,
      secretStorage: "db",
      ...(seed.settings ?? {})
    }
  };

  const callbacks = new Map();
  const listeners = new Map();
  let nextCallback = 1;
  const calls = [];

  const findTask = id => state.tasks.find(item => item.id === id && !item.deletedAt);
  const findRecurring = id => state.recurringTasks.find(item => item.id === id && !item.deletedAt);
  const touch = item => {
    item.updatedAt = now();
  };
  const normalizeTags = tags => {
    const result = [];
    for (const raw of tags ?? []) {
      const tag = String(raw).trim().replace(/^[#＃]+/, "").replace(/[,，]/g, "").trim();
      if (tag && !result.some(existing => existing.toLowerCase() === tag.toLowerCase())) {
        result.push(tag);
      }
    }
    return result.slice(0, 10);
  };
  const stickySummary = item => ({
    taskId: item.id,
    title: item.description,
    noteType: "TASK",
    content: item.stickyContent ?? "",
    posX: 48,
    posY: 76,
    width: 284,
    height: 280,
    isOpen: !!item.stickyIsOpen,
    isPinned: false,
    createdAt: item.createdAt,
    updatedAt: item.updatedAt ?? item.createdAt,
    reminderTime: item.reminderTime ?? null,
    color: item.stickyColor ?? "",
    visible: !!item.stickyIsOpen
  });
  const emit = (event, payload) => {
    for (const handlerId of listeners.get(event) ?? []) {
      callbacks.get(handlerId)?.({ event, id: handlerId, payload });
    }
  };
  const dataChanged = () => setTimeout(() => emit("data-updated", null), 0);

  const commands = {
    list_active_tasks: () => state.tasks.filter(item => item.status === "PENDING" && !item.deletedAt),
    list_completed_tasks: () => state.tasks.filter(item => item.status === "COMPLETED" && !item.deletedAt),
    list_recurring_tasks: () => state.recurringTasks.filter(item => !item.deletedAt),
    list_reminder_records: () => state.reminderRecords.filter(item => !item.deletedAt),
    create_task: ({ payload }) => {
      const item = task(newId("task"), payload.description, {
        createdAt: now(),
        updatedAt: now(),
        stickyContent: payload.stickyContent ?? "",
        tags: normalizeTags(payload.tags),
        priority: payload.priority ?? 0,
        reminderTime: payload.reminderTime ?? null,
        dueAt: payload.dueAt ?? null
      });
      state.tasks.push(item);
      return item;
    },
    quick_add_task: ({ payload }) => commands.create_task({ payload }),
    update_task: ({ task: patch }) => {
      const item = findTask(patch.id);
      if (!item) throw "待办不存在";
      item.description = patch.description;
      if (patch.stickyContent !== undefined) item.stickyContent = patch.stickyContent ?? "";
      if (patch.reminderTime !== undefined) item.reminderTime = patch.reminderTime;
      if (patch.tags !== undefined) item.tags = normalizeTags(patch.tags);
      if (patch.priority !== undefined) item.priority = patch.priority;
      if (patch.dueAt !== undefined) item.dueAt = patch.dueAt;
      touch(item);
    },
    complete_task: ({ id }) => {
      const item = findTask(id);
      if (item) {
        item.status = "COMPLETED";
        item.completedAt = now();
        item.stickyIsOpen = false;
        touch(item);
      }
    },
    uncomplete_task: ({ id }) => {
      const item = findTask(id);
      if (item) {
        item.status = "PENDING";
        item.completedAt = null;
        touch(item);
      }
    },
    delete_task: ({ id }) => {
      const item = findTask(id);
      if (item) {
        item.deletedAt = now();
        touch(item);
      }
    },
    batch_update_tasks: ({ ids, payload }) => {
      let changed = 0;
      for (const id of ids) {
        const item = findTask(id);
        if (!item) continue;
        if (payload.action === "complete") commands.complete_task({ id });
        else if (payload.action === "delete") commands.delete_task({ id });
        else if (payload.action === "addTags") item.tags = normalizeTags([...(item.tags ?? []), ...payload.tags]);
        else if (payload.action === "setPriority") item.priority = payload.priority;
        else if (payload.action === "setReminder") item.reminderTime = payload.reminderTime;
        touch(item);
        changed += 1;
      }
      return changed;
    },
    set_task_order: ({ orders }) => {
      for (const { id, sortOrder } of orders) {
        const item = findTask(id);
        if (item) item.sortOrder = sortOrder;
      }
      return orders.length;
    },
    set_sticky_note_color: ({ taskId, color }) => {
      const item = findTask(taskId);
      if (item) item.stickyColor = color;
    },
    create_recurring_task: ({ payload }) => {
      const item = recurring(newId("rec"), payload.description, { ...payload, createdAt: now(), nextTrigger: at(1, 9), tags: normalizeTags(payload.tags) });
      state.recurringTasks.push(item);
      return item;
    },
    update_recurring_task: ({ task: patch }) => {
      const item = findRecurring(patch.id);
      if (item) Object.assign(item, patch, { tags: normalizeTags(patch.tags) });
    },
    pause_recurring_task: ({ id }) => {
      const item = findRecurring(id);
      if (item) item.isPaused = true;
    },
    resume_recurring_task: ({ id }) => {
      const item = findRecurring(id);
      if (item) item.isPaused = false;
    },
    skip_recurring_occurrence: ({ id }) => findRecurring(id),
    delete_recurring_task: ({ id }) => {
      const item = findRecurring(id);
      if (item) item.deletedAt = now();
    },
    delete_reminder_record: ({ id }) => {
      const item = state.reminderRecords.find(entry => entry.id === id);
      if (item) item.deletedAt = now();
    },
    delete_reminder_records: ({ ids }) => ids.forEach(id => commands.delete_reminder_record({ id })),
    list_trash: () => ({
      tasks: state.tasks.filter(item => item.deletedAt),
      recurringTasks: state.recurringTasks.filter(item => item.deletedAt),
      retentionDays: 7
    }),
    preview_recurring_triggers: () => [],
    get_settings: () => ({ ...state.settings }),
    save_settings: ({ settings }) => {
      state.settings = { ...state.settings, ...settings };
    },
    list_sticky_note_summaries: () =>
      state.tasks.filter(item => !item.deletedAt && item.status === "PENDING" && (item.stickyContent || item.stickyIsOpen)).map(stickySummary),
    show_sticky_note: ({ taskId }) => {
      const item = findTask(taskId);
      if (item) item.stickyIsOpen = true;
    },
    open_sticky_note: ({ payload }) => {
      const item = findTask(payload.taskId);
      if (item) item.stickyIsOpen = true;
      return item ? stickySummary(item) : null;
    },
    close_sticky_note: ({ taskId }) => {
      const item = findTask(taskId);
      if (item) item.stickyIsOpen = false;
    },
    get_sync_status: () => ({ status: "never", error: null, time: null }),
    get_holiday_years: () => [2025, 2026],
    check_holiday_updates: () => ({ changed: false, years: [2025, 2026] }),
    get_notification_queue: () => [],
    is_dev_mode: () => false,
    get_ui_state: () => null,
    emit_ui_state_changed: () => null,
    apply_quick_add_shortcut: () => null,
    list_backups: () => ({ backups: [], directory: "" }),
    "plugin:app|version": () => "2.1.1",
    "plugin:app|name": () => "TaskReminderApp",
    "plugin:event|listen": ({ event, handler }) => {
      const list = listeners.get(event) ?? [];
      list.push(handler);
      listeners.set(event, list);
      return handler;
    },
    "plugin:event|unlisten": ({ event, eventId }) => {
      listeners.set(
        event,
        (listeners.get(event) ?? []).filter(id => id !== eventId)
      );
    },
    "plugin:event|emit": ({ event, payload }) => emit(event, payload),
    "plugin:event|emit_to": ({ event, payload }) => emit(event, payload),
    "plugin:updater|check": () => null,
    "plugin:window|get_all_windows": () => ["main"],
    "plugin:webview|get_all_webviews": () => [{ windowLabel: "main", label: "main" }]
  };

  const MUTATIONS = new Set([
    "create_task",
    "quick_add_task",
    "update_task",
    "complete_task",
    "uncomplete_task",
    "delete_task",
    "batch_update_tasks",
    "set_task_order",
    "set_sticky_note_color",
    "create_recurring_task",
    "update_recurring_task",
    "pause_recurring_task",
    "resume_recurring_task",
    "delete_recurring_task",
    "save_settings"
  ]);

  window.__TAURI_MOCK__ = { state, calls, emit };
  window.__TAURI_INTERNALS__ = {
    metadata: {
      currentWindow: { label: "main" },
      currentWebview: { label: "main", windowLabel: "main" }
    },
    plugins: {},
    transformCallback(callback, once = false) {
      const id = nextCallback++;
      callbacks.set(id, payload => {
        if (once) callbacks.delete(id);
        return callback?.(payload);
      });
      return id;
    },
    unregisterCallback(id) {
      callbacks.delete(id);
    },
    convertFileSrc: path => path,
    async invoke(cmd, args = {}) {
      calls.push({ cmd, args: JSON.parse(JSON.stringify(args ?? {})) });
      const handler = commands[cmd];
      if (!handler) {
        // 窗口、托盘等其他插件命令在浏览器中没有意义，返回空值。
        return null;
      }
      const result = await handler(args);
      if (MUTATIONS.has(cmd)) dataChanged();
      return result === undefined ? null : JSON.parse(JSON.stringify(result));
    }
  };
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
    unregisterListener(event, eventId) {
      commands["plugin:event|unlisten"]({ event, eventId });
    }
  };
})();
