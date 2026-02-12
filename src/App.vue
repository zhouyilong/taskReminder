<template>
  <div class="app" :class="{ 'light-theme': isLightTheme, 'is-linux': isLinuxPlatform }">
    <div class="titlebar" data-tauri-drag-region @dblclick="toggleMaximize">
      <div class="titlebar-left" data-tauri-drag-region>
        <span class="app-title">任务提醒 {{ appVersion }}<span v-if="isDevMode" class="dev-tag"> [开发]</span></span>
        <span class="tag">{{ syncStatusLabel }}</span>
      </div>
      <div class="titlebar-actions">
        <button class="icon-button theme-toggle" type="button" title="切换主题" @click="toggleTheme">
          <transition name="theme" mode="out-in">
            <svg v-if="isLightTheme" key="sun" viewBox="0 0 24 24" aria-hidden="true" class="theme-icon">
              <path
                d="M12 3a1 1 0 0 1 1 1v2.1a1 1 0 1 1-2 0V4a1 1 0 0 1 1-1zM6.2 6.2a1 1 0 0 1 1.4 0l1.5 1.5a1 1 0 0 1-1.4 1.4L6.2 7.6a1 1 0 0 1 0-1.4zM3 12a1 1 0 0 1 1-1h2.1a1 1 0 1 1 0 2H4a1 1 0 0 1-1-1zM6.2 17.8a1 1 0 0 1 1.4 0l1.5 1.5a1 1 0 0 1-1.4 1.4l-1.5-1.5a1 1 0 0 1 0-1.4zM12 17a1 1 0 0 1 1 1v2.1a1 1 0 1 1-2 0V18a1 1 0 0 1 1-1zM17.8 17.8a1 1 0 0 1 1.4 0 1 1 0 0 1 0 1.4l-1.5 1.5a1 1 0 1 1-1.4-1.4l1.5-1.5zM18.7 12a1 1 0 0 1 1-1H22a1 1 0 1 1 0 2h-2.3a1 1 0 0 1-1-1zM17.8 6.2a1 1 0 0 1 1.4 1.4l-1.5 1.5a1 1 0 1 1-1.4-1.4l1.5-1.5z"
              />
              <circle cx="12" cy="12" r="4" />
            </svg>
            <svg v-else key="moon" viewBox="0 0 24 24" aria-hidden="true" class="theme-icon">
              <path
                d="M20 15.2a8.2 8.2 0 0 1-10.2-10 9 9 0 1 0 10.2 10z"
              />
            </svg>
          </transition>
        </button>
        <button class="icon-button" type="button" title="设置" @click="openSettings">
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path
              d="M19.4 13a7.6 7.6 0 0 0 0-2l2-1.2a1 1 0 0 0 .4-1.3l-2-3.5a1 1 0 0 0-1.2-.4l-2.2.9a7.8 7.8 0 0 0-1.7-1L14.5 2a1 1 0 0 0-1-.8h-4a1 1 0 0 0-1 .8l-.3 2.4a7.8 7.8 0 0 0-1.7 1l-2.2-.9a1 1 0 0 0-1.2.4l-2 3.5a1 1 0 0 0 .4 1.3l2 1.2a7.6 7.6 0 0 0 0 2l-2 1.2a1 1 0 0 0-.4 1.3l2 3.5a1 1 0 0 0 1.2.4l2.2-.9a7.8 7.8 0 0 0 1.7 1l.3 2.4a1 1 0 0 0 1 .8h4a1 1 0 0 0 1-.8l.3-2.4a7.8 7.8 0 0 0 1.7-1l2.2.9a1 1 0 0 0 1.2-.4l2-3.5a1 1 0 0 0-.4-1.3l-2-1.2zM12 15.5a3.5 3.5 0 1 1 0-7 3.5 3.5 0 0 1 0 7z"
            />
          </svg>
        </button>
        <button class="icon-button" type="button" title="云同步" @click="openWebdav">
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path
              d="M7.5 18.5a4.5 4.5 0 0 1 0-9 5.5 5.5 0 0 1 10.8 1.6A4 4 0 0 1 17 18.5H7.5z"
            />
            <path d="M12 8v6" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
            <path d="M9.5 12l2.5 2.5L14.5 12" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
          </svg>
        </button>
      </div>
      <div class="titlebar-controls">
        <button class="titlebar-button" type="button" title="最小化" @click="handleMinimize">
          <svg viewBox="0 0 10 10" aria-hidden="true">
            <rect x="1" y="5" width="8" height="1.5" />
          </svg>
        </button>
        <button class="titlebar-button" type="button" :title="isWindowMaximized ? '还原' : '最大化'" @click="handleMaximize">
          <svg v-if="!isWindowMaximized" viewBox="0 0 10 10" aria-hidden="true">
            <rect x="2" y="2" width="6" height="6" fill="none" stroke="currentColor" stroke-width="1" />
          </svg>
          <svg v-else viewBox="0 0 10 10" aria-hidden="true">
            <rect x="1.5" y="3" width="5.5" height="5.5" fill="none" stroke="currentColor" stroke-width="1" />
            <rect x="3" y="1.5" width="5.5" height="5.5" fill="none" stroke="currentColor" stroke-width="1" />
          </svg>
        </button>
        <button class="titlebar-button close" type="button" title="关闭" @click="handleClose">
          <svg viewBox="0 0 10 10" aria-hidden="true">
            <path d="M2 2 L8 8 M8 2 L2 8" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" />
          </svg>
        </button>
      </div>
    </div>

    <div class="main" :style="{ zoom: uiScale }">
      <aside class="sidebar" :class="{ collapsed: isSidebarCollapsed }">
        <div class="sidebar-header">
          <span class="sidebar-title" v-if="!isSidebarCollapsed">菜单</span>
          <button
            class="sidebar-toggle"
            type="button"
            :title="isSidebarCollapsed ? '展开菜单' : '收起菜单'"
            @click="toggleSidebar"
          >
            <svg viewBox="0 0 24 24" aria-hidden="true">
              <path d="M14.5 6L8.5 12L14.5 18" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" />
            </svg>
          </button>
        </div>
        <button class="tab-button" :class="{ active: activeTab === 'tasks' }" @click="activeTab = 'tasks'">
          <span class="tab-icon">
            <svg viewBox="0 0 24 24" aria-hidden="true">
              <path d="M6 7h12M6 12h12M6 17h8" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
            </svg>
          </span>
          <span class="tab-text">待办事项</span>
        </button>
        <button class="tab-button" :class="{ active: activeTab === 'completed' }" @click="activeTab = 'completed'">
          <span class="tab-icon">
            <svg viewBox="0 0 24 24" aria-hidden="true">
              <path d="M6 12l4 4 8-8" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" />
            </svg>
          </span>
          <span class="tab-text">已办事项</span>
        </button>
        <button class="tab-button" :class="{ active: activeTab === 'recurring' }" @click="activeTab = 'recurring'">
          <span class="tab-icon">
            <svg viewBox="0 0 24 24" aria-hidden="true">
              <path d="M4 12a8 8 0 0 1 13.6-5.6" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
              <path d="M20 6v5h-5" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" />
              <path d="M20 12a8 8 0 0 1-13.6 5.6" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
              <path d="M4 18v-5h5" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" />
            </svg>
          </span>
          <span class="tab-text">循环提醒</span>
        </button>
        <button class="tab-button" :class="{ active: activeTab === 'records' }" @click="activeTab = 'records'">
          <span class="tab-icon">
            <svg viewBox="0 0 24 24" aria-hidden="true">
              <rect x="5" y="4" width="14" height="16" rx="2" ry="2" fill="none" stroke="currentColor" stroke-width="1.6" />
              <path d="M8 9h8M8 13h8M8 17h6" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
            </svg>
          </span>
          <span class="tab-text">提醒记录</span>
        </button>
      </aside>

      <section class="content">
        <Transition name="fade" mode="out-in">
          <div v-if="activeTab === 'tasks'" key="tasks" class="tab-panel">
          <div class="section-heading">
            <div class="section-title">待办事项</div>
            <span class="section-meta">共 {{ tasks.length }} 条任务</span>
          </div>
          <div class="form-row compact">
            <label class="field-label">描述</label>
            <input class="input" v-model="newTaskDescription" placeholder="输入任务描述" style="flex: 1" />
            <button class="button" @click="handleAddTask">添加任务</button>
          </div>
          <div class="table-card">
            <div class="table-scroll">
              <table class="table">
                <thead>
                  <tr>
                    <th>完成</th>
                    <th class="col-desc">描述</th>
                    <th class="col-datetime">提醒时间</th>
                    <th class="col-datetime">创建时间</th>
                  </tr>
                </thead>
                <tbody>
                  <tr
                    v-for="task in tasksPage"
                    :key="task.id"
                    class="table-row"
                    @dblclick="openEditTask(task)"
                    @contextmenu.prevent.stop="openTaskMenu($event, task)"
                  >
                    <td>
                      <input type="checkbox" :checked="task.status === 'COMPLETED'" @change="toggleTask(task)" />
                    </td>
                    <td class="col-desc" :title="task.description">{{ task.description }}</td>
                    <td class="col-datetime" :title="formatDateTime(task.reminderTime)">{{ formatDateTime(task.reminderTime) }}</td>
                    <td class="col-datetime" :title="formatDateTime(task.createdAt)">{{ formatDateTime(task.createdAt) }}</td>
                  </tr>
                </tbody>
              </table>
            </div>
            <div class="pagination">
              <span>共 {{ tasks.length }} 条</span>
              <select class="select" v-model.number="tasksPageSize">
                <option :value="10">10</option>
                <option :value="20">20</option>
                <option :value="50">50</option>
              </select>
              <button class="button secondary" :disabled="tasksPageIndex === 1" @click="tasksPageIndex--">上一页</button>
              <span>第 {{ tasksPageIndex }} / {{ tasksTotalPages }} 页</span>
              <button class="button secondary" :disabled="tasksPageIndex === tasksTotalPages" @click="tasksPageIndex++">下一页</button>
            </div>
          </div>
          </div>
          <div v-else-if="activeTab === 'completed'" key="completed" class="tab-panel">
          <div class="section-heading">
            <div class="section-title">已办事项</div>
            <span class="section-meta">筛选后 {{ filteredCompleted.length }} 条</span>
          </div>
          <div class="form-row compact">
            <label class="field-label">描述</label>
            <input class="input" v-model="completedFilter" placeholder="按描述过滤" style="flex: 1" />
          </div>
          <div class="table-card">
            <div class="table-scroll">
              <table class="table">
                <thead>
                  <tr>
                    <th>取消完成</th>
                    <th class="col-desc">描述</th>
                    <th class="col-datetime">创建时间</th>
                    <th class="col-datetime">完成时间</th>
                  </tr>
                </thead>
                <tbody>
                  <tr
                    v-for="task in completedPage"
                    :key="task.id"
                    class="table-row"
                    @dblclick="openTaskDetail(task)"
                    @contextmenu.prevent.stop="openCompletedMenu($event, task)"
                  >
                    <td>
                      <input type="checkbox" checked @change="toggleTask(task)" />
                    </td>
                    <td class="col-desc" :title="task.description">{{ task.description }}</td>
                    <td class="col-datetime" :title="formatDateTime(task.createdAt)">{{ formatDateTime(task.createdAt) }}</td>
                    <td class="col-datetime" :title="formatDateTime(task.completedAt)">{{ formatDateTime(task.completedAt) }}</td>
                  </tr>
                </tbody>
              </table>
            </div>
            <div class="pagination">
              <span>共 {{ filteredCompleted.length }} 条</span>
              <select class="select" v-model.number="completedPageSize">
                <option :value="10">10</option>
                <option :value="20">20</option>
                <option :value="50">50</option>
              </select>
              <button class="button secondary" :disabled="completedPageIndex === 1" @click="completedPageIndex--">上一页</button>
              <span>第 {{ completedPageIndex }} / {{ completedTotalPages }} 页</span>
              <button class="button secondary" :disabled="completedPageIndex === completedTotalPages" @click="completedPageIndex++">下一页</button>
            </div>
          </div>
          </div>
          <div v-else-if="activeTab === 'recurring'" key="recurring" class="tab-panel">
          <div class="section-heading">
            <div class="section-title">循环提醒</div>
            <span class="section-meta">共 {{ recurringTasks.length }} 条配置</span>
          </div>
          <div class="form-row compact">
            <label class="field-label">描述</label>
            <input class="input" v-model="newRecurringDescription" placeholder="输入提醒描述" style="flex: 1" />
            <label class="field-label">模式</label>
            <select class="select" v-model="newRecurringMode" style="width: 140px">
              <option v-for="mode in recurringModeOptions" :key="mode.value" :value="mode.value">{{ mode.label }}</option>
            </select>
            <button class="button" @click="handleAddRecurring">添加提醒</button>
          </div>
          <div class="form-row compact">
            <template v-if="newRecurringMode === 'INTERVAL_RANGE'">
              <label class="field-label">间隔</label>
              <input class="input" type="number" v-model.number="newRecurringInterval" min="1" placeholder="分钟" style="width: 100px" />
              <label class="field-label">开始</label>
              <input class="input" type="time" v-model="newRecurringStart" style="width: 120px" />
              <label class="field-label">结束</label>
              <input class="input" type="time" v-model="newRecurringEnd" style="width: 120px" />
            </template>
            <template v-else-if="newRecurringMode === 'DAILY'">
              <label class="field-label">每天</label>
              <input class="input" type="time" v-model="newRecurringScheduleTime" style="width: 140px" />
            </template>
            <template v-else-if="newRecurringMode === 'WEEKLY'">
              <label class="field-label">周几</label>
              <select class="select" v-model.number="newRecurringWeekday" style="width: 120px">
                <option v-for="item in weekdayOptions" :key="item.value" :value="item.value">{{ item.label }}</option>
              </select>
              <label class="field-label">时间</label>
              <input class="input" type="time" v-model="newRecurringScheduleTime" style="width: 140px" />
            </template>
            <template v-else-if="newRecurringMode === 'MONTHLY'">
              <label class="field-label">每月几号</label>
              <input class="input" type="number" min="1" max="31" v-model.number="newRecurringDayOfMonth" style="width: 120px" />
              <label class="field-label">时间</label>
              <input class="input" type="time" v-model="newRecurringScheduleTime" style="width: 140px" />
            </template>
            <template v-else>
              <label class="field-label">Cron</label>
              <input class="input" v-model="newRecurringCronExpression" placeholder="如: 0 9 * * *" style="flex: 1" />
            </template>
          </div>
          <div class="table-card">
            <div class="table-scroll">
              <table class="table recurring-table">
                <thead>
                  <tr>
                    <th class="col-desc">描述</th>
                    <th class="col-mode">模式</th>
                    <th class="col-rule">规则</th>
                    <th class="col-datetime col-next-trigger">下次触发</th>
                    <th class="col-status">状态</th>
                  </tr>
                </thead>
                <tbody>
                  <tr
                    v-for="task in recurringPage"
                    :key="task.id"
                    class="table-row"
                    @dblclick="openEditRecurring(task)"
                    @contextmenu.prevent.stop="openRecurringMenu($event, task)"
                  >
                    <td class="col-desc" :title="task.description">{{ task.description }}</td>
                    <td class="col-mode" :title="formatRecurringMode(task.repeatMode)">{{ formatRecurringMode(task.repeatMode) }}</td>
                    <td class="col-rule" :title="formatRecurringRule(task)">{{ formatRecurringRule(task) }}</td>
                    <td class="col-datetime col-next-trigger" :title="formatDateTime(task.nextTrigger)">{{ formatDateTime(task.nextTrigger) }}</td>
                    <td class="col-status" :title="task.isPaused ? '已暂停' : '运行中'">{{ task.isPaused ? "已暂停" : "运行中" }}</td>
                  </tr>
                </tbody>
              </table>
            </div>
            <div class="pagination">
              <span>共 {{ recurringTasks.length }} 条</span>
              <select class="select" v-model.number="recurringPageSize">
                <option :value="10">10</option>
                <option :value="20">20</option>
                <option :value="50">50</option>
              </select>
              <button class="button secondary" :disabled="recurringPageIndex === 1" @click="recurringPageIndex--">上一页</button>
              <span>第 {{ recurringPageIndex }} / {{ recurringTotalPages }} 页</span>
              <button class="button secondary" :disabled="recurringPageIndex === recurringTotalPages" @click="recurringPageIndex++">下一页</button>
            </div>
          </div>
          </div>
          <div v-else-if="activeTab === 'records'" key="records" class="tab-panel">
          <div class="section-heading">
            <div class="section-title">提醒记录</div>
            <span class="section-meta">筛选后 {{ filteredRecords.length }} 条</span>
          </div>
          <div class="form-row compact">
            <label class="field-label">开始</label>
            <input class="input" type="date" v-model="recordFilterStart" @change="handleRecordDatePicked" />
            <label class="field-label">结束</label>
            <input class="input" type="date" v-model="recordFilterEnd" @change="handleRecordDatePicked" />
            <label class="field-label">类型</label>
            <select class="select" v-model="recordFilterType">
              <option value="all">全部</option>
              <option value="TASK">任务</option>
              <option value="RECURRING">循环</option>
            </select>
            <button class="button secondary" @click="applyRecordFilter">应用过滤</button>
            <button class="button secondary" @click="clearRecordFilter">清除过滤</button>
            <button class="button danger" @click="deleteSelectedRecords">批量删除</button>
          </div>
          <div class="table-card">
            <div class="table-scroll">
              <table class="table records-table">
                <thead>
                  <tr>
                    <th>选择</th>
                    <th class="col-desc">描述</th>
                    <th>类型</th>
                    <th class="col-datetime">触发时间</th>
                    <th class="col-datetime">关闭时间</th>
                    <th>操作</th>
                  </tr>
                </thead>
                <tbody>
                  <tr
                    v-for="record in recordPage"
                    :key="record.id"
                    class="table-row"
                    @dblclick="openRecordDetail(record)"
                    @contextmenu.prevent.stop="openRecordMenu($event, record)"
                  >
                    <td>
                      <input type="checkbox" v-model="selectedRecords" :value="record.id" />
                    </td>
                    <td class="col-desc" :title="record.description">{{ record.description }}</td>
                    <td :title="record.type === 'TASK' ? '任务' : '循环'">{{ record.type === 'TASK' ? '任务' : '循环' }}</td>
                    <td class="col-datetime" :title="formatDateTime(record.triggerTime)">{{ formatDateTime(record.triggerTime) }}</td>
                    <td class="col-datetime" :title="formatDateTime(record.closeTime)">{{ formatDateTime(record.closeTime) }}</td>
                    <td :title="formatAction(record.action)">{{ formatAction(record.action) }}</td>
                  </tr>
                </tbody>
              </table>
            </div>
            <div class="pagination">
              <span>共 {{ filteredRecords.length }} 条</span>
              <select class="select" v-model.number="recordPageSize">
                <option :value="10">10</option>
                <option :value="20">20</option>
                <option :value="50">50</option>
              </select>
              <button class="button secondary" :disabled="recordPageIndex === 1" @click="recordPageIndex--">上一页</button>
              <span>第 {{ recordPageIndex }} / {{ recordTotalPages }} 页</span>
              <button class="button secondary" :disabled="recordPageIndex === recordTotalPages" @click="recordPageIndex++">下一页</button>
            </div>
          </div>
          </div>
        </Transition>
      </section>
    </div>

    <Modal :open="editTaskOpen" title="编辑任务" :showDelete="true" @close="editTaskOpen = false" @confirm="saveTaskEdit" @delete="deleteTaskFromModal">
      <div class="form-row">
        <input class="input" v-model="editTaskDescription" placeholder="任务描述" style="flex: 1" />
      </div>
      <div class="form-row">
        <input
          ref="editTaskReminderInput"
          class="input"
          type="datetime-local"
          v-model="editTaskReminder"
          @change="handleTaskReminderPicked"
        />
        <button class="button secondary" @click="clearTaskReminder">清除提醒</button>
      </div>
    </Modal>

    <Modal :open="editRecurringOpen" title="编辑循环提醒" :showDelete="true" @close="editRecurringOpen = false" @confirm="saveRecurringEdit" @delete="deleteRecurringFromModal">
      <div class="form-row">
        <input class="input" v-model="editRecurringDescription" placeholder="提醒描述" style="flex: 1" />
      </div>
      <div class="form-row">
        <label class="field-label">模式</label>
        <select class="select" v-model="editRecurringMode" style="width: 180px">
          <option v-for="mode in recurringModeOptions" :key="mode.value" :value="mode.value">{{ mode.label }}</option>
        </select>
      </div>
      <div class="form-row">
        <template v-if="editRecurringMode === 'INTERVAL_RANGE'">
          <input class="input" type="number" min="1" v-model.number="editRecurringInterval" style="width: 120px" />
          <input class="input" type="time" v-model="editRecurringStart" style="width: 140px" />
          <input class="input" type="time" v-model="editRecurringEnd" style="width: 140px" />
        </template>
        <template v-else-if="editRecurringMode === 'DAILY'">
          <input class="input" type="time" v-model="editRecurringScheduleTime" style="width: 160px" />
        </template>
        <template v-else-if="editRecurringMode === 'WEEKLY'">
          <select class="select" v-model.number="editRecurringWeekday" style="width: 120px">
            <option v-for="item in weekdayOptions" :key="item.value" :value="item.value">{{ item.label }}</option>
          </select>
          <input class="input" type="time" v-model="editRecurringScheduleTime" style="width: 160px" />
        </template>
        <template v-else-if="editRecurringMode === 'MONTHLY'">
          <input class="input" type="number" min="1" max="31" v-model.number="editRecurringDayOfMonth" style="width: 120px" />
          <input class="input" type="time" v-model="editRecurringScheduleTime" style="width: 160px" />
        </template>
        <template v-else>
          <input class="input" v-model="editRecurringCronExpression" placeholder="如: 0 9 * * *" style="flex: 1" />
        </template>
      </div>
    </Modal>

    <div
      v-if="contextMenu.visible"
      class="context-menu"
      :style="{ top: contextMenu.y + 'px', left: contextMenu.x + 'px' }"
      @click.stop
    >
      <button
        v-for="item in contextMenu.items"
        :key="item.label"
        class="context-menu-item"
        :class="{ danger: item.danger }"
        type="button"
        @click="item.action()"
      >
        {{ item.label }}
      </button>
    </div>

    <Modal :open="detailOpen" :title="detailTitle" @close="detailOpen = false" @confirm="detailOpen = false">
      <div class="detail-list">
        <div v-for="item in detailItems" :key="item.label" class="detail-item">
          <span class="detail-label">{{ item.label }}</span>
          <span class="detail-value">{{ item.value }}</span>
        </div>
      </div>
    </Modal>

    <Modal :open="settingsOpen" title="应用设置" @close="settingsOpen = false" @confirm="saveSettings">
      <div class="modal-section">
        <div class="form-row compact">
          <label>
            <input type="checkbox" v-model="settingsDraft.autoStartEnabled" /> 开机自启
          </label>
          <label>
            <input type="checkbox" v-model="settingsDraft.soundEnabled" /> 提示音
          </label>
        </div>
        <div class="form-row compact">
          <label>稍后提醒分钟数</label>
          <input class="input" type="number" min="1" v-model.number="settingsDraft.snoozeMinutes" />
        </div>
      </div>
      <div class="modal-section">
        <div class="form-row compact">
          <label>界面缩放</label>
          <input class="input" type="range" min="0.8" max="1.2" step="0.05" v-model.number="uiScale" style="flex: 1" />
          <span class="tag">{{ uiScalePercent }}%</span>
        </div>
      </div>
      <div class="modal-section">
        <div class="form-row compact">
          <label>提醒弹窗主题</label>
          <select class="select" v-model="settingsDraft.notificationTheme">
            <option value="system">跟随系统</option>
            <option value="app">跟随应用</option>
            <option value="light">浅色</option>
            <option value="dark">深色</option>
          </select>
        </div>
      </div>
    </Modal>

    <Modal :open="webdavOpen" title="云同步设置" @close="webdavOpen = false" @confirm="saveWebdavSettings">
      <div class="modal-section">
        <div class="form-row compact">
          <label>
            <input type="checkbox" v-model="settingsDraft.webdavEnabled" /> 启用 WebDAV
          </label>
        </div>
      </div>
      <div class="modal-section">
        <div class="form-row compact">
          <input class="input" v-model="settingsDraft.webdavUrl" placeholder="WebDAV 地址" style="flex: 1" />
        </div>
        <div class="form-row compact">
          <input class="input" v-model="settingsDraft.webdavUsername" placeholder="用户名" style="flex: 1" />
          <input
            class="input"
            :type="webdavPasswordVisible ? 'text' : 'password'"
            v-model="settingsDraft.webdavPassword"
            placeholder="密码"
            style="flex: 1"
          />
          <button class="button secondary" type="button" @click="webdavPasswordVisible = !webdavPasswordVisible">
            {{ webdavPasswordVisible ? "隐藏" : "显示" }}
          </button>
        </div>
        <div class="form-row compact">
          <input class="input" v-model="settingsDraft.webdavRootPath" placeholder="远端路径" style="flex: 1" />
          <input class="input" type="number" min="1" v-model.number="settingsDraft.webdavSyncIntervalMinutes" placeholder="同步频率(分钟)" style="width: 160px" />
        </div>
      </div>
      <div class="modal-section">
        <div class="form-row compact" style="gap: 8px;">
          <button class="button secondary" type="button" @click="handleTestWebdav">测试连接</button>
          <button class="button secondary" type="button" @click="handleSyncNow">立即同步</button>
        </div>
      </div>
      <div class="modal-section">
        <div class="form-row compact sync-status-panel">
          <div class="sync-status-row">
            <span class="sync-status-label">上次同步:</span>
            <span class="sync-status-value">{{ formatDateTime(settingsDraft.webdavLastSyncTime) }}</span>
          </div>
          <div class="sync-status-row">
            <span class="sync-status-label">上次本地变更:</span>
            <span class="sync-status-value">{{ formatDateTime(settingsDraft.webdavLastLocalChangeTime) }}</span>
          </div>
          <div class="sync-status-row">
            <span class="sync-status-label">同步状态:</span>
            <span class="sync-status-value">{{ settingsDraft.webdavLastSyncStatus || "未同步" }}</span>
          </div>
          <div class="sync-status-row">
            <span class="sync-status-label">最近错误:</span>
            <span class="sync-status-value">{{ settingsDraft.webdavLastSyncError || "无" }}</span>
          </div>
        </div>
      </div>
    </Modal>

    <Modal :open="confirmDeleteOpen" title="确认删除" @close="closeDeleteConfirm" @confirm="handleConfirmDelete">
      <div class="modal-text">{{ confirmDeleteMessage }}</div>
    </Modal>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow, type Window as TauriWindow } from "@tauri-apps/api/window";
