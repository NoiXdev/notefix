export interface Note {
  id: string;
  content: string; // Tiptap HTML
  updatedAt: number;
  pinned: boolean;
  archived: boolean;
  color: string;
  dueAt: number | null;
}

export interface Stats {
  notes: number;
  archived: number;
  characters: number;
  words: number;
}
