import { describe, it, expect } from 'vitest';
import { renderMarkdown } from './markdown';

describe('markdown', () => {
  it('renders bold', () => {
    const html = renderMarkdown('**hi**');
    expect(html).toContain('<strong>hi</strong>');
  });
});