import Modal from "./components/Modal.vue";
import { api } from "./api";
import { safeStorage } from "./safeStorage";
import type {
  Task,
  RecurringTask,
  RecurringMode,
  ReminderRecord,
  AppSettings,
  SyncStatus
} from "./types";

const activeTab = ref("tasks");
const isLightTheme = ref(safeStorage.getItem("appTheme") === "light");
const appVersion = ref("");
const isDevMode = ref(false);
const isWindowMaximized = ref(false);
const resolveCurrentWindow = (): TauriWindow | null => {
  try {
    return getCurrentWindow();
  } catch {
    return null;
  }
};
const appWindow = resolveCurrentWindow();
const uiScale = ref(Number(safeStorage.getItem("uiScale") ?? "1"));
const isSidebarCollapsed = ref(safeStorage.getItem("sidebarCollapsed") === "1");

const tasks = ref<Task[]>([]);
const completedTasks = ref<Task[]>([]);
const recurringTasks = ref<RecurringTask[]>([]);
const reminderRecords = ref<ReminderRecord[]>([]);
const syncStatus = ref<SyncStatus | null>(null);

const recurringModeOptions: { value: RecurringMode; label: string }[] = [
  { value: "INTERVAL_RANGE", label: "区间间隔" },
  { value: "DAILY", label: "每天固定时间" },
  { value: "WEEKLY", label: "每周固定时间" },
  { value: "MONTHLY", label: "每月固定时间" },
  { value: "CRON", label: "Cron 表达式" },
];

