export interface Note {
  id: string;
  content: string; // Tiptap HTML
  updatedAt: number;
  pinned: boolean;
  archived: boolean;
  color: string;
  dueAt: number | null;
  folderId: string | null;
  position: number;
}

export interface Folder {
  id: string;
  name: string;
  parentId: string | null;
  position: number;
  icon: string;
  color: string;
}

export interface Stats {
  notes: number;
  archived: number;
  characters: number;
  words: number;
}
