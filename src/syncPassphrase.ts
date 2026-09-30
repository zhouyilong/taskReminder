// 同步密码的前端校验（与后端 `sync_crypto::MIN_PASSPHRASE_CHARS`、`validate_passphrase_change` 一致）。

export const MIN_PASSPHRASE_CHARS = 8;

/** 按字符（而非 UTF-16 码元）计数，与后端 `chars().count()` 一致。 */
export const passphraseLength = (value: string) => [...value].length;

/** 更换同步密码表单的错误提示；为空表示可以提交。未填完的字段不报错，只返回 `incomplete`。 */
export const passphraseChangeError = (
  current: string,
  next: string,
  confirm: string,
): { message: string; incomplete: boolean } => {
  if (!current || !next || !confirm) {
    return { message: "", incomplete: true };
  }
  if (passphraseLength(next) < MIN_PASSPHRASE_CHARS) {
    return { message: `新密码至少需要 ${MIN_PASSPHRASE_CHARS} 个字符`, incomplete: false };
  }
  if (next !== confirm) {
    return { message: "两次输入的新密码不一致", incomplete: false };
  }
  if (next === current) {
    return { message: "新密码与当前密码相同", incomplete: false };
  }
  return { message: "", incomplete: false };
};