const weekdayOptions: { value: number; label: string }[] = [
  { value: 1, label: "周一" },
  { value: 2, label: "周二" },
  { value: 3, label: "周三" },
  { value: 4, label: "周四" },
  { value: 5, label: "周五" },
  { value: 6, label: "周六" },
  { value: 7, label: "周日" },
];

const newTaskDescription = ref("");
const newRecurringDescription = ref("");
const newRecurringInterval = ref(60);
const newRecurringStart = ref("08:00");
const newRecurringEnd = ref("17:30");
const newRecurringMode = ref<RecurringMode>("INTERVAL_RANGE");
const newRecurringScheduleTime = ref("09:00");
const newRecurringWeekday = ref(1);
const newRecurringDayOfMonth = ref(1);
const newRecurringCronExpression = ref("0 9 * * *");

const editTaskOpen = ref(false);
const editTaskId = ref("");
const editTaskDescription = ref("");
const editTaskReminder = ref("");
const editTaskReminderInput = ref<HTMLInputElement | null>(null);
const isLinuxPlatform =
  typeof navigator !== "undefined" && /linux/i.test(navigator.userAgent);
const shouldAutoCloseDateTimePicker = !isLinuxPlatform;

const editRecurringOpen = ref(false);
const editRecurringId = ref("");
const editRecurringDescription = ref("");
const editRecurringInterval = ref(60);
const editRecurringStart = ref("");
const editRecurringEnd = ref("");
const editRecurringMode = ref<RecurringMode>("INTERVAL_RANGE");
const editRecurringScheduleTime = ref("09:00");
const editRecurringWeekday = ref(1);
const editRecurringDayOfMonth = ref(1);
const editRecurringCronExpression = ref("");

