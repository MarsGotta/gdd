// A page of the site as Markdown, for llms.txt and for whoever reads the site with an agent:
// the same text as the page, without the layout. It knows only the HTML the documents use:
// a list item holds either inline text or blocks, never both.
import * as cheerio from 'cheerio';

const espacios = (s) => s.replace(/\s+/g, ' ');

export const aMarkdown = (html, enlace) => {
  const $ = cheerio.load(html, null, false);
  $('script, .boton, [aria-hidden="true"]').remove();

  const enLinea = (nodo) => $(nodo).contents().map((_, n) => {
    if (n.type === 'text') return espacios(n.data);
    if (n.type !== 'tag') return '';
    const dentro = enLinea(n).trim();
    switch (n.name) {
      case 'strong': return dentro ? `**${dentro}**` : '';
      case 'em': return dentro ? `*${dentro}*` : '';
      case 'code': return `\`${$(n).text()}\``;
      case 'a': return `[${dentro}](${enlace($(n).attr('href') ?? '')})`;
      default: return enLinea(n);
    }
  }).get().join('');

  const fila = (celdas) => `| ${celdas.map((c) => enLinea(c).trim().replace(/\|/g, '\\|')).join(' | ')} |`;

  const bloques = (nodo) => $(nodo).children().map((_, n) => {
    const $n = $(n);
    switch (n.name) {
      case 'h2': case 'h3': return `${'#'.repeat(Number(n.name[1]))} ${enLinea(n).trim()}`;
      case 'p': case 'figcaption': return enLinea(n).trim();
      case 'svg': return $n.attr('aria-label') ? `*${$n.attr('aria-label')}*` : '';
      case 'pre': case 'ki-code-block': return `\`\`\`\n${$n.text().replace(/\n$/, '')}\n\`\`\``;
      case 'blockquote': return bloques(n).split('\n').map((l) => `> ${l}`.trimEnd()).join('\n');
      case 'ul': case 'ol': return $n.children('li').map((k, li) => {
        const marca = n.name === 'ol' ? `${Number($n.attr('start') ?? 1) + k}. ` : '- ';
        const texto = $(li).children('p, ul, ol, pre, ki-code-block, table').length ? bloques(li) : enLinea(li).trim();
        return marca + texto.split('\n').join(`\n${' '.repeat(marca.length)}`);
      }).get().join('\n');
      case 'table': {
        const cabeza = $n.find('thead tr').first().children('th, td').get();
        const filas = $n.find('tbody tr').map((_, tr) => fila($(tr).children('th, td').get())).get();
        return [fila(cabeza), `|${cabeza.map(() => ' --- |').join('')}`, ...filas].join('\n');
      }
      default: return bloques(n);
    }
  }).get().filter((b) => b.trim()).join('\n\n');

  return bloques($.root());
};
