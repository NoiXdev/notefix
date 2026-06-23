interface Props {
  folderName: string;
  noteCount: number;
  subfolderCount: number;
  onReparent: () => void;
  onRecursive: () => void;
  onCancel: () => void;
}

export default function DeleteFolderModal({ folderName, noteCount, subfolderCount, onReparent, onRecursive, onCancel }: Props) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center" style={{ background: 'rgba(0,0,0,0.45)' }} onClick={onCancel}>
      <div className="w-96 rounded-lg bg-gray-900 border border-gray-700 p-5" onClick={e => e.stopPropagation()}>
        <h2 className="text-gray-100 text-base font-semibold mb-2">Ordner „{folderName}" löschen</h2>
        <p className="text-gray-400 text-sm mb-5">Enthält {noteCount} Notiz(en) und {subfolderCount} Unterordner. Was soll passieren?</p>
        <div className="flex flex-col gap-2">
          <button onClick={onReparent} className="text-left px-3 py-2 rounded text-sm bg-gray-800 text-gray-100 hover:bg-gray-700">Inhalt in Elternordner verschieben</button>
          <button onClick={onRecursive} className="text-left px-3 py-2 rounded text-sm text-red-300 hover:bg-gray-800">Alles löschen</button>
          <button onClick={onCancel} className="text-left px-3 py-2 rounded text-sm text-gray-400 hover:bg-gray-800">Abbrechen</button>
        </div>
      </div>
    </div>
  );
}