const settingsOpen = ref(false);
const webdavOpen = ref(false);
const webdavPasswordVisible = ref(false);
const settingsDraft = reactive<AppSettings>({
  autoStartEnabled: false,
  soundEnabled: true,
  snoozeMinutes: 5,
  webdavEnabled: false,
  webdavUrl: "",
  webdavUsername: "",
  webdavPassword: "",
  webdavRootPath: "",
  webdavSyncIntervalMinutes: 60,
  webdavDeviceId: "",
  notificationTheme: "app"
});

const tasksPageIndex = ref(1);
const tasksPageSize = ref(20);
const completedFilter = ref("");
const completedPageIndex = ref(1);
const completedPageSize = ref(20);
const recurringPageIndex = ref(1);
const recurringPageSize = ref(20);

const recordFilterStart = ref("");
const recordFilterEnd = ref("");
const recordFilterType = ref("all");
const recordPageIndex = ref(1);
const recordPageSize = ref(20);
const selectedRecords = ref<string[]>([]);

const detailOpen = ref(false);
const detailTitle = ref("");
const detailItems = ref<{ label: string; value: string }[]>([]);

type PendingDelete =
  | { kind: "task"; id: string; closeEdit?: boolean }
  | { kind: "recurring"; id: string; closeEdit?: boolean }
  | { kind: "record"; id: string }
  | { kind: "records"; ids: string[] };

