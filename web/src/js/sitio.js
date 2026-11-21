// Small conveniences shared by every page: light or dark, and the current section in the index.
// The code blocks are ki-code-block, which brings its own copy button.
(() => {
  // The system decides until the reader chooses, and the choice is remembered.
  // The script in the head of each page applies it before anything is painted, as
  // data-ki-color-scheme on <html>, the attribute Kimen's tokens follow.
  const raiz = document.documentElement;
  const t = (es, en) => (raiz.lang === 'en' ? en : es);
  const botonTema = document.querySelector('[data-tema-boton]');
  const sistemaOscuro = window.matchMedia('(prefers-color-scheme: dark)');
  const esOscuro = () => {
    const elegido = raiz.dataset.kiColorScheme;
    return elegido ? elegido === 'dark' : sistemaOscuro.matches;
  };
  const decir = () => {
    botonTema?.setAttribute('label', esOscuro() ? t('Pasar a modo claro', 'Switch to light mode') : t('Pasar a modo oscuro', 'Switch to dark mode'));
    if (!raiz.dataset.kiColorScheme) return;
    if (document.querySelector('.cabecera--plano')) return;
    document.querySelectorAll('meta[name="theme-color"]').forEach((meta) => { meta.content = esOscuro() ? '#12458b' : '#f7f9fb'; });
  };
  botonTema?.addEventListener('click', () => {
    raiz.classList.add('sin-transicion');
    raiz.dataset.kiColorScheme = esOscuro() ? 'light' : 'dark';
    requestAnimationFrame(() => requestAnimationFrame(() => raiz.classList.remove('sin-transicion')));
    try {
      localStorage.setItem('tema', raiz.dataset.kiColorScheme === 'dark' ? 'oscuro' : 'claro');
    } catch {
      // Without storage the choice lasts until the reader changes page.
    }
    decir();
  });
  sistemaOscuro.addEventListener?.('change', decir);
  decir();

  // "Antes de cruzar": the chapter's door opens on the right answer, and each answer says why.
  document.querySelectorAll('.cruzar').forEach((puerta) => {
    const opciones = puerta.querySelector('ki-radio-group');
    opciones?.addEventListener('change', () => {
      puerta.dataset.respuesta = opciones.value === 'bien' ? 'bien' : 'mal';
    });
  });

  // The map of the documentation: a way in numbers its stops and says where to start. Until Kimen
  // registers the control, the map stays as it is written, with no numbers.
  const control = document.querySelector('.rutas__control');
  const mapa = document.querySelector('.mapa');
  const rutas = document.querySelector('.rutas');
  const elegir = (r) => { mapa.dataset.ruta = r; rutas.dataset.ruta = r; };
  if (control && mapa && rutas) {
    customElements.whenDefined('ki-segmented-control').then(() => {
      control.addEventListener('ki-change', (e) => elegir(e.detail.value));
      elegir(control.getAttribute('value') || 'a');
    });
  }

  // The workshop's lab: with the four questions on, the gate holds; each one turned off says what
  // slips through, and the door of the plan opens. Until the switches are registered, every answer
  // and both outcomes stay in view, as without scripts.
  document.querySelectorAll('.laboratorio').forEach((lab) => {
    const mandos = [...lab.querySelectorAll('ki-switch')];
    const pintar = () => {
      const fuera = mandos.filter((m) => !m.checked).map((m) => m.dataset.n);
      lab.dataset.estado = fuera.length ? 'abre' : 'cierra';
      lab.querySelectorAll('.laboratorio__porque').forEach((p) => { p.hidden = !fuera.includes(p.dataset.n); });
    };
    customElements.whenDefined('ki-switch').then(() => {
      mandos.forEach((m) => m.addEventListener('change', pintar));
      pintar();
    });
  });

  // The index is open beside the text on wide screens and folded above it on narrow ones.
  const indice = document.querySelector('.indice details, .capitulos details');
  if (indice && window.matchMedia('(min-width: 901px)').matches) indice.open = true;

  const enlaces = [...document.querySelectorAll('.indice a[href^="#"], .capitulos__apartados a[href^="#"]')];
  if (!enlaces.length || !('IntersectionObserver' in window)) return;
  const destinos = new Map();
  enlaces.forEach((a) => {
    const destino = document.getElementById(decodeURIComponent(a.hash.slice(1)));
    if (destino) destinos.set(destino, a);
  });
  const marcar = (a) => {
    enlaces.forEach((otro) => otro.removeAttribute('aria-current'));
    a.setAttribute('aria-current', 'true');
  };
  const observador = new IntersectionObserver((entradas) => {
    entradas.filter((e) => e.isIntersecting).forEach((e) => marcar(destinos.get(e.target)));
  }, { rootMargin: '-10% 0px -80% 0px' });
  destinos.forEach((_, destino) => observador.observe(destino));
})();
