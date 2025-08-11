export function cls(...classes: (string | undefined | null | false)[]) {
  return classes.filter(Boolean).join(' ');
}