const confirmDeleteOpen = ref(false);
const confirmDeleteMessage = ref("");
const pendingDelete = ref<PendingDelete | null>(null);

type ContextMenuItem = { label: string; action: () => void; danger?: boolean };
const contextMenu = reactive({
  visible: false,
  x: 0,
  y: 0,
  items: [] as ContextMenuItem[],
});

const syncStatusLabel = computed(() => {
  if (!syncStatus.value || !syncStatus.value.status) {
    return "同步: 未同步";
  }
  return `同步: ${syncStatus.value.status}`;
});

const uiScalePercent = computed(() => Math.round(uiScale.value * 100));

const tasksTotalPages = computed(() => {
  return Math.max(1, Math.ceil(tasks.value.length / tasksPageSize.value));
});

const tasksPage = computed(() => {
  const start = (tasksPageIndex.value - 1) * tasksPageSize.value;
  return tasks.value.slice(start, start + tasksPageSize.value);
});

const filteredCompleted = computed(() => {
  const keyword = completedFilter.value.trim().toLowerCase();
  if (!keyword) {
    return completedTasks.value;
  }
  return completedTasks.value.filter(task => task.description.toLowerCase().includes(keyword));
});

const completedTotalPages = computed(() => {
  return Math.max(1, Math.ceil(filteredCompleted.value.length / completedPageSize.value));
});

const completedPage = computed(() => {
  const start = (completedPageIndex.value - 1) * completedPageSize.value;
  return filteredCompleted.value.slice(start, start + completedPageSize.value);
});

const recurringTotalPages = computed(() => {
  return Math.max(1, Math.ceil(recurringTasks.value.length / recurringPageSize.value));
});

const recurringPage = computed(() => {
  const start = (recurringPageIndex.value - 1) * recurringPageSize.value;
  return recurringTasks.value.slice(start, start + recurringPageSize.value);
});

