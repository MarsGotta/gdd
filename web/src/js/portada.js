// The home page: each room's door opens as the room comes into view, and the hero's animation can
// be paused. The animation only runs with scripts on (.js), so it can always be stopped; without
// them the doors are drawn open, and with reduced motion nothing moves.
(() => {
  const g = document.querySelector('.g');
  if (!g) return;

  const quieta = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  const pausa = g.querySelector('[data-portada-pausa]');
  if (pausa && !quieta) {
    pausa.hidden = false;
    pausa.addEventListener('click', () => {
      const pausada = pausa.getAttribute('aria-pressed') !== 'true';
      pausa.setAttribute('aria-pressed', String(pausada));
      g.classList.toggle('is-paused', pausada);
    });
  }

  if (!('IntersectionObserver' in window) || quieta) return;
  const muros = g.querySelectorAll('.g-wall');
  if (!muros.length) return;
  g.classList.add('is-armed');
  const observador = new IntersectionObserver((entradas) => {
    entradas.filter((e) => e.isIntersecting).forEach((e) => {
      e.target.classList.add('is-open');
      observador.unobserve(e.target);
    });
  }, { threshold: 1, rootMargin: '0px 0px -18% 0px' });
  muros.forEach((m) => observador.observe(m));
})();
