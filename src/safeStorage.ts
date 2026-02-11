const hasWindow = typeof window !== "undefined";

export const safeStorage = {
  getItem(key: string): string | null {
    if (!hasWindow) {
      return null;
    }
    try {
      return window.localStorage.getItem(key);
    } catch (error) {
      console.warn(`[storage] 读取 ${key} 失败`, error);
      return null;
    }
  },
  setItem(key: string, value: string): void {
    if (!hasWindow) {
      return;
    }
    try {
      window.localStorage.setItem(key, value);
    } catch (error) {
      console.warn(`[storage] 写入 ${key} 失败`, error);
    }
  }
};