const filteredRecords = computed(() => {
  return reminderRecords.value.filter(record => {
    if (recordFilterType.value !== "all" && record.type !== recordFilterType.value) {
      return false;
    }
    if (recordFilterStart.value) {
      if (record.triggerTime < `${recordFilterStart.value}T00:00:00`) {
        return false;
      }
    }
    if (recordFilterEnd.value) {
      if (record.triggerTime > `${recordFilterEnd.value}T23:59:59`) {
        return false;
      }
    }
    return true;
  });
});

const recordTotalPages = computed(() => {
  return Math.max(1, Math.ceil(filteredRecords.value.length / recordPageSize.value));
});

const recordPage = computed(() => {
  const start = (recordPageIndex.value - 1) * recordPageSize.value;
  return filteredRecords.value.slice(start, start + recordPageSize.value);
});

watch([tasks, tasksPageSize], () => {
  if (tasksPageIndex.value > tasksTotalPages.value) {
    tasksPageIndex.value = tasksTotalPages.value;
  }
});

watch([filteredCompleted, completedPageSize], () => {
  if (completedPageIndex.value > completedTotalPages.value) {
    completedPageIndex.value = completedTotalPages.value;
  }
});

watch(completedFilter, () => {
  completedPageIndex.value = 1;
});

watch([recurringTasks, recurringPageSize], () => {
  if (recurringPageIndex.value > recurringTotalPages.value) {
    recurringPageIndex.value = recurringTotalPages.value;
  }
});

watch([filteredRecords, recordPageSize], () => {
  if (recordPageIndex.value > recordTotalPages.value) {
    recordPageIndex.value = recordTotalPages.value;
  }
});

const formatDateTime = (value?: string | null) => {
  if (!value) {
    return "-";
  }
  return value.replace("T", " ");
};

const formatAction = (action: string) => {
  switch (action) {
    case "DISMISSED":
      return "已关闭";
    case "SNOOZED":
      return "已推迟";
    case "COMPLETED":
      return "已完成";
    case "PENDING":
      return "待处理";
    default:
      return action;
  }
};

const weekdayLabel = (value?: number | null) => {
  return weekdayOptions.find(item => item.value === value)?.label ?? "-";
};

const formatRecurringMode = (mode?: RecurringMode | string | null) => {
  const resolved = recurringModeOptions.find(item => item.value === mode);
  return resolved ? resolved.label : "区间间隔";
};

const formatRecurringRule = (task: RecurringTask) => {
  switch (task.repeatMode) {
    case "DAILY":
      return `每天 ${task.scheduleTime || "-"}`;
    case "WEEKLY":
      return `${weekdayLabel(task.scheduleWeekday)} ${task.scheduleTime || "-"}`;
    case "MONTHLY":
      return `每月 ${task.scheduleDay || "-"} 日 ${task.scheduleTime || "-"}`;
    case "CRON":
      return task.cronExpression || "-";
    case "INTERVAL_RANGE":
    default: {
      const start = task.startTime || "00:00";
      const end = task.endTime || "23:59";
      return `每 ${task.intervalMinutes} 分钟（${start} - ${end}）`;
    }
  }
};

type RecurringDraft = {
  mode: RecurringMode;
  intervalMinutes: number;
  startTime: string;
  endTime: string;
  scheduleTime: string;
  scheduleWeekday: number;
  scheduleDay: number;
  cronExpression: string;
};

const validateRecurringDraft = (draft: RecurringDraft) => {
  switch (draft.mode) {
    case "INTERVAL_RANGE":
      if (!Number.isFinite(draft.intervalMinutes) || draft.intervalMinutes < 1) {
        alert("间隔分钟数必须大于 0");
        return false;
      }
      if (draft.startTime && draft.endTime && draft.startTime > draft.endTime) {
        alert("开始时间不能晚于结束时间");
        return false;
      }
      return true;
    case "DAILY":
      if (!draft.scheduleTime) {
        alert("每日模式需要选择触发时间");
        return false;
      }
      return true;
    case "WEEKLY":
      if (!draft.scheduleTime) {
        alert("每周模式需要选择触发时间");
        return false;
      }
      if (draft.scheduleWeekday < 1 || draft.scheduleWeekday > 7) {
        alert("每周模式中的周几必须在 1 到 7 之间");
        return false;
      }
      return true;
    case "MONTHLY":
      if (!draft.scheduleTime) {
        alert("每月模式需要选择触发时间");
        return false;
      }
      if (draft.scheduleDay < 1 || draft.scheduleDay > 31) {
        alert("每月模式中的几号必须在 1 到 31 之间");
        return false;
      }
      return true;
    case "CRON":
      if (!draft.cronExpression.trim()) {
        alert("Cron 表达式不能为空");
        return false;
      }
      return true;
    default:
      return false;
  }
};

const buildRecurringPayload = (draft: RecurringDraft) => {
  const payload = {
    intervalMinutes: Math.max(1, draft.intervalMinutes || 1),
    startTime: null as string | null,
    endTime: null as string | null,
    repeatMode: draft.mode,
    scheduleTime: null as string | null,
    scheduleWeekday: null as number | null,
    scheduleDay: null as number | null,
    cronExpression: null as string | null,
  };
  switch (draft.mode) {
    case "INTERVAL_RANGE":
      payload.startTime = draft.startTime || null;
      payload.endTime = draft.endTime || null;
      break;
    case "DAILY":
      payload.scheduleTime = draft.scheduleTime || null;
      break;
    case "WEEKLY":
      payload.scheduleTime = draft.scheduleTime || null;
      payload.scheduleWeekday = draft.scheduleWeekday;
      break;
    case "MONTHLY":
      payload.scheduleTime = draft.scheduleTime || null;
      payload.scheduleDay = draft.scheduleDay;
      break;
    case "CRON":
      payload.cronExpression = draft.cronExpression.trim() || null;
      break;
    default:
      break;
  }
  return payload;
};

const resetNewRecurringForm = () => {
  newRecurringDescription.value = "";
  newRecurringInterval.value = 60;
  newRecurringStart.value = "08:00";
  newRecurringEnd.value = "17:30";
  newRecurringMode.value = "INTERVAL_RANGE";
  newRecurringScheduleTime.value = "09:00";
  newRecurringWeekday.value = 1;
  newRecurringDayOfMonth.value = 1;
  newRecurringCronExpression.value = "0 9 * * *";
};

const openDeleteConfirm = (message: string, payload: PendingDelete) => {
  confirmDeleteMessage.value = message;
  pendingDelete.value = payload;
  confirmDeleteOpen.value = true;
};

const closeDeleteConfirm = () => {
  confirmDeleteOpen.value = false;
  pendingDelete.value = null;
};

