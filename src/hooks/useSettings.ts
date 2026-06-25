import { useState, useEffect, useCallback } from 'react';
import { api } from '../api';
import type { DateFormat } from '../dates';
import type { LangSetting } from '../i18n/lang';
import { parseShortcuts } from '../shortcuts';

export type { LangSetting };

export type CloseAction = 'ask' | 'minimize' | 'quit';
export type PinnedScope = 'perFolder' | 'global';
export type FolderColorStyle = 'icon' | 'bar' | 'row';
export type StartView = 'dashboard' | 'lastNote';
export type SidebarMode = 'switcher' | 'combined';

export interface DashboardWidget { key: string; x: number; y: number; w: number; h: number; }

export interface AppSettings {
  startMinimized: boolean;
  dateFormat: DateFormat;
  pinnedScope: PinnedScope;
  folderColorStyle: FolderColorStyle;
  revisionLimit: number;
  autosaveDelay: number;
  startView: StartView;
  sidebarMode: SidebarMode;
  dashboardLayout: DashboardWidget[];
  compactTree: boolean;
  treeProgress: boolean;
  trashEnabled: boolean;
  trashRetentionDays: number;
  closeAction: CloseAction;
  shortcuts: Record<string, string>;
  language: LangSetting;
  linkPreviewEnabled: boolean;
  linkPreviewMode: 'url' | 'inline' | 'card';
  copyFormat: import('../copyFormat').CopyFormat;
  mcpEnabled: boolean;
  mcpBind: 'internal' | 'external';
  mcpPort: number;
  mcpAuthRequired: boolean;
  mcpToken: string;
  mcpAllowWrite: boolean;
}

const DEFAULT_LAYOUT: DashboardWidget[] = [
  { key: 'recent', x: 0, y: 0, w: 6, h: 4 },
  { key: 'due', x: 6, y: 0, w: 6, h: 4 },
  { key: 'stats', x: 0, y: 4, w: 4, h: 3 },
  { key: 'pinned', x: 4, y: 4, w: 4, h: 3 },
];

const DEFAULTS: AppSettings = {
  startMinimized: false,
  dateFormat: 'auto',
  pinnedScope: 'perFolder',
  folderColorStyle: 'icon',
  revisionLimit: 50,
  autosaveDelay: 400,
  startView: 'lastNote',
  sidebarMode: 'switcher',
  dashboardLayout: DEFAULT_LAYOUT,
  compactTree: false,
  treeProgress: true,
  trashEnabled: true,
  trashRetentionDays: 30,
  closeAction: 'ask',
  shortcuts: {},
  language: 'system',
  linkPreviewEnabled: true,
  linkPreviewMode: 'card',
  copyFormat: 'md',
  mcpEnabled: false,
  mcpBind: 'internal',
  mcpPort: 4357,
  mcpAuthRequired: true,
  mcpToken: '',
  mcpAllowWrite: false,
};

function isGridWidget(x: unknown): x is DashboardWidget {
  const o = x as Record<string, unknown> | null;
  return !!o && typeof o.key === 'string' && typeof o.x === 'number' && typeof o.y === 'number' && typeof o.w === 'number' && typeof o.h === 'number';
}

function stack(items: { key: string; w: number }[]): DashboardWidget[] {
  let y = 0;
  return items.map(it => {
    const node = { key: it.key, x: 0, y, w: it.w, h: 4 };
    y += 4;
    return node;
  });
}

export function parseLayout(raw: string | undefined): DashboardWidget[] {
  if (!raw) return DEFAULT_LAYOUT;
  try {
    const v = JSON.parse(raw);
    if (!Array.isArray(v)) return DEFAULT_LAYOUT;
    if (v.every(isGridWidget)) return v as DashboardWidget[]; // neu (inkl. leeres [])
    if (v.every((x: unknown) => typeof x === 'string')) return stack((v as string[]).map(k => ({ key: k, w: 6 })));
    if (v.every((x: unknown) => { const o = x as Record<string, unknown> | null; return !!o && typeof o.key === 'string' && (o.w === 1 || o.w === 2); })) {
      return stack((v as { key: string; w: number }[]).map(o => ({ key: o.key, w: o.w === 2 ? 12 : 6 })));
    }
    return DEFAULT_LAYOUT;
  } catch {
    return DEFAULT_LAYOUT;
  }
}

export function useSettings() {
  const [settings, setSettings] = useState<AppSettings>(DEFAULTS);
  const [loaded, setLoaded] = useState(false);

  const reload = useCallback(async () => {
    const raw = await api.settings.load();
    setSettings({
      startMinimized: raw.startMinimized === 'true',
      dateFormat: (['de', 'iso', 'us'].includes(raw.dateFormat) ? raw.dateFormat : 'auto') as DateFormat,
      pinnedScope: raw.pinnedScope === 'global' ? 'global' : 'perFolder',
      folderColorStyle: (['bar', 'row'].includes(raw.folderColorStyle) ? raw.folderColorStyle : 'icon') as FolderColorStyle,
      revisionLimit: Number(raw.revisionLimit) > 0 ? Number(raw.revisionLimit) : 50,
      autosaveDelay: Number(raw.autosaveDelay) >= 100 ? Number(raw.autosaveDelay) : 400,
      startView: raw.startView === 'dashboard' ? 'dashboard' : 'lastNote',
      sidebarMode: raw.sidebarMode === 'combined' ? 'combined' : 'switcher',
      dashboardLayout: parseLayout(raw.dashboardLayout),
      compactTree: raw.compactTree === 'true',
      treeProgress: raw.treeProgress !== 'false',
      trashEnabled: raw.trashEnabled !== 'false',
      trashRetentionDays: Number(raw.trashRetentionDays) > 0 ? Number(raw.trashRetentionDays) : 30,
      closeAction: (['minimize', 'quit'].includes(raw.closeAction) ? raw.closeAction : 'ask') as CloseAction,
      shortcuts: parseShortcuts(raw.shortcuts),
      language: (['en', 'de', 'fr'].includes(raw.language) ? raw.language : 'system') as LangSetting,
      linkPreviewEnabled: raw.linkPreviewEnabled !== 'false',
      linkPreviewMode: (['url', 'inline', 'card'].includes(raw.linkPreviewMode) ? raw.linkPreviewMode : 'card') as 'url' | 'inline' | 'card',
      copyFormat: (['richtext', 'html', 'md', 'text'].includes(raw.copyFormat) ? raw.copyFormat : 'md') as import('../copyFormat').CopyFormat,
      mcpEnabled: raw.mcpEnabled === 'true',
      mcpBind: raw.mcpBind === 'external' ? 'external' : 'internal',
      mcpPort: Number(raw.mcpPort) > 0 ? Number(raw.mcpPort) : 4357,
      mcpAuthRequired: raw.mcpAuthRequired !== 'false',
      mcpToken: typeof raw.mcpToken === 'string' ? raw.mcpToken : '',
      mcpAllowWrite: raw.mcpAllowWrite === 'true',
    });
    setLoaded(true);
  }, []);

  useEffect(() => {
    void reload();
  }, [reload]);

  const setSetting = useCallback(
    async <K extends keyof AppSettings>(key: K, value: AppSettings[K]) => {
      setSettings(prev => ({ ...prev, [key]: value }));
      const serialized = value !== null && typeof value === 'object' ? JSON.stringify(value) : String(value);
      await api.settings.set(key, serialized);
    },
    [],
  );

  return { settings, setSetting, loaded, reload };
}
