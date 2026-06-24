import { api } from './api';
import { extFromType } from './imageType';
export async function saveImageFile(noteId: string, file: File): Promise<string> {
  const buf = await file.arrayBuffer();
  const name = `${crypto.randomUUID()}.${extFromType(file.type)}`;
  return api.saveImage(noteId, name, Array.from(new Uint8Array(buf)));
}