const handleConfirmDelete = async () => {
  const payload = pendingDelete.value;
  if (!payload) {
    closeDeleteConfirm();
    return;
  }
  try {
    switch (payload.kind) {
      case "task":
        await api.deleteTask(payload.id);
        if (payload.closeEdit) {
          editTaskOpen.value = false;
        }
        break;
      case "recurring":
        await api.deleteRecurringTask(payload.id);
        if (payload.closeEdit) {
          editRecurringOpen.value = false;
        }
        break;
      case "record":
        await api.deleteReminderRecord(payload.id);
        break;
      case "records":
        await api.deleteReminderRecords(payload.ids);
        selectedRecords.value = [];
        break;
      default:
        break;
    }
    await refreshAll();
  } finally {
    closeDeleteConfirm();
  }
};

const toDatetimeLocal = (value?: string | null) => {
  if (!value) {
    return "";
  }
  return value.slice(0, 16);
};

const fromDatetimeLocal = (value: string) => {
  if (!value) {
    return null;
  }
  return value.length === 16 ? `${value}:00` : value;
};

const refreshAll = async () => {
  tasks.value = await api.listActiveTasks();
  completedTasks.value = await api.listCompletedTasks();
  recurringTasks.value = await api.listRecurringTasks();
  reminderRecords.value = await api.listReminderRecords();
};

const loadSettings = async () => {
  const data = await api.getSettings();
  Object.assign(settingsDraft, data);
};


const handleAddTask = async () => {
  if (!newTaskDescription.value.trim()) {
    return;
  }
  await api.createTask(newTaskDescription.value.trim());
  newTaskDescription.value = "";
  await refreshAll();
};

const toggleTask = async (task: Task) => {
  if (task.status === "COMPLETED") {
    await api.uncompleteTask(task.id);
  } else {
    await api.completeTask(task.id);
  }
  await refreshAll();
};

const openEditTask = (task: Task) => {
  editTaskId.value = task.id;
  editTaskDescription.value = task.description;
  editTaskReminder.value = toDatetimeLocal(task.reminderTime ?? null);
  editTaskOpen.value = true;
};

const clearTaskReminder = () => {
  editTaskReminder.value = "";
};

const closeTaskReminderPicker = () => {
  requestAnimationFrame(() => {
    editTaskReminderInput.value?.blur();
  });
};

const handleTaskReminderPicked = () => {
  if (!shouldAutoCloseDateTimePicker) {
    return;
  }
  closeTaskReminderPicker();
};

const handleRecordDatePicked = (event: Event) => {
  if (!isLinuxPlatform) {
    return;
  }
  const target = event.target;
  if (!(target instanceof HTMLInputElement)) {
    return;
  }
  requestAnimationFrame(() => {
    target.blur();
  });
};

const saveTaskEdit = async () => {
  await api.updateTask({
    id: editTaskId.value,
    description: editTaskDescription.value,
    reminderTime: fromDatetimeLocal(editTaskReminder.value)
  });
  editTaskOpen.value = false;
  await refreshAll();
};

const deleteTaskFromModal = async () => {
  const task = tasks.value.find(item => item.id === editTaskId.value) || completedTasks.value.find(item => item.id === editTaskId.value);
  if (!task) {
    editTaskOpen.value = false;
    return;
  }
  openDeleteConfirm("确定要删除此任务吗？", { kind: "task", id: task.id, closeEdit: true });
};

const handleAddRecurring = async () => {
  if (!newRecurringDescription.value.trim()) {
    return;
  }
  const draft: RecurringDraft = {
    mode: newRecurringMode.value,
    intervalMinutes: newRecurringInterval.value,
    startTime: newRecurringStart.value,
    endTime: newRecurringEnd.value,
    scheduleTime: newRecurringScheduleTime.value,
    scheduleWeekday: newRecurringWeekday.value,
    scheduleDay: newRecurringDayOfMonth.value,
    cronExpression: newRecurringCronExpression.value,
  };
  if (!validateRecurringDraft(draft)) {
    return;
  }
  await api.createRecurringTask({
    description: newRecurringDescription.value.trim(),
    ...buildRecurringPayload(draft)
  });
  resetNewRecurringForm();
  await refreshAll();
};

const openEditRecurring = (task: RecurringTask) => {
  editRecurringId.value = task.id;
  editRecurringDescription.value = task.description;
  editRecurringInterval.value = task.intervalMinutes;
  editRecurringStart.value = task.startTime ?? "";
  editRecurringEnd.value = task.endTime ?? "";
  editRecurringMode.value = (task.repeatMode || "INTERVAL_RANGE") as RecurringMode;
  editRecurringScheduleTime.value = task.scheduleTime ?? "09:00";
  editRecurringWeekday.value = task.scheduleWeekday ?? 1;
  editRecurringDayOfMonth.value = task.scheduleDay ?? 1;
  editRecurringCronExpression.value = task.cronExpression ?? "";
  editRecurringOpen.value = true;
};

const saveRecurringEdit = async () => {
  const target = recurringTasks.value.find(item => item.id === editRecurringId.value);
  if (!target) {
    return;
  }
  const draft: RecurringDraft = {
    mode: editRecurringMode.value,
    intervalMinutes: editRecurringInterval.value,
    startTime: editRecurringStart.value,
    endTime: editRecurringEnd.value,
    scheduleTime: editRecurringScheduleTime.value,
    scheduleWeekday: editRecurringWeekday.value,
    scheduleDay: editRecurringDayOfMonth.value,
    cronExpression: editRecurringCronExpression.value,
  };
  if (!validateRecurringDraft(draft)) {
    return;
  }
  await api.updateRecurringTask({
    ...target,
    description: editRecurringDescription.value,
    ...buildRecurringPayload(draft)
  });
  editRecurringOpen.value = false;
  await refreshAll();
};

const toggleRecurring = async (task: RecurringTask) => {
  if (task.isPaused) {
    await api.resumeRecurringTask(task.id);
  } else {
    await api.pauseRecurringTask(task.id);
  }
  await refreshAll();
};

const deleteRecurringFromModal = async () => {
  const task = recurringTasks.value.find(item => item.id === editRecurringId.value);
  if (!task) {
    editRecurringOpen.value = false;
    return;
  }
  openDeleteConfirm("确定要删除此循环提醒吗？", { kind: "recurring", id: task.id, closeEdit: true });
};

const applyRecordFilter = () => {
  recordPageIndex.value = 1;
};

const clearRecordFilter = () => {
  recordFilterStart.value = "";
  recordFilterEnd.value = "";
  recordFilterType.value = "all";
  recordPageIndex.value = 1;
};

const deleteSelectedRecords = async () => {
  if (selectedRecords.value.length === 0) {
    return;
  }
  const targetIds = [...selectedRecords.value];
  openDeleteConfirm(`确定要删除 ${targetIds.length} 条记录吗？`, { kind: "records", ids: targetIds });
};

const openSettings = async () => {
  await loadSettings();
  settingsOpen.value = true;
};

