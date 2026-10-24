/** Risk of a classic OAuth scope held by a GitHub CLI token. */
export type ScopeRisk = "high" | "elevated" | "normal";

type ScopeInfo = { description: string; risk: ScopeRisk };

const SCOPES: Record<string, ScopeInfo> = {
  repo: { description: "读写全部公开和私有仓库（代码、Issue、PR、设置）", risk: "elevated" },
  public_repo: { description: "读写公开仓库", risk: "elevated" },
  "repo:status": { description: "读写提交状态", risk: "normal" },
  repo_deployment: { description: "读写部署状态", risk: "normal" },
  "repo:invite": { description: "接受或拒绝仓库协作邀请", risk: "normal" },
  security_events: { description: "读写代码扫描等安全事件", risk: "elevated" },
  workflow: { description: "修改 GitHub Actions 工作流文件", risk: "elevated" },
  delete_repo: { description: "删除仓库", risk: "high" },
  gist: { description: "创建和修改 Gist", risk: "normal" },
  notifications: { description: "读取通知", risk: "normal" },
  "read:org": { description: "读取组织与团队成员信息", risk: "normal" },
  "write:org": { description: "管理组织与团队成员", risk: "elevated" },
  "admin:org": { description: "完全管理组织（成员、团队、设置）", risk: "high" },
  "read:public_key": { description: "读取 SSH 公钥", risk: "normal" },
  "write:public_key": { description: "添加 SSH 公钥", risk: "elevated" },
  "admin:public_key": { description: "完全管理账号 SSH 公钥", risk: "high" },
  "admin:repo_hook": { description: "完全管理仓库 Webhook", risk: "high" },
  "write:repo_hook": { description: "修改仓库 Webhook", risk: "elevated" },
  "read:repo_hook": { description: "读取仓库 Webhook", risk: "normal" },
  "admin:org_hook": { description: "完全管理组织 Webhook", risk: "high" },
  "read:gpg_key": { description: "读取 GPG 密钥", risk: "normal" },
  "write:gpg_key": { description: "添加 GPG 密钥", risk: "elevated" },
  "admin:gpg_key": { description: "完全管理 GPG 密钥", risk: "high" },
  "admin:ssh_signing_key": { description: "完全管理 SSH 签名密钥", risk: "high" },
  user: { description: "读写个人资料", risk: "elevated" },
  "read:user": { description: "读取个人资料", risk: "normal" },
  "user:email": { description: "读取邮箱地址", risk: "normal" },
  "user:follow": { description: "关注或取消关注用户", risk: "normal" },
  project: { description: "读写 Projects", risk: "normal" },
  "read:project": { description: "读取 Projects", risk: "normal" },
  "read:packages": { description: "下载 Packages", risk: "normal" },
  "write:packages": { description: "发布 Packages", risk: "elevated" },
  "delete:packages": { description: "删除 Packages", risk: "high" },
  codespace: { description: "管理 Codespaces", risk: "elevated" },
  copilot: { description: "管理 Copilot 设置", risk: "normal" },
  "read:audit_log": { description: "读取审计日志", risk: "normal" },
  audit_log: { description: "读取审计日志", risk: "normal" },
  "admin:enterprise": { description: "完全管理企业", risk: "high" },
  site_admin: { description: "GitHub Enterprise Server 站点管理员", risk: "high" },
};

export function scopeInfo(scope: string): ScopeInfo {
  return SCOPES[scope] ?? { description: "未收录的权限", risk: "normal" };
}

export function highRiskScopes(scopes: string[]): string[] {
  return scopes.filter((scope) => scopeInfo(scope).risk === "high");
}

const RISK_ORDER: Record<ScopeRisk, number> = { high: 0, elevated: 1, normal: 2 };

export function sortScopes(scopes: string[]): string[] {
  return [...scopes].sort(
    (left, right) =>
      RISK_ORDER[scopeInfo(left).risk] - RISK_ORDER[scopeInfo(right).risk] ||
      left.localeCompare(right),
  );
}
