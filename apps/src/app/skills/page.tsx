"use client";

import {
  useMemo,
  useRef,
  useState,
  type ChangeEvent,
  type FormEvent,
} from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  AlertTriangle,
  Archive,
  FolderInput,
  Loader2,
  RefreshCw,
  Search,
  ShieldCheck,
  Store,
  Trash2,
  WandSparkles,
} from "lucide-react";
import { toast } from "sonner";
import { ConfirmDialog } from "@/components/modals/confirm-dialog";
import {
  PageHeader,
  PageWorkspace,
  WorkPanel,
} from "@/components/layout/page-workspace";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { CardContent } from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import { useDeferredDesktopActivation } from "@/hooks/useDeferredDesktopActivation";
import { useDesktopPageActive } from "@/hooks/useDesktopPageActive";
import { usePageTransitionReady } from "@/hooks/usePageTransitionReady";
import { useRuntimeCapabilities } from "@/hooks/useRuntimeCapabilities";
import {
  CODEX_SKILLS_QUERY_KEY,
  MAX_CODEX_SKILL_ZIP_BYTES,
  codexSkillsClient,
} from "@/lib/api/codex-skills-client";
import { getAppErrorMessage } from "@/lib/api/transport";
import { useI18n } from "@/lib/i18n/provider";
import { useAppStore } from "@/lib/store/useAppStore";
import { cn } from "@/lib/utils";
import type { CodexSkillSummary, CodexSkillsInventory } from "@/types";
import { SkillsMarketplaceDialog } from "./marketplace-dialog";

const BASE64_CHUNK_BYTES = 32 * 1024;

export function encodeArrayBufferBase64(buffer: ArrayBuffer): string {
  const bytes = new Uint8Array(buffer);
  let binary = "";
  for (let offset = 0; offset < bytes.length; offset += BASE64_CHUNK_BYTES) {
    binary += String.fromCharCode(
      ...bytes.subarray(offset, offset + BASE64_CHUNK_BYTES),
    );
  }
  return btoa(binary);
}

function formatMiB(bytes: number): string {
  return `${Math.round(bytes / (1024 * 1024))} MiB`;
}

function SkillRow({
  item,
  expanded,
  onToggleDescription,
  onDelete,
}: {
  item: CodexSkillSummary;
  expanded: boolean;
  onToggleDescription: () => void;
  onDelete: () => void;
}) {
  const { t } = useI18n();
  const description = item.description || t("暂无描述");
  const canExpand = item.description.length > 160;

  return (
    <div className="flex flex-col gap-3 border-b border-border/60 px-4 py-4 last:border-b-0 sm:flex-row sm:items-start sm:justify-between">
      <div className="min-w-0 flex-1">
        <div className="flex flex-wrap items-center gap-2">
          <span className="min-w-0 truncate text-sm font-semibold" title={item.name}>
            {item.name}
          </span>
          {item.source === "system" ? (
            <Badge variant="secondary" className="gap-1">
              <ShieldCheck className="size-3" />
              {t("系统内置 · 只读")}
            </Badge>
          ) : (
            <Badge variant="outline">{t("用户安装")}</Badge>
          )}
          {!item.valid ? (
            <Badge className="border-amber-500/25 bg-amber-500/10 text-amber-700">
              {t("配置无效")}
            </Badge>
          ) : null}
        </div>
        <p className="mt-1 truncate font-mono text-[11px] text-muted-foreground" title={item.directoryName}>
          {item.directoryName}
        </p>
        <p
          className={cn(
            "mt-2 break-words text-sm leading-6 text-muted-foreground",
            !expanded && "line-clamp-2",
          )}
          title={description}
        >
          {description}
        </p>
        {canExpand ? (
          <Button
            type="button"
            variant="link"
            size="sm"
            className="mt-1 h-auto px-0 text-xs"
            onClick={onToggleDescription}
          >
            {expanded ? t("收起描述") : t("展开描述")}
          </Button>
        ) : null}
        {item.error ? (
          <p className="mt-2 flex items-start gap-1.5 text-xs text-amber-700" title={item.error}>
            <AlertTriangle className="mt-0.5 size-3.5 shrink-0" />
            <span className="break-words">{item.error}</span>
          </p>
        ) : null}
      </div>
      <div className="flex w-full flex-wrap items-center gap-2 sm:w-auto sm:shrink-0 sm:justify-end">
        {item.deletable ? (
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="w-full gap-1.5 text-destructive hover:text-destructive sm:w-auto"
            onClick={onDelete}
          >
            <Trash2 className="size-3.5" />
            {t("删除")}
          </Button>
        ) : (
          <span className="text-xs text-muted-foreground">
            {item.source === "system" ? t("由 Codex 管理") : t("不可安全删除")}
          </span>
        )}
      </div>
    </div>
  );
}

