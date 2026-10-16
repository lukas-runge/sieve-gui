// daisyUI dropdowns open while focused. Triggers are focusable divs, not
// buttons, because Safari/WKWebView doesn't focus buttons on click.

import type { Attachment } from "svelte/attachments";

/** Closes the dropdown once a menu item was chosen (`{@attach closesMenu}`). */
export const closesMenu: Attachment<HTMLElement> = (menu) => {
  const close = () => (document.activeElement as HTMLElement | null)?.blur();
  menu.addEventListener("click", close);
  return () => menu.removeEventListener("click", close);
};
