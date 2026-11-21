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
