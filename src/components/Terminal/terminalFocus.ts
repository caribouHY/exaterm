export function focusTerminalUnlessModal(focus: () => void) {
  if (!document.querySelector(".ui-overlay")) focus();
}