export default function SkillsPage() {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const fileInputRef = useRef<HTMLInputElement>(null);
  const serviceConnected = useAppStore((state) => state.serviceStatus.connected);
  const { canAccessManagementRpc } = useRuntimeCapabilities();
  const isPageActive = useDesktopPageActive("/skills/");
  const isReady = useDeferredDesktopActivation(
    isPageActive && serviceConnected && canAccessManagementRpc,
  );

  const [search, setSearch] = useState("");
  const [marketplaceDialogOpen, setMarketplaceDialogOpen] = useState(false);
  const [importDialogOpen, setImportDialogOpen] = useState(false);
  const [sourcePath, setSourcePath] = useState("");
  const [pendingDelete, setPendingDelete] = useState<CodexSkillSummary | null>(
    null,
  );
  const [expandedDescriptions, setExpandedDescriptions] = useState<Set<string>>(
    () => new Set(),
  );
  const [preparingZip, setPreparingZip] = useState(false);

  const inventoryQuery = useQuery({
    queryKey: CODEX_SKILLS_QUERY_KEY,
    queryFn: () => codexSkillsClient.list(),
    enabled: isReady,
    staleTime: 15_000,
    retry: 1,
  });
  usePageTransitionReady(
    "/skills/",
    !serviceConnected || !canAccessManagementRpc || !inventoryQuery.isLoading,
  );

  const storeInventory = (inventory: CodexSkillsInventory) => {
    queryClient.setQueryData(CODEX_SKILLS_QUERY_KEY, inventory);
  };

  const installMutation = useMutation({
    mutationFn: codexSkillsClient.installZip,
    onSuccess: (inventory) => {
      storeInventory(inventory);
      toast.success(t("Skill ZIP 已安装"));
    },
    onError: (error) => {
      toast.error(`${t("安装失败")}: ${getAppErrorMessage(error)}`);
    },
  });

  const importMutation = useMutation({
    mutationFn: codexSkillsClient.importDirectory,
    onSuccess: (inventory) => {
      storeInventory(inventory);
      setImportDialogOpen(false);
      setSourcePath("");
      toast.success(t("Skill 目录已导入"));
    },
    onError: (error) => {
      toast.error(`${t("导入失败")}: ${getAppErrorMessage(error)}`);
    },
  });

  const deleteMutation = useMutation({
    mutationFn: codexSkillsClient.delete,
    onSuccess: (inventory) => {
      storeInventory(inventory);
      setPendingDelete(null);
      toast.success(t("Skill 已删除"));
    },
    onError: (error) => {
      toast.error(`${t("删除失败")}: ${getAppErrorMessage(error)}`);
    },
  });

  const inventory = inventoryQuery.data;
  const filteredItems = useMemo(() => {
    const needle = search.trim().toLocaleLowerCase();
    if (!needle) return inventory?.items ?? [];
    return (inventory?.items ?? []).filter((item) =>
      [item.name, item.description, item.directoryName].some((value) =>
        value.toLocaleLowerCase().includes(needle),
      ),
    );
  }, [inventory?.items, search]);
  const systemCount = inventory?.items.filter((item) => item.source === "system").length ?? 0;
  const userCount = (inventory?.items.length ?? 0) - systemCount;
  const anyMutationPending =
    preparingZip ||
    installMutation.isPending ||
    importMutation.isPending ||
    deleteMutation.isPending;

  const chooseZip = () => {
    const input = fileInputRef.current;
    if (!input || anyMutationPending) return;
    input.value = "";
    input.click();
  };

  const handleZipChange = async (event: ChangeEvent<HTMLInputElement>) => {
    const input = event.currentTarget;
    const file = input.files?.[0];
    if (!file) {
      input.value = "";
      return;
    }
    setPreparingZip(true);
    try {
      if (!file.name.toLocaleLowerCase().endsWith(".zip")) {
        throw new Error(t("请选择 ZIP 文件"));
      }
      if (file.size > MAX_CODEX_SKILL_ZIP_BYTES) {
        throw new Error(
          t("ZIP 文件不能超过 {size}", {
            size: formatMiB(MAX_CODEX_SKILL_ZIP_BYTES),
          }),
        );
      }
      const archiveBase64 = encodeArrayBufferBase64(await file.arrayBuffer());
      try {
        await installMutation.mutateAsync({
          fileName: file.name,
          archiveBase64,
        });
      } catch {
        // The mutation displays the normalized backend error.
      }
    } catch (error) {
      toast.error(getAppErrorMessage(error));
    } finally {
      setPreparingZip(false);
      input.value = "";
    }
  };

  const submitImport = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const normalizedPath = sourcePath.trim();
    if (!normalizedPath) {
      toast.error(t("请输入服务主机上的绝对路径"));
      return;
    }
    try {
      await importMutation.mutateAsync({ sourcePath: normalizedPath });
    } catch {
      // The mutation displays the normalized backend error.
    }
  };

  const toggleDescription = (directoryName: string) => {
    setExpandedDescriptions((current) => {
      const next = new Set(current);
      if (next.has(directoryName)) next.delete(directoryName);
      else next.add(directoryName);
      return next;
    });
  };

  return (
    <PageWorkspace>
      <input
        ref={fileInputRef}
        type="file"
        accept=".zip,application/zip"
        className="hidden"
        onChange={(event) => void handleZipChange(event)}
      />

      <PageHeader
        eyebrow="CODEX"
        title={t("Skills 管理")}
        description={t("扫描并管理 codexmanager-service 主机上的 Codex Skills。")}
        meta={
          <>
            <Badge variant="outline">{t("用户安装")} {userCount}</Badge>
            <Badge variant="secondary">{t("系统只读")} {systemCount}</Badge>
          </>
        }
        actions={
          <>
            <Button
              type="button"
              variant="outline"
              className="w-full gap-2 sm:w-auto"
              disabled={!isReady}
              onClick={() => setMarketplaceDialogOpen(true)}
            >
              <Store className="size-4" />
              {t("Skills 市场")}
            </Button>
            <Button
              type="button"
              variant="outline"
              className="w-full gap-2 sm:w-auto"
              disabled={!isReady || anyMutationPending}
              onClick={() => setImportDialogOpen(true)}
            >
              <FolderInput className="size-4" />
              {t("导入已有目录")}
            </Button>
            <Button
              type="button"
              className="w-full gap-2 sm:w-auto"
              disabled={!isReady || anyMutationPending}
              onClick={chooseZip}
            >
              {preparingZip || installMutation.isPending ? (
                <Loader2 className="size-4 animate-spin" />
              ) : (
                <Archive className="size-4" />
              )}
              {t("安装 ZIP")}
            </Button>
            <Button
              type="button"
              variant="ghost"
              size="icon"
              disabled={!isReady || inventoryQuery.isFetching}
              title={t("刷新")}
              aria-label={t("刷新")}
              onClick={() => void inventoryQuery.refetch()}
            >
              <RefreshCw
                className={cn("size-4", inventoryQuery.isFetching && "animate-spin")}
              />
            </Button>
          </>
        }
      />

      <WorkPanel>
        <CardContent className="space-y-3 px-4 py-4">
          <div className="flex items-start gap-3">
            <ShieldCheck className="mt-0.5 size-4 shrink-0 text-primary" />
            <div className="min-w-0 text-xs leading-5 text-muted-foreground">
              <p className="font-medium text-foreground">{t("服务主机文件系统")}</p>
              <p>{t("这里的安装、导入和删除都发生在 codexmanager-service 所在主机，不是浏览器所在设备。")}</p>
              {inventory?.skillsRoot ? (
                <p className="mt-1 break-all font-mono" title={inventory.skillsRoot}>
                  {inventory.skillsRoot}
                </p>
              ) : null}
            </div>
          </div>
          {inventory?.warnings.map((warning) => (
            <p
              key={warning}
              className="flex items-start gap-2 rounded-md border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-xs text-amber-800"
            >
              <AlertTriangle className="mt-0.5 size-3.5 shrink-0" />
              <span className="break-words">{warning}</span>
            </p>
          ))}
        </CardContent>
      </WorkPanel>

      <WorkPanel>
        <CardContent className="border-b border-border/60 px-4 py-3">
          <div className="relative max-w-xl">
            <Search className="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
            <Input
              value={search}
              onChange={(event) => setSearch(event.target.value)}
              placeholder={t("搜索名称、描述或目录")}
              aria-label={t("搜索名称、描述或目录")}
              className="pl-9"
            />
          </div>
        </CardContent>

        {!serviceConnected || !canAccessManagementRpc ? (
          <Empty className="min-h-64">
            <EmptyMedia variant="icon">
              <AlertTriangle />
            </EmptyMedia>
            <EmptyHeader>
              <EmptyTitle>{t("当前无法读取 Skills")}</EmptyTitle>
              <EmptyDescription>
                {t("请确认管理 RPC 可用并已连接 codexmanager-service。")}
              </EmptyDescription>
            </EmptyHeader>
          </Empty>
        ) : inventoryQuery.isLoading ? (
          <div className="space-y-3 p-4">
            {Array.from({ length: 3 }).map((_, index) => (
              <div key={index} className="space-y-2 rounded-md border p-4">
                <Skeleton className="h-4 w-40" />
                <Skeleton className="h-3 w-64 max-w-full" />
                <Skeleton className="h-3 w-full max-w-2xl" />
              </div>
            ))}
          </div>
        ) : inventoryQuery.error ? (
          <Empty className="min-h-64">
            <EmptyMedia variant="icon">
              <AlertTriangle />
            </EmptyMedia>
            <EmptyHeader>
              <EmptyTitle>{t("Skills 加载失败")}</EmptyTitle>
              <EmptyDescription>
                {getAppErrorMessage(inventoryQuery.error)}
              </EmptyDescription>
            </EmptyHeader>
            <Button variant="outline" onClick={() => void inventoryQuery.refetch()}>
              {t("重试")}
            </Button>
          </Empty>
        ) : filteredItems.length === 0 ? (
          <Empty className="min-h-64">
            <EmptyMedia variant="icon">
              <WandSparkles />
            </EmptyMedia>
            <EmptyHeader>
              <EmptyTitle>
                {search ? t("没有匹配的 Skill") : t("尚未发现 Skill")}
              </EmptyTitle>
              <EmptyDescription>
                {search
                  ? t("请调整搜索条件。")
                  : t("可以安装一个 ZIP，或从服务主机导入已有目录。")}
              </EmptyDescription>
            </EmptyHeader>
          </Empty>
        ) : (
          <div>
            {filteredItems.map((item) => (
              <SkillRow
                key={`${item.source}:${item.directoryName}`}
                item={item}
                expanded={expandedDescriptions.has(item.directoryName)}
                onToggleDescription={() => toggleDescription(item.directoryName)}
                onDelete={() => setPendingDelete(item)}
              />
            ))}
          </div>
        )}
      </WorkPanel>

      <SkillsMarketplaceDialog
        open={marketplaceDialogOpen}
        onOpenChange={setMarketplaceDialogOpen}
        enabled={isReady}
      />

      <Dialog open={importDialogOpen} onOpenChange={setImportDialogOpen}>
        <DialogContent showCloseButton={!importMutation.isPending}>
          <form onSubmit={(event) => void submitImport(event)}>
            <DialogHeader>
              <DialogTitle>{t("导入已有 Skill 目录")}</DialogTitle>
              <DialogDescription>
                {t("输入 codexmanager-service 主机上的绝对路径。目录根部必须包含 SKILL.md。")}
              </DialogDescription>
            </DialogHeader>
            <div className="py-5">
              <label htmlFor="skill-source-path" className="mb-2 block text-sm font-medium">
                {t("服务主机绝对路径")}
              </label>
              <Input
                id="skill-source-path"
                value={sourcePath}
                onChange={(event) => setSourcePath(event.target.value)}
                placeholder={t("例如 /opt/codex-skills/my-skill")}
                autoComplete="off"
                autoFocus
                disabled={importMutation.isPending}
              />
            </div>
            <DialogFooter>
              <Button
                type="button"
                variant="outline"
                disabled={importMutation.isPending}
                onClick={() => setImportDialogOpen(false)}
              >
                {t("取消")}
              </Button>
              <Button type="submit" disabled={importMutation.isPending || !sourcePath.trim()}>
                {importMutation.isPending ? (
                  <Loader2 className="size-4 animate-spin" />
                ) : (
                  <FolderInput className="size-4" />
                )}
                {t("导入")}
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>

      <ConfirmDialog
        open={Boolean(pendingDelete)}
        onOpenChange={(open) => {
          if (!open) setPendingDelete(null);
        }}
        title={t("删除 Skill")}
        description={t("将从服务主机永久删除“{name}”目录。此操作不可撤销。", {
          name: pendingDelete?.name || "",
        })}
        confirmText={t("确认删除")}
        confirmVariant="destructive"
        onConfirm={async () => {
          if (!pendingDelete) return false;
          try {
            await deleteMutation.mutateAsync({
              directoryName: pendingDelete.directoryName,
            });
            return true;
          } catch {
            return false;
          }
        }}
      />
    </PageWorkspace>
  );
}
