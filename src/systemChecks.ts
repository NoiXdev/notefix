import { api } from './api';
import i18n from './i18n';
import type { AppSettings } from './hooks/useSettings';

export type CheckStatus = 'ok' | 'warn' | 'error';
export interface SystemCheck {
  key: string;
  label: string;
  status: CheckStatus;
  detail: string;
  action?: 'changeLocation';
}

export async function runSystemChecks(_settings: AppSettings): Promise<SystemCheck[]> {
  const paths = await api.checkPaths();
  const autostartOn = await api.autostart.isEnabled().catch(() => false);
  const windowOk = await api.windowProbe();
  return [
    {
      key: 'db', label: i18n.t('diagnostics.checks.db'),
      status: paths.dbWritable ? 'ok' : 'error',
      detail: paths.dbPath,
      action: paths.dbWritable ? undefined : 'changeLocation',
    },
    {
      key: 'images', label: i18n.t('diagnostics.checks.images'),
      status: paths.imagesWritable ? 'ok' : 'error',
      detail: paths.imagesPath,
      action: paths.imagesWritable ? undefined : 'changeLocation',
    },
    {
      key: 'autostart', label: i18n.t('diagnostics.checks.autostart'),
      status: 'ok',
      detail: autostartOn ? i18n.t('diagnostics.autostartActive') : i18n.t('diagnostics.autostartInactive'),
    },
    {
      key: 'window', label: i18n.t('diagnostics.checks.window'),
      status: windowOk ? 'ok' : 'error',
      detail: windowOk ? i18n.t('diagnostics.windowAvailable') : i18n.t('diagnostics.windowMissing'),
    },
  ];
}
