import { describe, expect, it } from "vitest";
import { passphraseChangeError, passphraseLength } from "./syncPassphrase";

describe("passphraseLength", () => {
  it("按字符计数", () => {
    expect(passphraseLength("八个字的中文密码")).toBe(8);
    expect(passphraseLength("😀😀")).toBe(2);
  });
});

describe("passphraseChangeError", () => {
  it("未填完时不提示错误", () => {
    expect(passphraseChangeError("", "", "")).toEqual({ message: "", incomplete: true });
    expect(passphraseChangeError("old password", "new password", "")).toEqual({ message: "", incomplete: true });
  });

  it("校验长度、两次输入一致与不同于当前密码", () => {
    expect(passphraseChangeError("old password", "short", "short").message).toContain("至少需要 8 个字符");
    expect(passphraseChangeError("old password", "new password", "new passw0rd").message).toBe("两次输入的新密码不一致");
    expect(passphraseChangeError("same password", "same password", "same password").message).toBe("新密码与当前密码相同");
  });

  it("有效时可以提交", () => {
    expect(passphraseChangeError("old password", "new password", "new password")).toEqual({
      message: "",
      incomplete: false,
    });
  });
});
