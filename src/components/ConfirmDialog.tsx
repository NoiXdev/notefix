interface Props {
  title: string;
  message: string;
  confirmLabel: string;
  danger?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

export default function ConfirmDialog({ title, message, confirmLabel, danger, onConfirm, onCancel }: Props) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center" style={{ background: 'rgba(0,0,0,0.45)' }} onClick={onCancel}>
      <div className="w-96 rounded-lg bg-gray-900 border border-gray-700 p-5" onClick={e => e.stopPropagation()}>
        <h2 className="text-gray-100 text-base font-semibold mb-2">{title}</h2>
        <p className="text-gray-400 text-sm mb-5">{message}</p>
        <div className="flex justify-end gap-2">
          <button onClick={onCancel} className="px-3 py-1.5 rounded text-sm text-gray-300 hover:bg-gray-800">Abbrechen</button>
          <button onClick={onConfirm} className="px-3 py-1.5 rounded text-sm font-medium" style={danger ? { background: '#dc2626', color: 'white' } : { background: '#fde047', color: '#1c1917' }}>{confirmLabel}</button>
        </div>
      </div>
    </div>
  );
}
