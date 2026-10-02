import { useTranslation } from 'react-i18next';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import type { IconDefinition } from '@fortawesome/fontawesome-svg-core';
import { faThumbtack, faBoxArchive, faRightLong, faTrash, faFileExport, faFolder, faNoteSticky, faLock, faLockOpen, faEyeSlash, faEye, faPrint } from '@fortawesome/free-solid-svg-icons';
import type { NoteMeta, Folder } from '../types';
import ContextMenu, { type ContextMenuItem } from './ContextMenu';
import ConfirmDialog from './ConfirmDialog';
import { NOTE_COLORS } from '../colors';
import { isMobilePlatform } from '../platform';

const fa = (icon: IconDefinition) => <FontAwesomeIcon icon={icon} />;

/** The actions a note's context menu can offer; an absent handler hides its item. */
export interface NoteMenuActions {
  folders: Folder[];
  vaultUnlocked?: boolean;
  onCreateBeside?: (note: NoteMeta, mode: 'before' | 'after') => void;
  onTogglePin?: (id: string, pinned: boolean) => void;
  onArchive?: (id: string, archived: boolean) => void;
  onSetColor?: (id: string, color: string) => void;
  onMoveNote?: (id: string, folderId: string | null) => void;
  /** Ask to delete — the caller owns the confirmation (`DeleteNoteDialog`),
   *  since the menu itself unmounts on click. */
  onRequestDelete: (id: string) => void;
  onExportNote: (note: NoteMeta) => void;
  /** Print a note. Desktop only — hidden on mobile, where WebViews can't print. */
  onPrintNote?: (note: NoteMeta) => void;
  onProtectNote?: (id: string, next: boolean) => void;
  onSetNoteMcpHidden?: (id: string, next: boolean) => void;
}

interface Props extends NoteMenuActions {
  x: number;
  y: number;
  note: NoteMeta;
  onClose: () => void;
}

/** The note context menu — shared by the note list (right-click) and the
 *  mobile editor header (⋯ button), so both always offer the same actions. */
export default function NoteContextMenu({ x, y, note, onClose, folders, vaultUnlocked, onCreateBeside, onTogglePin, onArchive, onSetColor, onMoveNote, onRequestDelete, onExportNote, onPrintNote, onProtectNote, onSetNoteMcpHidden }: Props) {
  const { t } = useTranslation();

  // Move-to submenu: all folders indented by depth + root.
  const moveSubmenu = (): ContextMenuItem[] => {
    const byParent = (pid: string | null): Folder[] => folders.filter(f => (f.parentId ?? null) === pid).sort((a, b) => a.position - b.position);
    const items: ContextMenuItem[] = [{ label: t('noteList.moveRoot'), icon: fa(faFolder), onClick: () => onMoveNote?.(note.id, null) }];
    const walk = (pid: string | null, depth: number) => {
      for (const f of byParent(pid)) {
        items.push({ label: `${'  '.repeat(depth)}${f.name}`, icon: fa(faFolder), onClick: () => onMoveNote?.(note.id, f.id) });
        walk(f.id, depth + 1);
      }
    };
    walk(null, 0);
    return items;
  };

  return (
    <ContextMenu
      x={x} y={y}
      swatches={onSetColor ? { colors: NOTE_COLORS, current: note.color, onPick: c => onSetColor(note.id, c) } : undefined}
      items={[
        ...(onCreateBeside ? [
          { label: t('noteList.menu.newNoteAbove'), icon: fa(faNoteSticky), onClick: () => onCreateBeside(note, 'before') },
          { label: t('noteList.menu.newNoteBelow'), icon: fa(faNoteSticky), onClick: () => onCreateBeside(note, 'after') },
        ] : []),
        ...(onTogglePin ? [{ label: note.pinned ? t('noteList.menu.unpin') : t('noteList.menu.pin'), icon: fa(faThumbtack), onClick: () => onTogglePin(note.id, !note.pinned) }] : []),
        ...(onArchive ? [{ label: note.archived ? t('noteList.menu.restore') : t('noteList.menu.archive'), icon: fa(faBoxArchive), onClick: () => onArchive(note.id, !note.archived) }] : []),
        ...(onMoveNote ? [{ label: t('noteList.menu.moveTo'), icon: fa(faRightLong), submenu: moveSubmenu() }] : []),
        { label: t('noteList.menu.delete'), icon: fa(faTrash), onClick: () => onRequestDelete(note.id) },
        { label: t('noteList.menu.export'), icon: fa(faFileExport), onClick: () => onExportNote(note) },
        // A protected note can't be opened while the vault is locked.
        ...(onPrintNote && !isMobilePlatform && !(note.protected && !vaultUnlocked) ? [{ label: t('noteList.menu.print'), icon: fa(faPrint), onClick: () => onPrintNote(note) }] : []),
        ...(onProtectNote ? [{ label: note.protected ? t('vault.unlockNote') : t('vault.lockNote'), icon: fa(note.protected ? faLockOpen : faLock), onClick: () => onProtectNote(note.id, !note.protected) }] : []),
        ...(onSetNoteMcpHidden ? [{ label: note.mcpHidden ? t('vault.showToMcp') : t('vault.hideFromMcp'), icon: fa(note.mcpHidden ? faEye : faEyeSlash), onClick: () => onSetNoteMcpHidden(note.id, !note.mcpHidden) }] : []),
      ]}
      onClose={onClose}
    />
  );
}

/** Confirmation for the menu's "Löschen" — wording follows whether the trash is on. */
export function DeleteNoteDialog({ trashEnabled, onConfirm, onCancel }: { trashEnabled: boolean; onConfirm: () => void; onCancel: () => void }) {
  const { t } = useTranslation();
  return (
    <ConfirmDialog
      title={t('noteList.confirm.deleteTitle')}
      message={trashEnabled ? t('noteList.confirm.deleteTrashMessage') : t('noteList.confirm.deletePermanentMessage')}
      confirmLabel={trashEnabled ? t('noteList.confirm.moveToTrash') : t('noteList.confirm.deletePermanent')}
      danger={!trashEnabled}
      onConfirm={onConfirm}
      onCancel={onCancel}
    />
  );
}
