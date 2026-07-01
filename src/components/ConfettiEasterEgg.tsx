import { useEffect, useState } from 'react';
import Confetti from 'react-confetti';

interface Burst { id: number; x: number; y: number; }

/** Easter egg: a middle-mouse click bursts confetti from the cursor. */
export default function ConfettiEasterEgg() {
  const [bursts, setBursts] = useState<Burst[]>([]);
  const [size, setSize] = useState({ w: window.innerWidth, h: window.innerHeight });

  useEffect(() => {
    const onResize = () => setSize({ w: window.innerWidth, h: window.innerHeight });
    window.addEventListener('resize', onResize);
    return () => window.removeEventListener('resize', onResize);
  }, []);

  useEffect(() => {
    let nextId = 0;
    const onDown = (e: MouseEvent) => {
      if (e.button !== 1) return; // middle mouse button only
      e.preventDefault(); // suppress the autoscroll cursor
      const id = nextId++;
      setBursts(prev => [...prev, { id, x: e.clientX, y: e.clientY }]);
    };
    window.addEventListener('mousedown', onDown);
    return () => window.removeEventListener('mousedown', onDown);
  }, []);

  if (bursts.length === 0) return null;

  return (
    <div className="pointer-events-none fixed inset-0 z-[9999]">
      {bursts.map(b => (
        <Confetti
          key={b.id}
          width={size.w}
          height={size.h}
          numberOfPieces={180}
          recycle={false}
          gravity={0.28}
          initialVelocityX={7}
          initialVelocityY={14}
          confettiSource={{ x: b.x, y: b.y, w: 12, h: 12 }}
          onConfettiComplete={() => setBursts(prev => prev.filter(x => x.id !== b.id))}
        />
      ))}
    </div>
  );
}
