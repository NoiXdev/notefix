import html2canvas from 'html2canvas-pro';
import { jsPDF } from 'jspdf';

async function renderToCanvas(html: string): Promise<HTMLCanvasElement> {
  const host = document.createElement('div');
  Object.assign(host.style, {
    position: 'fixed', left: '-10000px', top: '0', width: '794px', padding: '32px',
    background: '#ffffff', color: '#111827', fontFamily: 'system-ui, sans-serif',
    fontSize: '15px', lineHeight: '1.6', boxSizing: 'border-box',
  } as CSSStyleDeclaration);
  host.innerHTML = html;
  document.body.appendChild(host);
  try {
    return await html2canvas(host, { backgroundColor: '#ffffff', scale: 2 });
  } finally {
    document.body.removeChild(host);
  }
}

export async function htmlToPdf(html: string): Promise<Uint8Array> {
  const canvas = await renderToCanvas(html);
  const pdf = new jsPDF({ unit: 'pt', format: 'a4' });
  const pageW = pdf.internal.pageSize.getWidth();
  const pageH = pdf.internal.pageSize.getHeight();
  const imgH = (canvas.height * pageW) / canvas.width;
  const imgData = canvas.toDataURL('image/jpeg', 0.92);
  let position = 0;
  let remaining = imgH;
  pdf.addImage(imgData, 'JPEG', 0, position, pageW, imgH);
  remaining -= pageH;
  while (remaining > 0) {
    position -= pageH;
    pdf.addPage();
    pdf.addImage(imgData, 'JPEG', 0, position, pageW, imgH);
    remaining -= pageH;
  }
  return new Uint8Array(pdf.output('arraybuffer'));
}

export async function htmlToJpg(html: string): Promise<Uint8Array> {
  const canvas = await renderToCanvas(html);
  const b64 = canvas.toDataURL('image/jpeg', 0.92).split(',')[1];
  return Uint8Array.from(atob(b64), c => c.charCodeAt(0));
}