const openWebdav = async () => {
  await loadSettings();
  webdavPasswordVisible.value = false;
  webdavOpen.value = true;
};

const saveSettings = async () => {
  await api.saveSettings({ ...settingsDraft });
  await api.setAutoStart(settingsDraft.autoStartEnabled);
  settingsOpen.value = false;
  syncStatus.value = await api.getSyncStatus();
};

const saveWebdavSettings = async () => {
  await api.saveSettings({ ...settingsDraft });
  await api.setAutoStart(settingsDraft.autoStartEnabled);
  webdavPasswordVisible.value = false;
  webdavOpen.value = false;
  syncStatus.value = await api.getSyncStatus();
};

const handleTestWebdav = async () => {
  const result = await api.testWebDav({ ...settingsDraft });
  alert(result.message);
  await loadSettings();
};

const handleSyncNow = async () => {
  await api.syncNow("manual");
};

const toggleTheme = () => {
  isLightTheme.value = !isLightTheme.value;
  safeStorage.setItem("appTheme", isLightTheme.value ? "light" : "dark");
};

const handleMinimize = async () => {
  if (!appWindow) {
    return;
  }
  await appWindow.minimize();
};

const handleMaximize = async () => {
  if (!appWindow) {
    return;
  }
  const isMax = await appWindow.isMaximized();
  isWindowMaximized.value = !isMax;
  if (isMax) {
    await appWindow.unmaximize();
  } else {
    await appWindow.maximize();
  }
};

const toggleMaximize = async () => {
  await handleMaximize();
};

const handleClose = async () => {
  // 默认“关闭”改为最小化到托盘：隐藏主窗口，保留后台运行（托盘可重新打开/退出）。
  if (!appWindow) {
    return;
  }
  await appWindow.hide();
};

const toggleSidebar = () => {
  isSidebarCollapsed.value = !isSidebarCollapsed.value;
  safeStorage.setItem("sidebarCollapsed", isSidebarCollapsed.value ? "1" : "0");
};

const showContextMenu = (event: MouseEvent, items: ContextMenuItem[]) => {
  contextMenu.x = event.clientX;
  contextMenu.y = event.clientY;
  contextMenu.items = items.map(item => ({
    ...item,
    action: () => {
      hideContextMenu();
      item.action();
    }
  }));
  contextMenu.visible = true;
};

const hideContextMenu = () => {
  contextMenu.visible = false;
};

const openTaskMenu = (event: MouseEvent, task: Task) => {
  showContextMenu(event, [
    { label: "编辑", action: () => openEditTask(task) },
    {
      label: task.status === "COMPLETED" ? "取消完成" : "标记完成",
      action: () => toggleTask(task),
    },
    { label: "删除", action: () => openDeleteConfirm("确定要删除此任务吗？", { kind: "task", id: task.id }), danger: true },
  ]);
};

const openCompletedMenu = (event: MouseEvent, task: Task) => {
  showContextMenu(event, [
    { label: "查看详情", action: () => openTaskDetail(task) },
    { label: "取消完成", action: () => toggleTask(task) },
    { label: "删除", action: () => openDeleteConfirm("确定要删除此任务吗？", { kind: "task", id: task.id }), danger: true },
  ]);
};

const openRecurringMenu = (event: MouseEvent, task: RecurringTask) => {
  showContextMenu(event, [
    { label: "编辑", action: () => openEditRecurring(task) },
    { label: task.isPaused ? "恢复" : "暂停", action: () => toggleRecurring(task) },
    { label: "删除", action: () => openDeleteConfirm("确定要删除此循环提醒吗？", { kind: "recurring", id: task.id }), danger: true },
  ]);
};

const openRecordMenu = (event: MouseEvent, record: ReminderRecord) => {
  showContextMenu(event, [
    { label: "查看详情", action: () => openRecordDetail(record) },
    { label: "删除", action: () => openDeleteConfirm("确定要删除该记录吗？", { kind: "record", id: record.id }), danger: true },
  ]);
};

const openDetail = (title: string, items: { label: string; value: string }[]) => {
  detailTitle.value = title;
  detailItems.value = items;
  detailOpen.value = true;
};

const openTaskDetail = (task: Task) => {
  openDetail("任务详情", [
    { label: "描述", value: task.description },
    { label: "状态", value: task.status === "COMPLETED" ? "已完成" : "待办" },
    { label: "创建时间", value: formatDateTime(task.createdAt) },
    { label: "完成时间", value: formatDateTime(task.completedAt) },
    { label: "提醒时间", value: formatDateTime(task.reminderTime) },
  ]);
};

const openRecordDetail = (record: ReminderRecord) => {
  openDetail("提醒记录详情", [
    { label: "描述", value: record.description },
    { label: "类型", value: record.type === "TASK" ? "任务" : "循环" },
    { label: "触发时间", value: formatDateTime(record.triggerTime) },
    { label: "关闭时间", value: formatDateTime(record.closeTime) },
    { label: "操作", value: formatAction(record.action) },
  ]);
};

onMounted(async () => {
  try {
    const { getVersion } = await import("@tauri-apps/api/app");
    appVersion.value = await getVersion();
    isDevMode.value = await api.isDevMode();
  } catch {
    // 浏览器直接访问 http://127.0.0.1:5173/ 时没有 Tauri API，忽略即可。
  }
  try {
    await refreshAll();
    await loadSettings();
    syncStatus.value = await api.getSyncStatus();
  } catch (error) {
    console.error("[main] 初始化数据失败", error);
  }
  if (appWindow) {
    try {
      isWindowMaximized.value = await appWindow.isMaximized();
      appWindow.onResized(async () => {
        isWindowMaximized.value = await appWindow.isMaximized();
      });
    } catch (error) {
      console.error("[main] 初始化窗口状态失败", error);
    }
  }
  window.addEventListener("click", hideContextMenu);
  try {
    await listen<SyncStatus>("sync-status", event => {
      syncStatus.value = event.payload;
    });
  } catch (error) {
    console.error("[main] 监听 sync-status 失败", error);
  }
  try {
    await listen("data-updated", async () => {
      await refreshAll();
      await loadSettings();
      syncStatus.value = await api.getSyncStatus();
    });
  } catch (error) {
    console.error("[main] 监听 data-updated 失败", error);
  }
  try {
    await listen("open-sync-settings", () => {
      openWebdav();
    });
  } catch (error) {
    console.error("[main] 监听 open-sync-settings 失败", error);
  }
});

onBeforeUnmount(() => {
  window.removeEventListener("click", hideContextMenu);
});

watch(uiScale, value => {
  const normalized = Math.min(1.2, Math.max(0.8, Number(value) || 1));
  if (normalized !== uiScale.value) {
    uiScale.value = normalized;
  }
  safeStorage.setItem("uiScale", normalized.toString());
});
</script>
