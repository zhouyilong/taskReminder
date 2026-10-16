// 标签与项目管理弹窗（v2.2.3）的开关与最近一次改名（模块级单例）：
// 弹窗挂在 App.vue，待办视图据 `lastChange` 让筛选跟随新名称。
import { ref } from "vue";

export interface LabelChange {
  kind: "tag" | "project";
  from: string;
  /** 新名称；移除标签、移出项目时为空字符串。 */
  to: string;
}

const labelManagerOpen = ref(false);
const lastChange = ref<LabelChange | null>(null);

export const useLabelManager = () => ({
  labelManagerOpen,
  lastChange,
  openLabelManager: () => {
    labelManagerOpen.value = true;
  }
});
