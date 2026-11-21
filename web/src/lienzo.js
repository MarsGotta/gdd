// Runs each page's own script from the design canvas in the browser. The build wrote the page's
// first state and, next to every attribute that changes, its template (data-dc-*). The page's
// Component comes in window.DC; on every setState its renderVals() gives the values again and
// each template is filled. The reader's light or dark is the system's until they choose, and the
// choice is remembered from page to page.
(() => {
  if (typeof window.DC !== 'function') return;
  const html = document.documentElement;
  const oscuro = html.dataset.tema ? html.dataset.tema === 'oscuro' : window.matchMedia('(prefers-color-scheme: dark)').matches;
  const props = { ...window.DC_PROPS };
  if ('esquema' in props) props.esquema = oscuro ? 'nocturno' : 'delineado';

  const marcas = [...document.querySelectorAll('*')].filter((el) => [...el.attributes].some((a) => a.name.startsWith('data-dc-')));
  const rellenar = (plantilla, v) => plantilla.replace(/\{\{(\w+)\}\}/g, (_, k) => (v[k] === undefined || typeof v[k] === 'function' ? '' : String(v[k])));
  let componente;
  let esquema;

  const pintar = () => {
    const v = componente.renderVals();
    for (const el of marcas) {
      for (const { name, value } of [...el.attributes]) {
        if (!name.startsWith('data-dc-')) continue;
        const destino = name.slice('data-dc-'.length);
        if (destino.startsWith('on')) el[destino] = typeof v[value] === 'function' ? () => v[value]() : null;
        else el.setAttribute(destino, rellenar(value, v));
      }
    }
    // The first paint follows the reader; a later change of scheme is their choice, and stays.
    if (v.scheme && v.scheme !== esquema) {
      if (esquema) {
        const tema = v.scheme === 'dark' ? 'oscuro' : 'claro';
        html.dataset.tema = tema;
        try { localStorage.setItem('tema', tema); } catch { /* without storage it lasts this page */ }
      }
      esquema = v.scheme;
    }
  };

  class DCLogic {
    constructor(p) { this.props = p; this.state = null; }
    setState(cambio) { this.state = { ...this.state, ...cambio }; pintar(); }
  }
  const Component = window.DC(DCLogic);
  componente = new Component(props);
  pintar();
  if (typeof componente.componentDidMount === 'function') componente.componentDidMount();
})();
