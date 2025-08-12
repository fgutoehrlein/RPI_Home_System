import MarkdownIt from 'markdown-it';
import DOMPurify from 'dompurify';

const md = new MarkdownIt({
  linkify: true,
  breaks: true,
});

export function renderMarkdown(src: string): string {
  const html = md.render(src);
  if (typeof window === 'undefined') {
    return html;
  }
  return DOMPurify.sanitize(html, { ADD_ATTR: ['target'] });
}
