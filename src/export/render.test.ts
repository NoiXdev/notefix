import { describe, it, expect, vi, beforeEach } from 'vitest';

// jsdom has no real <canvas> 2D backing (no `canvas` npm package installed),
// so html2canvas-pro/jsPDF are mocked at the module boundary; the tests
// verify render.ts's OWN logic — host element setup/cleanup, image sizing,
// and multi-page pagination — not the libraries' internals.
const { html2canvasMock, jsPDFCtorMock, addImageMock, addPageMock, outputMock } = vi.hoisted(() => ({
  html2canvasMock: vi.fn(),
  jsPDFCtorMock: vi.fn(),
  addImageMock: vi.fn(),
  addPageMock: vi.fn(),
  outputMock: vi.fn(() => new ArrayBuffer(4)),
}));

vi.mock('html2canvas-pro', () => ({ default: html2canvasMock }));
vi.mock('jspdf', () => ({
  jsPDF: jsPDFCtorMock,
}));

import { htmlToPdf, htmlToJpg } from './render';

function fakeCanvas(width: number, height: number, dataUrl = 'data:image/jpeg;base64,QUJD'): HTMLCanvasElement {
  return {
    width,
    height,
    toDataURL: vi.fn(() => dataUrl),
  } as unknown as HTMLCanvasElement;
}

beforeEach(() => {
  vi.clearAllMocks();
  outputMock.mockReturnValue(new ArrayBuffer(4));
  jsPDFCtorMock.mockImplementation(function (this: Record<string, unknown>) {
    this.internal = { pageSize: { getWidth: () => 595, getHeight: () => 842 } };
    this.addImage = addImageMock;
    this.addPage = addPageMock;
    this.output = outputMock;
  } as unknown as () => void);
});

describe('htmlToPdf', () => {
  it('renders the html into an off-screen host, passes it to html2canvas, then removes the host', async () => {
    let hostSeenDuringRender: HTMLElement | null = null;
    html2canvasMock.mockImplementationOnce(async (host: HTMLElement) => {
      hostSeenDuringRender = host;
      // While html2canvas "runs", the host must be attached and hold our html.
      expect(document.body.contains(host)).toBe(true);
      expect(host.innerHTML).toBe('<p>Hello</p>');
      return fakeCanvas(794, 400);
    });

    await htmlToPdf('<p>Hello</p>');

    expect(html2canvasMock).toHaveBeenCalledTimes(1);
    expect(hostSeenDuringRender).not.toBeNull();
    // Cleaned up afterwards regardless of success.
    expect(document.body.contains(hostSeenDuringRender as unknown as HTMLElement)).toBe(false);
  });

  it('removes the host even when html2canvas throws', async () => {
    let hostSeenDuringRender: HTMLElement | null = null;
    html2canvasMock.mockImplementationOnce(async (host: HTMLElement) => {
      hostSeenDuringRender = host;
      throw new Error('render failed');
    });

    await expect(htmlToPdf('<p>boom</p>')).rejects.toThrow('render failed');
    expect(document.body.contains(hostSeenDuringRender as unknown as HTMLElement)).toBe(false);
  });

  it('fits a short canvas onto a single page (no addPage call)', async () => {
    // pageW=595, pageH=842; canvas 794x400 -> imgH = 400*595/794 ≈ 299.9 < 842
    html2canvasMock.mockResolvedValueOnce(fakeCanvas(794, 400));

    const bytes = await htmlToPdf('<p>short</p>');

    expect(addImageMock).toHaveBeenCalledTimes(1);
    const [imgData, format, x, y, w, h] = addImageMock.mock.calls[0];
    expect(format).toBe('JPEG');
    expect(x).toBe(0);
    expect(y).toBe(0);
    expect(w).toBe(595);
    expect(h).toBeCloseTo((400 * 595) / 794, 5);
    expect(imgData).toBe('data:image/jpeg;base64,QUJD');
    expect(addPageMock).not.toHaveBeenCalled();
    expect(bytes).toBeInstanceOf(Uint8Array);
    expect(bytes.byteLength).toBe(4);
  });

  it('paginates a tall canvas across multiple pages', async () => {
    // imgH = 4000*595/794 ≈ 2998.7; pageH=842 -> needs 3 addPage calls (4 pages total)
    html2canvasMock.mockResolvedValueOnce(fakeCanvas(794, 4000));

    await htmlToPdf('<p>long</p>');

    expect(addImageMock).toHaveBeenCalledTimes(4); // 1 initial + 3 continuation pages
    expect(addPageMock).toHaveBeenCalledTimes(3);
    // Each continuation page shifts the image up by one page height.
    const yPositions = addImageMock.mock.calls.map(call => call[3]);
    expect(yPositions[0]).toBe(0);
    expect(yPositions[1]).toBeCloseTo(-842, 5);
    expect(yPositions[2]).toBeCloseTo(-842 * 2, 5);
    expect(yPositions[3]).toBeCloseTo(-842 * 3, 5);
  });

  it('creates the jsPDF document in pt units, a4 format', async () => {
    html2canvasMock.mockResolvedValueOnce(fakeCanvas(794, 400));
    await htmlToPdf('<p>x</p>');
    expect(jsPDFCtorMock).toHaveBeenCalledExactlyOnceWith({ unit: 'pt', format: 'a4' });
  });
});

describe('htmlToJpg', () => {
  it('renders via html2canvas and decodes the base64 JPEG data URL into bytes', async () => {
    // "QUJD" base64-decodes to the ASCII bytes for "ABC" (65, 66, 67).
    html2canvasMock.mockResolvedValueOnce(fakeCanvas(100, 100, 'data:image/jpeg;base64,QUJD'));

    const bytes = await htmlToJpg('<p>pic</p>');

    expect(bytes).toBeInstanceOf(Uint8Array);
    expect(Array.from(bytes)).toEqual([65, 66, 67]);
  });

  it('removes the off-screen host after rendering', async () => {
    let host: HTMLElement | null = null;
    html2canvasMock.mockImplementationOnce(async (h: HTMLElement) => {
      host = h;
      return fakeCanvas(10, 10, 'data:image/jpeg;base64,QQ==');
    });

    await htmlToJpg('<p>pic</p>');

    expect(document.body.contains(host as unknown as HTMLElement)).toBe(false);
  });
});
