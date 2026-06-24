import { api } from './api';
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
      key: 'db', label: 'DB-Ordner schreibbar',
      status: paths.dbWritable ? 'ok' : 'error',
      detail: paths.dbPath,
      action: paths.dbWritable ? undefined : 'changeLocation',
    },
    {
      key: 'images', label: 'Bilder-Ordner schreibbar',
      status: paths.imagesWritable ? 'ok' : 'error',
      detail: paths.imagesPath,
      action: paths.imagesWritable ? undefined : 'changeLocation',
    },
    {
      key: 'autostart', label: 'Autostart (Login-Item)',
      status: 'ok',
      detail: autostartOn ? 'aktiv' : 'inaktiv',
    },
    {
      key: 'window', label: 'Fenster-Steuerung (Verschieben/Größe/Schließen)',
      status: windowOk ? 'ok' : 'error',
      detail: windowOk ? 'verfügbar' : 'Fenster-Capabilities fehlen — Build/Config',
    },
  ];
}
