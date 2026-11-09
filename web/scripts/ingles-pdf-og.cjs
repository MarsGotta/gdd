// Una vez por versión, después de npm run build: el PDF del manifiesto en inglés y la imagen para compartir.
// Necesita Playwright: node scripts/ingles-pdf-og.cjs
const { chromium } = require('playwright');
const http = require('http'); const fs = require('fs'); const path = require('path');
const raiz = path.join(__dirname, '..'); const dist = raiz + '/dist';
const tipos = { '.html': 'text/html; charset=utf-8', '.css': 'text/css', '.js': 'text/javascript', '.svg': 'image/svg+xml', '.woff2': 'font/woff2' };
const srv = http.createServer((q, r) => { let p = decodeURIComponent(q.url.split('?')[0]); if (p.endsWith('/')) p += 'index.html'; const f = path.join(dist, p); if (!fs.existsSync(f)) { r.writeHead(404); r.end(); return; } r.writeHead(200, { 'content-type': tipos[path.extname(f)] || 'application/octet-stream' }); fs.createReadStream(f).pipe(r); });
(async () => {
  await new Promise((ok) => srv.listen(8897, ok));
  const b = await chromium.launch();
  // PDF: la propia página, con el estilo de impresión de la web.
  const p = await b.newPage();
  await p.goto('http://localhost:8897/en/manifesto/', { waitUntil: 'networkidle' });
  await p.emulateMedia({ media: 'print' });
  await p.addStyleTag({ content: '.indice, .saltar, .copiar, .tema, .idioma, .navegacion, .pie { display: none !important; } .con-indice { display: block !important; } .lectura { max-width: none !important; } .pagina-cabeza p:last-of-type { display: none !important; }' });
  await p.pdf({ path: raiz + '/content/en/manifesto-gdd-v0.7.pdf', format: 'A4', margin: { top: '20mm', bottom: '20mm', left: '20mm', right: '20mm' }, printBackground: true, displayHeaderFooter: false, tagged: true, outline: true });
  // Imagen para compartir: el plano en rojo, con la misma composición que la española.
  const q = await b.newPage({ viewport: { width: 1200, height: 630 } });
  await q.goto('http://localhost:8897/en/', { waitUntil: 'networkidle' });
  await q.evaluate(() => {
    const plano = document.querySelector('.plano__marco').cloneNode(true);
    const figura = document.createElement('figure'); figura.className = 'plano'; figura.dataset.riesgo = 'rojo'; figura.style.cssText = 'margin:0;position:absolute;left:48px;top:214px;width:1104px';
    figura.append(plano);
    plano.querySelectorAll('.puerta').forEach((x) => { x.dataset.estado = { 1: 'ganada', 2: 'ganada', 3: 'cerrada' }[x.dataset.puerta]; });
    plano.querySelector('.plano__agente').style.setProperty('--x', '55.5%');
    plano.querySelector('.plano__nombre').hidden = true; plano.querySelector('.plano__globo').hidden = true;
    const h = document.createElement('div'); h.style.cssText = 'position:absolute;left:48px;top:40px;font:500 58px/1.1 "IBM Plex Sans";letter-spacing:-.02em;color:#15181b;width:1110px';
    h.innerHTML = 'Add gates to your project and automate with AI agents without losing quality.<br>Your agent says "done." The gate checks if it\'s true.';
    h.style.fontSize = '42px';
    const sprite = document.querySelector('.sprite');
    document.body.replaceChildren(sprite, h, figura);
    document.body.style.cssText = 'margin:0;background:#fff;width:1200px;height:630px;overflow:hidden';
    document.documentElement.dataset.tema = 'claro';
  });
  await q.addStyleTag({ content: '*, *::before, *::after { transition: none !important; animation: none !important; }' });
  await q.waitForTimeout(200);
  await q.screenshot({ path: raiz + '/src/img/og-en.png' });
  await b.close(); srv.close();
})();
