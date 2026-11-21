"use client";

import { useEffect, useMemo, useState } from "react";
import Image from "next/image";
import qrcode from "qrcode-generator";
import { ShieldCheck } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useI18n } from "@/lib/i18n/provider";
import { getAppErrorMessage } from "@/lib/api/transport";
import { twoFactorClient, type TwoFactorSetup, type TwoFactorStatus } from "@/lib/api/two-factor-client";

export function TwoFactorPanel({ open, mode, disabled }: { open: boolean; mode: string; disabled: boolean }) {
  const { t } = useI18n();
  const [status, setStatus] = useState<TwoFactorStatus | null>(null);
  const [setup, setSetup] = useState<TwoFactorSetup | null>(null);
  const [password, setPassword] = useState("");
  const [code, setCode] = useState("");
  const [recoveryCodes, setRecoveryCodes] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    if (!open || disabled) return;
    let active = true;
    void twoFactorClient.status().then((value) => { if (active) setStatus(value); }).catch((err) => { if (active) setError(getAppErrorMessage(err)); });
    return () => { active = false; };
  }, [open, disabled]);

  const qr = useMemo(() => {
    if (!setup) return "";
    const image = qrcode(0, "M");
    image.addData(setup.otpauthUri); image.make();
    return image.createDataURL(5, 4);
  }, [setup]);

  const act = async (operation: () => Promise<void>) => {
    setBusy(true); setError("");
    try { await operation(); } catch (err) { setError(getAppErrorMessage(err)); } finally { setBusy(false); }
  };

  const ready = !disabled && mode !== "none" && status !== null && !busy;
  return (
    <Card size="sm" data-testid="two-factor-panel">
      <CardContent className="grid gap-3">
        <div className="flex items-center gap-2 text-sm font-medium"><ShieldCheck className="h-4 w-4" />{t("两步验证（2FA）")}</div>
        <p className="text-xs text-muted-foreground">
          {status?.enabled ? t("已启用。管理登录需要密码和动态验证码。") : t("可选启用验证器，为现有管理登录增加保护。")}
        </p>
        {mode === "none" ? <p className="text-xs text-muted-foreground">{t("请先设置并保存登录密码，再绑定验证器。")}</p> : null}
        {error ? <p role="alert" className="text-sm text-destructive">{error}</p> : null}
        {recoveryCodes.length ? (
          <div className="grid gap-3">
            <p className="text-sm font-medium">{t("绑定成功，请保存恢复码")}</p>
            <p className="text-xs text-muted-foreground">{t("恢复码仅显示一次，每条只能使用一次。遗失验证器时可代替动态验证码登录。")}</p>
            <pre className="select-all overflow-x-auto rounded-md bg-muted p-3 text-xs" data-testid="two-factor-recovery-codes">{recoveryCodes.join("\n")}</pre>
            <Button type="button" variant="outline" onClick={async () => {
              try { await navigator.clipboard.writeText(recoveryCodes.join("\n")); toast.success(t("已复制恢复码")); }
              catch { toast.error(t("复制失败，请手动选择并保存恢复码。")); }
            }}>{t("复制恢复码")}</Button>
          </div>
        ) : (
          <>
            <div className="grid gap-2">
              <Label htmlFor="two-factor-password">{t("当前登录密码")}</Label>
              <Input id="two-factor-password" type="password" autoComplete="current-password" value={password} disabled={!ready} onChange={(e) => setPassword(e.target.value)} />
            </div>
            {setup ? (
              <div className="grid justify-items-start gap-2">
                <p className="text-xs text-muted-foreground">{t("使用验证器扫描二维码，或手动输入下面的密钥。")}</p>
                <Image src={qr} width={180} height={180} unoptimized alt={t("验证器绑定二维码")} className="rounded-md bg-white p-2" />
                <Input aria-label={t("验证器密钥")} readOnly value={setup.secret} className="font-mono" />
              </div>
            ) : null}
            {setup || status?.enabled ? (
              <div className="grid gap-2">
                <Label htmlFor="two-factor-code">{setup ? t("6 位动态验证码") : t("动态验证码或恢复码")}</Label>
                <Input id="two-factor-code" autoComplete="one-time-code" inputMode={setup ? "numeric" : "text"} maxLength={setup ? 6 : 64} value={code} disabled={!ready} onChange={(e) => setCode(e.target.value)} />
              </div>
            ) : null}
            <div className="flex flex-wrap gap-2">
              {setup ? (
                <>
                  <Button type="button" disabled={!ready || !password || code.length !== 6} onClick={() => void act(async () => {
                    const result = await twoFactorClient.confirm(password, setup.setupToken, code);
                    setSetup(null); setPassword(""); setCode(""); setRecoveryCodes(result.recoveryCodes);
                    setStatus({ enabled: true, recoveryCodesRemaining: result.recoveryCodes.length });
                  })}>{t("验证并启用 2FA")}</Button>
                  <Button type="button" variant="outline" disabled={busy} onClick={() => { setSetup(null); setCode(""); }}>{t("取消绑定")}</Button>
                </>
              ) : status?.enabled ? (
                <Button type="button" variant="outline" disabled={!ready || !password || !code} onClick={() => void act(async () => {
                  await twoFactorClient.disable(password, code); setStatus({ enabled: false, recoveryCodesRemaining: 0 }); setPassword(""); setCode(""); toast.success(t("已关闭两步验证"));
                })}>{t("验证并关闭 2FA")}</Button>
              ) : (
                <Button type="button" variant="outline" disabled={!ready || !password} onClick={() => void act(async () => { setSetup(await twoFactorClient.setup(password)); setCode(""); })}>{t("绑定验证器")}</Button>
              )}
            </div>
            {status?.enabled ? <p className="text-xs text-muted-foreground">{t("剩余恢复码：{count}", { count: status.recoveryCodesRemaining })}</p> : null}
          </>
        )}
      </CardContent>
    </Card>
  );
}
