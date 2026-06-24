export default function StatusBar({ text }: { text: string }) {
  return (
    <div className="shrink-0 px-3 py-1 text-[11px] font-mono text-gray-500 border-t border-yellow-200 select-none truncate" style={{ background: '#fef9c3' }}>
      {text}
    </div>
  );
}
