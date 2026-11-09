// Small conveniences shared by every page: light or dark, copy buttons and the current section in the index.
(() => {
  // The system decides until the reader chooses, and the choice is remembered.
  // The script in the head of each page applies it before anything is painted.
  const raiz = document.documentElement;
  const t = (es, en) => (raiz.lang === 'en' ? en : es);
  const botonTema = document.querySelector('[data-tema-boton]');
  const sistemaOscuro = window.matchMedia('(prefers-color-scheme: dark)');
  const esOscuro = () => (raiz.dataset.tema ? raiz.dataset.tema === 'oscuro' : sistemaOscuro.matches);
  const colorDelNavegador = () => {
    if (!raiz.dataset.tema) return;
    document.querySelectorAll('meta[name="theme-color"]').forEach((meta) => { meta.content = esOscuro() ? '#15181b' : '#ffffff'; });
  };
  botonTema?.addEventListener('click', () => {
    raiz.classList.add('sin-transicion');
    raiz.dataset.tema = esOscuro() ? 'claro' : 'oscuro';
    requestAnimationFrame(() => requestAnimationFrame(() => raiz.classList.remove('sin-transicion')));
    try {
      localStorage.setItem('tema', raiz.dataset.tema);
    } catch {
      // Without storage the choice lasts until the reader changes page.
    }
    colorDelNavegador();
  });
  colorDelNavegador();

  document.querySelectorAll('pre[data-copiar]').forEach((pre) => {
    const boton = document.createElement('button');
    boton.type = 'button';
    boton.className = 'copiar';
    boton.setAttribute('aria-live', 'polite');
    boton.textContent = t('Copiar', 'Copy');
    boton.addEventListener('click', async () => {
      try {
        await navigator.clipboard.writeText(pre.querySelector('code').textContent);
        boton.textContent = t('Copiado', 'Copied');
      } catch {
        boton.textContent = t('Selecciónalo a mano', 'Select it by hand');
      }
      setTimeout(() => { boton.textContent = t('Copiar', 'Copy'); }, 2000);
    });
    pre.append(boton);
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
